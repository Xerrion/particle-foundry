//! FLUID-DAM acceptance probe at equal physical size and accepted time.
//!
//! The initial water column comes from engine/fixtures/engine-reference-v1.json.
//! This test fixes its own observable definitions before running the solver.

use particle_sim::{Grid, OUTER_DT_S, contracts::Boundary};
use particle_sim_cpu::{
    ReferenceSession,
    coupled::{CoupledStepConfig, CoupledStepError, CoupledStepOutcome},
    fluid::{FaceValues, PressureFields},
    solver::SolveConfig,
    substep::SubstepClock,
    transport::{TransportError, TransportInventory},
};

const GRAVITY_M_S2: f64 = 9.80665;
const LIQUID_DENSITY_KG_M3: f64 = 1000.0;
const CARRIER_DENSITY_KG_M3: f64 = 1.2;
const OUTER_TICKS: usize = 12;
const TARGET_TIME_S: f64 = 0.2;
// The 99th mass percentile tracks the leading liquid without a one-cell
// occupancy threshold that can saturate at the far wall.
const FRONT_MASS_QUANTILE: f64 = 0.99;
const REFINEMENT_GAP_RATIO: f64 = 0.85;
const FRONT_RESOLUTION_MEDIUM_CELLS: f64 = 0.5;
const CENTER_RESOLUTION_MEDIUM_CELLS: f64 = 0.25;
const MIRROR_ALPHA_MEAN_TOL: f64 = 1e-8;
const MIRROR_OBSERVABLE_TOL_M: f64 = 1e-8;
const MASS_REL_TOL: f64 = 1e-10;
const MASS_ABS_TOL_KG: f64 = 1e-14;
const FRACTION_ABS_TOL: f64 = 1e-12;

#[derive(Clone, Copy, Debug)]
struct DamResult {
    dx_m: f64,
    accepted_time_s: f64,
    substeps: u32,
    max_face_correction_cfl: f64,
    liquid_mass_kg: f64,
    carrier_mass_kg: f64,
    front_m: f64,
    center_of_mass_x_m: f64,
    center_of_mass_y_m: f64,
}

fn zero_faces(grid: Grid) -> FaceValues {
    FaceValues {
        u: vec![0.0; grid.u_faces()],
        v: vec![0.0; grid.v_faces()],
    }
}

fn full_interior_aperture(grid: Grid) -> FaceValues {
    let mut aperture = zero_faces(grid);
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
    aperture
}

fn dam_session(refinement: u32, mirrored: bool) -> ReferenceSession {
    let grid = Grid::with_cell_width(
        f64::from(32 * refinement),
        f64::from(16 * refinement),
        0.01 / f64::from(refinement),
    )
    .unwrap();
    let mut liquid_mass_kg = vec![0.0; grid.cells()];
    let mut carrier_mass_kg = vec![0.0; grid.cells()];
    let mut density_kg_m3 = vec![0.0; grid.cells()];
    for y in 0..grid.height() {
        for x in 0..grid.width() {
            let cell = grid.cell_index(x, y).unwrap();
            let in_column = if mirrored {
                x >= grid.width() - 8 * refinement
            } else {
                x < 8 * refinement
            };
            let liquid = in_column && y >= 4 * refinement;
            density_kg_m3[cell] = if liquid {
                LIQUID_DENSITY_KG_M3
            } else {
                CARRIER_DENSITY_KG_M3
            };
            if liquid {
                liquid_mass_kg[cell] = LIQUID_DENSITY_KG_M3 * grid.cell_volume_m3();
            } else {
                carrier_mass_kg[cell] = CARRIER_DENSITY_KG_M3 * grid.cell_volume_m3();
            }
        }
    }
    let inventory = TransportInventory::new(
        grid,
        LIQUID_DENSITY_KG_M3,
        CARRIER_DENSITY_KG_M3,
        liquid_mass_kg,
        carrier_mass_kg,
        vec![0.0; grid.cells()],
        vec![0.0; grid.cells()],
        vec![false; grid.cells()],
    )
    .unwrap();
    let fields = PressureFields::new(
        grid,
        [Boundary::Closed; 4],
        density_kg_m3,
        vec![0.0; grid.cells()],
        zero_faces(grid),
        full_interior_aperture(grid),
    )
    .unwrap();
    let mut session = ReferenceSession::new(grid);
    session.set_pressure_fields(fields).unwrap();
    session.set_transport_inventory(inventory).unwrap();
    session
}

fn assert_conserved(initial_kg: f64, final_kg: f64, phase: &str, refinement: u32) {
    let error_kg = (final_kg - initial_kg).abs();
    let allowed_kg = MASS_ABS_TOL_KG.max(MASS_REL_TOL * initial_kg);
    assert!(
        error_kg <= allowed_kg,
        "{phase} mass drift at refinement {refinement}: error {error_kg:e} kg, limit {allowed_kg:e} kg"
    );
}

fn measure(
    session: &ReferenceSession,
    accepted_time_s: f64,
    substeps: u32,
    max_face_correction_cfl: f64,
    mirrored: bool,
) -> DamResult {
    let inventory = session.transport_inventory().unwrap();
    let grid = inventory.grid();
    let width_m = f64::from(grid.width()) * grid.cell_width_m();
    let liquid_mass_kg: f64 = inventory.liquid_mass_kg().iter().sum();
    let carrier_mass_kg: f64 = inventory.carrier_mass_kg().iter().sum();
    let mut weighted_x_m_kg = 0.0;
    let mut weighted_y_m_kg = 0.0;
    let mut column_liquid_mass_kg = vec![0.0; grid.width() as usize];
    for y in 0..grid.height() {
        for x in 0..grid.width() {
            let cell = grid.cell_index(x, y).unwrap();
            let alpha = inventory.alpha(cell).unwrap();
            assert!(
                (-FRACTION_ABS_TOL..=1.0 + FRACTION_ABS_TOL).contains(&alpha),
                "alpha out of bounds at cell ({x}, {y}): {alpha:e}"
            );
            let cell_center_m = (f64::from(x) + 0.5) * grid.cell_width_m();
            weighted_x_m_kg += inventory.liquid_mass_kg()[cell] * cell_center_m;
            weighted_y_m_kg +=
                inventory.liquid_mass_kg()[cell] * (f64::from(y) + 0.5) * grid.cell_width_m();
            let column_from_home_wall = if mirrored { grid.width() - 1 - x } else { x };
            column_liquid_mass_kg[column_from_home_wall as usize] +=
                inventory.liquid_mass_kg()[cell];
        }
    }
    let target_mass_kg = FRONT_MASS_QUANTILE * liquid_mass_kg;
    let mut cumulative_mass_kg = 0.0;
    let mut front_m = None;
    for (column, &mass_kg) in column_liquid_mass_kg.iter().enumerate() {
        if cumulative_mass_kg + mass_kg >= target_mass_kg {
            assert!(mass_kg > 0.0, "liquid front fell in an empty column");
            let within_column = (target_mass_kg - cumulative_mass_kg) / mass_kg;
            front_m = Some((column as f64 + within_column) * grid.cell_width_m());
            break;
        }
        cumulative_mass_kg += mass_kg;
    }
    let front_m = front_m.expect("liquid mass percentile must lie within the domain");
    assert!(front_m > 0.0 && front_m < width_m);
    DamResult {
        dx_m: grid.cell_width_m(),
        accepted_time_s,
        substeps,
        max_face_correction_cfl,
        liquid_mass_kg,
        carrier_mass_kg,
        front_m,
        center_of_mass_x_m: weighted_x_m_kg / liquid_mass_kg,
        center_of_mass_y_m: weighted_y_m_kg / liquid_mass_kg,
    }
}

fn run_dam(refinement: u32, mirrored: bool) -> (ReferenceSession, DamResult) {
    let mut session = dam_session(refinement, mirrored);
    let initial = session.transport_inventory().unwrap().totals();
    let mut accepted_time_s = 0.0;
    let mut substeps = 0;
    let mut max_face_correction_cfl = 0.0_f64;
    let pressure = SolveConfig {
        max_iterations: 1024,
        ..SolveConfig::default()
    };
    for tick in 0..OUTER_TICKS {
        let mut clock = SubstepClock::new(OUTER_DT_S, 256).unwrap();
        loop {
            match session.advance_coupled_substep(
                &mut clock,
                CoupledStepConfig {
                    gravity_m_s2: GRAVITY_M_S2,
                    cell_dynamic_viscosity_pa_s: None,
                    pressure,
                },
            ) {
                Ok(CoupledStepOutcome::Advanced { volume_closure, .. }) => {
                    max_face_correction_cfl =
                        max_face_correction_cfl.max(volume_closure.max_face_correction_cfl);
                }
                Ok(CoupledStepOutcome::Complete) => break,
                Ok(CoupledStepOutcome::Paused {
                    reason,
                    remaining_s,
                }) => panic!(
                    "FLUID-DAM refinement {refinement}, mirrored {mirrored}, tick {tick}: {reason:?}, {remaining_s:e} s remains"
                ),
                Err(error) => {
                    let donor = match &error {
                        CoupledStepError::Transport(TransportError::InvalidCell {
                            index, ..
                        }) => {
                            let inventory = session.transport_inventory().unwrap();
                            format!(
                                ", donor {index}, alpha {:e}, liquid mass {:e} kg, carrier mass {:e} kg",
                                inventory.alpha(*index).unwrap(),
                                inventory.liquid_mass_kg()[*index],
                                inventory.carrier_mass_kg()[*index]
                            )
                        }
                        _ => String::new(),
                    };
                    panic!(
                        "FLUID-DAM refinement {refinement}, mirrored {mirrored}, tick {tick}, accepted {accepted_time_s:e} s + {:e} s in {} substeps: {error:?}{donor}",
                        clock.accepted_time_s(),
                        clock.accepted_substeps()
                    );
                }
            }
        }
        assert_eq!(clock.accepted_time_s(), OUTER_DT_S);
        accepted_time_s += clock.accepted_time_s();
        substeps += clock.accepted_substeps();
    }
    assert!((accepted_time_s - TARGET_TIME_S).abs() <= 1e-14);
    let result = measure(
        &session,
        accepted_time_s,
        substeps,
        max_face_correction_cfl,
        mirrored,
    );
    assert!(result.substeps >= OUTER_TICKS as u32);
    assert!(result.max_face_correction_cfl.is_finite());
    eprintln!("FLUID-DAM refinement {refinement}, mirrored {mirrored}: {result:?}");
    assert_conserved(
        initial.liquid_mass_kg,
        result.liquid_mass_kg,
        "liquid",
        refinement,
    );
    assert_conserved(
        initial.carrier_mass_kg,
        result.carrier_mass_kg,
        "carrier",
        refinement,
    );
    (session, result)
}

fn assert_refining(
    coarse: f64,
    medium: f64,
    fine: f64,
    medium_dx_m: f64,
    resolution_medium_cells: f64,
    observable: &str,
) {
    let coarse_gap_m = (medium - coarse).abs();
    let fine_gap_m = (fine - medium).abs();
    let resolution_m = resolution_medium_cells * medium_dx_m;
    if coarse_gap_m <= resolution_m {
        assert!(
            fine_gap_m <= resolution_m,
            "{observable} is unresolved at medium grid: coarse gap {coarse_gap_m:e} m, fine gap {fine_gap_m:e} m, resolution {resolution_m:e} m"
        );
    } else {
        let allowed_m = REFINEMENT_GAP_RATIO * coarse_gap_m;
        assert!(
            fine_gap_m <= allowed_m,
            "{observable} does not refine: coarse gap {coarse_gap_m:e} m, fine gap {fine_gap_m:e} m, limit {allowed_m:e} m"
        );
    }
}

#[test]
fn fluid_dam_baseline_keeps_both_phase_masses_and_bounded_alpha() {
    let (_, result) = run_dam(1, false);
    assert!(result.front_m > 0.08, "dam front never moved: {result:?}");
}

#[test]
fn fluid_dam_front_and_center_of_mass_refine_at_equal_time() {
    let (_, coarse) = run_dam(1, false);
    let (_, medium) = run_dam(2, false);
    let (_, fine) = run_dam(4, false);
    assert_eq!(coarse.accepted_time_s, medium.accepted_time_s);
    assert_eq!(medium.accepted_time_s, fine.accepted_time_s);
    let coarse_average_dt_s = coarse.accepted_time_s / f64::from(coarse.substeps);
    let medium_average_dt_s = medium.accepted_time_s / f64::from(medium.substeps);
    let fine_average_dt_s = fine.accepted_time_s / f64::from(fine.substeps);
    assert!(
        coarse_average_dt_s > medium_average_dt_s && medium_average_dt_s > fine_average_dt_s,
        "average accepted dt did not decrease under refinement: coarse {coarse_average_dt_s:e} s, medium {medium_average_dt_s:e} s, fine {fine_average_dt_s:e} s"
    );
    assert_refining(
        coarse.front_m,
        medium.front_m,
        fine.front_m,
        medium.dx_m,
        FRONT_RESOLUTION_MEDIUM_CELLS,
        "99th percentile liquid front",
    );
    assert_refining(
        coarse.center_of_mass_x_m,
        medium.center_of_mass_x_m,
        fine.center_of_mass_x_m,
        medium.dx_m,
        CENTER_RESOLUTION_MEDIUM_CELLS,
        "liquid x center of mass",
    );
    assert_refining(
        coarse.center_of_mass_y_m,
        medium.center_of_mass_y_m,
        fine.center_of_mass_y_m,
        medium.dx_m,
        CENTER_RESOLUTION_MEDIUM_CELLS,
        "liquid y center of mass",
    );
}

#[test]
fn fluid_dam_mirror_has_the_same_occupancy_at_equal_time() {
    let (left, left_result) = run_dam(1, false);
    let (right, right_result) = run_dam(1, true);
    assert_eq!(left_result.accepted_time_s, right_result.accepted_time_s);
    let grid = left.transport_inventory().unwrap().grid();
    let domain_width_m = f64::from(grid.width()) * grid.cell_width_m();
    let mut total_alpha_difference = 0.0;
    for y in 0..grid.height() {
        for x in 0..grid.width() {
            let left_cell = grid.cell_index(x, y).unwrap();
            let right_cell = grid.cell_index(grid.width() - 1 - x, y).unwrap();
            let left_alpha = left
                .transport_inventory()
                .unwrap()
                .alpha(left_cell)
                .unwrap();
            let right_alpha = right
                .transport_inventory()
                .unwrap()
                .alpha(right_cell)
                .unwrap();
            total_alpha_difference += (left_alpha - right_alpha).abs();
        }
    }
    let mean_alpha_difference = total_alpha_difference / grid.cells() as f64;
    assert!(
        mean_alpha_difference <= MIRROR_ALPHA_MEAN_TOL,
        "mirror mean alpha difference {mean_alpha_difference:e} exceeds {MIRROR_ALPHA_MEAN_TOL:e}"
    );
    assert!(
        (left_result.center_of_mass_x_m + right_result.center_of_mass_x_m - domain_width_m).abs()
            <= MIRROR_OBSERVABLE_TOL_M
    );
    assert!(
        (left_result.center_of_mass_y_m - right_result.center_of_mass_y_m).abs()
            <= MIRROR_OBSERVABLE_TOL_M
    );
    assert!((left_result.front_m - right_result.front_m).abs() <= MIRROR_OBSERVABLE_TOL_M);
}
