//! Conservative first-order momentum transport on staggered dual volumes.
//!
//! A u face owns half of each adjacent cell mass. A v face uses the same rule
//! above and below. Closed outer faces own half of one cell. Each dual edge
//! receives the average of the two adjacent primal mass fluxes from the
//! liquid and carrier ledger. This construction makes dual mass change match
//! cell mass change, including transverse flux. Aligned u.x and v.y edges
//! upwind their net interpolated flux. Transverse u.y and v.x edges retain two
//! signed subfaces and upwind each one before adding momentum. A fixed face
//! discards its resulting momentum through an explicit wall impulse. This
//! module does not apply viscosity,
//! pressure, gravity, or a higher-order momentum reconstruction.

use particle_sim::{CELL_WIDTH_M, Grid, REPRESENTED_DEPTH_M, contracts::Boundary};

use crate::{
    fluid::{FaceValues, PressureFields},
    transport::{TransportCandidate, TransportFaceFluxes, TransportInventory},
};

const LEDGER_REL_TOL: f64 = 1e-12;

/// Invalid transport ledger, geometry, or momentum result.
#[derive(Clone, Debug, PartialEq)]
pub enum MomentumError {
    /// Inventory and pressure fields do not use the same grid.
    GridMismatch,
    /// The candidate changes a density or a fixed-wall cell.
    MaterialOrWallMismatch,
    /// A reservoir mass and momentum ledger is required at an open side.
    UnsupportedOpenBoundary,
    /// The substep duration is not positive and finite.
    InvalidTimeStep,
    /// An array does not match its cell or face layout.
    Length {
        /// Rejected array name.
        field: &'static str,
        /// Required number of entries.
        expected: usize,
        /// Supplied number of entries.
        actual: usize,
    },
    /// A supplied scalar or array entry is nonfinite.
    NonFiniteInput {
        /// Rejected value name.
        field: &'static str,
        /// Entry index, or zero for a scalar.
        index: usize,
    },
    /// An aperture, face velocity, or signed transport flux violates geometry.
    InvalidFace {
        /// `u` or `v` face array.
        axis: &'static str,
        /// Row-major face index.
        index: usize,
        /// Failed condition.
        reason: &'static str,
    },
    /// The candidate cell mass does not follow the supplied face ledger.
    InconsistentCellLedger {
        /// Liquid or carrier mass.
        phase: &'static str,
        /// Row-major cell index.
        cell: usize,
    },
    /// A dual mass does not follow its derived dual-edge fluxes.
    InconsistentDualLedger {
        /// `u` or `v` dual grid.
        axis: &'static str,
        /// Row-major dual cell index.
        index: usize,
    },
    /// A dual cell would send more mass than it owns at the start of the step.
    DualDonorOverdraw {
        /// `u` or `v` dual grid.
        axis: &'static str,
        /// Row-major dual cell index.
        index: usize,
    },
    /// A positive-aperture face has zero dual mass.
    EmptyOpenFace {
        /// `u` or `v` face array.
        axis: &'static str,
        /// Row-major face index.
        index: usize,
    },
    /// Finite inputs produced an unrepresentable mass, flux, velocity, or impulse.
    NonFiniteResult {
        /// Calculation that failed.
        field: &'static str,
        /// Row-major cell, face, or dual-edge index.
        index: usize,
    },
}

impl std::fmt::Display for MomentumError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for MomentumError {}

/// Signed total mass across the x and y edges of one dual grid.
///
/// A positive x flux points right. A positive y flux points down. The u dual
/// grid has `(W+1) × H` cells, `(W+2) × H` x edges, and `(W+1) × (H+1)` y edges.
/// The v dual grid has `W × (H+1)` cells, `(W+1) × (H+1)` x edges, and
/// `W × (H+2)` y edges.
#[derive(Debug, PartialEq)]
pub struct DualFaceFluxes {
    /// Signed mass in kg across x edges.
    pub x_mass_kg: Vec<f64>,
    /// Signed mass in kg across y edges.
    pub y_mass_kg: Vec<f64>,
}

/// Dual-edge fluxes derived from the liquid and carrier cell-face ledger.
#[derive(Debug, PartialEq)]
pub struct DualMassFluxes {
    /// Fluxes around u-face dual cells.
    pub u: DualFaceFluxes,
    /// Fluxes around v-face dual cells.
    pub v: DualFaceFluxes,
}

// The net flux determines dual mass. Momentum must retain each signed half-face
// flux until after it chooses an upwind velocity. Opposite directions can cancel
// in the net mass ledger while their momentum transfers add.
struct DualSubfaceFluxes {
    x_mass_kg: Vec<[f64; 2]>,
    y_mass_kg: Vec<[f64; 2]>,
}

struct DualSubfaceLedger {
    u: DualSubfaceFluxes,
    v: DualSubfaceFluxes,
}

/// Detached velocity and accounting result for one transport candidate.
#[derive(Debug, PartialEq)]
pub struct MomentumCandidate {
    /// Advected normal velocities in m/s. Blocked faces remain zero.
    pub velocity_m_s: FaceValues,
    /// Dual masses in kg before the cell transport candidate.
    pub dual_mass_before_kg: FaceValues,
    /// Dual masses in kg after the cell transport candidate.
    pub dual_mass_after_kg: FaceValues,
    /// Signed integrated mass fluxes across each dual edge.
    pub dual_mass_fluxes: DualMassFluxes,
    /// Impulse applied to the fluid by fixed walls, for u and v, in kg m/s.
    pub wall_impulse_kg_m_s: [f64; 2],
    /// Staggered kinetic energy before advection, in J.
    pub kinetic_before_j: f64,
    /// Staggered kinetic energy after advection and fixed-wall clamping, in J.
    pub kinetic_after_j: f64,
    /// `kinetic_after_j - kinetic_before_j`, in J. This is not a heat entry.
    pub numerical_change_j: f64,
}

/// Advances both face-momentum components with the phase transport's mass ledger.
///
/// This calculation reads the source inventory and the complete detached
/// transport candidate. It checks each candidate cell mass against its signed
/// primal ledger and each volume flux against the supplied velocity and `dt_s`
/// before building dual mass and momentum. It rejects open
/// boundaries until they have a mass and momentum reservoir contract. The
/// returned candidate does not change any authoritative scene state.
pub fn momentum_candidate(
    source: &TransportInventory,
    transport: &TransportCandidate,
    fields: &PressureFields,
    velocity_m_s: &FaceValues,
    dt_s: f64,
) -> Result<MomentumCandidate, MomentumError> {
    if !dt_s.is_finite() || dt_s <= 0.0 {
        return Err(MomentumError::InvalidTimeStep);
    }
    let grid = source.grid();
    if transport.inventory.grid() != grid || fields.grid() != grid {
        return Err(MomentumError::GridMismatch);
    }
    if source.liquid_density_kg_m3() != transport.inventory.liquid_density_kg_m3()
        || source.carrier_density_kg_m3() != transport.inventory.carrier_density_kg_m3()
        || source.fixed_wall() != transport.inventory.fixed_wall()
    {
        return Err(MomentumError::MaterialOrWallMismatch);
    }
    if fields
        .boundaries()
        .iter()
        .any(|side| *side != Boundary::Closed)
    {
        return Err(MomentumError::UnsupportedOpenBoundary);
    }
    if !transport.max_face_cfl.is_finite() || transport.max_face_cfl < 0.0 {
        return Err(MomentumError::NonFiniteInput {
            field: "max_face_cfl",
            index: 0,
        });
    }

    validate_face_inputs(grid, source, fields, velocity_m_s, &transport.fluxes, dt_s)?;
    validate_cell_ledger(source, &transport.inventory, &transport.fluxes)?;

    let primal_mass_flux = total_primal_mass_flux(grid, &transport.fluxes)?;
    let (dual_mass_fluxes, subfaces) = dual_mass_fluxes(grid, &primal_mass_flux)?;
    let dual_mass_before_kg = dual_mass(grid, source)?;
    let dual_mass_after_kg = dual_mass(grid, &transport.inventory)?;

    let u_width = grid.width() as usize + 1;
    let u_height = grid.height() as usize;
    let (u_velocity, u_wall_impulse) = advect_component(
        "u",
        u_width,
        u_height,
        &dual_mass_before_kg.u,
        &dual_mass_after_kg.u,
        &dual_mass_fluxes.u,
        &subfaces.u,
        &velocity_m_s.u,
        &fields.aperture().u,
    )?;
    let v_width = grid.width() as usize;
    let v_height = grid.height() as usize + 1;
    let (v_velocity, v_wall_impulse) = advect_component(
        "v",
        v_width,
        v_height,
        &dual_mass_before_kg.v,
        &dual_mass_after_kg.v,
        &dual_mass_fluxes.v,
        &subfaces.v,
        &velocity_m_s.v,
        &fields.aperture().v,
    )?;

    let kinetic_before_j = kinetic_energy(
        &dual_mass_before_kg,
        velocity_m_s,
        "kinetic energy before advection",
    )?;
    let velocity_after = FaceValues {
        u: u_velocity,
        v: v_velocity,
    };
    let kinetic_after_j = kinetic_energy(
        &dual_mass_after_kg,
        &velocity_after,
        "kinetic energy after advection",
    )?;
    let numerical_change_j = kinetic_after_j - kinetic_before_j;
    if !numerical_change_j.is_finite() {
        return Err(MomentumError::NonFiniteResult {
            field: "kinetic energy change",
            index: 0,
        });
    }

    Ok(MomentumCandidate {
        velocity_m_s: velocity_after,
        dual_mass_before_kg,
        dual_mass_after_kg,
        dual_mass_fluxes,
        wall_impulse_kg_m_s: [u_wall_impulse, v_wall_impulse],
        kinetic_before_j,
        kinetic_after_j,
        numerical_change_j,
    })
}

fn kinetic_energy(
    dual_mass_kg: &FaceValues,
    velocity_m_s: &FaceValues,
    field: &'static str,
) -> Result<f64, MomentumError> {
    let mut energy = 0.0;
    for (index, (mass, speed)) in dual_mass_kg
        .u
        .iter()
        .zip(&velocity_m_s.u)
        .chain(dual_mass_kg.v.iter().zip(&velocity_m_s.v))
        .enumerate()
    {
        let contribution = 0.5 * mass * speed * speed;
        energy += contribution;
        if !contribution.is_finite() || !energy.is_finite() {
            return Err(MomentumError::NonFiniteResult { field, index });
        }
    }
    Ok(energy)
}

fn check_length(field: &'static str, actual: usize, expected: usize) -> Result<(), MomentumError> {
    if actual != expected {
        return Err(MomentumError::Length {
            field,
            expected,
            actual,
        });
    }
    Ok(())
}

fn check_finite(field: &'static str, values: &[f64]) -> Result<(), MomentumError> {
    for (index, value) in values.iter().enumerate() {
        if !value.is_finite() {
            return Err(MomentumError::NonFiniteInput { field, index });
        }
    }
    Ok(())
}

fn validate_face_inputs(
    grid: Grid,
    source: &TransportInventory,
    fields: &PressureFields,
    velocity: &FaceValues,
    fluxes: &TransportFaceFluxes,
    dt_s: f64,
) -> Result<(), MomentumError> {
    let u_count = grid.u_faces();
    let v_count = grid.v_faces();
    for (name, face_values) in [
        ("velocity", velocity),
        ("flux.volume_m3", &fluxes.volume_m3),
        ("flux.liquid_mass_kg", &fluxes.liquid_mass_kg),
        ("flux.carrier_mass_kg", &fluxes.carrier_mass_kg),
        ("flux.liquid_marker", &fluxes.liquid_marker),
        ("flux.carrier_marker", &fluxes.carrier_marker),
    ] {
        check_length(name, face_values.u.len(), u_count)?;
        check_length(name, face_values.v.len(), v_count)?;
        check_finite(name, &face_values.u)?;
        check_finite(name, &face_values.v)?;
    }

    let width = grid.width() as usize;
    let height = grid.height() as usize;
    for (axis, count) in [("u", u_count), ("v", v_count)] {
        for index in 0..count {
            let (speed, open, volume, liquid, carrier, liquid_marker, carrier_marker) =
                if axis == "u" {
                    (
                        velocity.u[index],
                        fields.aperture().u[index],
                        fluxes.volume_m3.u[index],
                        fluxes.liquid_mass_kg.u[index],
                        fluxes.carrier_mass_kg.u[index],
                        fluxes.liquid_marker.u[index],
                        fluxes.carrier_marker.u[index],
                    )
                } else {
                    (
                        velocity.v[index],
                        fields.aperture().v[index],
                        fluxes.volume_m3.v[index],
                        fluxes.liquid_mass_kg.v[index],
                        fluxes.carrier_mass_kg.v[index],
                        fluxes.liquid_marker.v[index],
                        fluxes.carrier_marker.v[index],
                    )
                };
            let outer = if axis == "u" {
                index % (width + 1) == 0 || index % (width + 1) == width
            } else {
                index / width == 0 || index / width == height
            };
            let any_flux = [volume, liquid, carrier, liquid_marker, carrier_marker]
                .iter()
                .any(|value| *value != 0.0);
            if (outer || open == 0.0) && (speed != 0.0 || any_flux || (outer && open != 0.0)) {
                return Err(MomentumError::InvalidFace {
                    axis,
                    index,
                    reason: "closed or blocked face carries normal motion or flux",
                });
            }
            if !(0.0..=1.0).contains(&open) {
                return Err(MomentumError::InvalidFace {
                    axis,
                    index,
                    reason: "aperture outside zero to one",
                });
            }
            if volume != 0.0 && (speed == 0.0 || volume.signum() != speed.signum()) {
                return Err(MomentumError::InvalidFace {
                    axis,
                    index,
                    reason: "volume flux disagrees with face velocity",
                });
            }
            let expected_volume = speed * dt_s * CELL_WIDTH_M * REPRESENTED_DEPTH_M * open;
            if !expected_volume.is_finite() {
                return Err(MomentumError::NonFiniteResult {
                    field: "expected face volume flux",
                    index,
                });
            }
            if !near_ledger(volume, expected_volume, &[volume, expected_volume]) {
                return Err(MomentumError::InvalidFace {
                    axis,
                    index,
                    reason: "volume flux disagrees with velocity and time step",
                });
            }
            for phase_flux in [liquid, carrier, liquid_marker, carrier_marker] {
                if phase_flux != 0.0 && (volume == 0.0 || phase_flux.signum() != volume.signum()) {
                    return Err(MomentumError::InvalidFace {
                        axis,
                        index,
                        reason: "phase or marker flux disagrees with volume flux",
                    });
                }
            }
        }
    }

    for cell in 0..grid.cells() {
        if !source.fixed_wall()[cell] {
            continue;
        }
        let x = cell % width;
        let y = cell / width;
        for face in [y * (width + 1) + x, y * (width + 1) + x + 1] {
            if fields.aperture().u[face] != 0.0 {
                return Err(MomentumError::InvalidFace {
                    axis: "u",
                    index: face,
                    reason: "fixed-wall incident aperture is open",
                });
            }
        }
        for face in [y * width + x, (y + 1) * width + x] {
            if fields.aperture().v[face] != 0.0 {
                return Err(MomentumError::InvalidFace {
                    axis: "v",
                    index: face,
                    reason: "fixed-wall incident aperture is open",
                });
            }
        }
    }
    Ok(())
}

fn validate_cell_ledger(
    source: &TransportInventory,
    after: &TransportInventory,
    fluxes: &TransportFaceFluxes,
) -> Result<(), MomentumError> {
    let grid = source.grid();
    let width = grid.width() as usize;
    for cell in 0..grid.cells() {
        let x = cell % width;
        let y = cell / width;
        let left = y * (width + 1) + x;
        let right = left + 1;
        let top = y * width + x;
        let bottom = top + width;
        for (phase, before, actual, flux) in [
            (
                "liquid",
                source.liquid_mass_kg()[cell],
                after.liquid_mass_kg()[cell],
                &fluxes.liquid_mass_kg,
            ),
            (
                "carrier",
                source.carrier_mass_kg()[cell],
                after.carrier_mass_kg()[cell],
                &fluxes.carrier_mass_kg,
            ),
        ] {
            let predicted = before + flux.u[left] - flux.u[right] + flux.v[top] - flux.v[bottom];
            if !predicted.is_finite() {
                return Err(MomentumError::NonFiniteResult {
                    field: "cell mass ledger",
                    index: cell,
                });
            }
            if !near_ledger(
                actual,
                predicted,
                &[
                    before,
                    flux.u[left],
                    flux.u[right],
                    flux.v[top],
                    flux.v[bottom],
                ],
            ) {
                return Err(MomentumError::InconsistentCellLedger { phase, cell });
            }
        }
    }
    Ok(())
}

fn near_ledger(actual: f64, expected: f64, terms: &[f64]) -> bool {
    let scale = terms
        .iter()
        .fold(actual.abs().max(expected.abs()), |scale, value| {
            scale.max(value.abs())
        });
    (actual - expected).abs() <= LEDGER_REL_TOL * scale
}

fn total_primal_mass_flux(
    grid: Grid,
    fluxes: &TransportFaceFluxes,
) -> Result<FaceValues, MomentumError> {
    let mut total = FaceValues {
        u: vec![0.0; grid.u_faces()],
        v: vec![0.0; grid.v_faces()],
    };
    for (axis, output, liquid, carrier) in [
        (
            "u",
            &mut total.u,
            &fluxes.liquid_mass_kg.u,
            &fluxes.carrier_mass_kg.u,
        ),
        (
            "v",
            &mut total.v,
            &fluxes.liquid_mass_kg.v,
            &fluxes.carrier_mass_kg.v,
        ),
    ] {
        for (index, ((out, liquid), carrier)) in
            output.iter_mut().zip(liquid).zip(carrier).enumerate()
        {
            *out = liquid + carrier;
            if !out.is_finite() {
                return Err(MomentumError::NonFiniteResult {
                    field: if axis == "u" {
                        "u mass flux"
                    } else {
                        "v mass flux"
                    },
                    index,
                });
            }
        }
    }
    Ok(total)
}

fn dual_mass(grid: Grid, inventory: &TransportInventory) -> Result<FaceValues, MomentumError> {
    let width = grid.width() as usize;
    let height = grid.height() as usize;
    let mut cell_mass = vec![0.0; grid.cells()];
    for (cell, total) in cell_mass.iter_mut().enumerate() {
        *total = inventory.liquid_mass_kg()[cell] + inventory.carrier_mass_kg()[cell];
        if !total.is_finite() {
            return Err(MomentumError::NonFiniteResult {
                field: "cell total mass",
                index: cell,
            });
        }
    }
    let mut dual = FaceValues {
        u: vec![0.0; grid.u_faces()],
        v: vec![0.0; grid.v_faces()],
    };
    for y in 0..height {
        for x in 0..=width {
            let face = y * (width + 1) + x;
            dual.u[face] = if x == 0 {
                0.5 * cell_mass[y * width]
            } else if x == width {
                0.5 * cell_mass[y * width + width - 1]
            } else {
                0.5 * cell_mass[y * width + x - 1] + 0.5 * cell_mass[y * width + x]
            };
            if !dual.u[face].is_finite() {
                return Err(MomentumError::NonFiniteResult {
                    field: "u dual mass",
                    index: face,
                });
            }
        }
    }
    for y in 0..=height {
        for x in 0..width {
            let face = y * width + x;
            dual.v[face] = if y == 0 {
                0.5 * cell_mass[x]
            } else if y == height {
                0.5 * cell_mass[(height - 1) * width + x]
            } else {
                0.5 * cell_mass[(y - 1) * width + x] + 0.5 * cell_mass[y * width + x]
            };
            if !dual.v[face].is_finite() {
                return Err(MomentumError::NonFiniteResult {
                    field: "v dual mass",
                    index: face,
                });
            }
        }
    }
    Ok(dual)
}

fn dual_mass_fluxes(
    grid: Grid,
    primal: &FaceValues,
) -> Result<(DualMassFluxes, DualSubfaceLedger), MomentumError> {
    let width = grid.width() as usize;
    let height = grid.height() as usize;
    let mut u = DualFaceFluxes {
        x_mass_kg: vec![0.0; (width + 2) * height],
        y_mass_kg: vec![0.0; (width + 1) * (height + 1)],
    };
    let mut v = DualFaceFluxes {
        x_mass_kg: vec![0.0; (width + 1) * (height + 1)],
        y_mass_kg: vec![0.0; width * (height + 2)],
    };
    let mut u_subfaces = DualSubfaceFluxes {
        x_mass_kg: vec![[0.0; 2]; u.x_mass_kg.len()],
        y_mass_kg: vec![[0.0; 2]; u.y_mass_kg.len()],
    };
    let mut v_subfaces = DualSubfaceFluxes {
        x_mass_kg: vec![[0.0; 2]; v.x_mass_kg.len()],
        y_mass_kg: vec![[0.0; 2]; v.y_mass_kg.len()],
    };

    for y in 0..height {
        for edge_x in 1..=width {
            let first = y * (width + 1) + edge_x - 1;
            let edge = y * (width + 2) + edge_x;
            set_dual_flux(
                &mut u.x_mass_kg[edge],
                &mut u_subfaces.x_mass_kg[edge],
                primal.u[first],
                primal.u[first + 1],
                "u dual x flux",
                edge,
            )?;
        }
    }
    for edge_y in 0..=height {
        for x in 0..=width {
            let first = if x == 0 {
                0.0
            } else {
                primal.v[edge_y * width + x - 1]
            };
            let second = if x == width {
                0.0
            } else {
                primal.v[edge_y * width + x]
            };
            let edge = edge_y * (width + 1) + x;
            set_dual_flux(
                &mut u.y_mass_kg[edge],
                &mut u_subfaces.y_mass_kg[edge],
                first,
                second,
                "u dual y flux",
                edge,
            )?;
        }
    }
    for y in 0..=height {
        for edge_x in 0..=width {
            let first = if y == 0 {
                0.0
            } else {
                primal.u[(y - 1) * (width + 1) + edge_x]
            };
            let second = if y == height {
                0.0
            } else {
                primal.u[y * (width + 1) + edge_x]
            };
            let edge = y * (width + 1) + edge_x;
            set_dual_flux(
                &mut v.x_mass_kg[edge],
                &mut v_subfaces.x_mass_kg[edge],
                first,
                second,
                "v dual x flux",
                edge,
            )?;
        }
    }
    for edge_y in 1..=height {
        for x in 0..width {
            let first = (edge_y - 1) * width + x;
            let edge = edge_y * width + x;
            set_dual_flux(
                &mut v.y_mass_kg[edge],
                &mut v_subfaces.y_mass_kg[edge],
                primal.v[first],
                primal.v[first + width],
                "v dual y flux",
                edge,
            )?;
        }
    }

    Ok((
        DualMassFluxes { u, v },
        DualSubfaceLedger {
            u: u_subfaces,
            v: v_subfaces,
        },
    ))
}

fn set_dual_flux(
    net: &mut f64,
    subfaces: &mut [f64; 2],
    first_primal: f64,
    second_primal: f64,
    field: &'static str,
    index: usize,
) -> Result<(), MomentumError> {
    *subfaces = [0.5 * first_primal, 0.5 * second_primal];
    *net = subfaces[0] + subfaces[1];
    if !subfaces[0].is_finite() || !subfaces[1].is_finite() || !net.is_finite() {
        return Err(MomentumError::NonFiniteResult { field, index });
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn advect_component(
    axis: &'static str,
    width: usize,
    height: usize,
    mass_before: &[f64],
    mass_after: &[f64],
    flux: &DualFaceFluxes,
    subfaces: &DualSubfaceFluxes,
    velocity_before: &[f64],
    aperture: &[f64],
) -> Result<(Vec<f64>, f64), MomentumError> {
    let mut momentum = vec![0.0; width * height];
    for index in 0..momentum.len() {
        check_dual_mass_balance(axis, index, width, mass_before, mass_after, flux)?;
        check_dual_outgoing(axis, index, width, mass_before, flux, subfaces)?;
        momentum[index] = mass_before[index] * velocity_before[index];
        if !momentum[index].is_finite() {
            return Err(MomentumError::NonFiniteResult {
                field: "initial dual momentum",
                index,
            });
        }
    }

    for y in 0..height {
        for edge_x in 1..width {
            let edge = y * (width + 1) + edge_x;
            let left = y * width + edge_x - 1;
            let right = left + 1;
            let x_fluxes = if axis == "u" {
                [flux.x_mass_kg[edge], 0.0]
            } else {
                subfaces.x_mass_kg[edge]
            };
            for mass_flux in x_fluxes {
                transfer_momentum(
                    &mut momentum,
                    velocity_before,
                    mass_flux,
                    left,
                    right,
                    "dual x momentum flux",
                    edge,
                )?;
            }
        }
    }
    for edge_y in 1..height {
        for x in 0..width {
            let edge = edge_y * width + x;
            let top = (edge_y - 1) * width + x;
            let bottom = top + width;
            let y_fluxes = if axis == "v" {
                [flux.y_mass_kg[edge], 0.0]
            } else {
                subfaces.y_mass_kg[edge]
            };
            for mass_flux in y_fluxes {
                transfer_momentum(
                    &mut momentum,
                    velocity_before,
                    mass_flux,
                    top,
                    bottom,
                    "dual y momentum flux",
                    edge,
                )?;
            }
        }
    }

    let mut velocity_after = vec![0.0; width * height];
    let mut wall_impulse = 0.0;
    for index in 0..velocity_after.len() {
        if aperture[index] == 0.0 {
            wall_impulse -= momentum[index];
            if !wall_impulse.is_finite() {
                return Err(MomentumError::NonFiniteResult {
                    field: "wall impulse",
                    index,
                });
            }
        } else {
            if mass_before[index] <= 0.0 || mass_after[index] <= 0.0 {
                return Err(MomentumError::EmptyOpenFace { axis, index });
            }
            velocity_after[index] = momentum[index] / mass_after[index];
            if !velocity_after[index].is_finite() {
                return Err(MomentumError::NonFiniteResult {
                    field: "advected face velocity",
                    index,
                });
            }
        }
    }
    Ok((velocity_after, wall_impulse))
}

fn check_dual_outgoing(
    axis: &'static str,
    index: usize,
    width: usize,
    mass_before: &[f64],
    flux: &DualFaceFluxes,
    subfaces: &DualSubfaceFluxes,
) -> Result<(), MomentumError> {
    let x = index % width;
    let y = index / width;
    let left = subfaces.x_mass_kg[y * (width + 1) + x];
    let right = subfaces.x_mass_kg[y * (width + 1) + x + 1];
    let top = subfaces.y_mass_kg[y * width + x];
    let bottom = subfaces.y_mass_kg[(y + 1) * width + x];
    let outgoing_x = if axis == "u" {
        (-flux.x_mass_kg[y * (width + 1) + x]).max(0.0)
            + flux.x_mass_kg[y * (width + 1) + x + 1].max(0.0)
    } else {
        left.into_iter().map(|value| (-value).max(0.0)).sum::<f64>()
            + right.into_iter().map(|value| value.max(0.0)).sum::<f64>()
    };
    let outgoing_y = if axis == "v" {
        (-flux.y_mass_kg[y * width + x]).max(0.0) + flux.y_mass_kg[(y + 1) * width + x].max(0.0)
    } else {
        top.into_iter().map(|value| (-value).max(0.0)).sum::<f64>()
            + bottom.into_iter().map(|value| value.max(0.0)).sum::<f64>()
    };
    let outgoing = outgoing_x + outgoing_y;
    if !outgoing.is_finite() {
        return Err(MomentumError::NonFiniteResult {
            field: "dual outgoing mass",
            index,
        });
    }
    if outgoing > mass_before[index] * (1.0 + LEDGER_REL_TOL) {
        return Err(MomentumError::DualDonorOverdraw { axis, index });
    }
    Ok(())
}

fn check_dual_mass_balance(
    axis: &'static str,
    index: usize,
    width: usize,
    mass_before: &[f64],
    mass_after: &[f64],
    flux: &DualFaceFluxes,
) -> Result<(), MomentumError> {
    let x = index % width;
    let y = index / width;
    let left = flux.x_mass_kg[y * (width + 1) + x];
    let right = flux.x_mass_kg[y * (width + 1) + x + 1];
    let top = flux.y_mass_kg[y * width + x];
    let bottom = flux.y_mass_kg[(y + 1) * width + x];
    let predicted = mass_before[index] + left - right + top - bottom;
    if !predicted.is_finite() {
        return Err(MomentumError::NonFiniteResult {
            field: "dual mass ledger",
            index,
        });
    }
    if !near_ledger(
        mass_after[index],
        predicted,
        &[mass_before[index], left, right, top, bottom],
    ) {
        return Err(MomentumError::InconsistentDualLedger { axis, index });
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn transfer_momentum(
    momentum: &mut [f64],
    velocity: &[f64],
    mass_flux: f64,
    negative_cell: usize,
    positive_cell: usize,
    field: &'static str,
    edge: usize,
) -> Result<(), MomentumError> {
    if mass_flux == 0.0 {
        return Ok(());
    }
    let donor = if mass_flux > 0.0 {
        negative_cell
    } else {
        positive_cell
    };
    let momentum_flux = mass_flux * velocity[donor];
    let negative_next = momentum[negative_cell] - momentum_flux;
    let positive_next = momentum[positive_cell] + momentum_flux;
    if !momentum_flux.is_finite() || !negative_next.is_finite() || !positive_next.is_finite() {
        return Err(MomentumError::NonFiniteResult { field, index: edge });
    }
    momentum[negative_cell] = negative_next;
    momentum[positive_cell] = positive_next;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use particle_sim::{CELL_WIDTH_M, contracts::Boundary};

    use crate::transport::CELL_VOLUME_M3;

    use super::*;

    fn closed_aperture(grid: Grid) -> FaceValues {
        let width = grid.width() as usize;
        let height = grid.height() as usize;
        let mut aperture = FaceValues {
            u: vec![1.0; grid.u_faces()],
            v: vec![1.0; grid.v_faces()],
        };
        for y in 0..height {
            aperture.u[y * (width + 1)] = 0.0;
            aperture.u[y * (width + 1) + width] = 0.0;
        }
        for x in 0..width {
            aperture.v[x] = 0.0;
            aperture.v[height * width + x] = 0.0;
        }
        aperture
    }

    fn inventory(grid: Grid, alpha: &[f64]) -> TransportInventory {
        TransportInventory::new(
            grid,
            1000.0,
            1.0,
            alpha
                .iter()
                .map(|fraction| fraction * CELL_VOLUME_M3 * 1000.0)
                .collect(),
            alpha
                .iter()
                .map(|fraction| (1.0 - fraction) * CELL_VOLUME_M3)
                .collect(),
            vec![0.0; grid.cells()],
            vec![0.0; grid.cells()],
            vec![false; grid.cells()],
        )
        .unwrap()
    }

    fn zero_velocity(grid: Grid) -> FaceValues {
        FaceValues {
            u: vec![0.0; grid.u_faces()],
            v: vec![0.0; grid.v_faces()],
        }
    }

    fn pressure_fields(
        grid: Grid,
        boundaries: [Boundary; 4],
        aperture: FaceValues,
    ) -> PressureFields {
        PressureFields::new(
            grid,
            boundaries,
            vec![1.0; grid.cells()],
            vec![0.0; grid.cells()],
            zero_velocity(grid),
            aperture,
        )
        .unwrap()
    }

    fn zero_fluxes(grid: Grid) -> TransportFaceFluxes {
        let zeros = || FaceValues {
            u: vec![0.0; grid.u_faces()],
            v: vec![0.0; grid.v_faces()],
        };
        TransportFaceFluxes {
            volume_m3: zeros(),
            liquid_mass_kg: zeros(),
            carrier_mass_kg: zeros(),
            liquid_marker: zeros(),
            carrier_marker: zeros(),
        }
    }

    fn vortex_velocity(grid: Grid) -> FaceValues {
        let width = grid.width() as usize;
        let height = grid.height() as usize;
        let stream = |x: usize, y: usize| {
            if x == 0 || x == width || y == 0 || y == height {
                0.0
            } else {
                1e-5 * (std::f64::consts::PI * x as f64 / width as f64).sin()
                    * (std::f64::consts::PI * y as f64 / height as f64).sin()
            }
        };
        let mut velocity = zero_velocity(grid);
        for y in 0..height {
            for x in 0..=width {
                velocity.u[y * (width + 1) + x] = (stream(x, y + 1) - stream(x, y)) / CELL_WIDTH_M;
            }
        }
        for y in 0..=height {
            for x in 0..width {
                velocity.v[y * width + x] = -(stream(x + 1, y) - stream(x, y)) / CELL_WIDTH_M;
            }
        }
        velocity
    }

    fn total_momentum(mass: &[f64], velocity: &[f64]) -> f64 {
        mass.iter()
            .zip(velocity)
            .map(|(mass, speed)| mass * speed)
            .sum()
    }

    fn assert_accounted_momentum(
        before_mass: &[f64],
        before_velocity: &[f64],
        after_mass: &[f64],
        after_velocity: &[f64],
        wall_impulse: f64,
    ) {
        let before = total_momentum(before_mass, before_velocity);
        let after = total_momentum(after_mass, after_velocity);
        let scale: f64 = before_mass
            .iter()
            .zip(before_velocity)
            .map(|(mass, speed)| (mass * speed).abs())
            .sum::<f64>()
            + after_mass
                .iter()
                .zip(after_velocity)
                .map(|(mass, speed)| (mass * speed).abs())
                .sum::<f64>()
            + wall_impulse.abs();
        assert!(
            (after - before - wall_impulse).abs() <= 1e-12 * scale,
            "before {before}, after {after}, wall impulse {wall_impulse}, scale {scale}"
        );
    }

    fn assert_near(actual: f64, expected: f64) {
        let scale = actual.abs().max(expected.abs()).max(1e-12);
        assert!(
            (actual - expected).abs() <= 1e-11 * scale,
            "expected {expected}, got {actual}"
        );
    }

    fn upwind_momentum(mass_flux: f64, negative_velocity: f64, positive_velocity: f64) -> f64 {
        mass_flux
            * if mass_flux >= 0.0 {
                negative_velocity
            } else {
                positive_velocity
            }
    }

    #[test]
    fn constant_velocity_is_preserved_when_dual_mass_moves() {
        let flux = DualFaceFluxes {
            x_mass_kg: vec![0.0, 0.2, 0.2, 0.0],
            y_mass_kg: vec![0.0; 6],
        };
        let subfaces = DualSubfaceFluxes {
            x_mass_kg: vec![[0.0; 2], [0.2, 0.0], [0.2, 0.0], [0.0; 2]],
            y_mass_kg: vec![[0.0; 2]; 6],
        };
        let (velocity, wall_impulse) = advect_component(
            "u",
            3,
            1,
            &[1.0, 1.0, 1.0],
            &[0.8, 1.0, 1.2],
            &flux,
            &subfaces,
            &[2.0, 2.0, 2.0],
            &[1.0, 1.0, 1.0],
        )
        .unwrap();
        for speed in velocity {
            assert_near(speed, 2.0);
        }
        assert_eq!(wall_impulse, 0.0);
    }

    #[test]
    fn circulation_uses_transverse_dual_flux_and_accounts_for_wall_impulse() {
        let grid = Grid::new(4.0, 4.0).unwrap();
        let source = inventory(grid, &vec![0.5; grid.cells()]);
        let velocity = vortex_velocity(grid);
        let fields = pressure_fields(grid, [Boundary::Closed; 4], closed_aperture(grid));
        let transport = source.candidate(&fields, &velocity, 0.001).unwrap();
        let result = momentum_candidate(&source, &transport, &fields, &velocity, 0.001).unwrap();

        assert!(
            result
                .dual_mass_fluxes
                .u
                .y_mass_kg
                .iter()
                .any(|v| *v != 0.0)
        );
        assert!(
            result
                .dual_mass_fluxes
                .v
                .x_mass_kg
                .iter()
                .any(|v| *v != 0.0)
        );
        assert_accounted_momentum(
            &result.dual_mass_before_kg.u,
            &velocity.u,
            &result.dual_mass_after_kg.u,
            &result.velocity_m_s.u,
            result.wall_impulse_kg_m_s[0],
        );
        assert_accounted_momentum(
            &result.dual_mass_before_kg.v,
            &velocity.v,
            &result.dual_mass_after_kg.v,
            &result.velocity_m_s.v,
            result.wall_impulse_kg_m_s[1],
        );
        for (open, speed) in fields.aperture().u.iter().zip(&result.velocity_m_s.u) {
            if *open == 0.0 {
                assert_eq!(*speed, 0.0);
            }
        }
        for (open, speed) in fields.aperture().v.iter().zip(&result.velocity_m_s.v) {
            if *open == 0.0 {
                assert_eq!(*speed, 0.0);
            }
        }
    }

    #[test]
    fn opposite_transverse_subface_fluxes_still_transfer_vortex_momentum() {
        let grid = Grid::new(2.0, 2.0).unwrap();
        let source = inventory(grid, &vec![0.5; grid.cells()]);
        let velocity = vortex_velocity(grid);
        let fields = pressure_fields(grid, [Boundary::Closed; 4], closed_aperture(grid));
        let transport = source.candidate(&fields, &velocity, 0.001).unwrap();
        let result = momentum_candidate(&source, &transport, &fields, &velocity, 0.001).unwrap();
        let primal_u = |x: u32, y: u32| {
            let face = grid.u_face_index(x, y).unwrap();
            transport.fluxes.liquid_mass_kg.u[face] + transport.fluxes.carrier_mass_kg.u[face]
        };
        let primal_v = |x: u32, y: u32| {
            let face = grid.v_face_index(x, y).unwrap();
            transport.fluxes.liquid_mass_kg.v[face] + transport.fluxes.carrier_mass_kg.v[face]
        };

        let top_u = grid.u_face_index(1, 0).unwrap();
        let bottom_u = grid.u_face_index(1, 1).unwrap();
        let transverse_edge = grid.u_face_index(1, 1).unwrap();
        assert_near(result.dual_mass_fluxes.u.y_mass_kg[transverse_edge], 0.0);
        let u_left_momentum = upwind_momentum(
            0.5 * primal_u(0, 0),
            velocity.u[grid.u_face_index(0, 0).unwrap()],
            velocity.u[top_u],
        ) + upwind_momentum(
            0.5 * primal_u(1, 0),
            velocity.u[grid.u_face_index(0, 0).unwrap()],
            velocity.u[top_u],
        );
        let u_right_momentum = upwind_momentum(
            0.5 * primal_u(1, 0),
            velocity.u[top_u],
            velocity.u[grid.u_face_index(2, 0).unwrap()],
        ) + upwind_momentum(
            0.5 * primal_u(2, 0),
            velocity.u[top_u],
            velocity.u[grid.u_face_index(2, 0).unwrap()],
        );
        let u_bottom_momentum = upwind_momentum(
            0.5 * primal_v(0, 1),
            velocity.u[top_u],
            velocity.u[bottom_u],
        ) + upwind_momentum(
            0.5 * primal_v(1, 1),
            velocity.u[top_u],
            velocity.u[bottom_u],
        );
        assert!(u_bottom_momentum > 0.0);
        let expected_u = (result.dual_mass_before_kg.u[top_u] * velocity.u[top_u]
            + u_left_momentum
            - u_right_momentum
            - u_bottom_momentum)
            / result.dual_mass_after_kg.u[top_u];
        assert_near(result.velocity_m_s.u[top_u], expected_u);

        let left_v = grid.v_face_index(0, 1).unwrap();
        let right_v = grid.v_face_index(1, 1).unwrap();
        assert_near(result.dual_mass_fluxes.v.x_mass_kg[transverse_edge], 0.0);
        let v_right_momentum = upwind_momentum(
            0.5 * primal_u(1, 0),
            velocity.v[left_v],
            velocity.v[right_v],
        ) + upwind_momentum(
            0.5 * primal_u(1, 1),
            velocity.v[left_v],
            velocity.v[right_v],
        );
        let v_top_momentum = upwind_momentum(
            0.5 * primal_v(0, 0),
            velocity.v[grid.v_face_index(0, 0).unwrap()],
            velocity.v[left_v],
        ) + upwind_momentum(
            0.5 * primal_v(0, 1),
            velocity.v[grid.v_face_index(0, 0).unwrap()],
            velocity.v[left_v],
        );
        let v_bottom_momentum = upwind_momentum(
            0.5 * primal_v(0, 1),
            velocity.v[left_v],
            velocity.v[grid.v_face_index(0, 2).unwrap()],
        ) + upwind_momentum(
            0.5 * primal_v(0, 2),
            velocity.v[left_v],
            velocity.v[grid.v_face_index(0, 2).unwrap()],
        );
        assert!(v_right_momentum < 0.0);
        let expected_v = (result.dual_mass_before_kg.v[left_v] * velocity.v[left_v]
            - v_right_momentum
            + v_top_momentum
            - v_bottom_momentum)
            / result.dual_mass_after_kg.v[left_v];
        assert_near(result.velocity_m_s.v[left_v], expected_v);
    }

    #[test]
    fn opposite_aligned_samples_cancel_before_upwinding() {
        let grid = Grid::new(3.0, 2.0).unwrap();
        let source = inventory(grid, &vec![0.5; grid.cells()]);
        let mut velocity = zero_velocity(grid);
        let speed = 0.001;
        for (x, y, value) in [(1, 0, speed), (2, 0, -speed), (1, 1, -speed), (2, 1, speed)] {
            velocity.u[grid.u_face_index(x, y).unwrap()] = value;
        }
        for (x, value) in [(0, -speed), (1, 2.0 * speed), (2, -speed)] {
            velocity.v[grid.v_face_index(x, 1).unwrap()] = value;
        }
        let fields = pressure_fields(grid, [Boundary::Closed; 4], closed_aperture(grid));
        let transport = source.candidate(&fields, &velocity, 0.001).unwrap();
        let result = momentum_candidate(&source, &transport, &fields, &velocity, 0.001).unwrap();
        let primal_u = |x: u32, y: u32| {
            let face = grid.u_face_index(x, y).unwrap();
            transport.fluxes.liquid_mass_kg.u[face] + transport.fluxes.carrier_mass_kg.u[face]
        };
        let primal_v = |x: u32, y: u32| {
            let face = grid.v_face_index(x, y).unwrap();
            transport.fluxes.liquid_mass_kg.v[face] + transport.fluxes.carrier_mass_kg.v[face]
        };

        let center = grid.u_face_index(1, 0).unwrap();
        let left = grid.u_face_index(0, 0).unwrap();
        let right = grid.u_face_index(2, 0).unwrap();
        let below = grid.u_face_index(1, 1).unwrap();
        let aligned_left_mass = 0.5 * primal_u(0, 0) + 0.5 * primal_u(1, 0);
        let aligned_right_mass = 0.5 * primal_u(1, 0) + 0.5 * primal_u(2, 0);
        assert_near(aligned_right_mass, 0.0);
        let transverse_momentum =
            upwind_momentum(0.5 * primal_v(0, 1), velocity.u[center], velocity.u[below])
                + upwind_momentum(0.5 * primal_v(1, 1), velocity.u[center], velocity.u[below]);
        let expected = (result.dual_mass_before_kg.u[center] * velocity.u[center]
            + upwind_momentum(aligned_left_mass, velocity.u[left], velocity.u[center])
            - upwind_momentum(aligned_right_mass, velocity.u[center], velocity.u[right])
            - transverse_momentum)
            / result.dual_mass_after_kg.u[center];
        assert_near(result.velocity_m_s.u[center], expected);

        let v_flux = DualFaceFluxes {
            x_mass_kg: vec![0.0; 4],
            y_mass_kg: vec![0.0; 3],
        };
        let v_subfaces = DualSubfaceFluxes {
            x_mass_kg: vec![[0.0; 2]; 4],
            y_mass_kg: vec![[0.0; 2], [0.2, -0.2], [0.0; 2]],
        };
        let (v_after, wall_impulse) = advect_component(
            "v",
            1,
            2,
            &[1.0, 1.0],
            &[1.0, 1.0],
            &v_flux,
            &v_subfaces,
            &[2.0, -3.0],
            &[1.0, 1.0],
        )
        .unwrap();
        assert_eq!(v_after, vec![2.0, -3.0]);
        assert_eq!(wall_impulse, 0.0);
    }

    #[test]
    fn liquid_carrier_density_contrast_and_shear_have_bounded_face_speeds() {
        let grid = Grid::new(4.0, 4.0).unwrap();
        let mut alpha = vec![0.0; grid.cells()];
        for y in 0..4 {
            for x in 0..2 {
                alpha[grid.cell_index(x, y).unwrap()] = 1.0;
            }
        }
        let source = inventory(grid, &alpha);
        let velocity = vortex_velocity(grid);
        let fields = pressure_fields(grid, [Boundary::Closed; 4], closed_aperture(grid));
        let transport = source.candidate(&fields, &velocity, 0.001).unwrap();
        let result = momentum_candidate(&source, &transport, &fields, &velocity, 0.001).unwrap();

        let dense_face = grid.u_face_index(1, 1).unwrap();
        let light_face = grid.u_face_index(3, 1).unwrap();
        assert_near(
            result.dual_mass_before_kg.u[dense_face] / result.dual_mass_before_kg.u[light_face],
            1000.0,
        );
        assert!(transport.fluxes.liquid_mass_kg.u.iter().any(|v| *v != 0.0));
        assert!(transport.fluxes.carrier_mass_kg.u.iter().any(|v| *v != 0.0));
        let maximum_before = velocity
            .u
            .iter()
            .chain(&velocity.v)
            .fold(0.0_f64, |maximum, speed| maximum.max(speed.abs()));
        let maximum_after = result
            .velocity_m_s
            .u
            .iter()
            .chain(&result.velocity_m_s.v)
            .fold(0.0_f64, |maximum, speed| maximum.max(speed.abs()));
        assert!(maximum_after <= maximum_before * (1.0 + 1e-10));
        assert!(
            result
                .velocity_m_s
                .u
                .iter()
                .chain(&result.velocity_m_s.v)
                .all(|speed| speed.is_finite())
        );
        assert!(result.kinetic_before_j.is_finite());
        assert!(result.kinetic_after_j.is_finite());
        assert!(result.numerical_change_j.is_finite());
        assert_near(
            result.numerical_change_j,
            result.kinetic_after_j - result.kinetic_before_j,
        );
    }

    #[test]
    fn volume_ledger_must_match_velocity_and_substep_duration() {
        let grid = Grid::new(4.0, 4.0).unwrap();
        let source = inventory(grid, &vec![0.5; grid.cells()]);
        let velocity = vortex_velocity(grid);
        let fields = pressure_fields(grid, [Boundary::Closed; 4], closed_aperture(grid));
        let transport = source.candidate(&fields, &velocity, 0.001).unwrap();
        assert!(matches!(
            momentum_candidate(&source, &transport, &fields, &velocity, 0.002),
            Err(MomentumError::InvalidFace {
                reason: "volume flux disagrees with velocity and time step",
                ..
            })
        ));

        let mut different_velocity = vortex_velocity(grid);
        let face = grid.u_face_index(1, 0).unwrap();
        different_velocity.u[face] *= 2.0;
        assert!(matches!(
            momentum_candidate(&source, &transport, &fields, &different_velocity, 0.001),
            Err(MomentumError::InvalidFace {
                axis: "u",
                index,
                reason: "volume flux disagrees with velocity and time step",
            }) if index == face
        ));
        assert_eq!(
            momentum_candidate(&source, &transport, &fields, &velocity, 0.0),
            Err(MomentumError::InvalidTimeStep)
        );
    }

    #[test]
    fn malformed_fluxes_open_boundaries_and_blocked_motion_fail() {
        let grid = Grid::new(2.0, 1.0).unwrap();
        let source = inventory(grid, &[0.5, 0.5]);
        let fields = pressure_fields(grid, [Boundary::Closed; 4], closed_aperture(grid));
        let velocity = zero_velocity(grid);
        let mut transport = TransportCandidate {
            inventory: source.clone(),
            fluxes: zero_fluxes(grid),
            max_face_cfl: 0.0,
        };
        transport.fluxes.liquid_mass_kg.u.pop();
        assert!(matches!(
            momentum_candidate(&source, &transport, &fields, &velocity, 0.001),
            Err(MomentumError::Length { .. })
        ));
        transport.fluxes = zero_fluxes(grid);
        transport.fluxes.carrier_mass_kg.u[1] = f64::NAN;
        assert!(matches!(
            momentum_candidate(&source, &transport, &fields, &velocity, 0.001),
            Err(MomentumError::NonFiniteInput { .. })
        ));
        transport.fluxes = zero_fluxes(grid);

        let mut open_aperture = closed_aperture(grid);
        open_aperture.u[0] = 0.5;
        let open_fields = pressure_fields(
            grid,
            [
                Boundary::Open {
                    reservoir: NonZeroU32::new(7).unwrap(),
                },
                Boundary::Closed,
                Boundary::Closed,
                Boundary::Closed,
            ],
            open_aperture,
        );
        assert_eq!(
            momentum_candidate(&source, &transport, &open_fields, &velocity, 0.001),
            Err(MomentumError::UnsupportedOpenBoundary)
        );

        let mut blocked_aperture = closed_aperture(grid);
        blocked_aperture.u[1] = 0.0;
        let blocked_fields = pressure_fields(grid, [Boundary::Closed; 4], blocked_aperture);
        let mut bad_velocity = zero_velocity(grid);
        bad_velocity.u[1] = 1.0;
        assert!(matches!(
            momentum_candidate(&source, &transport, &blocked_fields, &bad_velocity, 0.001),
            Err(MomentumError::InvalidFace {
                axis: "u",
                index: 1,
                ..
            })
        ));
    }
}
