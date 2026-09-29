//! Detached tangential-viscosity candidate for the f64 MAC reference.
//!
//! A vertex owns one shear stress and applies equal, opposite momentum impulses
//! to its two horizontal and two vertical velocity faces. Closed and blocked
//! faces are free slip: no stress crosses their corner. Normal viscous stresses,
//! momentum advection, and conversion of lost kinetic energy to internal energy
//! are separate stages; the diagnostics here do not close a thermal ledger.

use particle_sim::{Grid, REPRESENTED_DEPTH_M, contracts::Boundary};

use crate::fluid::{FaceValues, PressureFields};

/// Rejection before a viscosity candidate can replace an owned velocity field.
#[derive(Clone, Debug, PartialEq)]
pub enum ViscosityError {
    /// Open reservoirs need a prescribed viscous traction and momentum ledger.
    UnsupportedOpenBoundary,
    /// A cell or MAC face array has the wrong length.
    Length {
        /// Rejected field name.
        field: &'static str,
        /// Required number of entries.
        expected: usize,
        /// Supplied number of entries.
        actual: usize,
    },
    /// Viscosity, time step, or an internal coefficient is not finite and positive.
    InvalidScalar(&'static str),
    /// A supplied velocity is not finite, or moves through a blocked/closed face.
    InvalidVelocity {
        /// `u` or `v` face array.
        axis: &'static str,
        /// Row-major face index.
        index: usize,
    },
    /// A face aperture is nonfinite or outside zero to one.
    InvalidAperture {
        /// `u` or `v` face array.
        axis: &'static str,
        /// Row-major face index.
        index: usize,
    },
    /// The explicit shear step would exceed its conservative diffusion bound.
    TimeStepTooLarge {
        /// Rejected step duration.
        dt_s: f64,
        /// Largest permitted explicit step for these coefficients.
        max_dt_s: f64,
    },
    /// Finite inputs produced a nonfinite stress, candidate, or diagnostic.
    NonFiniteResult(&'static str),
}

impl std::fmt::Display for ViscosityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ViscosityError {}

/// Mechanical observations for one proposed shear step, in joules.
///
/// `initial_stress_work_j` evaluates the starting shear rate over the proposed
/// time step. `kinetic_loss_j` is the actual MAC kinetic-energy difference after
/// explicit Euler. Their difference includes time-discretization effects and is
/// not deposited into a temperature or internal-energy field.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViscosityDiagnostics {
    /// MAC kinetic energy before the proposed step.
    pub kinetic_before_j: f64,
    /// MAC kinetic energy after the proposed step.
    pub kinetic_after_j: f64,
    /// Initial minus final MAC kinetic energy, including integration effects.
    pub kinetic_loss_j: f64,
    /// Initial shear stress power integrated over the proposed time step.
    pub initial_stress_work_j: f64,
    /// Conservative explicit step bound for these density and viscosity fields.
    pub max_stable_dt_s: f64,
}

/// A complete proposed face-velocity field; source arrays are unchanged.
#[derive(Debug, PartialEq)]
pub struct ViscosityCandidate {
    /// Proposed face velocities in the same MAC layout as the input.
    pub velocity_m_s: FaceValues,
    /// Mechanical energy observations for this candidate.
    pub diagnostics: ViscosityDiagnostics,
}

/// Calculates a bounded explicit tangential-viscosity step on a closed MAC grid.
///
/// `cell_dynamic_viscosity_pa_s` has one finite positive value per cell. Density,
/// apertures, and geometry come from the validated pressure owner. All outer
/// faces must be closed; a zero-aperture face receives no impulse. Fractional
/// apertures scale a corner traction by its least-open incident face, retaining
/// paired impulses but not claiming a complete cut-cell stress construction.
/// This function does not mutate `fields` or `velocity_m_s` on failure.
pub fn shear_viscosity_candidate(
    fields: &PressureFields,
    velocity_m_s: &FaceValues,
    cell_dynamic_viscosity_pa_s: &[f64],
    dt_s: f64,
) -> Result<ViscosityCandidate, ViscosityError> {
    if fields
        .boundaries()
        .iter()
        .any(|side| *side != Boundary::Closed)
    {
        return Err(ViscosityError::UnsupportedOpenBoundary);
    }
    candidate_core(
        fields,
        velocity_m_s,
        fields.aperture(),
        cell_dynamic_viscosity_pa_s,
        dt_s,
        fields.grid().cell_width_m(),
        EdgeMode::Closed,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EdgeMode {
    Closed,
    #[cfg(test)]
    Periodic,
}

impl EdgeMode {
    fn periodic(self) -> bool {
        match self {
            Self::Closed => false,
            #[cfg(test)]
            Self::Periodic => true,
        }
    }
}

fn candidate_core(
    fields: &PressureFields,
    velocity_m_s: &FaceValues,
    aperture: &FaceValues,
    cell_dynamic_viscosity_pa_s: &[f64],
    dt_s: f64,
    dx_m: f64,
    edges: EdgeMode,
) -> Result<ViscosityCandidate, ViscosityError> {
    let grid = fields.grid();
    check_len(
        "cell_dynamic_viscosity_pa_s",
        cell_dynamic_viscosity_pa_s.len(),
        grid.cells(),
    )?;
    check_len("velocity.u_m_s", velocity_m_s.u.len(), grid.u_faces())?;
    check_len("velocity.v_m_s", velocity_m_s.v.len(), grid.v_faces())?;
    check_len("aperture.u", aperture.u.len(), grid.u_faces())?;
    check_len("aperture.v", aperture.v.len(), grid.v_faces())?;
    if !dt_s.is_finite() || dt_s <= 0.0 {
        return Err(ViscosityError::InvalidScalar("dt_s"));
    }
    if !dx_m.is_finite() || dx_m <= 0.0 {
        return Err(ViscosityError::InvalidScalar("dx_m"));
    }
    let mut max_viscosity = 0.0_f64;
    for &viscosity in cell_dynamic_viscosity_pa_s {
        if !viscosity.is_finite() || viscosity <= 0.0 {
            return Err(ViscosityError::InvalidScalar("cell_dynamic_viscosity_pa_s"));
        }
        max_viscosity = max_viscosity.max(viscosity);
    }
    let mut min_density = f64::INFINITY;
    for &density in fields.density_kg_m3() {
        if !density.is_finite() || density <= 0.0 {
            return Err(ViscosityError::InvalidScalar("density_kg_m3"));
        }
        min_density = min_density.min(density);
    }
    let max_nu = max_viscosity / min_density;
    if !max_nu.is_finite() || max_nu <= 0.0 {
        return Err(ViscosityError::InvalidScalar("kinematic_viscosity_m2_s"));
    }
    // The shear-gradient operator has spectral radius at most 8 nu/dx².
    // Explicit Euler is energy stable for dt <= 2/radius.
    let max_dt_s = ((dx_m * dx_m) / 4.0 / max_nu).min(f64::MAX);
    if !max_dt_s.is_finite() || max_dt_s <= 0.0 {
        return Err(ViscosityError::InvalidScalar("max_stable_dt_s"));
    }
    if dt_s > max_dt_s {
        return Err(ViscosityError::TimeStepTooLarge { dt_s, max_dt_s });
    }
    validate_velocity(grid, velocity_m_s, aperture, edges)?;

    let width = grid.width() as usize;
    let height = grid.height() as usize;
    let density = fields.density_kg_m3();
    let mut delta = FaceValues {
        u: vec![0.0; grid.u_faces()],
        v: vec![0.0; grid.v_faces()],
    };
    let vertex_volume_m3 = dx_m * dx_m * REPRESENTED_DEPTH_M;
    if !vertex_volume_m3.is_finite() || vertex_volume_m3 <= 0.0 {
        return Err(ViscosityError::InvalidScalar("vertex_volume_m3"));
    }
    let mut initial_stress_work_j = 0.0;
    let start_x = usize::from(!edges.periodic());
    let start_y = usize::from(!edges.periodic());
    for y in start_y..height {
        let above_y = if y == 0 { height - 1 } else { y - 1 };
        for x in start_x..width {
            let left_x = if x == 0 { width - 1 } else { x - 1 };
            let u_above = above_y * (width + 1) + x;
            let u_below = y * (width + 1) + x;
            let v_left = y * width + left_x;
            let v_right = y * width + x;
            let corner_open = aperture.u[u_above]
                .min(aperture.u[u_below])
                .min(aperture.v[v_left])
                .min(aperture.v[v_right]);
            if corner_open == 0.0 {
                continue;
            }

            let cells = [
                above_y * width + left_x,
                above_y * width + x,
                y * width + left_x,
                y * width + x,
            ];
            let viscosity_pa_s = cells
                .iter()
                .map(|&cell| cell_dynamic_viscosity_pa_s[cell] / 4.0)
                .sum::<f64>();
            let shear_rate_per_s = (velocity_m_s.u[u_below] - velocity_m_s.u[u_above]
                + velocity_m_s.v[v_right]
                - velocity_m_s.v[v_left])
                / dx_m;
            let stress_pa = viscosity_pa_s * corner_open * shear_rate_per_s;
            if !stress_pa.is_finite() || !shear_rate_per_s.is_finite() {
                return Err(ViscosityError::NonFiniteResult("shear_stress_pa"));
            }
            let rho_u_above = mean(density[cells[0]], density[cells[1]]);
            let rho_u_below = mean(density[cells[2]], density[cells[3]]);
            let rho_v_left = mean(density[cells[0]], density[cells[2]]);
            let rho_v_right = mean(density[cells[1]], density[cells[3]]);
            add_impulse(&mut delta.u[u_above], dt_s, stress_pa, rho_u_above, dx_m)?;
            add_impulse(&mut delta.u[u_below], dt_s, -stress_pa, rho_u_below, dx_m)?;
            add_impulse(&mut delta.v[v_left], dt_s, stress_pa, rho_v_left, dx_m)?;
            add_impulse(&mut delta.v[v_right], dt_s, -stress_pa, rho_v_right, dx_m)?;
            initial_stress_work_j += dt_s * stress_pa * shear_rate_per_s * vertex_volume_m3;
            if !initial_stress_work_j.is_finite() {
                return Err(ViscosityError::NonFiniteResult("initial_stress_work_j"));
            }
        }
    }

    let mut candidate = FaceValues {
        u: velocity_m_s
            .u
            .iter()
            .zip(&delta.u)
            .map(|(&value, &change)| value + change)
            .collect(),
        v: velocity_m_s
            .v
            .iter()
            .zip(&delta.v)
            .map(|(&value, &change)| value + change)
            .collect(),
    };
    if edges.periodic() {
        for y in 0..height {
            candidate.u[y * (width + 1) + width] = candidate.u[y * (width + 1)];
        }
        for x in 0..width {
            candidate.v[height * width + x] = candidate.v[x];
        }
    }
    check_finite_result("candidate.u_m_s", &candidate.u)?;
    check_finite_result("candidate.v_m_s", &candidate.v)?;
    let kinetic_before_j = kinetic_energy_j(grid, density, velocity_m_s, vertex_volume_m3, edges)?;
    let kinetic_after_j = kinetic_energy_j(grid, density, &candidate, vertex_volume_m3, edges)?;
    let kinetic_loss_j = kinetic_before_j - kinetic_after_j;
    if !kinetic_loss_j.is_finite() {
        return Err(ViscosityError::NonFiniteResult("kinetic_loss_j"));
    }
    Ok(ViscosityCandidate {
        velocity_m_s: candidate,
        diagnostics: ViscosityDiagnostics {
            kinetic_before_j,
            kinetic_after_j,
            kinetic_loss_j,
            initial_stress_work_j,
            max_stable_dt_s: max_dt_s,
        },
    })
}

fn mean(a: f64, b: f64) -> f64 {
    a + (b - a) / 2.0
}

fn add_impulse(
    velocity_delta_m_s: &mut f64,
    dt_s: f64,
    stress_pa: f64,
    density_kg_m3: f64,
    dx_m: f64,
) -> Result<(), ViscosityError> {
    *velocity_delta_m_s += dt_s * stress_pa / (density_kg_m3 * dx_m);
    if !velocity_delta_m_s.is_finite() {
        return Err(ViscosityError::NonFiniteResult("face_velocity_delta"));
    }
    Ok(())
}

fn check_len(field: &'static str, actual: usize, expected: usize) -> Result<(), ViscosityError> {
    if actual != expected {
        return Err(ViscosityError::Length {
            field,
            expected,
            actual,
        });
    }
    Ok(())
}

fn check_finite_result(field: &'static str, values: &[f64]) -> Result<(), ViscosityError> {
    if values.iter().all(|value| value.is_finite()) {
        Ok(())
    } else {
        Err(ViscosityError::NonFiniteResult(field))
    }
}

fn validate_velocity(
    grid: Grid,
    velocity: &FaceValues,
    aperture: &FaceValues,
    edges: EdgeMode,
) -> Result<(), ViscosityError> {
    let width = grid.width() as usize;
    let height = grid.height() as usize;
    for (axis, values, openings) in [
        ("u", &velocity.u, &aperture.u),
        ("v", &velocity.v, &aperture.v),
    ] {
        for (index, (&value, &opening)) in values.iter().zip(openings).enumerate() {
            if !opening.is_finite() || !(0.0..=1.0).contains(&opening) {
                return Err(ViscosityError::InvalidAperture { axis, index });
            }
            if !value.is_finite() || (opening == 0.0 && value != 0.0) {
                return Err(ViscosityError::InvalidVelocity { axis, index });
            }
        }
    }
    if edges.periodic() {
        for y in 0..height {
            let first = y * (width + 1);
            let last = first + width;
            if velocity.u[first] != velocity.u[last] || aperture.u[first] != aperture.u[last] {
                return Err(ViscosityError::InvalidVelocity {
                    axis: "u",
                    index: last,
                });
            }
        }
        for x in 0..width {
            let last = height * width + x;
            if velocity.v[x] != velocity.v[last] || aperture.v[x] != aperture.v[last] {
                return Err(ViscosityError::InvalidVelocity {
                    axis: "v",
                    index: last,
                });
            }
        }
    } else {
        for y in 0..height {
            for x in [0, width] {
                let index = y * (width + 1) + x;
                if velocity.u[index] != 0.0 {
                    return Err(ViscosityError::InvalidVelocity { axis: "u", index });
                }
            }
        }
        for y in [0, height] {
            for x in 0..width {
                let index = y * width + x;
                if velocity.v[index] != 0.0 {
                    return Err(ViscosityError::InvalidVelocity { axis: "v", index });
                }
            }
        }
    }
    Ok(())
}

fn kinetic_energy_j(
    grid: Grid,
    density: &[f64],
    velocity: &FaceValues,
    dual_volume_m3: f64,
    edges: EdgeMode,
) -> Result<f64, ViscosityError> {
    let width = grid.width() as usize;
    let height = grid.height() as usize;
    let x_start = usize::from(!edges.periodic());
    let y_start = usize::from(!edges.periodic());
    let mut kinetic_j = 0.0;
    for y in 0..height {
        for x in x_start..width {
            let left_x = if x == 0 { width - 1 } else { x - 1 };
            let rho = mean(density[y * width + left_x], density[y * width + x]);
            let speed = velocity.u[y * (width + 1) + x];
            kinetic_j += 0.5 * rho * dual_volume_m3 * speed * speed;
        }
    }
    for y in y_start..height {
        let above_y = if y == 0 { height - 1 } else { y - 1 };
        for x in 0..width {
            let rho = mean(density[above_y * width + x], density[y * width + x]);
            let speed = velocity.v[y * width + x];
            kinetic_j += 0.5 * rho * dual_volume_m3 * speed * speed;
        }
    }
    if !kinetic_j.is_finite() {
        return Err(ViscosityError::NonFiniteResult("kinetic_energy_j"));
    }
    Ok(kinetic_j)
}

#[cfg(test)]
mod tests {
    use std::f64::consts::TAU;

    use super::*;

    fn fixture(width: usize, height: usize) -> PressureFields {
        use particle_sim::contracts::Boundary;

        let grid = Grid::new(width as f64, height as f64).unwrap();
        let boundaries = [Boundary::Closed; 4];
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
        PressureFields::new(
            grid,
            boundaries,
            vec![1.2; grid.cells()],
            vec![0.0; grid.cells()],
            FaceValues {
                u: vec![0.0; grid.u_faces()],
                v: vec![0.0; grid.v_faces()],
            },
            aperture,
        )
        .unwrap()
    }

    fn periodic_shear_errors(size: usize, steps: usize) -> (f64, f64) {
        let fields = fixture(size, size);
        let grid = fields.grid();
        // Periodicity exists only in this oracle. The production fields stay closed.
        let periodic_aperture = FaceValues {
            u: vec![1.0; grid.u_faces()],
            v: vec![1.0; grid.v_faces()],
        };
        let nu = 1e-4;
        let rho = 1.2;
        let mu = rho * nu;
        let length_m = 0.32;
        let dx_m = length_m / size as f64;
        let duration_s = 0.1;
        let dt_s = duration_s / steps as f64;
        let mut velocity = FaceValues {
            u: vec![0.0; grid.u_faces()],
            v: vec![0.0; grid.v_faces()],
        };
        for y in 0..size {
            let speed = 0.1 * (TAU * (y as f64 + 0.5) / size as f64).sin();
            for x in 0..=size {
                velocity.u[y * (size + 1) + x] = speed;
            }
        }
        for _ in 0..steps {
            let result = candidate_core(
                &fields,
                &velocity,
                &periodic_aperture,
                &vec![mu; grid.cells()],
                dt_s,
                dx_m,
                EdgeMode::Periodic,
            )
            .unwrap();
            assert!(result.diagnostics.kinetic_loss_j >= -1e-15);
            assert!(result.diagnostics.initial_stress_work_j >= 0.0);
            velocity = result.velocity_m_s;
        }
        let continuum_decay = (-nu * (TAU / length_m).powi(2) * duration_s).exp();
        let discrete_wave_number_squared =
            4.0 * (std::f64::consts::PI / size as f64).sin().powi(2) / (dx_m * dx_m);
        let discrete_decay = (-nu * discrete_wave_number_squared * duration_s).exp();
        let error_against = |decay: f64| {
            (0..size)
                .map(|y| {
                    let initial = 0.1 * (TAU * (y as f64 + 0.5) / size as f64).sin();
                    (velocity.u[y * (size + 1) + size / 2] - decay * initial).abs()
                })
                .fold(0.0, f64::max)
        };
        (
            error_against(continuum_decay),
            error_against(discrete_decay),
        )
    }

    #[test]
    fn periodic_shear_tracks_analytic_decay_and_refines_in_time_and_space() {
        let (_, temporal_coarse) = periodic_shear_errors(32, 1);
        let (spatial_coarse, temporal_fine) = periodic_shear_errors(32, 64);
        let (spatial_fine, _) = periodic_shear_errors(64, 128);
        assert!(
            temporal_fine < temporal_coarse,
            "{temporal_fine} < {temporal_coarse}"
        );
        assert!(
            spatial_fine < spatial_coarse,
            "{spatial_fine} < {spatial_coarse}"
        );
        assert!(spatial_fine < 1e-6, "error: {spatial_fine}");
    }

    #[test]
    fn public_shear_candidate_uses_grid_cell_width() {
        let default = fixture(4, 4);
        let refined_grid = Grid::with_cell_width(4.0, 4.0, 0.005).unwrap();
        let refined = PressureFields::new(
            refined_grid,
            default.boundaries(),
            default.density_kg_m3().to_vec(),
            default.correction_pressure_pa().to_vec(),
            FaceValues {
                u: vec![0.0; refined_grid.u_faces()],
                v: vec![0.0; refined_grid.v_faces()],
            },
            FaceValues {
                u: default.aperture().u.clone(),
                v: default.aperture().v.clone(),
            },
        )
        .unwrap();
        let viscosity = vec![1.2e-4; default.grid().cells()];
        let default_candidate =
            shear_viscosity_candidate(&default, default.velocity_m_s(), &viscosity, 1e-5).unwrap();
        let refined_candidate =
            shear_viscosity_candidate(&refined, refined.velocity_m_s(), &viscosity, 1e-5).unwrap();
        assert_eq!(
            refined_candidate.diagnostics.max_stable_dt_s,
            default_candidate.diagnostics.max_stable_dt_s / 4.0
        );
    }

    #[test]
    fn closed_and_blocked_faces_keep_zero_speed_without_cross_wall_shear() {
        let mut fields = fixture(4, 4);
        let mut velocity = FaceValues {
            u: vec![0.0; fields.grid().u_faces()],
            v: vec![0.0; fields.grid().v_faces()],
        };
        velocity.u[6] = 0.2;
        // A solid divider at y=2 blocks the two v faces incident to the
        // relevant vertices; no tangential impulse crosses it.
        let mut aperture = FaceValues {
            u: fields.aperture().u.clone(),
            v: fields.aperture().v.clone(),
        };
        for x in 0..4 {
            aperture.v[2 * 4 + x] = 0.0;
        }
        fields = PressureFields::new(
            fields.grid(),
            fields.boundaries(),
            fields.density_kg_m3().to_vec(),
            fields.correction_pressure_pa().to_vec(),
            FaceValues {
                u: vec![0.0; fields.grid().u_faces()],
                v: vec![0.0; fields.grid().v_faces()],
            },
            aperture,
        )
        .unwrap();
        let candidate = shear_viscosity_candidate(&fields, &velocity, &[0.001; 16], 0.001).unwrap();
        assert_eq!(candidate.velocity_m_s.u[0], 0.0);
        assert_eq!(candidate.velocity_m_s.u[4], 0.0);
        assert_eq!(candidate.velocity_m_s.v[8], 0.0);
        assert_eq!(candidate.velocity_m_s.u[2 * 5 + 1], 0.0);
        assert!(candidate.diagnostics.kinetic_loss_j >= 0.0);
    }

    #[test]
    fn two_density_shear_preserves_weighted_face_momentum() {
        let base = fixture(4, 4);
        let grid = base.grid();
        let density: Vec<f64> = (0..grid.cells())
            .map(|cell| if cell / 4 < 2 { 1.2 } else { 1200.0 })
            .collect();
        let fields = PressureFields::new(
            grid,
            base.boundaries(),
            density.clone(),
            vec![0.0; grid.cells()],
            FaceValues {
                u: vec![0.0; grid.u_faces()],
                v: vec![0.0; grid.v_faces()],
            },
            FaceValues {
                u: base.aperture().u.clone(),
                v: base.aperture().v.clone(),
            },
        )
        .unwrap();
        let mut velocity = FaceValues {
            u: vec![0.0; grid.u_faces()],
            v: vec![0.0; grid.v_faces()],
        };
        velocity.u[grid.u_face_index(1, 1).unwrap()] = 0.2;
        velocity.u[grid.u_face_index(2, 2).unwrap()] = -0.1;
        velocity.v[grid.v_face_index(1, 1).unwrap()] = 0.15;
        velocity.v[grid.v_face_index(2, 2).unwrap()] = -0.05;
        let weighted_momentum = |faces: &FaceValues| {
            let mut u = 0.0;
            let mut v = 0.0;
            for y in 0..4 {
                for x in 1..4 {
                    let left = y * 4 + x - 1;
                    let right = y * 4 + x;
                    u += mean(density[left], density[right]) * faces.u[y * 5 + x];
                }
            }
            for y in 1..4 {
                for x in 0..4 {
                    let above = (y - 1) * 4 + x;
                    let below = y * 4 + x;
                    v += mean(density[above], density[below]) * faces.v[y * 4 + x];
                }
            }
            (u, v)
        };
        let before = weighted_momentum(&velocity);
        let result = shear_viscosity_candidate(&fields, &velocity, &[0.001; 16], 0.001).unwrap();
        let after = weighted_momentum(&result.velocity_m_s);
        assert!((after.0 - before.0).abs() <= 1e-12);
        assert!((after.1 - before.1).abs() <= 1e-12);
        assert!(result.diagnostics.kinetic_after_j <= result.diagnostics.kinetic_before_j);
    }

    #[test]
    fn rejects_invalid_inputs_and_unstable_step_without_mutation() {
        let fields = fixture(4, 4);
        let mut velocity = FaceValues {
            u: vec![0.0; fields.grid().u_faces()],
            v: vec![0.0; fields.grid().v_faces()],
        };
        velocity.u[6] = 0.1;
        let original = velocity.u.clone();
        assert!(matches!(
            shear_viscosity_candidate(&fields, &velocity, &[0.001; 15], 0.01),
            Err(ViscosityError::Length { .. })
        ));
        assert!(matches!(
            shear_viscosity_candidate(&fields, &velocity, &[0.001; 16], 1.0),
            Err(ViscosityError::TimeStepTooLarge { .. })
        ));
        assert!(matches!(
            shear_viscosity_candidate(&fields, &velocity, &[0.0; 16], 0.01),
            Err(ViscosityError::InvalidScalar(_))
        ));
        assert_eq!(velocity.u, original);
    }
}
