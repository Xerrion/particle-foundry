//! Matching staggered divergence and pressure gradient for the CPU reference.

use std::{collections::BTreeMap, num::NonZeroU32};

use particle_sim::contracts::Boundary;

use crate::fluid::{FaceValues, PressureFieldError, PressureFields};

/// Rejection by a MAC operator before it produces a partial result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PressureOperatorError {
    /// An input face or cell array violates the validated field contract.
    Field(PressureFieldError),
    /// An open side names a reservoir without a prescribed pressure value.
    MissingReservoirPressure {
        /// Stable reservoir identity on the boundary.
        reservoir: NonZeroU32,
    },
    /// A prescribed reservoir pressure is NaN or infinite.
    NonFiniteReservoirPressure {
        /// Stable reservoir identity on the boundary.
        reservoir: NonZeroU32,
    },
    /// Two entries prescribe different pressures for the same reservoir.
    ConflictingReservoirPressure {
        /// Stable reservoir identity on the boundary.
        reservoir: NonZeroU32,
    },
    /// Finite operands overflowed while evaluating a cell or face operator.
    NonFiniteOutput {
        /// Divergence, horizontal gradient, or vertical gradient.
        field: &'static str,
        /// Row-major cell or face index.
        index: usize,
    },
}

impl std::fmt::Display for PressureOperatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for PressureOperatorError {}

/// Prescribed mechanical pressure for a declared open-boundary reservoir.
///
/// Repeated identical entries are harmless; conflicting entries are rejected.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReservoirPressure {
    /// Stable ID referenced by `Boundary::Open`.
    pub reservoir: NonZeroU32,
    /// Pressure in Pa, in the same correction-pressure frame as the cell field.
    pub pressure_pa: f64,
}

/// Returns the aperture-weighted volume-velocity divergence in s⁻¹.
///
/// Positive `u` points right and positive `v` points down. The cell result is
/// `(a_r u_r - a_l u_l + a_b v_b - a_t v_t) / grid.cell_width_m()`. An open face uses
/// its supplied normal velocity, including inward flow; this operator does not
/// infer a velocity from reservoir pressure. It reads, but never advances or
/// changes, the owning pressure fields.
pub fn divergence_per_s(
    fields: &PressureFields,
    velocity_m_s: &FaceValues,
) -> Result<Vec<f64>, PressureOperatorError> {
    validate_velocity(fields, velocity_m_s)?;

    let grid = fields.grid();
    let width = grid.width() as usize;
    let height = grid.height() as usize;
    let cell_width_m = grid.cell_width_m();
    let aperture = fields.aperture();
    let mut divergence = vec![0.0; grid.cells()];

    for y in 0..height {
        for x in 0..width {
            let left = y * (width + 1) + x;
            let right = left + 1;
            let top = y * width + x;
            let bottom = top + width;
            divergence[y * width + x] = (aperture.u[right] * velocity_m_s.u[right]
                - aperture.u[left] * velocity_m_s.u[left]
                + aperture.v[bottom] * velocity_m_s.v[bottom]
                - aperture.v[top] * velocity_m_s.v[top])
                / cell_width_m;
        }
    }
    reject_nonfinite_output("divergence_per_s", &divergence)?;
    Ok(divergence)
}

/// Returns the centered pressure difference across each interior face in Pa/m.
///
/// A positive `u` gradient is right-minus-left; a positive `v` gradient is
/// bottom-minus-top. Closed outer faces and blocked interior faces have zero
/// gradient. The aperture appears inside `divergence_per_s`, so `D(G p)` is
/// symmetric and negative semidefinite for closed boundaries.
/// An open face needs an explicit reservoir pressure and is rejected here.
pub fn pressure_gradient_pa_per_m(
    fields: &PressureFields,
    pressure_pa: &[f64],
) -> Result<FaceValues, PressureOperatorError> {
    pressure_gradient_with_reservoirs_pa_per_m(fields, pressure_pa, &[])
}

/// Returns cell-center and prescribed-reservoir pressure differences in Pa/m.
///
/// Open outer faces use a half-cell distance and the reservoir identified by
/// `Boundary::Open`. The caller supplies pressure in the same frame as the cell
/// correction pressure. Missing, nonfinite, or conflicting reservoir values
/// reject the whole operation before a gradient is returned.
pub fn pressure_gradient_with_reservoirs_pa_per_m(
    fields: &PressureFields,
    pressure_pa: &[f64],
    reservoir_pressures: &[ReservoirPressure],
) -> Result<FaceValues, PressureOperatorError> {
    let grid = fields.grid();
    validate_pressure(grid.cells(), pressure_pa)?;
    let boundary_pressure = boundary_pressures(fields, reservoir_pressures)?;

    let width = grid.width() as usize;
    let height = grid.height() as usize;
    let cell_width_m = grid.cell_width_m();
    let aperture = fields.aperture();
    let mut gradient = FaceValues {
        u: vec![0.0; grid.u_faces()],
        v: vec![0.0; grid.v_faces()],
    };

    for y in 0..height {
        for x in 1..width {
            let face = y * (width + 1) + x;
            if aperture.u[face] > 0.0 {
                gradient.u[face] =
                    (pressure_pa[y * width + x] - pressure_pa[y * width + x - 1]) / cell_width_m;
            }
        }
    }
    for y in 1..height {
        for x in 0..width {
            let face = y * width + x;
            if aperture.v[face] > 0.0 {
                gradient.v[face] =
                    (pressure_pa[y * width + x] - pressure_pa[(y - 1) * width + x]) / cell_width_m;
            }
        }
    }
    let half_cell_width_m = cell_width_m / 2.0;
    for y in 0..height {
        let left_face = y * (width + 1);
        let right_face = left_face + width;
        if let Some(reservoir_pa) = boundary_pressure[0]
            && aperture.u[left_face] > 0.0
        {
            gradient.u[left_face] = (pressure_pa[y * width] - reservoir_pa) / half_cell_width_m;
        }
        if let Some(reservoir_pa) = boundary_pressure[1]
            && aperture.u[right_face] > 0.0
        {
            gradient.u[right_face] =
                (reservoir_pa - pressure_pa[y * width + width - 1]) / half_cell_width_m;
        }
    }
    for x in 0..width {
        let top_face = x;
        let bottom_face = height * width + x;
        if let Some(reservoir_pa) = boundary_pressure[2]
            && aperture.v[top_face] > 0.0
        {
            gradient.v[top_face] = (pressure_pa[x] - reservoir_pa) / half_cell_width_m;
        }
        if let Some(reservoir_pa) = boundary_pressure[3]
            && aperture.v[bottom_face] > 0.0
        {
            gradient.v[bottom_face] =
                (reservoir_pa - pressure_pa[(height - 1) * width + x]) / half_cell_width_m;
        }
    }
    reject_nonfinite_output("gradient.u_pa_per_m", &gradient.u)?;
    reject_nonfinite_output("gradient.v_pa_per_m", &gradient.v)?;
    Ok(gradient)
}

fn reject_nonfinite_output(
    field: &'static str,
    values: &[f64],
) -> Result<(), PressureOperatorError> {
    for (index, value) in values.iter().enumerate() {
        if !value.is_finite() {
            return Err(PressureOperatorError::NonFiniteOutput { field, index });
        }
    }
    Ok(())
}

fn boundary_pressures(
    fields: &PressureFields,
    reservoir_pressures: &[ReservoirPressure],
) -> Result<[Option<f64>; 4], PressureOperatorError> {
    let mut by_reservoir = BTreeMap::new();
    for entry in reservoir_pressures {
        if !entry.pressure_pa.is_finite() {
            return Err(PressureOperatorError::NonFiniteReservoirPressure {
                reservoir: entry.reservoir,
            });
        }
        if let Some(existing_pa) = by_reservoir.get(&entry.reservoir) {
            if *existing_pa != entry.pressure_pa {
                return Err(PressureOperatorError::ConflictingReservoirPressure {
                    reservoir: entry.reservoir,
                });
            }
        } else {
            by_reservoir.insert(entry.reservoir, entry.pressure_pa);
        }
    }

    let mut prescribed = [None; 4];
    for (side, boundary) in fields.boundaries().into_iter().enumerate() {
        if let Boundary::Open { reservoir } = boundary {
            prescribed[side] = Some(
                *by_reservoir
                    .get(&reservoir)
                    .ok_or(PressureOperatorError::MissingReservoirPressure { reservoir })?,
            );
        }
    }
    Ok(prescribed)
}

fn validate_velocity(
    fields: &PressureFields,
    velocity_m_s: &FaceValues,
) -> Result<(), PressureOperatorError> {
    let grid = fields.grid();
    for (name, actual, expected) in [
        ("velocity.u_m_s", velocity_m_s.u.len(), grid.u_faces()),
        ("velocity.v_m_s", velocity_m_s.v.len(), grid.v_faces()),
    ] {
        if actual != expected {
            return Err(PressureOperatorError::Field(PressureFieldError::Length {
                field: name,
                expected,
                actual,
            }));
        }
    }
    for (axis, name, values, apertures) in [
        ("u", "velocity.u_m_s", &velocity_m_s.u, &fields.aperture().u),
        ("v", "velocity.v_m_s", &velocity_m_s.v, &fields.aperture().v),
    ] {
        for (index, (&velocity, &aperture)) in values.iter().zip(apertures).enumerate() {
            if !velocity.is_finite() {
                return Err(PressureOperatorError::Field(
                    PressureFieldError::NonFinite { field: name, index },
                ));
            }
            if aperture == 0.0 && velocity != 0.0 {
                return Err(PressureOperatorError::Field(
                    PressureFieldError::BlockedFaceVelocity { axis, index },
                ));
            }
        }
    }
    Ok(())
}

fn validate_pressure(cells: usize, pressure_pa: &[f64]) -> Result<(), PressureOperatorError> {
    if pressure_pa.len() != cells {
        return Err(PressureOperatorError::Field(PressureFieldError::Length {
            field: "pressure_pa",
            expected: cells,
            actual: pressure_pa.len(),
        }));
    }
    for (index, value) in pressure_pa.iter().enumerate() {
        if !value.is_finite() {
            return Err(PressureOperatorError::Field(
                PressureFieldError::NonFinite {
                    field: "pressure_pa",
                    index,
                },
            ));
        }
    }
    Ok(())
}
