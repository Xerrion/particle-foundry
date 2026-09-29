//! End-to-end evidence for the sealed f64 MAC pressure projection.

use particle_sim::{Grid, contracts::Boundary};
use particle_sim_cpu::{
    assembly::{PressureAssembly, PressureAssemblyError},
    fluid::{FaceValues, PressureFields},
    operator::pressure_gradient_pa_per_m,
    solver::{PressureSolveError, SolveConfig, gravity_predictor_closed, project_closed},
};

const DT_S: f64 = 1.0 / 60.0;
const GRAVITY_M_S2: f64 = 9.80665;

fn zeros(grid: Grid) -> FaceValues {
    FaceValues {
        u: vec![0.0; grid.u_faces()],
        v: vec![0.0; grid.v_faces()],
    }
}

fn full_interior_apertures(grid: Grid) -> FaceValues {
    let width = grid.width() as usize;
    let height = grid.height() as usize;
    let mut aperture = zeros(grid);
    for y in 0..height {
        for x in 1..width {
            aperture.u[y * (width + 1) + x] = 1.0;
        }
    }
    for y in 1..height {
        for x in 0..width {
            aperture.v[y * width + x] = 1.0;
        }
    }
    aperture
}

fn closed_fields(
    grid: Grid,
    density: Vec<f64>,
    pressure: Vec<f64>,
    velocity: FaceValues,
    aperture: FaceValues,
) -> PressureFields {
    PressureFields::new(
        grid,
        [Boundary::Closed; 4],
        density,
        pressure,
        velocity,
        aperture,
    )
    .unwrap()
}

fn strict() -> SolveConfig {
    SolveConfig {
        max_iterations: 256,
        scaled_residual_tolerance: 1e-12,
        scaled_divergence_tolerance: 1e-12,
    }
}

fn max_face_speed(velocity: &FaceValues) -> f64 {
    velocity
        .u
        .iter()
        .chain(&velocity.v)
        .fold(0.0_f64, |maximum, speed| maximum.max(speed.abs()))
}

#[test]
fn manufactured_pressure_is_recovered_as_zero_corrected_velocity() {
    let grid = Grid::new(4.0, 3.0).unwrap();
    let density = (0..grid.cells())
        .map(|cell| if cell / 4 >= 2 { 1000.0 } else { 1.2 })
        .collect();
    let fields = closed_fields(
        grid,
        density,
        vec![0.0; grid.cells()],
        zeros(grid),
        full_interior_apertures(grid),
    );
    let known_pressure = vec![
        0.3, 1.1, -0.6, 0.7, -0.8, 0.2, 1.7, -0.2, 0.9, -1.3, 0.4, -0.5,
    ];
    let gradient = pressure_gradient_pa_per_m(&fields, &known_pressure).unwrap();
    let beta = PressureAssembly::new(&fields).unwrap();
    let mut predictor = zeros(grid);
    for face in 0..grid.u_faces() {
        predictor.u[face] = DT_S * beta.inverse_face_density_m3_kg().u[face] * gradient.u[face];
    }
    for face in 0..grid.v_faces() {
        predictor.v[face] = DT_S * beta.inverse_face_density_m3_kg().v[face] * gradient.v[face];
    }

    let result = project_closed(
        &fields,
        &predictor,
        &vec![0.0; grid.cells()],
        DT_S,
        strict(),
    )
    .unwrap();
    assert!(result.diagnostics.iterations > 0);
    assert!(result.diagnostics.scaled_residual <= 1e-12);
    assert!(result.diagnostics.scaled_divergence <= 1e-12);
    assert!(max_face_speed(&result.velocity_m_s) < 1e-10);
}

#[test]
fn zero_rhs_and_uniform_pressure_need_no_iterations() {
    let grid = Grid::new(3.0, 2.0).unwrap();
    let fields = closed_fields(
        grid,
        vec![1.2; grid.cells()],
        vec![101_325.0; grid.cells()],
        zeros(grid),
        full_interior_apertures(grid),
    );
    let result = project_closed(
        &fields,
        fields.velocity_m_s(),
        &vec![0.0; grid.cells()],
        DT_S,
        strict(),
    )
    .unwrap();
    assert_eq!(result.diagnostics.iterations, 0);
    assert_eq!(result.diagnostics.scaled_residual, 0.0);
    assert_eq!(result.diagnostics.scaled_divergence, 0.0);
    assert_eq!(max_face_speed(&result.velocity_m_s), 0.0);
}

#[test]
fn iteration_cap_reports_failure_instead_of_returning_a_partial_projection() {
    let grid = Grid::new(3.0, 2.0).unwrap();
    let mut predictor = zeros(grid);
    predictor.u[1] = 0.2;
    let fields = closed_fields(
        grid,
        vec![1.2; grid.cells()],
        vec![0.0; grid.cells()],
        zeros(grid),
        full_interior_apertures(grid),
    );
    let config = SolveConfig {
        max_iterations: 0,
        ..strict()
    };
    let error =
        project_closed(&fields, &predictor, &vec![0.0; grid.cells()], DT_S, config).unwrap_err();
    match error {
        PressureSolveError::DidNotConverge { diagnostics } => {
            assert_eq!(diagnostics.iterations, 0);
            assert!(diagnostics.scaled_residual > config.scaled_residual_tolerance);
            assert!(diagnostics.scaled_divergence > config.scaled_divergence_tolerance);
        }
        other => panic!("expected honest cap failure, got {other:?}"),
    }
    assert_eq!(fields.velocity_m_s().u[1], 0.0);
    assert_eq!(fields.correction_pressure_pa(), &[0.0; 6]);
}

#[test]
fn disconnected_chambers_and_isolated_cell_have_independent_gauges() {
    let grid = Grid::new(5.0, 1.0).unwrap();
    let mut aperture = zeros(grid);
    aperture.u[1] = 0.7;
    aperture.u[4] = 1.0;
    let fields = closed_fields(
        grid,
        vec![1.2, 1.2, 1000.0, 1000.0, 1000.0],
        vec![80.0, 80.0, 100.0, -22.0, -22.0],
        zeros(grid),
        aperture,
    );
    let assembly = PressureAssembly::new(&fields).unwrap();
    assert_eq!(assembly.component_sizes(), &[2, 1, 2]);
    let mut predictor = zeros(grid);
    predictor.u[1] = 0.1;
    predictor.u[4] = -0.2;
    let result = project_closed(&fields, &predictor, &[0.0; 5], DT_S, strict()).unwrap();
    assert!(max_face_speed(&result.velocity_m_s) < 1e-10);
    assert_eq!(result.pressure_pa[2], 0.0);
    assert!((result.pressure_pa[0] + result.pressure_pa[1]).abs() < 1e-10);
    assert!((result.pressure_pa[3] + result.pressure_pa[4]).abs() < 1e-10);

    let error = project_closed(
        &fields,
        &predictor,
        &[0.0, 0.0, 1.0, 0.0, 0.0],
        DT_S,
        strict(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        PressureSolveError::Assembly(PressureAssemblyError::IncompatibleRhs { component: 1, .. })
    ));
}

#[test]
fn rejects_bad_dt_source_and_tolerance_before_solving() {
    let grid = Grid::new(1.0, 1.0).unwrap();
    let fields = closed_fields(grid, vec![1.2], vec![0.0], zeros(grid), zeros(grid));
    assert_eq!(
        project_closed(&fields, fields.velocity_m_s(), &[0.0], 0.0, strict()).unwrap_err(),
        PressureSolveError::InvalidTimeStep
    );
    assert_eq!(
        project_closed(&fields, fields.velocity_m_s(), &[f64::NAN], DT_S, strict()).unwrap_err(),
        PressureSolveError::NonFiniteSource { cell: 0 }
    );
    assert_eq!(
        project_closed(
            &fields,
            fields.velocity_m_s(),
            &[0.0],
            DT_S,
            SolveConfig {
                scaled_residual_tolerance: f64::INFINITY,
                ..strict()
            }
        )
        .unwrap_err(),
        PressureSolveError::InvalidTolerance {
            field: "scaled_residual_tolerance"
        }
    );
}

#[test]
fn stratified_hydrostatic_pool_stays_at_rest_for_1000_ticks() {
    let grid = Grid::new(16.0, 16.0).unwrap();
    let density: Vec<f64> = (0..grid.cells())
        .map(|cell| if cell / 16 >= 8 { 1000.0 } else { 1.2 })
        .collect();
    let aperture = full_interior_apertures(grid);
    let mut fields = closed_fields(
        grid,
        density.clone(),
        vec![0.0; grid.cells()],
        zeros(grid),
        FaceValues {
            u: aperture.u.clone(),
            v: aperture.v.clone(),
        },
    );
    let mut worst_speed: f64 = 0.0;
    let mut worst_divergence: f64 = 0.0;
    for _ in 0..1000 {
        let predictor = gravity_predictor_closed(&fields, DT_S, GRAVITY_M_S2).unwrap();
        let result = project_closed(
            &fields,
            &predictor,
            &vec![0.0; grid.cells()],
            DT_S,
            strict(),
        )
        .unwrap();
        worst_speed = worst_speed.max(max_face_speed(&result.velocity_m_s));
        worst_divergence = worst_divergence.max(result.diagnostics.scaled_divergence);
        fields = closed_fields(
            grid,
            density.clone(),
            result.pressure_pa,
            result.velocity_m_s,
            FaceValues {
                u: aperture.u.clone(),
                v: aperture.v.clone(),
            },
        );
    }
    assert!(worst_speed <= 1e-8, "1000-tick max speed {worst_speed:e}");
    assert!(
        worst_divergence <= 1e-8,
        "1000-tick max scaled divergence {worst_divergence:e}"
    );
}
