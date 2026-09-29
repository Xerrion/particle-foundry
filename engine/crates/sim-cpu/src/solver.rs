//! Atomic, closed-boundary f64 pressure projection for the CPU reference.

use particle_sim::contracts::Boundary;

use crate::{
    assembly::{PressureAssembly, PressureAssemblyError},
    fluid::{FaceValues, PressureFields},
    operator::{PressureOperatorError, divergence_per_s, pressure_gradient_pa_per_m},
};

/// A bounded pressure solve must meet both dimensionless gates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolveConfig {
    /// Maximum matrix-vector CG iterations; zero permits only an already valid initial field.
    pub max_iterations: usize,
    /// Maximum accepted `dt² max|b - Aπ|`.
    pub scaled_residual_tolerance: f64,
    /// Maximum accepted `dt max|D u_next - S_volume|`.
    pub scaled_divergence_tolerance: f64,
}

impl Default for SolveConfig {
    fn default() -> Self {
        Self {
            max_iterations: 256,
            scaled_residual_tolerance: 1e-8,
            scaled_divergence_tolerance: 1e-8,
        }
    }
}

/// Measurements from the same candidate pressure and independently corrected velocity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolveDiagnostics {
    /// Number of matrix-vector CG updates performed.
    pub iterations: usize,
    /// True equation residual, recomputed as `dt² max|b - Aπ|`.
    pub scaled_residual: f64,
    /// Corrected-face divergence, recomputed as `dt max|D u_next - S_volume|`.
    pub scaled_divergence: f64,
}

/// A committed candidate, produced only after both numerical gates pass.
#[derive(Debug, PartialEq)]
pub struct Projection {
    /// One zero-mean pressure correction per sealed connected component, in Pa.
    pub pressure_pa: Vec<f64>,
    /// Face velocities after the matching pressure correction, in m/s.
    pub velocity_m_s: FaceValues,
    /// Independently checked convergence measurements.
    pub diagnostics: SolveDiagnostics,
}

/// Rejection before any caller-owned state is changed.
#[derive(Clone, Debug, PartialEq)]
pub enum PressureSolveError {
    /// Time step must be finite and strictly positive.
    InvalidTimeStep,
    /// A convergence tolerance must be finite and strictly positive.
    InvalidTolerance {
        /// Name of the rejected convergence gate.
        field: &'static str,
    },
    /// Gravity must be finite.
    InvalidGravity,
    /// The volume-source array must contain one value per cell.
    SourceLength {
        /// Number of cells in the validated grid.
        expected: usize,
        /// Number of values supplied by the caller.
        actual: usize,
    },
    /// The volume source has NaN or infinity.
    NonFiniteSource {
        /// Row-major index of the first invalid cell.
        cell: usize,
    },
    /// A validated field, operator, or matrix assembly rejected input/arithmetic.
    Operator(PressureOperatorError),
    /// A matrix assembly or sealed RHS check failed.
    Assembly(PressureAssemblyError),
    /// The cached matrix and pressure fields use different grid geometry.
    AssemblyGridMismatch,
    /// An intermediate f64 calculation was not representable or lost a positive CG norm.
    NumericalBreakdown {
        /// Calculation that became unrepresentable.
        stage: &'static str,
    },
    /// The bounded solve stopped without meeting both convergence gates.
    DidNotConverge {
        /// Measurements at the last candidate, not a committed result.
        diagnostics: SolveDiagnostics,
    },
}

impl std::fmt::Display for PressureSolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for PressureSolveError {}

impl From<PressureAssemblyError> for PressureSolveError {
    fn from(value: PressureAssemblyError) -> Self {
        Self::Assembly(value)
    }
}

impl From<PressureOperatorError> for PressureSolveError {
    fn from(value: PressureOperatorError) -> Self {
        Self::Operator(value)
    }
}

/// Applies downward gravity to open interior vertical faces before projection.
///
/// Positive `gravity_m_s2` points down, matching the positive `v` axis. Closed
/// and blocked faces retain zero normal velocity. The input fields are unchanged.
/// The same face density coefficient in the later pressure correction permits
/// a hydrostatic pressure gradient to cancel this acceleration.
pub fn gravity_predictor_closed(
    fields: &PressureFields,
    dt_s: f64,
    gravity_m_s2: f64,
) -> Result<FaceValues, PressureSolveError> {
    validate_dt(dt_s)?;
    if !gravity_m_s2.is_finite() {
        return Err(PressureSolveError::InvalidGravity);
    }
    for (side, boundary) in ["left", "right", "top", "bottom"]
        .into_iter()
        .zip(fields.boundaries())
    {
        if matches!(boundary, Boundary::Open { .. }) {
            return Err(PressureSolveError::Assembly(
                PressureAssemblyError::OpenBoundaryRequiresReservoir { side },
            ));
        }
    }
    let grid = fields.grid();
    let width = grid.width() as usize;
    let height = grid.height() as usize;
    let mut velocity = copy_faces(fields.velocity_m_s());
    for y in 1..height {
        for x in 0..width {
            let face = y * width + x;
            if fields.aperture().v[face] > 0.0 {
                let next = velocity.v[face] + dt_s * gravity_m_s2;
                if !next.is_finite() {
                    return Err(PressureSolveError::NumericalBreakdown {
                        stage: "gravity predictor",
                    });
                }
                velocity.v[face] = next;
            }
        }
    }
    Ok(velocity)
}

/// Projects one predicted MAC velocity without mutating the caller's world.
///
/// `A = -D[(1/rho_face)G]` is positive semidefinite on sealed components and
/// `b = (S_volume - D u_star)/dt`. Face apertures appear exactly once in `D`;
/// the same assembled inverse face density multiplies `G` in the correction.
/// A Jacobi-preconditioned CG solve removes each component's constant-pressure
/// mode after every preconditioning/search update. Both convergence gates are
/// checked against freshly evaluated operators. Failure returns no pressure or
/// velocity state for a caller to commit.
pub fn project_closed(
    fields: &PressureFields,
    predictor_m_s: &FaceValues,
    volume_source_per_s: &[f64],
    dt_s: f64,
    config: SolveConfig,
) -> Result<Projection, PressureSolveError> {
    let assembly = PressureAssembly::new(fields)?;
    project_closed_with_assembly(
        fields,
        &assembly,
        predictor_m_s,
        volume_source_per_s,
        dt_s,
        config,
    )
}

/// Uses a session-owned assembly after its geometry, aperture, and density
/// versions have been checked by the owning cache. This is crate-private so
/// external callers cannot supply an unrelated or stale coefficient matrix.
pub(crate) fn project_closed_with_assembly(
    fields: &PressureFields,
    assembly: &PressureAssembly,
    predictor_m_s: &FaceValues,
    volume_source_per_s: &[f64],
    dt_s: f64,
    config: SolveConfig,
) -> Result<Projection, PressureSolveError> {
    if assembly.grid() != fields.grid() {
        return Err(PressureSolveError::AssemblyGridMismatch);
    }
    validate_dt(dt_s)?;
    validate_tolerance(
        "scaled_residual_tolerance",
        config.scaled_residual_tolerance,
    )?;
    validate_tolerance(
        "scaled_divergence_tolerance",
        config.scaled_divergence_tolerance,
    )?;
    let grid = fields.grid();
    if volume_source_per_s.len() != grid.cells() {
        return Err(PressureSolveError::SourceLength {
            expected: grid.cells(),
            actual: volume_source_per_s.len(),
        });
    }
    for (cell, value) in volume_source_per_s.iter().enumerate() {
        if !value.is_finite() {
            return Err(PressureSolveError::NonFiniteSource { cell });
        }
    }

    let predictor_divergence = divergence_per_s(fields, predictor_m_s)?;
    let mut rhs = vec![0.0; grid.cells()];
    for cell in 0..grid.cells() {
        rhs[cell] = (volume_source_per_s[cell] - predictor_divergence[cell]) / dt_s;
        if !rhs[cell].is_finite() {
            return Err(PressureSolveError::NumericalBreakdown { stage: "RHS" });
        }
    }
    assembly.check_compatible_rhs(&rhs)?;

    let mut pressure = fields.correction_pressure_pa().to_vec();
    remove_component_means(&mut pressure, assembly)?;
    let mut candidate = evaluate_candidate(
        fields,
        assembly,
        predictor_m_s,
        volume_source_per_s,
        &rhs,
        dt_s,
        &pressure,
        0,
    )?;
    if converged(candidate.diagnostics, config) {
        return Ok(candidate.commit(pressure));
    }
    if config.max_iterations == 0 {
        return Err(PressureSolveError::DidNotConverge {
            diagnostics: candidate.diagnostics,
        });
    }

    let mut residual = candidate.residual;
    remove_component_means(&mut residual, assembly)?;
    let mut preconditioned = jacobi(&residual, assembly)?;
    remove_component_means(&mut preconditioned, assembly)?;
    let mut search = preconditioned.clone();
    let mut rho = dot(&residual, &preconditioned, "initial CG norm")?;

    for iteration in 1..=config.max_iterations {
        if rho <= 0.0 {
            return Err(PressureSolveError::NumericalBreakdown { stage: "CG norm" });
        }
        let applied_search = assembly.apply(&search)?;
        let denominator = dot(&search, &applied_search, "CG denominator")?;
        if denominator <= 0.0 {
            return Err(PressureSolveError::NumericalBreakdown {
                stage: "CG denominator",
            });
        }
        let alpha = rho / denominator;
        if !alpha.is_finite() {
            return Err(PressureSolveError::NumericalBreakdown { stage: "CG step" });
        }
        for cell in 0..grid.cells() {
            pressure[cell] += alpha * search[cell];
            if !pressure[cell].is_finite() {
                return Err(PressureSolveError::NumericalBreakdown {
                    stage: "pressure update",
                });
            }
        }
        remove_component_means(&mut pressure, assembly)?;
        candidate = evaluate_candidate(
            fields,
            assembly,
            predictor_m_s,
            volume_source_per_s,
            &rhs,
            dt_s,
            &pressure,
            iteration,
        )?;
        if converged(candidate.diagnostics, config) {
            return Ok(candidate.commit(pressure));
        }
        if iteration == config.max_iterations {
            return Err(PressureSolveError::DidNotConverge {
                diagnostics: candidate.diagnostics,
            });
        }

        residual = candidate.residual;
        remove_component_means(&mut residual, assembly)?;
        preconditioned = jacobi(&residual, assembly)?;
        remove_component_means(&mut preconditioned, assembly)?;
        let next_rho = dot(&residual, &preconditioned, "CG norm")?;
        if next_rho < 0.0 {
            return Err(PressureSolveError::NumericalBreakdown { stage: "CG norm" });
        }
        let beta = next_rho / rho;
        if !beta.is_finite() {
            return Err(PressureSolveError::NumericalBreakdown {
                stage: "CG search update",
            });
        }
        for cell in 0..grid.cells() {
            search[cell] = preconditioned[cell] + beta * search[cell];
            if !search[cell].is_finite() {
                return Err(PressureSolveError::NumericalBreakdown {
                    stage: "CG search update",
                });
            }
        }
        remove_component_means(&mut search, assembly)?;
        rho = next_rho;
    }
    unreachable!("the bounded loop returns on convergence or exhaustion")
}

struct Candidate {
    velocity: FaceValues,
    residual: Vec<f64>,
    diagnostics: SolveDiagnostics,
}

impl Candidate {
    fn commit(self, pressure_pa: Vec<f64>) -> Projection {
        Projection {
            pressure_pa,
            velocity_m_s: self.velocity,
            diagnostics: self.diagnostics,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn evaluate_candidate(
    fields: &PressureFields,
    assembly: &PressureAssembly,
    predictor: &FaceValues,
    source: &[f64],
    rhs: &[f64],
    dt_s: f64,
    pressure: &[f64],
    iterations: usize,
) -> Result<Candidate, PressureSolveError> {
    let applied = assembly.apply(pressure)?;
    let mut residual = vec![0.0; rhs.len()];
    let mut scaled_residual: f64 = 0.0;
    for cell in 0..rhs.len() {
        residual[cell] = rhs[cell] - applied[cell];
        let scaled = (residual[cell] * dt_s) * dt_s;
        if !scaled.is_finite() {
            return Err(PressureSolveError::NumericalBreakdown {
                stage: "scaled residual",
            });
        }
        scaled_residual = scaled_residual.max(scaled.abs());
    }
    let gradient = pressure_gradient_pa_per_m(fields, pressure)?;
    let inverse_density = assembly.inverse_face_density_m3_kg();
    let mut velocity = copy_faces(predictor);
    for (values, grad, beta) in [
        (&mut velocity.u, &gradient.u, &inverse_density.u),
        (&mut velocity.v, &gradient.v, &inverse_density.v),
    ] {
        for face in 0..values.len() {
            values[face] -= dt_s * beta[face] * grad[face];
            if !values[face].is_finite() {
                return Err(PressureSolveError::NumericalBreakdown {
                    stage: "velocity correction",
                });
            }
        }
    }
    let corrected_divergence = divergence_per_s(fields, &velocity)?;
    let mut scaled_divergence: f64 = 0.0;
    for cell in 0..rhs.len() {
        let scaled = (corrected_divergence[cell] - source[cell]) * dt_s;
        if !scaled.is_finite() {
            return Err(PressureSolveError::NumericalBreakdown {
                stage: "scaled divergence",
            });
        }
        scaled_divergence = scaled_divergence.max(scaled.abs());
    }
    Ok(Candidate {
        velocity,
        residual,
        diagnostics: SolveDiagnostics {
            iterations,
            scaled_residual,
            scaled_divergence,
        },
    })
}

fn converged(diagnostics: SolveDiagnostics, config: SolveConfig) -> bool {
    diagnostics.scaled_residual <= config.scaled_residual_tolerance
        && diagnostics.scaled_divergence <= config.scaled_divergence_tolerance
}

fn validate_dt(dt_s: f64) -> Result<(), PressureSolveError> {
    if !dt_s.is_finite() || dt_s <= 0.0 {
        return Err(PressureSolveError::InvalidTimeStep);
    }
    Ok(())
}

fn validate_tolerance(field: &'static str, tolerance: f64) -> Result<(), PressureSolveError> {
    if !tolerance.is_finite() || tolerance <= 0.0 {
        return Err(PressureSolveError::InvalidTolerance { field });
    }
    Ok(())
}

fn copy_faces(values: &FaceValues) -> FaceValues {
    FaceValues {
        u: values.u.clone(),
        v: values.v.clone(),
    }
}

fn jacobi(residual: &[f64], assembly: &PressureAssembly) -> Result<Vec<f64>, PressureSolveError> {
    let mut result = vec![0.0; residual.len()];
    for cell in 0..residual.len() {
        let diagonal = assembly.diagonal_m_kg()[cell];
        if diagonal > 0.0 {
            result[cell] = residual[cell] / diagonal;
            if !result[cell].is_finite() {
                return Err(PressureSolveError::NumericalBreakdown {
                    stage: "Jacobi preconditioner",
                });
            }
        }
    }
    Ok(result)
}

fn dot(a: &[f64], b: &[f64], stage: &'static str) -> Result<f64, PressureSolveError> {
    let mut sum = 0.0;
    for (&x, &y) in a.iter().zip(b) {
        sum += x * y;
        if !sum.is_finite() {
            return Err(PressureSolveError::NumericalBreakdown { stage });
        }
    }
    Ok(sum)
}

fn remove_component_means(
    values: &mut [f64],
    assembly: &PressureAssembly,
) -> Result<(), PressureSolveError> {
    let component_of = assembly.component_of_cell();
    let sizes = assembly.component_sizes();
    let mut sums = vec![0.0; sizes.len()];
    let mut correction = vec![0.0; sizes.len()];
    for (cell, &value) in values.iter().enumerate() {
        let component = component_of[cell];
        let next = sums[component] + value;
        if sums[component].abs() >= value.abs() {
            correction[component] += (sums[component] - next) + value;
        } else {
            correction[component] += (value - next) + sums[component];
        }
        sums[component] = next;
        if !next.is_finite() || !correction[component].is_finite() {
            return Err(PressureSolveError::NumericalBreakdown {
                stage: "component mean",
            });
        }
    }
    for (cell, value) in values.iter_mut().enumerate() {
        let component = component_of[cell];
        *value -= (sums[component] + correction[component]) / sizes[component] as f64;
        if !value.is_finite() {
            return Err(PressureSolveError::NumericalBreakdown {
                stage: "gauge projection",
            });
        }
    }
    Ok(())
}
