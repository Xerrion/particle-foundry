//! Closed CPU scene checks from engine/fixtures/engine-reference-v1.json.
//!
//! The JSON fixes scene names, scale, duration, and numerical tolerances. These
//! tests exercise the owned coupled session at equal accepted physical time.

use particle_sim::{Grid, OUTER_DT_S, contracts::Boundary};
use particle_sim_cpu::{
    PressureSessionError, ReferenceSession,
    assembly::{PressureAssembly, PressureAssemblyError},
    coupled::{CoupledStepConfig, CoupledStepOutcome},
    fluid::{FaceValues, PressureFields},
    solver::{PressureSolveError, SolveConfig},
    substep::SubstepClock,
    transport::{TransportInventory, TransportTotals},
};

const LIQUID_DENSITY_KG_M3: f64 = 1000.0;
const CARRIER_DENSITY_KG_M3: f64 = 1.2;
const GRAVITY_M_S2: f64 = 9.80665;
const MASS_RELATIVE_TOL: f64 = 1e-10;
const MASS_ABSOLUTE_KG_TOL: f64 = 1e-14;
const MARKER_ABSOLUTE_TOL: f64 = 1e-14;
const FRACTION_ABSOLUTE_TOL: f64 = 1e-12;
const SCALED_DIVERGENCE_TOL: f64 = 1e-8;
const REST_VELOCITY_M_S_TOL: f64 = 1e-8;

fn grid_16() -> Grid {
    Grid::new(16.0, 16.0).unwrap()
}

fn zero_faces(grid: Grid) -> FaceValues {
    FaceValues {
        u: vec![0.0; grid.u_faces()],
        v: vec![0.0; grid.v_faces()],
    }
}

fn closed_aperture(grid: Grid, wall: &[bool]) -> FaceValues {
    let width = grid.width() as usize;
    let height = grid.height() as usize;
    let mut aperture = zero_faces(grid);
    for y in 0..height {
        for x in 1..width {
            if !wall[y * width + x - 1] && !wall[y * width + x] {
                aperture.u[y * (width + 1) + x] = 1.0;
            }
        }
    }
    for y in 1..height {
        for x in 0..width {
            if !wall[(y - 1) * width + x] && !wall[y * width + x] {
                aperture.v[y * width + x] = 1.0;
            }
        }
    }
    aperture
}

fn inventory(grid: Grid, wall: Vec<bool>, alpha: Vec<f64>) -> TransportInventory {
    assert_eq!(wall.len(), grid.cells());
    assert_eq!(alpha.len(), grid.cells());
    let volume = grid.cell_volume_m3();
    let liquid_mass = alpha
        .iter()
        .zip(&wall)
        .map(|(&fraction, &solid)| {
            if solid {
                0.0
            } else {
                fraction * LIQUID_DENSITY_KG_M3 * volume
            }
        })
        .collect();
    let carrier_mass = alpha
        .iter()
        .zip(&wall)
        .map(|(&fraction, &solid)| {
            if solid {
                0.0
            } else {
                (1.0 - fraction) * CARRIER_DENSITY_KG_M3 * volume
            }
        })
        .collect();
    let carrier_marker = alpha
        .iter()
        .zip(&wall)
        .map(|(&fraction, &solid)| if solid { 0.0 } else { 1.0 - fraction })
        .collect();
    let liquid_marker = alpha
        .iter()
        .zip(&wall)
        .map(|(&fraction, &solid)| if solid { 0.0 } else { fraction })
        .collect();
    TransportInventory::new(
        grid,
        LIQUID_DENSITY_KG_M3,
        CARRIER_DENSITY_KG_M3,
        liquid_mass,
        carrier_mass,
        liquid_marker,
        carrier_marker,
        wall,
    )
    .unwrap()
}

fn make_session(inventory: TransportInventory, velocity: FaceValues) -> ReferenceSession {
    let grid = inventory.grid();
    let density = (0..grid.cells())
        .map(|cell| {
            if inventory.fixed_wall()[cell] {
                CARRIER_DENSITY_KG_M3
            } else {
                (inventory.liquid_mass_kg()[cell] + inventory.carrier_mass_kg()[cell])
                    / grid.cell_volume_m3()
            }
        })
        .collect();
    let aperture = closed_aperture(grid, inventory.fixed_wall());
    let fields = PressureFields::new(
        grid,
        [Boundary::Closed; 4],
        density,
        vec![0.0; grid.cells()],
        velocity,
        aperture,
    )
    .unwrap();
    let mut session = ReferenceSession::new(grid);
    session.set_pressure_fields(fields).unwrap();
    session.set_transport_inventory(inventory).unwrap();
    session
}

fn config(gravity_m_s2: f64) -> CoupledStepConfig<'static> {
    CoupledStepConfig {
        gravity_m_s2,
        cell_dynamic_viscosity_pa_s: None,
        pressure: SolveConfig::default(),
    }
}

fn advance_outer_tick(session: &mut ReferenceSession, gravity_m_s2: f64, tick: usize) -> f64 {
    let mut clock = SubstepClock::new(OUTER_DT_S, 64).unwrap();
    loop {
        match session.advance_coupled_substep(&mut clock, config(gravity_m_s2)) {
            Ok(CoupledStepOutcome::Advanced { pressure, .. }) => {
                assert!(
                    pressure.scaled_divergence <= SCALED_DIVERGENCE_TOL,
                    "tick {tick}: scaled divergence {}",
                    pressure.scaled_divergence
                );
            }
            Ok(CoupledStepOutcome::Complete) => break,
            Ok(CoupledStepOutcome::Paused { remaining_s, .. }) => {
                panic!("tick {tick}: budget exhausted with {remaining_s} s remaining");
            }
            Err(error) => panic!(
                "tick {tick}: coupled substep rejected: {error:?}; source max speed {} m/s",
                max_speed(session)
            ),
        }
    }
    assert!(
        (clock.accepted_time_s() - OUTER_DT_S).abs() <= 1e-14,
        "tick {tick}: accepted {} s",
        clock.accepted_time_s()
    );
    clock.accepted_time_s()
}

fn assert_mass_conserved(before: TransportTotals, after: TransportTotals) {
    for (name, first, last) in [
        ("liquid mass", before.liquid_mass_kg, after.liquid_mass_kg),
        (
            "carrier mass",
            before.carrier_mass_kg,
            after.carrier_mass_kg,
        ),
    ] {
        let allowed = MASS_ABSOLUTE_KG_TOL.max(MASS_RELATIVE_TOL * first.abs());
        assert!(
            (last - first).abs() <= allowed,
            "{name} changed from {first} to {last}, allowed {allowed}"
        );
    }
    for (name, first, last) in [
        ("liquid marker", before.liquid_marker, after.liquid_marker),
        (
            "carrier marker",
            before.carrier_marker,
            after.carrier_marker,
        ),
    ] {
        let allowed = MARKER_ABSOLUTE_TOL + MASS_RELATIVE_TOL * first.abs();
        assert!(
            (last - first).abs() <= allowed,
            "{name} changed from {first} to {last}, allowed {allowed}"
        );
    }
}

fn assert_alpha_bounds(inventory: &TransportInventory) {
    for cell in 0..inventory.grid().cells() {
        if inventory.fixed_wall()[cell] {
            assert_eq!(inventory.alpha(cell), None);
        } else {
            let fraction = inventory.alpha(cell).unwrap();
            assert!(
                (-FRACTION_ABSOLUTE_TOL..=1.0 + FRACTION_ABSOLUTE_TOL).contains(&fraction),
                "cell {cell}: alpha {fraction}"
            );
        }
    }
}

fn max_speed(session: &ReferenceSession) -> f64 {
    let velocity = session.pressure_fields().unwrap().velocity_m_s();
    velocity
        .u
        .iter()
        .chain(&velocity.v)
        .map(|speed| speed.abs())
        .fold(0.0, f64::max)
}

fn region_totals(inventory: &TransportInventory, x_start: usize, x_end: usize) -> TransportTotals {
    let width = inventory.grid().width() as usize;
    let mut totals = TransportTotals {
        liquid_mass_kg: 0.0,
        carrier_mass_kg: 0.0,
        liquid_marker: 0.0,
        carrier_marker: 0.0,
    };
    for cell in 0..inventory.grid().cells() {
        let x = cell % width;
        if (x_start..x_end).contains(&x) {
            totals.liquid_mass_kg += inventory.liquid_mass_kg()[cell];
            totals.carrier_mass_kg += inventory.carrier_mass_kg()[cell];
            totals.liquid_marker += inventory.liquid_marker()[cell];
            totals.carrier_marker += inventory.carrier_marker()[cell];
        }
    }
    totals
}

fn assert_impermeable_faces_zero(session: &ReferenceSession) {
    let fields = session.pressure_fields().unwrap();
    for (axis, aperture, velocity) in [
        ("u", &fields.aperture().u, &fields.velocity_m_s().u),
        ("v", &fields.aperture().v, &fields.velocity_m_s().v),
    ] {
        for (face, (&opening, &speed)) in aperture.iter().zip(velocity).enumerate() {
            if opening == 0.0 {
                assert_eq!(speed, 0.0, "{axis} face {face} crossed a sealed wall");
            }
        }
    }
}

fn divider(grid: Grid, opening_y: Option<usize>) -> Vec<bool> {
    let width = grid.width() as usize;
    let height = grid.height() as usize;
    assert_eq!(width, 16);
    let mut wall = vec![false; grid.cells()];
    for y in 0..height {
        if Some(y) != opening_y {
            wall[y * width + 8] = true;
        }
    }
    wall
}

fn assert_face_parity(actual: &FaceValues, expected: &FaceValues, allowed: f64) {
    for (axis, a, b) in [("u", &actual.u, &expected.u), ("v", &actual.v, &expected.v)] {
        for (face, (&value, &reference)) in a.iter().zip(b).enumerate() {
            assert!(
                (value - reference).abs() <= allowed,
                "{axis} face {face}: {value} differs from {reference}"
            );
        }
    }
}

#[test]
fn fluid_rest_1000_outer_ticks_remains_stationary_and_conservative() {
    let grid = grid_16();
    let mut session = make_session(
        inventory(grid, vec![false; grid.cells()], vec![0.0; grid.cells()]),
        zero_faces(grid),
    );
    let first = session.transport_inventory().unwrap().totals();
    let mut accepted_time_s = 0.0;
    for tick in 0..1000 {
        accepted_time_s += advance_outer_tick(&mut session, 0.0, tick);
    }
    assert!((accepted_time_s - 1000.0 * OUTER_DT_S).abs() <= 1e-10);
    let last = session.transport_inventory().unwrap();
    assert_mass_conserved(first, last.totals());
    assert_alpha_bounds(last);
    assert!(max_speed(&session) <= REST_VELOCITY_M_S_TOL);
}

#[test]
fn closed_mixed_vortex_1000_outer_ticks_moves_phases_and_markers_conservatively() {
    let grid = grid_16();
    let mut alpha = vec![0.5; grid.cells()];
    let upper_left = grid.cell_index(4, 7).unwrap();
    let upper_right = grid.cell_index(5, 7).unwrap();
    alpha[upper_left] = 0.75;
    alpha[upper_right] = 0.25;
    alpha[grid.cell_index(4, 8).unwrap()] = 0.25;
    alpha[grid.cell_index(5, 8).unwrap()] = 0.75;
    let base = inventory(grid, vec![false; grid.cells()], alpha);
    let mut liquid_marker = vec![0.0; grid.cells()];
    let mut carrier_marker = vec![0.0; grid.cells()];
    liquid_marker[upper_right] = 0.25;
    carrier_marker[upper_left] = 0.25;
    let inventory = TransportInventory::new(
        grid,
        LIQUID_DENSITY_KG_M3,
        CARRIER_DENSITY_KG_M3,
        base.liquid_mass_kg().to_vec(),
        base.carrier_mass_kg().to_vec(),
        liquid_marker,
        carrier_marker,
        vec![false; grid.cells()],
    )
    .unwrap();
    let mut velocity = zero_faces(grid);
    velocity.u[grid.u_face_index(5, 7).unwrap()] = 0.03;
    velocity.u[grid.u_face_index(5, 8).unwrap()] = -0.03;
    velocity.v[grid.v_face_index(4, 8).unwrap()] = -0.03;
    velocity.v[grid.v_face_index(5, 8).unwrap()] = 0.03;
    let mut session = make_session(inventory, velocity);
    let first = session.transport_inventory().unwrap().totals();
    let original_liquid_fraction = session
        .transport_inventory()
        .unwrap()
        .alpha(upper_left)
        .unwrap();
    let original_liquid_marker =
        session.transport_inventory().unwrap().liquid_marker()[upper_right];
    let original_carrier_marker =
        session.transport_inventory().unwrap().carrier_marker()[upper_left];

    let mut accepted_time_s = 0.0;
    for tick in 0..1000 {
        accepted_time_s += advance_outer_tick(&mut session, 0.0, tick);
        if tick == 0 {
            let moved = session.transport_inventory().unwrap();
            assert!(
                (moved.alpha(upper_left).unwrap() - original_liquid_fraction).abs() > 1e-8,
                "liquid fraction did not move on the first accepted outer tick"
            );
            assert!(
                moved.liquid_marker()[upper_right] < original_liquid_marker - 1e-8,
                "liquid-associated marker did not move on the first accepted outer tick"
            );
            assert!(
                moved.carrier_marker()[upper_left] < original_carrier_marker - 1e-8,
                "carrier-associated marker did not move on the first accepted outer tick"
            );
        }
        assert!(
            (accepted_time_s - (tick + 1) as f64 * OUTER_DT_S).abs() <= 1e-10,
            "tick {tick}: outer interval was only partly accepted"
        );
    }
    let last = session.transport_inventory().unwrap();
    assert_mass_conserved(first, last.totals());
    assert_alpha_bounds(last);
    assert_impermeable_faces_zero(&session);
}

#[test]
fn fluid_hydrostatic_1000_outer_ticks_balances_gravity_and_pressure() {
    let grid = grid_16();
    let alpha: Vec<f64> = (0..grid.cells())
        .map(|cell| if cell / 16 >= 8 { 1.0 } else { 0.0 })
        .collect();
    let mut session = make_session(
        inventory(grid, vec![false; grid.cells()], alpha.clone()),
        zero_faces(grid),
    );
    let first = session.transport_inventory().unwrap().totals();
    let mut accepted_time_s = 0.0;
    for tick in 0..1000 {
        accepted_time_s += advance_outer_tick(&mut session, GRAVITY_M_S2, tick);
    }
    assert!((accepted_time_s - 1000.0 * OUTER_DT_S).abs() <= 1e-10);
    let last = session.transport_inventory().unwrap();
    assert_mass_conserved(first, last.totals());
    assert_alpha_bounds(last);
    assert!(max_speed(&session) <= REST_VELOCITY_M_S_TOL);
    for (cell, expected) in alpha.iter().enumerate() {
        assert!(
            (last.alpha(cell).unwrap() - expected).abs() <= FRACTION_ABSOLUTE_TOL,
            "cell {cell}: hydrostatic liquid fraction moved"
        );
    }
}

#[test]
fn fluid_disconnected_keeps_pressure_nullspaces_and_rejects_source_without_commit() {
    let grid = grid_16();
    let wall = divider(grid, None);
    let mut session = make_session(
        inventory(grid, wall, vec![0.0; grid.cells()]),
        zero_faces(grid),
    );
    let first = session.transport_inventory().unwrap().totals();
    let fields = session.pressure_fields().unwrap();
    let assembly = PressureAssembly::new(fields).unwrap();
    let left = grid.cell_index(0, 0).unwrap();
    let right = grid.cell_index(15, 0).unwrap();
    let wall_cell = grid.cell_index(8, 0).unwrap();
    assert_ne!(
        assembly.component_of_cell()[left],
        assembly.component_of_cell()[right]
    );
    assert_eq!(assembly.diagonal_m_kg()[wall_cell], 0.0);
    assert_eq!(
        assembly.component_sizes()[assembly.component_of_cell()[left]],
        128
    );
    assert_eq!(
        assembly.component_sizes()[assembly.component_of_cell()[right]],
        112
    );

    let accepted_time_s = advance_outer_tick(&mut session, 0.0, 0);
    assert!((accepted_time_s - OUTER_DT_S).abs() <= 1e-14);
    assert_mass_conserved(first, session.transport_inventory().unwrap().totals());
    assert_eq!(max_speed(&session), 0.0);

    let fields_before = session.pressure_fields().unwrap();
    let pressure_before = fields_before.correction_pressure_pa().to_vec();
    let velocity_before = FaceValues {
        u: fields_before.velocity_m_s().u.clone(),
        v: fields_before.velocity_m_s().v.clone(),
    };
    let inventory_before = session.transport_inventory().unwrap().clone();
    let versions_before = session.pressure_versions();
    let builds_before = session.pressure_assembly_builds();
    let mut incompatible_source = vec![0.0; grid.cells()];
    incompatible_source[left] = 1.0;
    assert!(matches!(
        session.project_pressure_forced_rebuild(
            &zero_faces(grid),
            &incompatible_source,
            OUTER_DT_S,
            SolveConfig::default(),
        ),
        Err(PressureSessionError::Solve(PressureSolveError::Assembly(
            PressureAssemblyError::IncompatibleRhs { .. }
        )))
    ));
    assert_eq!(
        session.pressure_fields().unwrap().correction_pressure_pa(),
        pressure_before
    );
    assert_face_parity(
        session.pressure_fields().unwrap().velocity_m_s(),
        &velocity_before,
        0.0,
    );
    assert_eq!(session.transport_inventory(), Some(&inventory_before));
    assert_eq!(session.pressure_versions(), versions_before);
    assert_eq!(session.pressure_assembly_builds(), builds_before);
}

#[test]
fn fluid_channel_60_ticks_conserves_phase_and_marker_behind_walls() {
    let grid = grid_16();
    let wall = divider(grid, Some(8));
    let half_fraction = vec![0.5; grid.cells()];
    let volume = grid.cell_volume_m3();
    let phase_mass = 0.5 * LIQUID_DENSITY_KG_M3 * volume;
    let base = inventory(grid, wall.clone(), half_fraction);
    let mut liquid_marker = vec![0.0; grid.cells()];
    let marked_cell = grid.cell_index(7, 8).unwrap();
    liquid_marker[marked_cell] = phase_mass;
    let inventory = TransportInventory::new(
        grid,
        LIQUID_DENSITY_KG_M3,
        CARRIER_DENSITY_KG_M3,
        base.liquid_mass_kg().to_vec(),
        base.carrier_mass_kg().to_vec(),
        liquid_marker,
        vec![0.0; grid.cells()],
        wall.clone(),
    )
    .unwrap();
    let mut velocity = zero_faces(grid);
    velocity.u[grid.u_face_index(7, 7).unwrap()] = 0.03;
    velocity.u[grid.u_face_index(7, 8).unwrap()] = -0.03;
    velocity.v[grid.v_face_index(6, 8).unwrap()] = -0.03;
    velocity.v[grid.v_face_index(7, 8).unwrap()] = 0.03;
    let mut session = make_session(inventory, velocity);
    let first = session.transport_inventory().unwrap().totals();
    let first_left = region_totals(session.transport_inventory().unwrap(), 0, 8);
    let first_throat = region_totals(session.transport_inventory().unwrap(), 8, 9);
    let first_right = region_totals(session.transport_inventory().unwrap(), 9, 16);
    let opening = grid.cell_index(8, 8).unwrap();
    assert!(!wall[opening]);
    let assembly = PressureAssembly::new(session.pressure_fields().unwrap()).unwrap();
    assert_eq!(
        assembly.component_of_cell()[grid.cell_index(0, 0).unwrap()],
        assembly.component_of_cell()[grid.cell_index(15, 0).unwrap()]
    );
    let mut accepted_time_s = 0.0;
    for tick in 0..60 {
        let mut clock = SubstepClock::new(OUTER_DT_S, 64).unwrap();
        loop {
            match session.advance_coupled_substep(&mut clock, config(0.0)) {
                Ok(CoupledStepOutcome::Advanced {
                    dt_s,
                    pressure,
                    volume_closure,
                    ..
                }) => {
                    assert!(pressure.scaled_divergence <= SCALED_DIVERGENCE_TOL);
                    assert!(
                        volume_closure.scaled_divergence_after <= SCALED_DIVERGENCE_TOL,
                        "tick {tick}: accepted volume closure exceeded the frozen tolerance"
                    );
                    let throat_velocity = &session.pressure_fields().unwrap().velocity_m_s().u;
                    // A sealed chamber with one opening cannot sustain net
                    // transfer. Bound each throat flux by measured closure error.
                    for (x, chamber_cells) in [(8, 128.0), (9, 112.0)] {
                        let face = grid.u_face_index(x, 8).unwrap();
                        let throat_cfl = throat_velocity[face].abs() * dt_s / grid.cell_width_m();
                        let residual_bound = chamber_cells * volume_closure.scaled_divergence_after
                            + 64.0 * f64::EPSILON;
                        assert!(
                            throat_cfl <= residual_bound,
                            "tick {tick}: throat face x={x} has CFL {throat_cfl}, residual bound {residual_bound}"
                        );
                    }
                    assert_impermeable_faces_zero(&session);
                }
                Ok(CoupledStepOutcome::Complete) => break,
                Ok(CoupledStepOutcome::Paused { remaining_s, .. }) => {
                    panic!("tick {tick}: budget exhausted with {remaining_s} s remaining");
                }
                Err(error) => panic!("tick {tick}: coupled substep rejected: {error:?}"),
            }
        }
        assert!((clock.accepted_time_s() - OUTER_DT_S).abs() <= 1e-14);
        accepted_time_s += clock.accepted_time_s();
        let current = session.transport_inventory().unwrap();
        assert_mass_conserved(first_left, region_totals(current, 0, 8));
        assert_mass_conserved(first_throat, region_totals(current, 8, 9));
        assert_mass_conserved(first_right, region_totals(current, 9, 16));
    }
    assert!((accepted_time_s - 60.0 * OUTER_DT_S).abs() <= 1e-13);
    let last = session.transport_inventory().unwrap();
    assert_mass_conserved(first, last.totals());
    assert_mass_conserved(first_left, region_totals(last, 0, 8));
    assert_mass_conserved(first_throat, region_totals(last, 8, 9));
    assert_mass_conserved(first_right, region_totals(last, 9, 16));
    assert_alpha_bounds(last);
    assert!(
        last.liquid_marker()[marked_cell] < first.liquid_marker * (1.0 - 1e-8),
        "marker did not move during the channel fixture"
    );
    for (cell, &solid) in wall.iter().enumerate() {
        if solid {
            assert_eq!(last.liquid_mass_kg()[cell], 0.0);
            assert_eq!(last.carrier_mass_kg()[cell], 0.0);
            assert_eq!(last.liquid_marker()[cell], 0.0);
            assert_eq!(last.carrier_marker()[cell], 0.0);
        }
    }
}

#[test]
fn fluid_wall_edit_rebuilds_topology_and_accounts_for_opened_cell() {
    let grid = grid_16();
    let closed_wall = divider(grid, None);
    let initial_inventory = inventory(grid, closed_wall, vec![0.0; grid.cells()]);
    let mut session = make_session(initial_inventory, zero_faces(grid));
    let first = session.transport_inventory().unwrap().totals();
    let zero_source = vec![0.0; grid.cells()];
    session
        .project_pressure_cached(
            &zero_faces(grid),
            &zero_source,
            OUTER_DT_S,
            SolveConfig::default(),
        )
        .unwrap();
    assert_eq!(session.pressure_assembly_builds(), 1);
    advance_outer_tick(&mut session, 0.0, 0);

    let opened_wall = divider(grid, Some(8));
    let opened_inventory = inventory(grid, opened_wall, vec![0.0; grid.cells()]);
    let opened_totals = opened_inventory.totals();
    let explicit_carrier_addition_kg = CARRIER_DENSITY_KG_M3 * grid.cell_volume_m3();
    assert!(
        (opened_totals.carrier_mass_kg - first.carrier_mass_kg - explicit_carrier_addition_kg)
            .abs()
            <= MASS_ABSOLUTE_KG_TOL
    );
    let previous_versions = session.pressure_versions();
    let opened_fields = {
        let opened_session = make_session(opened_inventory.clone(), zero_faces(grid));
        let fields = opened_session.pressure_fields().unwrap();
        PressureFields::new(
            grid,
            fields.boundaries(),
            fields.density_kg_m3().to_vec(),
            fields.correction_pressure_pa().to_vec(),
            zero_faces(grid),
            FaceValues {
                u: fields.aperture().u.clone(),
                v: fields.aperture().v.clone(),
            },
        )
        .unwrap()
    };
    session.set_pressure_fields(opened_fields).unwrap();
    session.set_transport_inventory(opened_inventory).unwrap();
    assert_eq!(
        session.pressure_versions().aperture,
        previous_versions.aperture + 1
    );

    let mut predictor = zero_faces(grid);
    predictor.u[grid.u_face_index(8, 8).unwrap()] = 0.01;
    predictor.u[grid.u_face_index(9, 8).unwrap()] = -0.01;
    let cached = session
        .project_pressure_cached(&predictor, &zero_source, OUTER_DT_S, SolveConfig::default())
        .unwrap();
    let forced = session
        .project_pressure_forced_rebuild(
            &predictor,
            &zero_source,
            OUTER_DT_S,
            SolveConfig::default(),
        )
        .unwrap();
    assert_eq!(session.pressure_assembly_builds(), 2);
    assert_eq!(cached, forced);
    advance_outer_tick(&mut session, 0.0, 1);
    assert_mass_conserved(
        opened_totals,
        session.transport_inventory().unwrap().totals(),
    );
    let after = session.transport_inventory().unwrap().totals();
    assert!(
        (after.carrier_mass_kg - first.carrier_mass_kg - explicit_carrier_addition_kg).abs()
            <= MASS_ABSOLUTE_KG_TOL
    );
}
