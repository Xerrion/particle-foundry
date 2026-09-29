//! Atomic closed-boundary CPU fluid substep staging.
//!
//! This first coupled path has one liquid and a carrier phase, fixed walls,
//! passive extensive markers, first-order momentum transport, optional
//! tangential viscosity, gravity, and pressure projection. It has no sources,
//! heat deposition, reservoirs, chemistry, or GPU execution. All stage results
//! are detached until the owning session accepts them with physical time.

use particle_sim::{CELL_WIDTH_M, Grid, contracts::Boundary};

use crate::{
    fluid::{FaceValues, PressureFieldError, PressureFields},
    momentum::{MomentumError, momentum_candidate},
    operator::{PressureOperatorError, divergence_per_s},
    solver::{
        PressureSolveError, SolveConfig, SolveDiagnostics, gravity_predictor_closed, project_closed,
    },
    substep::{PauseReason, SubstepCandidate, SubstepClock, SubstepError, SubstepSelection},
    transport::{CELL_VOLUME_M3, TransportError, TransportInventory, VOLUME_CLOSURE_REL_TOL},
    viscosity::{ViscosityDiagnostics, ViscosityError, shear_viscosity_candidate},
};

/// The maximum scaled source and projected divergence for this transport path.
/// This cap alone does not guarantee a valid transport candidate. A source cell
/// can already be near its allowed volume-closure error.
pub const MAX_SCALED_DIVERGENCE: f64 = VOLUME_CLOSURE_REL_TOL * 0.25;

/// Inputs for one fixed-wall fluid substep.
#[derive(Clone, Copy, Debug)]
pub struct CoupledStepConfig<'a> {
    /// Downward acceleration in m/s², including zero for unforced flow.
    pub gravity_m_s2: f64,
    /// Positive dynamic viscosity in Pa s for every cell. `None` omits shear.
    pub cell_dynamic_viscosity_pa_s: Option<&'a [f64]>,
    /// Iteration and convergence limits for the final pressure projection.
    pub pressure: SolveConfig,
}

/// Rejection before session fields, inventory, or accepted physical time change.
#[derive(Clone, Debug, PartialEq)]
pub enum CoupledStepError {
    /// The session has no pressure inputs.
    MissingPressureFields,
    /// The session has no phase inventory.
    MissingTransportInventory,
    /// The two owners or the session use different grid dimensions.
    GridMismatch,
    /// A reservoir ledger is required for an open outer side.
    UnsupportedOpenBoundary,
    /// The pressure coefficient differs from the density derived from owned mass.
    DensityMismatch {
        /// Row-major cell index.
        cell: usize,
    },
    /// A fixed-wall cell has an incident positive-aperture face.
    OpenWallFace {
        /// `u` or `v` face array.
        axis: &'static str,
        /// Row-major face index.
        face: usize,
    },
    /// Dynamic viscosity has the wrong cell count.
    ViscosityLength {
        /// Required number of cells.
        expected: usize,
        /// Supplied number of values.
        actual: usize,
    },
    /// A cell has nonpositive or nonfinite dynamic viscosity.
    InvalidViscosity {
        /// Row-major cell index.
        cell: usize,
    },
    /// The bound derived from phase density and viscosity is not representable.
    InvalidKinematicViscosity,
    /// Initial projected velocity exceeds this path's divergence cap.
    SourceDivergence {
        /// Maximum `dt * |D u|` over cells.
        scaled_divergence: f64,
    },
    /// The physical-time selector rejected an input or acceptance.
    Substep(SubstepError),
    /// A face divergence could not be evaluated.
    Operator(PressureOperatorError),
    /// Phase or marker transport rejected its detached candidate. A source
    /// inventory near its allowed volume-closure limit can reach that limit
    /// even when initial divergence is below the separate divergence cap.
    Transport(TransportError),
    /// Compatible momentum transport rejected its detached candidate.
    Momentum(MomentumError),
    /// The optional shear stage rejected its detached candidate.
    Viscosity(ViscosityError),
    /// The gravity or pressure stage rejected its detached candidate.
    Pressure(PressureSolveError),
    /// A derived field could not meet the validated pressure-field contract.
    Field(PressureFieldError),
}

impl std::fmt::Display for CoupledStepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for CoupledStepError {}

/// Mechanical accounting from the compatible momentum-advection stage.
///
/// `numerical_change_j` measures advection and wall-clamping effects only.
/// It is not heat deposited into a thermal state. Gravity, pressure work,
/// and later mechanical stages are outside this diagnostic.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MomentumDiagnostics {
    /// Impulse applied to the fluid by fixed walls in horizontal and vertical directions.
    pub wall_impulse_kg_m_s: [f64; 2],
    /// Staggered kinetic energy before momentum advection, in J.
    pub kinetic_before_j: f64,
    /// Staggered kinetic energy after momentum advection and wall clamping, in J.
    pub kinetic_after_j: f64,
    /// Kinetic change over momentum advection and wall clamping, in J.
    pub numerical_change_j: f64,
}

/// One completed state transition or a bounded stop without state changes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CoupledStepOutcome {
    /// A full substep and its physical duration were accepted.
    Advanced {
        /// Accepted duration in seconds.
        dt_s: f64,
        /// Largest geometric face displacement in cell widths.
        max_face_cfl: f64,
        /// Mechanical accounting from compatible momentum advection.
        momentum: MomentumDiagnostics,
        /// Mechanical shear observations when viscosity was requested.
        /// No energy is deposited into a thermal field by this CPU path.
        shear: Option<ViscosityDiagnostics>,
        /// Pressure residual and corrected divergence from this candidate.
        pressure: SolveDiagnostics,
    },
    /// The requested outer interval has completed.
    Complete,
    /// The work budget ended before the requested interval completed.
    Paused {
        /// Reason for the pause.
        reason: PauseReason,
        /// Requested physical time still unadvanced, in seconds.
        remaining_s: f64,
    },
}

pub(crate) struct StagedCoupledStep {
    pub(crate) clock_candidate: SubstepCandidate,
    pub(crate) pressure_fields: PressureFields,
    pub(crate) inventory: TransportInventory,
    pub(crate) outcome: CoupledStepOutcome,
}

pub(crate) enum CoupledSelection {
    Staged(Box<StagedCoupledStep>),
    NoChange(CoupledStepOutcome),
}

/// Builds all stages against immutable owners and an immutable outer clock.
pub(crate) fn stage_coupled_step(
    grid: Grid,
    fields: &PressureFields,
    inventory: &TransportInventory,
    clock: &SubstepClock,
    config: CoupledStepConfig<'_>,
) -> Result<CoupledSelection, CoupledStepError> {
    if fields.grid() != grid || inventory.grid() != grid {
        return Err(CoupledStepError::GridMismatch);
    }
    if fields
        .boundaries()
        .iter()
        .any(|side| *side != Boundary::Closed)
    {
        return Err(CoupledStepError::UnsupportedOpenBoundary);
    }
    if !config.gravity_m_s2.is_finite() {
        return Err(CoupledStepError::Pressure(
            PressureSolveError::InvalidGravity,
        ));
    }
    validate_owner_match(grid, fields, inventory)?;
    let max_nu_m2_s = validate_viscosity(inventory, config.cell_dynamic_viscosity_pa_s)?;
    let selected = clock
        .select(grid, fields.velocity_m_s(), CELL_WIDTH_M, max_nu_m2_s)
        .map_err(CoupledStepError::Substep)?;
    let clock_candidate = match selected {
        SubstepSelection::Complete => {
            return Ok(CoupledSelection::NoChange(CoupledStepOutcome::Complete));
        }
        SubstepSelection::Paused {
            reason,
            remaining_s,
        } => {
            return Ok(CoupledSelection::NoChange(CoupledStepOutcome::Paused {
                reason,
                remaining_s,
            }));
        }
        SubstepSelection::Candidate(candidate) => candidate,
    };
    let dt_s = clock_candidate.dt_s();
    let source_divergence =
        divergence_per_s(fields, fields.velocity_m_s()).map_err(CoupledStepError::Operator)?;
    let scaled_divergence = source_divergence
        .iter()
        .map(|divergence| dt_s * divergence.abs())
        .fold(0.0_f64, f64::max);
    if !scaled_divergence.is_finite() || scaled_divergence > MAX_SCALED_DIVERGENCE {
        return Err(CoupledStepError::SourceDivergence { scaled_divergence });
    }

    let transport = inventory
        .candidate(fields, fields.velocity_m_s(), dt_s)
        .map_err(CoupledStepError::Transport)?;
    let momentum = momentum_candidate(inventory, &transport, fields, fields.velocity_m_s(), dt_s)
        .map_err(CoupledStepError::Momentum)?;
    let momentum_diagnostics = MomentumDiagnostics {
        wall_impulse_kg_m_s: momentum.wall_impulse_kg_m_s,
        kinetic_before_j: momentum.kinetic_before_j,
        kinetic_after_j: momentum.kinetic_after_j,
        numerical_change_j: momentum.numerical_change_j,
    };
    let density_kg_m3 = derived_density(&transport.inventory)?;
    let advected_fields = make_fields(
        fields,
        density_kg_m3,
        fields.correction_pressure_pa().to_vec(),
        momentum.velocity_m_s,
    )?;
    let (velocity_after_shear, shear_diagnostics) =
        if let Some(viscosity) = config.cell_dynamic_viscosity_pa_s {
            let candidate = shear_viscosity_candidate(
                &advected_fields,
                advected_fields.velocity_m_s(),
                viscosity,
                dt_s,
            )
            .map_err(CoupledStepError::Viscosity)?;
            (candidate.velocity_m_s, Some(candidate.diagnostics))
        } else {
            (copy_faces(advected_fields.velocity_m_s()), None)
        };
    let shear_fields = make_fields(
        &advected_fields,
        advected_fields.density_kg_m3().to_vec(),
        advected_fields.correction_pressure_pa().to_vec(),
        velocity_after_shear,
    )?;
    let gravity_velocity = gravity_predictor_closed(&shear_fields, dt_s, config.gravity_m_s2)
        .map_err(CoupledStepError::Pressure)?;
    let mut pressure_config = config.pressure;
    pressure_config.scaled_divergence_tolerance = pressure_config
        .scaled_divergence_tolerance
        .min(MAX_SCALED_DIVERGENCE);
    let projection = project_closed(
        &shear_fields,
        &gravity_velocity,
        &vec![0.0; grid.cells()],
        dt_s,
        pressure_config,
    )
    .map_err(CoupledStepError::Pressure)?;
    let pressure = projection.diagnostics;
    let pressure_fields = make_fields(
        &shear_fields,
        shear_fields.density_kg_m3().to_vec(),
        projection.pressure_pa,
        projection.velocity_m_s,
    )?;
    Ok(CoupledSelection::Staged(Box::new(StagedCoupledStep {
        clock_candidate,
        pressure_fields,
        inventory: transport.inventory,
        outcome: CoupledStepOutcome::Advanced {
            dt_s,
            max_face_cfl: transport.max_face_cfl,
            momentum: momentum_diagnostics,
            shear: shear_diagnostics,
            pressure,
        },
    })))
}

fn validate_owner_match(
    grid: Grid,
    fields: &PressureFields,
    inventory: &TransportInventory,
) -> Result<(), CoupledStepError> {
    let density = derived_density(inventory)?;
    for (cell, (&actual, &expected)) in fields.density_kg_m3().iter().zip(&density).enumerate() {
        let allowed = 1e-12 * actual.abs().max(expected.abs());
        if (actual - expected).abs() > allowed {
            return Err(CoupledStepError::DensityMismatch { cell });
        }
    }
    let width = grid.width() as usize;
    let height = grid.height() as usize;
    let wall = inventory.fixed_wall();
    for y in 0..height {
        for x in 0..=width {
            let face = y * (width + 1) + x;
            if ((x > 0 && wall[y * width + x - 1]) || (x < width && wall[y * width + x]))
                && fields.aperture().u[face] != 0.0
            {
                return Err(CoupledStepError::OpenWallFace { axis: "u", face });
            }
        }
    }
    for y in 0..=height {
        for x in 0..width {
            let face = y * width + x;
            if ((y > 0 && wall[(y - 1) * width + x]) || (y < height && wall[y * width + x]))
                && fields.aperture().v[face] != 0.0
            {
                return Err(CoupledStepError::OpenWallFace { axis: "v", face });
            }
        }
    }
    Ok(())
}

fn derived_density(inventory: &TransportInventory) -> Result<Vec<f64>, CoupledStepError> {
    let mut density = Vec::with_capacity(inventory.grid().cells());
    for cell in 0..inventory.grid().cells() {
        let value = if inventory.fixed_wall()[cell] {
            inventory.carrier_density_kg_m3()
        } else {
            (inventory.liquid_mass_kg()[cell] + inventory.carrier_mass_kg()[cell]) / CELL_VOLUME_M3
        };
        if !value.is_finite() || value <= 0.0 {
            return Err(CoupledStepError::DensityMismatch { cell });
        }
        density.push(value);
    }
    Ok(density)
}

fn validate_viscosity(
    inventory: &TransportInventory,
    dynamic_pa_s: Option<&[f64]>,
) -> Result<f64, CoupledStepError> {
    let Some(dynamic_pa_s) = dynamic_pa_s else {
        return Ok(0.0);
    };
    if dynamic_pa_s.len() != inventory.grid().cells() {
        return Err(CoupledStepError::ViscosityLength {
            expected: inventory.grid().cells(),
            actual: dynamic_pa_s.len(),
        });
    }
    let mut max_dynamic_pa_s = 0.0_f64;
    for (cell, &viscosity) in dynamic_pa_s.iter().enumerate() {
        if !viscosity.is_finite() || viscosity <= 0.0 {
            return Err(CoupledStepError::InvalidViscosity { cell });
        }
        max_dynamic_pa_s = max_dynamic_pa_s.max(viscosity);
    }
    let min_density = inventory
        .liquid_density_kg_m3()
        .min(inventory.carrier_density_kg_m3());
    let max_nu_m2_s = max_dynamic_pa_s / min_density;
    if !max_nu_m2_s.is_finite() || max_nu_m2_s <= 0.0 {
        return Err(CoupledStepError::InvalidKinematicViscosity);
    }
    Ok(max_nu_m2_s)
}

fn make_fields(
    source: &PressureFields,
    density_kg_m3: Vec<f64>,
    pressure_pa: Vec<f64>,
    velocity_m_s: FaceValues,
) -> Result<PressureFields, CoupledStepError> {
    PressureFields::new(
        source.grid(),
        source.boundaries(),
        density_kg_m3,
        pressure_pa,
        velocity_m_s,
        copy_faces(source.aperture()),
    )
    .map_err(CoupledStepError::Field)
}

fn copy_faces(faces: &FaceValues) -> FaceValues {
    FaceValues {
        u: faces.u.clone(),
        v: faces.v.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ReferenceSession;
    use particle_sim::OUTER_DT_S;

    fn grid() -> Grid {
        Grid::new(2.0, 2.0).unwrap()
    }

    fn inventory(grid: Grid, liquid_first: bool) -> TransportInventory {
        let liquid_mass_kg = (0..grid.cells())
            .map(|cell| {
                if liquid_first && cell == 0 {
                    1000.0 * CELL_VOLUME_M3
                } else {
                    0.0
                }
            })
            .collect();
        let carrier_mass_kg = (0..grid.cells())
            .map(|cell| {
                if liquid_first && cell == 0 {
                    0.0
                } else {
                    1.2 * CELL_VOLUME_M3
                }
            })
            .collect();
        TransportInventory::new(
            grid,
            1000.0,
            1.2,
            liquid_mass_kg,
            carrier_mass_kg,
            vec![0.0; grid.cells()],
            (0..grid.cells())
                .map(|cell| if liquid_first && cell == 0 { 0.0 } else { 1.0 })
                .collect(),
            vec![false; grid.cells()],
        )
        .unwrap()
    }

    fn vortex(grid: Grid, speed_m_s: f64) -> FaceValues {
        let mut velocity = FaceValues {
            u: vec![0.0; grid.u_faces()],
            v: vec![0.0; grid.v_faces()],
        };
        velocity.u[grid.u_face_index(1, 0).unwrap()] = speed_m_s;
        velocity.u[grid.u_face_index(1, 1).unwrap()] = -speed_m_s;
        velocity.v[grid.v_face_index(0, 1).unwrap()] = -speed_m_s;
        velocity.v[grid.v_face_index(1, 1).unwrap()] = speed_m_s;
        velocity
    }

    fn fields(grid: Grid, inventory: &TransportInventory, velocity: FaceValues) -> PressureFields {
        let mut aperture = FaceValues {
            u: vec![0.0; grid.u_faces()],
            v: vec![0.0; grid.v_faces()],
        };
        for y in 0..grid.height() {
            for x in 1..grid.width() {
                aperture.u[grid.u_face_index(x, y).unwrap()] = 1.0;
            }
        }
        for y in 1..grid.height() {
            for x in 0..grid.width() {
                aperture.v[grid.v_face_index(x, y).unwrap()] = 1.0;
            }
        }
        PressureFields::new(
            grid,
            [Boundary::Closed; 4],
            derived_density(inventory).unwrap(),
            vec![0.0; grid.cells()],
            velocity,
            aperture,
        )
        .unwrap()
    }

    fn config(gravity_m_s2: f64) -> CoupledStepConfig<'static> {
        CoupledStepConfig {
            gravity_m_s2,
            cell_dynamic_viscosity_pa_s: None,
            pressure: SolveConfig::default(),
        }
    }

    #[test]
    fn accepted_step_commits_time_phase_mass_and_projected_velocity() {
        let grid = grid();
        let inventory = inventory(grid, true);
        let before = inventory.totals();
        let mut session = ReferenceSession::new(grid);
        session
            .set_pressure_fields(fields(grid, &inventory, vortex(grid, 0.05)))
            .unwrap();
        session.set_transport_inventory(inventory).unwrap();
        let initial_versions = session.pressure_versions();
        session
            .project_pressure_cached(
                &vortex(grid, 0.05),
                &vec![0.0; grid.cells()],
                0.001,
                SolveConfig::default(),
            )
            .unwrap();
        assert_eq!(session.pressure_assembly_builds(), 1);
        let mut clock = SubstepClock::new(0.001, 8).unwrap();

        let result = session
            .advance_coupled_substep(&mut clock, config(0.0))
            .unwrap();
        let CoupledStepOutcome::Advanced {
            dt_s,
            momentum,
            shear,
            ..
        } = result
        else {
            panic!("expected an accepted coupled step");
        };
        assert_eq!(dt_s, 0.001);
        assert_eq!(shear, None);
        assert!(momentum.kinetic_before_j.is_finite());
        assert!(momentum.kinetic_after_j.is_finite());
        assert_eq!(
            momentum.numerical_change_j,
            momentum.kinetic_after_j - momentum.kinetic_before_j
        );
        assert!(
            momentum
                .wall_impulse_kg_m_s
                .iter()
                .all(|value| value.is_finite())
        );
        assert_eq!(clock.accepted_time_s(), 0.001);
        assert_eq!(clock.accepted_substeps(), 1);
        assert_eq!(
            session.pressure_versions().density,
            initial_versions.density + 1
        );
        assert_eq!(session.pressure_assembly_builds(), 1);
        assert_eq!(
            session
                .advance_coupled_substep(&mut clock, config(0.0))
                .unwrap(),
            CoupledStepOutcome::Complete
        );
        let after = session.transport_inventory().unwrap().totals();
        assert!(
            (after.liquid_mass_kg - before.liquid_mass_kg).abs() <= 1e-12 * before.liquid_mass_kg
        );
        assert!(
            (after.carrier_mass_kg - before.carrier_mass_kg).abs()
                <= 1e-12 * before.carrier_mass_kg
        );
        let final_fields = session.pressure_fields().unwrap();
        let divergence = divergence_per_s(final_fields, final_fields.velocity_m_s()).unwrap();
        assert!(
            divergence
                .iter()
                .all(|value| value.abs() * 0.001 <= MAX_SCALED_DIVERGENCE)
        );
        let projected_velocity = copy_faces(final_fields.velocity_m_s());
        session
            .project_pressure_cached(
                &projected_velocity,
                &vec![0.0; grid.cells()],
                0.001,
                SolveConfig::default(),
            )
            .unwrap();
        assert_eq!(session.pressure_assembly_builds(), 2);
    }

    #[test]
    fn thousand_sealed_steps_keep_phase_mass_and_finite_projected_velocity() {
        let grid = grid();
        let inventory = inventory(grid, true);
        let before = inventory.totals();
        let mut session = ReferenceSession::new(grid);
        session
            .set_pressure_fields(fields(grid, &inventory, vortex(grid, 0.05)))
            .unwrap();
        session.set_transport_inventory(inventory).unwrap();
        for tick in 0..1000 {
            let mut clock = SubstepClock::new(0.001, 8).unwrap();
            assert!(
                matches!(
                    session.advance_coupled_substep(&mut clock, config(0.0)),
                    Ok(CoupledStepOutcome::Advanced { .. })
                ),
                "coupled step failed at tick {tick}"
            );
            assert_eq!(clock.accepted_time_s(), 0.001);
            assert_eq!(clock.accepted_substeps(), 1);
        }
        let after = session.transport_inventory().unwrap().totals();
        assert!(
            (after.liquid_mass_kg - before.liquid_mass_kg).abs() <= 1e-10 * before.liquid_mass_kg
        );
        assert!(
            (after.carrier_mass_kg - before.carrier_mass_kg).abs()
                <= 1e-10 * before.carrier_mass_kg
        );
        let velocity = session.pressure_fields().unwrap().velocity_m_s();
        assert!(
            velocity
                .u
                .iter()
                .chain(&velocity.v)
                .all(|speed| speed.is_finite())
        );
    }

    #[test]
    fn pressure_nonconvergence_keeps_every_owner_clock_and_cache() {
        let grid = grid();
        let inventory = inventory(grid, false);
        let initial_inventory = inventory.clone();
        let initial_fields = fields(grid, &inventory, vortex(grid, 0.0));
        let mut session = ReferenceSession::new(grid);
        session.set_pressure_fields(initial_fields).unwrap();
        session.set_transport_inventory(inventory).unwrap();
        let zero_faces = vortex(grid, 0.0);
        session
            .project_pressure_cached(
                &zero_faces,
                &vec![0.0; grid.cells()],
                0.001,
                SolveConfig::default(),
            )
            .unwrap();
        let versions = session.pressure_versions();
        let builds = session.pressure_assembly_builds();
        let mut clock = SubstepClock::new(0.001, 8).unwrap();
        let mut fail_config = config(9.81);
        fail_config.pressure.max_iterations = 0;

        assert!(matches!(
            session.advance_coupled_substep(&mut clock, fail_config),
            Err(CoupledStepError::Pressure(
                PressureSolveError::DidNotConverge { .. }
            ))
        ));
        assert_eq!(
            session.pressure_fields().unwrap(),
            &fields(grid, &initial_inventory, vortex(grid, 0.0))
        );
        assert_eq!(session.transport_inventory(), Some(&initial_inventory));
        assert_eq!(session.pressure_versions(), versions);
        assert_eq!(session.pressure_assembly_builds(), builds);
        assert_eq!(clock.accepted_time_s(), 0.0);
        assert_eq!(clock.accepted_substeps(), 0);
        session
            .project_pressure_cached(
                &zero_faces,
                &vec![0.0; grid.cells()],
                0.001,
                SolveConfig::default(),
            )
            .unwrap();
        assert_eq!(session.pressure_assembly_builds(), builds);
    }

    #[test]
    fn owner_mismatch_rejects_before_clock_or_cache_changes() {
        let grid = grid();
        let inventory = inventory(grid, false);
        let mut session = ReferenceSession::new(grid);
        let aperture = copy_faces(fields(grid, &inventory, vortex(grid, 0.0)).aperture());
        let wrong_density_fields = PressureFields::new(
            grid,
            [Boundary::Closed; 4],
            vec![1.3; grid.cells()],
            vec![0.0; grid.cells()],
            vortex(grid, 0.0),
            aperture,
        )
        .unwrap();
        session.set_pressure_fields(wrong_density_fields).unwrap();
        session.set_transport_inventory(inventory.clone()).unwrap();
        let versions = session.pressure_versions();
        let mut clock = SubstepClock::new(0.001, 8).unwrap();
        assert_eq!(
            session.advance_coupled_substep(&mut clock, config(0.0)),
            Err(CoupledStepError::DensityMismatch { cell: 0 })
        );
        assert_eq!(session.transport_inventory(), Some(&inventory));
        assert_eq!(session.pressure_versions(), versions);
        assert_eq!(session.pressure_assembly_builds(), 0);
        assert_eq!(clock.accepted_time_s(), 0.0);
    }

    #[test]
    fn open_face_incident_to_fixed_wall_rejects_without_mutation() {
        let grid = grid();
        let mut wall = vec![false; grid.cells()];
        wall[0] = true;
        let inventory = TransportInventory::new(
            grid,
            1000.0,
            1.2,
            vec![0.0; grid.cells()],
            vec![
                0.0,
                1.2 * CELL_VOLUME_M3,
                1.2 * CELL_VOLUME_M3,
                1.2 * CELL_VOLUME_M3,
            ],
            vec![0.0; grid.cells()],
            vec![0.0; grid.cells()],
            wall,
        )
        .unwrap();
        let mut session = ReferenceSession::new(grid);
        session
            .set_pressure_fields(fields(grid, &inventory, vortex(grid, 0.0)))
            .unwrap();
        session.set_transport_inventory(inventory.clone()).unwrap();
        let mut clock = SubstepClock::new(0.001, 8).unwrap();
        let versions = session.pressure_versions();
        assert_eq!(
            session.advance_coupled_substep(&mut clock, config(0.0)),
            Err(CoupledStepError::OpenWallFace { axis: "u", face: 1 })
        );
        assert_eq!(session.transport_inventory(), Some(&inventory));
        assert_eq!(session.pressure_versions(), versions);
        assert_eq!(clock.accepted_time_s(), 0.0);
    }

    #[test]
    fn near_limit_source_closure_rejects_large_step_without_accepting_time() {
        let grid = Grid::new(2.0, 1.0).unwrap();
        let carrier_density = 1.2;
        let relative_offset = 0.9e-12;
        let carrier_mass_kg = vec![
            carrier_density * CELL_VOLUME_M3 * (1.0 - relative_offset),
            carrier_density * CELL_VOLUME_M3 * (1.0 + relative_offset),
        ];
        let inventory = TransportInventory::new(
            grid,
            1000.0,
            carrier_density,
            vec![0.0; grid.cells()],
            carrier_mass_kg.clone(),
            vec![0.0; grid.cells()],
            vec![0.0; grid.cells()],
            vec![false; grid.cells()],
        )
        .unwrap();
        let source_speed_m_s = 2.0e-12;
        let make_fields = || {
            PressureFields::new(
                grid,
                [Boundary::Closed; 4],
                carrier_mass_kg
                    .iter()
                    .map(|mass| mass / CELL_VOLUME_M3)
                    .collect(),
                vec![0.0; grid.cells()],
                FaceValues {
                    u: vec![0.0, source_speed_m_s, 0.0],
                    v: vec![0.0; grid.v_faces()],
                },
                FaceValues {
                    u: vec![0.0, 1.0, 0.0],
                    v: vec![0.0; grid.v_faces()],
                },
            )
            .unwrap()
        };
        let mut session = ReferenceSession::new(grid);
        session.set_pressure_fields(make_fields()).unwrap();
        session.set_transport_inventory(inventory.clone()).unwrap();
        let versions = session.pressure_versions();
        let mut clock = SubstepClock::new(0.001, 8).unwrap();
        let scaled_source_divergence = 0.001 * source_speed_m_s / CELL_WIDTH_M;
        assert!(scaled_source_divergence < MAX_SCALED_DIVERGENCE);

        assert!(matches!(
            session.advance_coupled_substep(&mut clock, config(0.0)),
            Err(CoupledStepError::Transport(TransportError::InvalidCell {
                reason: "incompressible phase volume closure",
                ..
            }))
        ));
        assert_eq!(session.pressure_fields().unwrap(), &make_fields());
        assert_eq!(session.transport_inventory(), Some(&inventory));
        assert_eq!(session.pressure_versions(), versions);
        assert_eq!(session.pressure_assembly_builds(), 0);
        assert_eq!(clock.accepted_time_s(), 0.0);
        assert_eq!(clock.accepted_substeps(), 0);

        let mut shorter_clock = SubstepClock::new(0.0002, 8).unwrap();
        assert!(matches!(
            session.advance_coupled_substep(&mut shorter_clock, config(0.0)),
            Ok(CoupledStepOutcome::Advanced { .. })
        ));
        assert_eq!(shorter_clock.accepted_time_s(), 0.0002);
    }

    #[test]
    fn optional_viscosity_advances_and_invalid_coefficient_preserves_state() {
        let grid = grid();
        let inventory = inventory(grid, false);
        let mut session = ReferenceSession::new(grid);
        session
            .set_pressure_fields(fields(grid, &inventory, vortex(grid, 0.05)))
            .unwrap();
        session.set_transport_inventory(inventory.clone()).unwrap();
        let mut clock = SubstepClock::new(0.001, 8).unwrap();
        let viscosity = [0.001; 4];
        let config = CoupledStepConfig {
            gravity_m_s2: 0.0,
            cell_dynamic_viscosity_pa_s: Some(&viscosity),
            pressure: SolveConfig::default(),
        };
        let result = session.advance_coupled_substep(&mut clock, config).unwrap();
        let CoupledStepOutcome::Advanced {
            momentum,
            shear: Some(shear),
            ..
        } = result
        else {
            panic!("expected accepted momentum and shear diagnostics");
        };
        assert_eq!(
            momentum.numerical_change_j,
            momentum.kinetic_after_j - momentum.kinetic_before_j
        );
        assert_eq!(
            shear.kinetic_loss_j,
            shear.kinetic_before_j - shear.kinetic_after_j
        );
        assert!(shear.initial_stress_work_j.is_finite());
        assert_eq!(clock.accepted_time_s(), 0.001);
        let after = session.transport_inventory().unwrap().clone();
        let versions = session.pressure_versions();
        let builds = session.pressure_assembly_builds();
        let invalid = [0.001, 0.001, f64::NAN, 0.001];
        let invalid_config = CoupledStepConfig {
            gravity_m_s2: 0.0,
            cell_dynamic_viscosity_pa_s: Some(&invalid),
            pressure: SolveConfig::default(),
        };
        let mut next_clock = SubstepClock::new(0.001, 8).unwrap();
        assert_eq!(
            session.advance_coupled_substep(&mut next_clock, invalid_config),
            Err(CoupledStepError::InvalidViscosity { cell: 2 })
        );
        assert_eq!(session.transport_inventory(), Some(&after));
        assert_eq!(session.pressure_versions(), versions);
        assert_eq!(session.pressure_assembly_builds(), builds);
        assert_eq!(clock.accepted_time_s(), 0.001);
        assert_eq!(next_clock.accepted_time_s(), 0.0);
    }

    #[test]
    fn three_meters_per_second_uses_ten_clock_steps_and_pauses_at_budget() {
        let grid = grid();
        let velocity = vortex(grid, 3.0);
        let mut clock = SubstepClock::new(OUTER_DT_S, 10).unwrap();
        for accepted in 1..=10 {
            let SubstepSelection::Candidate(candidate) =
                clock.select(grid, &velocity, CELL_WIDTH_M, 0.0).unwrap()
            else {
                panic!("expected candidate {accepted}");
            };
            assert!((candidate.dt_s() - CELL_WIDTH_M / 6.0).abs() < 1e-15);
            clock.accept(candidate).unwrap();
            assert_eq!(clock.accepted_substeps(), accepted);
        }
        assert_eq!(
            clock.select(grid, &velocity, CELL_WIDTH_M, 0.0).unwrap(),
            SubstepSelection::Complete
        );

        let inventory = inventory(grid, false);
        let mut session = ReferenceSession::new(grid);
        session
            .set_pressure_fields(fields(grid, &inventory, velocity))
            .unwrap();
        session.set_transport_inventory(inventory).unwrap();
        let mut limited = SubstepClock::new(OUTER_DT_S, 1).unwrap();
        assert!(matches!(
            session.advance_coupled_substep(&mut limited, config(0.0)),
            Ok(CoupledStepOutcome::Advanced { .. })
        ));
        let owners = (
            session.pressure_versions(),
            session.pressure_assembly_builds(),
            session.transport_inventory().unwrap().clone(),
        );
        assert!(
            matches!(session.advance_coupled_substep(&mut limited, config(0.0)), Ok(CoupledStepOutcome::Paused { reason: PauseReason::SubstepBudgetExhausted, remaining_s }) if remaining_s > 0.0)
        );
        assert_eq!(session.pressure_versions(), owners.0);
        assert_eq!(session.pressure_assembly_builds(), owners.1);
        assert_eq!(session.transport_inventory(), Some(&owners.2));
    }
}
