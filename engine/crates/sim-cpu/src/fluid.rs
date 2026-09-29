//! Owned f64 fields for the CPU pressure reference.

use particle_sim::{
    Grid,
    contracts::{Boundaries, Boundary},
};

/// Values on the horizontal and vertical faces of a staggered MAC grid.
#[derive(Debug, PartialEq)]
pub struct FaceValues {
    /// Values on `(width + 1) * height` horizontal-velocity faces, row-major.
    pub u: Vec<f64>,
    /// Values on `width * (height + 1)` vertical-velocity faces, row-major.
    pub v: Vec<f64>,
}

/// Invalid input to a CPU pressure field or session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PressureFieldError {
    /// A field does not match its cell or face layout.
    Length {
        /// Name of the rejected field.
        field: &'static str,
        /// Required number of entries.
        expected: usize,
        /// Supplied number of entries.
        actual: usize,
    },
    /// A field contains NaN or infinity.
    NonFinite {
        /// Name of the rejected field.
        field: &'static str,
        /// Row-major entry index.
        index: usize,
    },
    /// Density must be strictly positive.
    NonPositiveDensity {
        /// Row-major cell index.
        index: usize,
    },
    /// An aperture must be in the inclusive interval zero to one.
    InvalidAperture {
        /// `u` or `v` face array.
        axis: &'static str,
        /// Row-major face index.
        index: usize,
    },
    /// A blocked face cannot carry normal velocity in the fixed-wall reference.
    BlockedFaceVelocity {
        /// `u` or `v` face array.
        axis: &'static str,
        /// Row-major face index.
        index: usize,
    },
    /// A fixed closed outer face must have zero aperture and normal velocity.
    ClosedBoundaryFace {
        /// Left, right, top, or bottom.
        side: &'static str,
        /// Row-major face index.
        index: usize,
    },
    /// The fields belong to a different session geometry.
    GridMismatch,
}

impl std::fmt::Display for PressureFieldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for PressureFieldError {}

/// Validated pressure inputs owned by one CPU session.
///
/// Density is a derived pressure coefficient supplied by the future material
/// owner, not a second transported mass inventory. Pressure is a supplied
/// correction field in Pa. Closed outer faces represent fixed walls here;
/// moving-wall and reservoir flux rules belong to later pressure stages.
#[derive(Debug, PartialEq)]
pub struct PressureFields {
    grid: Grid,
    boundaries: Boundaries,
    density_kg_m3: Vec<f64>,
    correction_pressure_pa: Vec<f64>,
    velocity_m_s: FaceValues,
    aperture: FaceValues,
}

impl PressureFields {
    /// Takes ownership of validated cell and face arrays without advancing time.
    pub fn new(
        grid: Grid,
        boundaries: Boundaries,
        density_kg_m3: Vec<f64>,
        correction_pressure_pa: Vec<f64>,
        velocity_m_s: FaceValues,
        aperture: FaceValues,
    ) -> Result<Self, PressureFieldError> {
        check_len("density_kg_m3", density_kg_m3.len(), grid.cells())?;
        check_len(
            "correction_pressure_pa",
            correction_pressure_pa.len(),
            grid.cells(),
        )?;
        check_len("velocity.u_m_s", velocity_m_s.u.len(), grid.u_faces())?;
        check_len("velocity.v_m_s", velocity_m_s.v.len(), grid.v_faces())?;
        check_len("aperture.u", aperture.u.len(), grid.u_faces())?;
        check_len("aperture.v", aperture.v.len(), grid.v_faces())?;

        check_finite("density_kg_m3", &density_kg_m3)?;
        check_finite("correction_pressure_pa", &correction_pressure_pa)?;
        for (index, density) in density_kg_m3.iter().enumerate() {
            if *density <= 0.0 {
                return Err(PressureFieldError::NonPositiveDensity { index });
            }
        }
        check_face_values("u", &velocity_m_s.u, &aperture.u)?;
        check_face_values("v", &velocity_m_s.v, &aperture.v)?;
        check_closed_boundaries(grid, boundaries, &velocity_m_s, &aperture)?;

        Ok(Self {
            grid,
            boundaries,
            density_kg_m3,
            correction_pressure_pa,
            velocity_m_s,
            aperture,
        })
    }

    /// Validated geometry used for cell and face indexing.
    pub fn grid(&self) -> Grid {
        self.grid
    }

    /// Left, right, top, and bottom outer boundary declarations.
    pub fn boundaries(&self) -> Boundaries {
        self.boundaries
    }

    /// Derived density input in kg/m³, ordered by `Grid::cell_index`.
    pub fn density_kg_m3(&self) -> &[f64] {
        &self.density_kg_m3
    }

    /// Supplied mechanical pressure correction in Pa.
    pub fn correction_pressure_pa(&self) -> &[f64] {
        &self.correction_pressure_pa
    }

    /// Normal face velocities in m/s, indexed by `Grid::u_face_index` and `Grid::v_face_index`.
    pub fn velocity_m_s(&self) -> &FaceValues {
        &self.velocity_m_s
    }

    /// Open-face area fractions in the inclusive interval zero to one.
    pub fn aperture(&self) -> &FaceValues {
        &self.aperture
    }
}

fn check_len(
    field: &'static str,
    actual: usize,
    expected: usize,
) -> Result<(), PressureFieldError> {
    if actual != expected {
        return Err(PressureFieldError::Length {
            field,
            expected,
            actual,
        });
    }
    Ok(())
}

fn check_finite(field: &'static str, values: &[f64]) -> Result<(), PressureFieldError> {
    for (index, value) in values.iter().enumerate() {
        if !value.is_finite() {
            return Err(PressureFieldError::NonFinite { field, index });
        }
    }
    Ok(())
}

fn check_face_values(
    axis: &'static str,
    velocity: &[f64],
    aperture: &[f64],
) -> Result<(), PressureFieldError> {
    check_finite(
        if axis == "u" {
            "velocity.u_m_s"
        } else {
            "velocity.v_m_s"
        },
        velocity,
    )?;
    check_finite(
        if axis == "u" {
            "aperture.u"
        } else {
            "aperture.v"
        },
        aperture,
    )?;
    for (index, (&speed, &open)) in velocity.iter().zip(aperture).enumerate() {
        if !(0.0..=1.0).contains(&open) {
            return Err(PressureFieldError::InvalidAperture { axis, index });
        }
        if open == 0.0 && speed != 0.0 {
            return Err(PressureFieldError::BlockedFaceVelocity { axis, index });
        }
    }
    Ok(())
}

fn check_closed_boundaries(
    grid: Grid,
    boundaries: Boundaries,
    velocity: &FaceValues,
    aperture: &FaceValues,
) -> Result<(), PressureFieldError> {
    let width = grid.width() as usize;
    let height = grid.height() as usize;
    for y in 0..height {
        for (side, boundary, index) in [
            ("left", boundaries[0], y * (width + 1)),
            ("right", boundaries[1], y * (width + 1) + width),
        ] {
            if boundary == Boundary::Closed
                && (aperture.u[index] != 0.0 || velocity.u[index] != 0.0)
            {
                return Err(PressureFieldError::ClosedBoundaryFace { side, index });
            }
        }
    }
    for x in 0..width {
        for (side, boundary, index) in [
            ("top", boundaries[2], x),
            ("bottom", boundaries[3], height * width + x),
        ] {
            if boundary == Boundary::Closed
                && (aperture.v[index] != 0.0 || velocity.v[index] != 0.0)
            {
                return Err(PressureFieldError::ClosedBoundaryFace { side, index });
            }
        }
    }
    Ok(())
}
