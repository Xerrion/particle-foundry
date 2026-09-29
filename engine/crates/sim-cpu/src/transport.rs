//! Conservative, bounded f64 phase transport for the CPU reference.
//!
//! The first transport kernel uses directional swept-strip reconstruction. It
//! does not claim the corner transport or multidimensional PLIC accuracy needed
//! to close the full E06 reference gate. Its candidate is detached: callers
//! replace the authoritative inventory only after all later stages succeed.

use particle_sim::{CELL_WIDTH_M, Grid, REPRESENTED_DEPTH_M, contracts::Boundary};

use crate::fluid::{FaceValues, PressureFields};

/// Default-scale whole-cell volume in m³. This kernel has no partial-solid cells.
pub const CELL_VOLUME_M3: f64 = CELL_WIDTH_M * CELL_WIDTH_M * REPRESENTED_DEPTH_M;
/// Maximum normal displacement in cell widths for one transport substep.
pub const MAX_FACE_CFL: f64 = 0.5;
/// Allowed relative closure error for a completed incompressible candidate.
pub const VOLUME_CLOSURE_REL_TOL: f64 = 1e-12;

/// Rejected input or candidate; the source inventory is never mutated.
#[derive(Clone, Debug, PartialEq)]
pub enum TransportError {
    /// The supplied pressure/geometry owner belongs to another grid.
    GridMismatch,
    /// A field does not match the cell or MAC face layout.
    Length {
        /// Rejected field.
        field: &'static str,
        /// Required entries.
        expected: usize,
        /// Supplied entries.
        actual: usize,
    },
    /// A density or time step is not positive and finite.
    InvalidScalar(&'static str),
    /// A cell mass or marker is nonfinite, negative, or violates phase closure.
    InvalidCell {
        /// Row-major cell index.
        index: usize,
        /// Failed invariant.
        reason: &'static str,
    },
    /// A face speed/aperture is nonfinite or violates a wall constraint.
    InvalidFace {
        /// `u` or `v` face array.
        axis: &'static str,
        /// Row-major face index.
        index: usize,
        /// Failed invariant.
        reason: &'static str,
    },
    /// Open boundaries need a finite reservoir mass/marker ledger first.
    UnsupportedOpenBoundary,
    /// The proposed face displacement exceeds the documented geometric bound.
    CflExceeded {
        /// `u` or `v` face array.
        axis: &'static str,
        /// Row-major face index.
        index: usize,
        /// Proposed normal displacement in cell widths.
        cfl: f64,
    },
    /// A directional donor would lose more of a phase or marker than it owns.
    DonorOverdraw {
        /// Row-major donor cell index.
        index: usize,
        /// Phase or marker that would become negative.
        field: &'static str,
    },
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for TransportError {}

/// One scene-owned incompressible phase inventory and its phase-associated markers.
///
/// Both phase masses are authoritative. Liquid fraction is always derived from
/// liquid mass, liquid density and geometric volume; no alpha array is evolved.
/// Markers are nonnegative extensive amounts, with no thermodynamic meaning.
/// A marker must be zero when its associated phase mass is zero.
#[derive(Clone, Debug, PartialEq)]
pub struct TransportInventory {
    grid: Grid,
    liquid_density_kg_m3: f64,
    carrier_density_kg_m3: f64,
    liquid_mass_kg: Vec<f64>,
    carrier_mass_kg: Vec<f64>,
    liquid_marker: Vec<f64>,
    carrier_marker: Vec<f64>,
    fixed_wall: Vec<bool>,
}

/// Signed integrated fluxes at MAC faces. Positive means rightward or downward.
///
/// The `u` and `v` arrays have the same indexing as [`FaceValues`]. An inner
/// face is stored once. Closed outer faces have zero flux. Fluxes describe one
/// candidate step and can be used by the later compatible-momentum stage.
#[derive(Debug, PartialEq)]
pub struct TransportFaceFluxes {
    /// Signed geometric volume in m³, including the aperture factor.
    pub volume_m3: FaceValues,
    /// Signed liquid mass in kg.
    pub liquid_mass_kg: FaceValues,
    /// Signed carrier mass in kg.
    pub carrier_mass_kg: FaceValues,
    /// Signed extensive liquid-associated marker amount.
    pub liquid_marker: FaceValues,
    /// Signed extensive carrier-associated marker amount.
    pub carrier_marker: FaceValues,
}

/// Fully calculated, validated next inventory with its signed face ledger.
#[derive(Debug, PartialEq)]
pub struct TransportCandidate {
    /// Detached next phase inventory.
    pub inventory: TransportInventory,
    /// Integrated face fluxes for this candidate.
    pub fluxes: TransportFaceFluxes,
    /// Largest normal displacement in cell widths on any face.
    pub max_face_cfl: f64,
}

/// Totals used to compare phase conservation before and after a sealed step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransportTotals {
    /// Total liquid mass in kg.
    pub liquid_mass_kg: f64,
    /// Total carrier mass in kg.
    pub carrier_mass_kg: f64,
    /// Total liquid-associated marker amount.
    pub liquid_marker: f64,
    /// Total carrier-associated marker amount.
    pub carrier_marker: f64,
}

impl TransportInventory {
    /// Takes owned arrays after checking phase closure and marker association.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        grid: Grid,
        liquid_density_kg_m3: f64,
        carrier_density_kg_m3: f64,
        liquid_mass_kg: Vec<f64>,
        carrier_mass_kg: Vec<f64>,
        liquid_marker: Vec<f64>,
        carrier_marker: Vec<f64>,
        fixed_wall: Vec<bool>,
    ) -> Result<Self, TransportError> {
        for (name, density) in [
            ("liquid density", liquid_density_kg_m3),
            ("carrier density", carrier_density_kg_m3),
        ] {
            if !density.is_finite() || density <= 0.0 {
                return Err(TransportError::InvalidScalar(name));
            }
        }
        for (name, len) in [
            ("liquid_mass_kg", liquid_mass_kg.len()),
            ("carrier_mass_kg", carrier_mass_kg.len()),
            ("liquid_marker", liquid_marker.len()),
            ("carrier_marker", carrier_marker.len()),
            ("fixed_wall", fixed_wall.len()),
        ] {
            check_len(name, len, grid.cells())?;
        }
        let inventory = Self {
            grid,
            liquid_density_kg_m3,
            carrier_density_kg_m3,
            liquid_mass_kg,
            carrier_mass_kg,
            liquid_marker,
            carrier_marker,
            fixed_wall,
        };
        inventory.validate_completed()?;
        Ok(inventory)
    }

    /// Validated geometry that owns these cell-major arrays.
    pub fn grid(&self) -> Grid {
        self.grid
    }

    /// Constant liquid density in kg/m³.
    pub fn liquid_density_kg_m3(&self) -> f64 {
        self.liquid_density_kg_m3
    }

    /// Constant carrier density in kg/m³.
    pub fn carrier_density_kg_m3(&self) -> f64 {
        self.carrier_density_kg_m3
    }

    /// Cell-major liquid mass in kg.
    pub fn liquid_mass_kg(&self) -> &[f64] {
        &self.liquid_mass_kg
    }

    /// Cell-major carrier mass in kg.
    pub fn carrier_mass_kg(&self) -> &[f64] {
        &self.carrier_mass_kg
    }

    /// Cell-major liquid-associated marker amount.
    pub fn liquid_marker(&self) -> &[f64] {
        &self.liquid_marker
    }

    /// Cell-major carrier-associated marker amount.
    pub fn carrier_marker(&self) -> &[f64] {
        &self.carrier_marker
    }

    /// Fixed solid occupancy. Wall cells own no phase mass or marker.
    pub fn fixed_wall(&self) -> &[bool] {
        &self.fixed_wall
    }

    /// Derived whole-cell liquid volume fraction; absent for an invalid index.
    pub fn alpha(&self, cell: usize) -> Option<f64> {
        self.liquid_mass_kg.get(cell).and_then(|mass| {
            (!self.fixed_wall[cell])
                .then(|| mass / self.liquid_density_kg_m3 / self.grid.cell_volume_m3())
        })
    }

    /// Reduces the four conserved amounts across all cells.
    pub fn totals(&self) -> TransportTotals {
        TransportTotals {
            liquid_mass_kg: self.liquid_mass_kg.iter().sum(),
            carrier_mass_kg: self.carrier_mass_kg.iter().sum(),
            liquid_marker: self.liquid_marker.iter().sum(),
            carrier_marker: self.carrier_marker.iter().sum(),
        }
    }

    /// Calculates a detached candidate using projected face velocity and the
    /// validated pressure owner's fixed geometry/apertures. No state is changed
    /// on failure. Open reservoirs are rejected until their ledgers exist.
    pub fn candidate(
        &self,
        fields: &PressureFields,
        velocity_m_s: &FaceValues,
        dt_s: f64,
    ) -> Result<TransportCandidate, TransportError> {
        if fields.grid() != self.grid {
            return Err(TransportError::GridMismatch);
        }
        if fields
            .boundaries()
            .iter()
            .any(|side| *side != Boundary::Closed)
        {
            return Err(TransportError::UnsupportedOpenBoundary);
        }
        self.candidate_core(velocity_m_s, fields.aperture(), dt_s, EdgeMode::Closed)
    }

    fn candidate_core(
        &self,
        velocity_m_s: &FaceValues,
        aperture: &FaceValues,
        dt_s: f64,
        edges: EdgeMode,
    ) -> Result<TransportCandidate, TransportError> {
        if !dt_s.is_finite() || dt_s <= 0.0 {
            return Err(TransportError::InvalidScalar("dt_s"));
        }
        check_len("velocity.u", velocity_m_s.u.len(), self.grid.u_faces())?;
        check_len("velocity.v", velocity_m_s.v.len(), self.grid.v_faces())?;
        check_len("aperture.u", aperture.u.len(), self.grid.u_faces())?;
        check_len("aperture.v", aperture.v.len(), self.grid.v_faces())?;
        validate_faces(self.grid, velocity_m_s, aperture, edges)?;
        validate_wall_apertures(self, aperture)?;

        let mut fluxes = TransportFaceFluxes::zeros(self.grid);
        let mut max_face_cfl = 0.0;
        let after_x = transport_axis(
            self,
            velocity_m_s,
            aperture,
            dt_s,
            Axis::X,
            edges,
            &mut fluxes,
            &mut max_face_cfl,
        )?;
        let inventory = transport_axis(
            &after_x,
            velocity_m_s,
            aperture,
            dt_s,
            Axis::Y,
            edges,
            &mut fluxes,
            &mut max_face_cfl,
        )?;
        inventory.validate_completed()?;
        Ok(TransportCandidate {
            inventory,
            fluxes,
            max_face_cfl,
        })
    }

    fn validate_nonnegative(&self) -> Result<(), TransportError> {
        for cell in 0..self.grid.cells() {
            for (name, value) in [
                ("liquid mass", self.liquid_mass_kg[cell]),
                ("carrier mass", self.carrier_mass_kg[cell]),
                ("liquid marker", self.liquid_marker[cell]),
                ("carrier marker", self.carrier_marker[cell]),
            ] {
                if !value.is_finite() || value < 0.0 {
                    return Err(TransportError::InvalidCell {
                        index: cell,
                        reason: name,
                    });
                }
            }
            if (self.liquid_mass_kg[cell] == 0.0 && self.liquid_marker[cell] != 0.0)
                || (self.carrier_mass_kg[cell] == 0.0 && self.carrier_marker[cell] != 0.0)
            {
                return Err(TransportError::InvalidCell {
                    index: cell,
                    reason: "marker without associated phase mass",
                });
            }
        }
        Ok(())
    }

    fn validate_completed(&self) -> Result<(), TransportError> {
        self.validate_nonnegative()?;
        let cell_volume_m3 = self.grid.cell_volume_m3();
        for cell in 0..self.grid.cells() {
            let liquid_volume = self.liquid_mass_kg[cell] / self.liquid_density_kg_m3;
            let carrier_volume = self.carrier_mass_kg[cell] / self.carrier_density_kg_m3;
            let closure = liquid_volume + carrier_volume;
            if self.fixed_wall[cell] {
                if closure != 0.0
                    || self.liquid_marker[cell] != 0.0
                    || self.carrier_marker[cell] != 0.0
                {
                    return Err(TransportError::InvalidCell {
                        index: cell,
                        reason: "fixed wall owns fluid amount",
                    });
                }
                continue;
            }
            if !closure.is_finite()
                || (self.liquid_mass_kg[cell] > 0.0 && liquid_volume == 0.0)
                || (self.carrier_mass_kg[cell] > 0.0 && carrier_volume == 0.0)
                || (closure - cell_volume_m3).abs() > VOLUME_CLOSURE_REL_TOL * cell_volume_m3
                || liquid_volume > cell_volume_m3 * (1.0 + VOLUME_CLOSURE_REL_TOL)
            {
                return Err(TransportError::InvalidCell {
                    index: cell,
                    reason: "incompressible phase volume closure",
                });
            }
        }
        let totals = self.totals();
        if [
            totals.liquid_mass_kg,
            totals.carrier_mass_kg,
            totals.liquid_marker,
            totals.carrier_marker,
        ]
        .iter()
        .any(|amount| !amount.is_finite())
        {
            return Err(TransportError::InvalidScalar(
                "nonfinite phase or marker total",
            ));
        }
        Ok(())
    }
}

impl TransportFaceFluxes {
    fn zeros(grid: Grid) -> Self {
        let faces = || FaceValues {
            u: vec![0.0; grid.u_faces()],
            v: vec![0.0; grid.v_faces()],
        };
        Self {
            volume_m3: faces(),
            liquid_mass_kg: faces(),
            carrier_mass_kg: faces(),
            liquid_marker: faces(),
            carrier_marker: faces(),
        }
    }
}

#[derive(Clone, Copy)]
enum Axis {
    X,
    Y,
}

impl Axis {
    fn label(self) -> &'static str {
        match self {
            Self::X => "u",
            Self::Y => "v",
        }
    }
}

#[derive(Clone, Copy)]
enum EdgeMode {
    Closed,
    #[cfg(test)]
    PeriodicX,
}

impl EdgeMode {
    fn periodic_x(self) -> bool {
        #[cfg(test)]
        {
            matches!(self, Self::PeriodicX)
        }
        #[cfg(not(test))]
        {
            false
        }
    }
}

#[derive(Clone, Copy, Default)]
struct FourAmounts {
    liquid_mass_kg: f64,
    carrier_mass_kg: f64,
    liquid_marker: f64,
    carrier_marker: f64,
}

impl FourAmounts {
    fn add(&mut self, other: Self) {
        self.liquid_mass_kg += other.liquid_mass_kg;
        self.carrier_mass_kg += other.carrier_mass_kg;
        self.liquid_marker += other.liquid_marker;
        self.carrier_marker += other.carrier_marker;
    }
}

fn check_len(field: &'static str, actual: usize, expected: usize) -> Result<(), TransportError> {
    if actual == expected {
        Ok(())
    } else {
        Err(TransportError::Length {
            field,
            expected,
            actual,
        })
    }
}

fn validate_faces(
    grid: Grid,
    velocity: &FaceValues,
    aperture: &FaceValues,
    edges: EdgeMode,
) -> Result<(), TransportError> {
    let width = grid.width() as usize;
    let height = grid.height() as usize;
    for (axis, speeds, opens) in [
        ("u", &velocity.u, &aperture.u),
        ("v", &velocity.v, &aperture.v),
    ] {
        for (index, (&speed, &open)) in speeds.iter().zip(opens).enumerate() {
            if !speed.is_finite() || !open.is_finite() || !(0.0..=1.0).contains(&open) {
                return Err(TransportError::InvalidFace {
                    axis,
                    index,
                    reason: "nonfinite velocity or aperture outside zero to one",
                });
            }
            if open == 0.0 && speed != 0.0 {
                return Err(TransportError::InvalidFace {
                    axis,
                    index,
                    reason: "blocked face has normal velocity",
                });
            }
            let is_outer = if axis == "u" {
                let x = index % (width + 1);
                x == 0 || x == width
            } else {
                let y = index / width;
                y == 0 || y == height
            };
            let periodic_x_outer = edges.periodic_x() && axis == "u";
            if is_outer && !periodic_x_outer && (speed != 0.0 || open != 0.0) {
                return Err(TransportError::InvalidFace {
                    axis,
                    index,
                    reason: "closed outer face has normal velocity or aperture",
                });
            }
        }
    }
    #[cfg(test)]
    if matches!(edges, EdgeMode::PeriodicX) {
        if width < 2 {
            return Err(TransportError::InvalidScalar(
                "periodic test needs width at least two",
            ));
        }
        for y in 0..height {
            let left = y * (width + 1);
            let right = left + width;
            if velocity.u[left] != velocity.u[right] || aperture.u[left] != aperture.u[right] {
                return Err(TransportError::InvalidFace {
                    axis: "u",
                    index: left,
                    reason: "periodic seam faces disagree",
                });
            }
        }
    }
    Ok(())
}

fn validate_wall_apertures(
    inventory: &TransportInventory,
    aperture: &FaceValues,
) -> Result<(), TransportError> {
    let grid = inventory.grid;
    let width = grid.width() as usize;
    for cell in 0..grid.cells() {
        if !inventory.fixed_wall[cell] {
            continue;
        }
        let x = cell % width;
        let y = cell / width;
        for face in [y * (width + 1) + x, y * (width + 1) + x + 1] {
            if aperture.u[face] != 0.0 {
                return Err(TransportError::InvalidFace {
                    axis: "u",
                    index: face,
                    reason: "fixed-wall incident aperture is open",
                });
            }
        }
        for face in [y * width + x, (y + 1) * width + x] {
            if aperture.v[face] != 0.0 {
                return Err(TransportError::InvalidFace {
                    axis: "v",
                    index: face,
                    reason: "fixed-wall incident aperture is open",
                });
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn transport_axis(
    inventory: &TransportInventory,
    velocity: &FaceValues,
    aperture: &FaceValues,
    dt_s: f64,
    axis: Axis,
    edges: EdgeMode,
    fluxes: &mut TransportFaceFluxes,
    max_face_cfl: &mut f64,
) -> Result<TransportInventory, TransportError> {
    let grid = inventory.grid;
    let width = grid.width() as usize;
    let height = grid.height() as usize;
    let mut outgoing = vec![FourAmounts::default(); grid.cells()];
    let mut incoming = vec![FourAmounts::default(); grid.cells()];
    let (face_width, face_height) = match axis {
        Axis::X => (width + 1, height),
        Axis::Y => (width, height + 1),
    };
    for y in 0..face_height {
        for x in 0..face_width {
            if matches!(axis, Axis::X) && x == width {
                continue; // zero closed face or duplicate periodic seam
            }
            if matches!(axis, Axis::Y) && y == height {
                continue; // zero closed face
            }
            let face = y * face_width + x;
            let (speed, open) = match axis {
                Axis::X => (velocity.u[face], aperture.u[face]),
                Axis::Y => (velocity.v[face], aperture.v[face]),
            };
            if speed == 0.0 || open == 0.0 {
                continue;
            }
            let (negative_cell, positive_cell) = match axis {
                Axis::X if x == 0 => {
                    #[cfg(test)]
                    if matches!(edges, EdgeMode::PeriodicX) {
                        (y * width + width - 1, y * width)
                    } else {
                        continue;
                    }
                    #[cfg(not(test))]
                    {
                        continue;
                    }
                }
                Axis::X => (y * width + x - 1, y * width + x),
                Axis::Y if y == 0 => continue,
                Axis::Y => ((y - 1) * width + x, y * width + x),
            };
            let (donor, receiver) = if speed > 0.0 {
                (negative_cell, positive_cell)
            } else {
                (positive_cell, negative_cell)
            };
            let cfl = speed.abs() * dt_s / grid.cell_width_m();
            if !cfl.is_finite() {
                return Err(TransportError::InvalidFace {
                    axis: axis.label(),
                    index: face,
                    reason: "nonfinite swept volume",
                });
            }
            if cfl > MAX_FACE_CFL * (1.0 + 1e-12) {
                return Err(TransportError::CflExceeded {
                    axis: axis.label(),
                    index: face,
                    cfl,
                });
            }
            *max_face_cfl = max_face_cfl.max(cfl);
            let swept_volume = cfl * open * grid.cell_volume_m3();
            let phase_flux = donor_flux(inventory, donor, swept_volume, axis, speed > 0.0, edges)?;
            outgoing[donor].add(phase_flux);
            incoming[receiver].add(phase_flux);
            let sign = speed.signum();
            match axis {
                Axis::X => {
                    fluxes.volume_m3.u[face] = sign * swept_volume;
                    fluxes.liquid_mass_kg.u[face] = sign * phase_flux.liquid_mass_kg;
                    fluxes.carrier_mass_kg.u[face] = sign * phase_flux.carrier_mass_kg;
                    fluxes.liquid_marker.u[face] = sign * phase_flux.liquid_marker;
                    fluxes.carrier_marker.u[face] = sign * phase_flux.carrier_marker;
                    #[cfg(test)]
                    if matches!(edges, EdgeMode::PeriodicX) && x == 0 {
                        let duplicate = y * (width + 1) + width;
                        fluxes.volume_m3.u[duplicate] = fluxes.volume_m3.u[face];
                        fluxes.liquid_mass_kg.u[duplicate] = fluxes.liquid_mass_kg.u[face];
                        fluxes.carrier_mass_kg.u[duplicate] = fluxes.carrier_mass_kg.u[face];
                        fluxes.liquid_marker.u[duplicate] = fluxes.liquid_marker.u[face];
                        fluxes.carrier_marker.u[duplicate] = fluxes.carrier_marker.u[face];
                    }
                }
                Axis::Y => {
                    fluxes.volume_m3.v[face] = sign * swept_volume;
                    fluxes.liquid_mass_kg.v[face] = sign * phase_flux.liquid_mass_kg;
                    fluxes.carrier_mass_kg.v[face] = sign * phase_flux.carrier_mass_kg;
                    fluxes.liquid_marker.v[face] = sign * phase_flux.liquid_marker;
                    fluxes.carrier_marker.v[face] = sign * phase_flux.carrier_marker;
                }
            }
        }
    }

    let mut next = inventory.clone();
    for cell in 0..grid.cells() {
        next.liquid_mass_kg[cell] = balance(
            inventory.liquid_mass_kg[cell],
            outgoing[cell].liquid_mass_kg,
            incoming[cell].liquid_mass_kg,
            cell,
            "liquid mass",
        )?;
        next.carrier_mass_kg[cell] = balance(
            inventory.carrier_mass_kg[cell],
            outgoing[cell].carrier_mass_kg,
            incoming[cell].carrier_mass_kg,
            cell,
            "carrier mass",
        )?;
        next.liquid_marker[cell] = balance(
            inventory.liquid_marker[cell],
            outgoing[cell].liquid_marker,
            incoming[cell].liquid_marker,
            cell,
            "liquid marker",
        )?;
        next.carrier_marker[cell] = balance(
            inventory.carrier_marker[cell],
            outgoing[cell].carrier_marker,
            incoming[cell].carrier_marker,
            cell,
            "carrier marker",
        )?;
    }
    next.validate_nonnegative()?;
    Ok(next)
}

fn balance(
    owned: f64,
    outgoing: f64,
    incoming: f64,
    cell: usize,
    field: &'static str,
) -> Result<f64, TransportError> {
    if !outgoing.is_finite() || !incoming.is_finite() || outgoing > owned {
        return Err(TransportError::DonorOverdraw { index: cell, field });
    }
    let next = (owned - outgoing) + incoming;
    if !next.is_finite() || next < 0.0 {
        return Err(TransportError::InvalidCell {
            index: cell,
            reason: "nonfinite or negative transported amount",
        });
    }
    Ok(next)
}

fn donor_flux(
    inventory: &TransportInventory,
    donor: usize,
    swept_volume_m3: f64,
    axis: Axis,
    toward_positive: bool,
    edges: EdgeMode,
) -> Result<FourAmounts, TransportError> {
    let liquid_mass = inventory.liquid_mass_kg[donor];
    let carrier_mass = inventory.carrier_mass_kg[donor];
    let liquid_volume = liquid_mass / inventory.liquid_density_kg_m3;
    let carrier_volume = carrier_mass / inventory.carrier_density_kg_m3;
    let donor_volume = liquid_volume + carrier_volume;
    if !donor_volume.is_finite() || donor_volume <= 0.0 {
        return Err(TransportError::InvalidCell {
            index: donor,
            reason: "empty or nonfinite directional donor volume",
        });
    }
    let swept_fraction = swept_volume_m3 / donor_volume;
    if !swept_fraction.is_finite() || swept_fraction > 1.0 {
        return Err(TransportError::DonorOverdraw {
            index: donor,
            field: "directional volume",
        });
    }
    let alpha = liquid_volume / donor_volume;
    let neighbor_gradient = phase_gradient(inventory, donor, axis, edges)?;
    let liquid_swept_volume = if alpha == 0.0 {
        0.0
    } else if alpha == 1.0 {
        swept_volume_m3
    } else if neighbor_gradient.abs() <= 1e-12 {
        swept_volume_m3 * alpha
    } else {
        // A swept strip touches one end of the donor. Compute its overlap in
        // volume units. Normalized end coordinates lose tiny fluxes when a
        // nearly full phase sits beside a much smaller swept strip.
        let liquid_at_outgoing_face = (neighbor_gradient > 0.0) == toward_positive;
        if liquid_at_outgoing_face {
            swept_volume_m3.min(liquid_volume)
        } else {
            (swept_volume_m3 - carrier_volume).max(0.0)
        }
    };
    if !liquid_swept_volume.is_finite()
        || liquid_swept_volume < 0.0
        || liquid_swept_volume > swept_volume_m3 * (1.0 + 1e-12)
    {
        return Err(TransportError::InvalidCell {
            index: donor,
            reason: "invalid reconstructed phase flux",
        });
    }
    let carrier_swept_volume = swept_volume_m3 - liquid_swept_volume;
    if carrier_swept_volume < 0.0 {
        return Err(TransportError::InvalidCell {
            index: donor,
            reason: "negative carrier strip volume",
        });
    }
    let liquid_flux = phase_mass_flux(liquid_mass, liquid_volume, liquid_swept_volume);
    let carrier_flux = phase_mass_flux(carrier_mass, carrier_volume, carrier_swept_volume);
    let liquid_marker_flux = marker_flux(inventory.liquid_marker[donor], liquid_mass, liquid_flux)?;
    let carrier_marker_flux =
        marker_flux(inventory.carrier_marker[donor], carrier_mass, carrier_flux)?;
    Ok(FourAmounts {
        liquid_mass_kg: liquid_flux,
        carrier_mass_kg: carrier_flux,
        liquid_marker: liquid_marker_flux,
        carrier_marker: carrier_marker_flux,
    })
}

fn phase_mass_flux(owned_mass: f64, owned_volume: f64, swept_volume: f64) -> f64 {
    if swept_volume >= owned_volume {
        // A saturated strip owns the whole phase. Dividing its mass by density
        // and multiplying back can otherwise overdraw the donor by one ULP.
        owned_mass
    } else {
        owned_mass * (swept_volume / owned_volume)
    }
}

fn marker_flux(marker: f64, phase_mass: f64, phase_flux: f64) -> Result<f64, TransportError> {
    let flux = if phase_mass == 0.0 {
        0.0
    } else {
        marker * (phase_flux / phase_mass)
    };
    if !flux.is_finite() || flux < 0.0 {
        return Err(TransportError::InvalidScalar(
            "nonfinite phase-associated marker flux",
        ));
    }
    Ok(flux)
}

fn phase_gradient(
    inventory: &TransportInventory,
    cell: usize,
    axis: Axis,
    _edges: EdgeMode,
) -> Result<f64, TransportError> {
    let width = inventory.grid.width() as usize;
    let height = inventory.grid.height() as usize;
    let x = cell % width;
    let y = cell / width;
    let left = match axis {
        Axis::X if x > 0 => cell - 1,
        #[cfg(test)]
        Axis::X if matches!(_edges, EdgeMode::PeriodicX) => cell + width - 1,
        Axis::X => cell,
        Axis::Y if y > 0 => cell - width,
        Axis::Y => cell,
    };
    let right = match axis {
        Axis::X if x + 1 < width => cell + 1,
        #[cfg(test)]
        Axis::X if matches!(_edges, EdgeMode::PeriodicX) => cell + 1 - width,
        Axis::X => cell,
        Axis::Y if y + 1 < height => cell + width,
        Axis::Y => cell,
    };
    let phase_fraction = |index: usize| {
        if inventory.fixed_wall[index] {
            return None;
        }
        let liquid = inventory.liquid_mass_kg[index] / inventory.liquid_density_kg_m3;
        let carrier = inventory.carrier_mass_kg[index] / inventory.carrier_density_kg_m3;
        let total = liquid + carrier;
        if total <= 0.0 || !total.is_finite() {
            None
        } else {
            Some(liquid / total)
        }
    };
    let center = phase_fraction(cell).ok_or(TransportError::InvalidCell {
        index: cell,
        reason: "invalid donor phase volume",
    })?;
    // An axis sweep can temporarily empty a neighboring fluid cell. Use the
    // donor fraction there. The other axis can refill that cell in this step.
    let a = phase_fraction(left).unwrap_or(center);
    let b = phase_fraction(right).unwrap_or(center);
    Ok(b - a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use particle_sim::contracts::Boundary;

    const LIQUID_RHO: f64 = 1000.0;
    const CARRIER_RHO: f64 = 1.2;

    fn grid(width: f64, height: f64) -> Grid {
        Grid::new(width, height).unwrap()
    }

    fn phase_state(grid: Grid, alpha: &[f64], wall: Vec<bool>) -> TransportInventory {
        let mut liquid = Vec::with_capacity(grid.cells());
        let mut carrier = Vec::with_capacity(grid.cells());
        for (cell, &fraction) in alpha.iter().enumerate() {
            let available = if wall[cell] {
                0.0
            } else {
                grid.cell_volume_m3()
            };
            liquid.push(fraction * available * LIQUID_RHO);
            carrier.push((1.0 - fraction) * available * CARRIER_RHO);
        }
        TransportInventory::new(
            grid,
            LIQUID_RHO,
            CARRIER_RHO,
            liquid,
            carrier,
            vec![0.0; grid.cells()],
            vec![0.0; grid.cells()],
            wall,
        )
        .unwrap()
    }

    fn face_values(grid: Grid, value: f64) -> FaceValues {
        FaceValues {
            u: vec![value; grid.u_faces()],
            v: vec![value; grid.v_faces()],
        }
    }

    fn closed_aperture(grid: Grid) -> FaceValues {
        let mut aperture = face_values(grid, 1.0);
        let width = grid.width() as usize;
        let height = grid.height() as usize;
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

    fn fields(grid: Grid, aperture: FaceValues) -> PressureFields {
        PressureFields::new(
            grid,
            [Boundary::Closed; 4],
            vec![1.0; grid.cells()],
            vec![0.0; grid.cells()],
            face_values(grid, 0.0),
            aperture,
        )
        .unwrap()
    }

    fn near(actual: f64, expected: f64, absolute: f64) {
        assert!(
            (actual - expected).abs() <= absolute,
            "actual {actual:e}, expected {expected:e}, tolerance {absolute:e}"
        );
    }

    #[test]
    fn alpha_is_derived_and_wall_volume_is_explicit() {
        let grid = grid(3.0, 1.0);
        let state = phase_state(grid, &[0.0, 0.5, 0.0], vec![false, false, true]);
        near(state.alpha(1).unwrap(), 0.5, 1e-15);
        assert_eq!(state.alpha(2), None);
        assert_eq!(state.alpha(3), None);

        let invalid = TransportInventory::new(
            grid,
            LIQUID_RHO,
            CARRIER_RHO,
            vec![0.0; 3],
            vec![CELL_VOLUME_M3 * CARRIER_RHO; 3],
            vec![0.0; 3],
            vec![0.0; 3],
            vec![false, false, true],
        );
        assert!(matches!(
            invalid,
            Err(TransportError::InvalidCell { index: 2, .. })
        ));
        let invalid_marker = TransportInventory::new(
            grid,
            LIQUID_RHO,
            CARRIER_RHO,
            vec![0.0; 3],
            vec![CELL_VOLUME_M3 * CARRIER_RHO; 3],
            vec![1.0, 0.0, 0.0],
            vec![0.0; 3],
            vec![false; 3],
        );
        assert!(matches!(
            invalid_marker,
            Err(TransportError::InvalidCell { index: 0, .. })
        ));
    }

    #[test]
    fn refined_grid_uses_its_own_phase_volume_and_swept_flux() {
        let refined = Grid::with_cell_width(2.0, 2.0, CELL_WIDTH_M / 2.0).unwrap();
        let source = phase_state(refined, &[0.5; 4], vec![false; 4]);
        near(source.alpha(0).unwrap(), 0.5, 1e-15);

        let wrong_volume = TransportInventory::new(
            refined,
            LIQUID_RHO,
            CARRIER_RHO,
            vec![0.0; 4],
            vec![CELL_VOLUME_M3 * CARRIER_RHO; 4],
            vec![0.0; 4],
            vec![0.0; 4],
            vec![false; 4],
        );
        assert!(matches!(
            wrong_volume,
            Err(TransportError::InvalidCell { index: 0, .. })
        ));

        let mut velocity = face_values(refined, 0.0);
        velocity.u[refined.u_face_index(1, 0).unwrap()] = 1.0;
        velocity.u[refined.u_face_index(1, 1).unwrap()] = -1.0;
        velocity.v[refined.v_face_index(0, 1).unwrap()] = -1.0;
        velocity.v[refined.v_face_index(1, 1).unwrap()] = 1.0;
        let refined_fields = fields(refined, closed_aperture(refined));
        let candidate = source
            .candidate(&refined_fields, &velocity, 0.00125)
            .unwrap();
        let face = refined.u_face_index(1, 0).unwrap();
        near(candidate.max_face_cfl, 0.25, 1e-15);
        near(
            candidate.fluxes.volume_m3.u[face],
            0.25 * refined.cell_volume_m3(),
            1e-21,
        );
        near(
            candidate.fluxes.liquid_mass_kg.u[face],
            0.125 * refined.cell_volume_m3() * LIQUID_RHO,
            1e-18,
        );
        for cell in 0..refined.cells() {
            near(candidate.inventory.alpha(cell).unwrap(), 0.5, 1e-12);
        }

        let default_scale = grid(2.0, 2.0);
        let foreign_fields = fields(default_scale, closed_aperture(default_scale));
        assert!(matches!(
            source.candidate(&foreign_fields, &velocity, 0.00125),
            Err(TransportError::GridMismatch)
        ));
    }

    #[test]
    fn swept_strip_reconstruction_uses_interface_orientation() {
        let grid = grid(3.0, 1.0);
        let state = phase_state(grid, &[0.0, 0.5, 1.0], vec![false; 3]);
        let right = donor_flux(
            &state,
            1,
            CELL_VOLUME_M3 * 0.25,
            Axis::X,
            true,
            EdgeMode::Closed,
        )
        .unwrap();
        let left = donor_flux(
            &state,
            1,
            CELL_VOLUME_M3 * 0.25,
            Axis::X,
            false,
            EdgeMode::Closed,
        )
        .unwrap();
        near(
            right.liquid_mass_kg,
            CELL_VOLUME_M3 * 0.25 * LIQUID_RHO,
            1e-18,
        );
        assert_eq!(left.liquid_mass_kg, 0.0);
        near(
            left.carrier_mass_kg,
            CELL_VOLUME_M3 * 0.25 * CARRIER_RHO,
            1e-21,
        );
    }

    #[test]
    fn swept_strip_keeps_tiny_and_intermediate_full_liquid_fluxes_bounded() {
        let grid = grid(3.0, 1.0);
        let mut state = phase_state(grid, &[0.0, 0.5, 1.0], vec![false; 3]);
        for (liquid_volume, carrier_volume, swept_volume) in [
            (
                1.0000000000000027e-6,
                2.1741867565575974e-21,
                4.1633363423443376e-22,
            ),
            (
                8.766006390343263e-7,
                1.2266945323446473e-7,
                1.3900031245618323e-7,
            ),
        ] {
            // The second state is a valid intermediate directional sweep.
            // Its whole-cell volume closes only after the other axis moves.
            state.liquid_mass_kg[1] = liquid_volume * LIQUID_RHO;
            state.carrier_mass_kg[1] = carrier_volume * CARRIER_RHO;
            let flux =
                donor_flux(&state, 1, swept_volume, Axis::X, true, EdgeMode::Closed).unwrap();
            let expected_liquid_mass = swept_volume * LIQUID_RHO;
            near(
                flux.liquid_mass_kg,
                expected_liquid_mass,
                4.0 * f64::EPSILON * expected_liquid_mass,
            );
            assert_eq!(flux.carrier_mass_kg, 0.0);
        }
    }

    #[test]
    fn saturated_liquid_strip_uses_exact_owned_mass() {
        let grid = grid(3.0, 1.0);
        let mut state = phase_state(grid, &[0.0, 0.5, 1.0], vec![false; 3]);
        let owned_liquid = 0.000030181535687548446;
        state.liquid_mass_kg[1] = owned_liquid;
        state.carrier_mass_kg[1] =
            (grid.cell_volume_m3() - owned_liquid / LIQUID_RHO) * CARRIER_RHO;
        let swept_volume = 2.5e-7;
        let flux = donor_flux(&state, 1, swept_volume, Axis::X, true, EdgeMode::Closed).unwrap();
        assert_eq!(flux.liquid_mass_kg, owned_liquid);
        let implied_volume = flux.liquid_mass_kg / LIQUID_RHO + flux.carrier_mass_kg / CARRIER_RHO;
        near(
            implied_volume,
            swept_volume,
            4.0 * f64::EPSILON * swept_volume,
        );
    }

    #[test]
    fn saturated_carrier_strip_uses_exact_owned_mass() {
        let grid = grid(3.0, 1.0);
        let mut state = phase_state(grid, &[1.0, 0.5, 0.0], vec![false; 3]);
        let owned_carrier = 1.009062743011089e-7;
        state.carrier_mass_kg[1] = owned_carrier;
        state.liquid_mass_kg[1] =
            (grid.cell_volume_m3() - owned_carrier / CARRIER_RHO) * LIQUID_RHO;
        let swept_volume = 2.5e-7;
        let flux = donor_flux(&state, 1, swept_volume, Axis::X, true, EdgeMode::Closed).unwrap();
        assert_eq!(flux.carrier_mass_kg, owned_carrier);
        let implied_volume = flux.liquid_mass_kg / LIQUID_RHO + flux.carrier_mass_kg / CARRIER_RHO;
        near(
            implied_volume,
            swept_volume,
            4.0 * f64::EPSILON * swept_volume,
        );
    }

    #[test]
    fn thousand_closed_vortex_steps_move_markers_and_conserve_totals() {
        let grid = grid(2.0, 2.0);
        let mut state = phase_state(grid, &[0.5; 4], vec![false; 4]);
        state.liquid_marker[0] = 1.0;
        state.carrier_marker[0] = 2.0;
        let prior = state.totals();
        let aperture = closed_aperture(grid);
        let fields = fields(grid, aperture);
        let mut velocity = face_values(grid, 0.0);
        velocity.u[grid.u_face_index(1, 0).unwrap()] = 1.0;
        velocity.u[grid.u_face_index(1, 1).unwrap()] = -1.0;
        velocity.v[grid.v_face_index(0, 1).unwrap()] = -1.0;
        velocity.v[grid.v_face_index(1, 1).unwrap()] = 1.0;
        let face = grid.u_face_index(1, 0).unwrap();
        for step in 0..1000 {
            let candidate = state.candidate(&fields, &velocity, 0.0025).unwrap();
            if step == 0 {
                assert!(candidate.inventory.liquid_marker[1] > 0.0);
                assert!(candidate.inventory.carrier_marker[1] > 0.0);
                near(
                    candidate.fluxes.volume_m3.u[face],
                    0.25 * CELL_VOLUME_M3,
                    1e-21,
                );
                assert!(candidate.fluxes.liquid_mass_kg.u[face] > 0.0);
                near(candidate.max_face_cfl, 0.25, 1e-15);
            }
            for cell in 0..grid.cells() {
                assert!(
                    (-1e-12..=1.0 + 1e-12).contains(&candidate.inventory.alpha(cell).unwrap()),
                    "step {step}, cell {cell}"
                );
            }
            state = candidate.inventory;
        }
        let after = state.totals();
        for (actual, expected) in [
            (after.liquid_mass_kg, prior.liquid_mass_kg),
            (after.carrier_mass_kg, prior.carrier_mass_kg),
            (after.liquid_marker, prior.liquid_marker),
            (after.carrier_marker, prior.carrier_marker),
        ] {
            assert!(expected > 0.0);
            assert!((actual - expected).abs() / expected <= 1e-10);
        }
    }

    #[test]
    fn sealed_crossflow_can_refill_a_cell_after_directional_outflow() {
        let grid = grid(3.0, 3.0);
        let mut state = phase_state(grid, &[0.5; 9], vec![false; 9]);
        state.liquid_marker[4] = 1.0;
        let before = state.totals();
        let pressure = fields(grid, closed_aperture(grid));
        let mut velocity = face_values(grid, 0.0);
        let mut stream = [[0.0; 4]; 4];
        stream[1][1] = 0.01;
        stream[1][2] = -0.01;
        stream[2][1] = -0.01;
        stream[2][2] = 0.01;
        for (y, rows) in stream.windows(2).enumerate() {
            for (x, (&below, &above)) in rows[1].iter().zip(&rows[0]).enumerate() {
                velocity.u[grid.u_face_index(x as u32, y as u32).unwrap()] =
                    (below - above) / CELL_WIDTH_M;
            }
        }
        for (y, row) in stream.iter().enumerate() {
            for (x, pair) in row.windows(2).enumerate() {
                velocity.v[grid.v_face_index(x as u32, y as u32).unwrap()] =
                    -(pair[1] - pair[0]) / CELL_WIDTH_M;
            }
        }
        let result = state.candidate(&pressure, &velocity, 0.0025).unwrap();
        let after = result.inventory.totals();
        near(after.liquid_mass_kg, before.liquid_mass_kg, 1e-15);
        near(after.carrier_mass_kg, before.carrier_mass_kg, 1e-18);
        near(after.liquid_marker, before.liquid_marker, 1e-12);
    }

    #[test]
    fn blocked_face_and_wall_cell_cannot_exchange_phase_mass() {
        let grid = grid(2.0, 1.0);
        let state = phase_state(grid, &[0.5, 0.0], vec![false, true]);
        let mut aperture = closed_aperture(grid);
        aperture.u[grid.u_face_index(1, 0).unwrap()] = 0.0;
        let fields = fields(grid, aperture);
        let velocity = face_values(grid, 0.0);
        let candidate = state.candidate(&fields, &velocity, 0.001).unwrap();
        assert_eq!(candidate.inventory, state);
        assert_eq!(candidate.fluxes.volume_m3.u[1], 0.0);
        let mut invalid_velocity = face_values(grid, 0.0);
        invalid_velocity.u[1] = 0.1;
        assert!(matches!(
            state.candidate(&fields, &invalid_velocity, 0.001),
            Err(TransportError::InvalidFace {
                axis: "u",
                index: 1,
                ..
            })
        ));
    }

    #[test]
    fn excessive_cfl_rejects_without_mutating_owned_inventory() {
        let grid = grid(2.0, 1.0);
        let state = phase_state(grid, &[0.5; 2], vec![false; 2]);
        let before = state.clone();
        let fields = fields(grid, closed_aperture(grid));
        let mut velocity = face_values(grid, 0.0);
        velocity.u[1] = 3.0;
        assert!(matches!(
            state.candidate(&fields, &velocity, 0.002),
            Err(TransportError::CflExceeded {
                axis: "u",
                index: 1,
                ..
            })
        ));
        assert_eq!(state, before);
    }

    #[test]
    fn aperture_does_not_hide_an_unsafe_face_speed() {
        let grid = grid(2.0, 1.0);
        let state = phase_state(grid, &[0.5; 2], vec![false; 2]);
        let mut aperture = closed_aperture(grid);
        aperture.u[1] = 0.1;
        let fields = fields(grid, aperture);
        let mut velocity = face_values(grid, 0.0);
        velocity.u[1] = 3.0;
        assert!(matches!(
            state.candidate(&fields, &velocity, 0.002),
            Err(TransportError::CflExceeded {
                axis: "u",
                index: 1,
                ..
            })
        ));
    }

    #[test]
    fn divergent_sealed_velocity_rejects_candidate_without_a_partial_commit() {
        let grid = grid(2.0, 1.0);
        let state = phase_state(grid, &[0.5; 2], vec![false; 2]);
        let before = state.clone();
        let fields = fields(grid, closed_aperture(grid));
        let mut velocity = face_values(grid, 0.0);
        velocity.u[1] = 1.0;
        assert!(matches!(
            state.candidate(&fields, &velocity, 0.0025),
            Err(TransportError::InvalidCell {
                reason: "incompressible phase volume closure",
                ..
            })
        ));
        assert_eq!(state, before);
    }

    #[test]
    fn periodic_test_marker_crossing_keeps_phase_and_marker_totals() {
        let grid = grid(32.0, 16.0);
        let mut state = phase_state(grid, &vec![0.5; grid.cells()], vec![false; grid.cells()]);
        for y in 4..8 {
            for x in 4..8 {
                state.liquid_marker[grid.cell_index(x, y).unwrap()] = 1.0;
            }
            for x in 10..14 {
                state.carrier_marker[grid.cell_index(x, y).unwrap()] = 1.0;
            }
        }
        let before = state.totals();
        let mut aperture = face_values(grid, 0.0);
        aperture.u.fill(1.0);
        let mut velocity = face_values(grid, 0.0);
        velocity.u.fill(0.2);
        for _ in 0..64 {
            let candidate = state
                .candidate_core(&velocity, &aperture, 0.025, EdgeMode::PeriodicX)
                .unwrap();
            state = candidate.inventory;
        }
        let after = state.totals();
        near(after.liquid_mass_kg, before.liquid_mass_kg, 1e-13);
        near(after.carrier_mass_kg, before.carrier_mass_kg, 1e-16);
        near(after.liquid_marker, before.liquid_marker, 1e-10);
        near(after.carrier_marker, before.carrier_marker, 1e-10);
        for cell in 0..grid.cells() {
            assert!((-1e-12..=1.0 + 1e-12).contains(&state.alpha(cell).unwrap()));
        }
        let circular_centroid = |marker: &[f64]| {
            let mut sin_sum = 0.0;
            let mut cos_sum = 0.0;
            for (cell, value) in marker.iter().enumerate() {
                let angle = std::f64::consts::TAU * ((cell % 32) as f64 + 0.5) / 32.0;
                sin_sum += value * angle.sin();
                cos_sum += value * angle.cos();
            }
            (sin_sum.atan2(cos_sum).rem_euclid(std::f64::consts::TAU)) * 32.0
                / std::f64::consts::TAU
        };
        near(circular_centroid(&state.liquid_marker), 6.0, 0.1);
        near(circular_centroid(&state.carrier_marker), 12.0, 0.1);
    }

    #[test]
    fn periodic_horizontal_flux_moves_a_nonuniform_liquid_interface_and_its_marker() {
        let grid = grid(4.0, 1.0);
        let mut state = phase_state(grid, &[0.0, 1.0, 0.0, 0.0], vec![false; 4]);
        state.liquid_marker[1] = 1.0;
        state.carrier_marker[0] = 1.0;
        let before = state.totals();
        let mut aperture = face_values(grid, 0.0);
        aperture.u.fill(1.0);
        let mut velocity = face_values(grid, 0.0);
        velocity.u.fill(0.2);
        let candidate = state
            .candidate_core(&velocity, &aperture, 0.025, EdgeMode::PeriodicX)
            .unwrap();
        let after = candidate.inventory.totals();
        near(after.liquid_mass_kg, before.liquid_mass_kg, 1e-18);
        near(after.carrier_mass_kg, before.carrier_mass_kg, 1e-21);
        near(after.liquid_marker, 1.0, 1e-15);
        near(after.carrier_marker, 1.0, 1e-15);
        near(candidate.inventory.alpha(1).unwrap(), 0.5, 1e-12);
        near(candidate.inventory.alpha(2).unwrap(), 0.5, 1e-12);
        near(candidate.inventory.liquid_marker[1], 0.5, 1e-12);
        near(candidate.inventory.liquid_marker[2], 0.5, 1e-12);
        near(candidate.inventory.carrier_marker[0], 0.5, 1e-12);
        near(candidate.inventory.carrier_marker[1], 0.5, 1e-12);
        near(
            candidate.fluxes.liquid_mass_kg.u[2],
            0.5 * CELL_VOLUME_M3 * LIQUID_RHO,
            1e-18,
        );
    }
}
