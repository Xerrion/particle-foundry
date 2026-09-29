//! Session-owned coefficient cache behavior across pressure input replacements.

use std::num::NonZeroU32;

use particle_sim::{Grid, contracts::Boundary};
use particle_sim_cpu::{
    PressureInputVersions, PressureSessionError, ReferenceSession,
    assembly::{PressureAssembly, PressureAssemblyError},
    fluid::{FaceValues, PressureFieldError, PressureFields},
    operator::pressure_gradient_pa_per_m,
    solver::{Projection, SolveConfig},
};

const DT_S: f64 = 1.0 / 60.0;

fn zeros(grid: Grid) -> FaceValues {
    FaceValues {
        u: vec![0.0; grid.u_faces()],
        v: vec![0.0; grid.v_faces()],
    }
}

fn interior_aperture(grid: Grid, block_first_u: bool) -> FaceValues {
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
    if block_first_u {
        aperture.u[grid.u_face_index(1, 0).unwrap()] = 0.0;
    }
    aperture
}

fn fields(
    grid: Grid,
    boundaries: [Boundary; 4],
    density: Vec<f64>,
    block_first_u: bool,
    pressure_offset_pa: f64,
    stored_velocity_m_s: f64,
) -> PressureFields {
    let aperture = interior_aperture(grid, block_first_u);
    let mut velocity = zeros(grid);
    let face = grid.u_face_index(1, 0).unwrap();
    if aperture.u[face] > 0.0 {
        velocity.u[face] = stored_velocity_m_s;
    }
    PressureFields::new(
        grid,
        boundaries,
        density,
        vec![pressure_offset_pa; grid.cells()],
        velocity,
        aperture,
    )
    .unwrap()
}

fn strict() -> SolveConfig {
    SolveConfig {
        max_iterations: 128,
        scaled_residual_tolerance: 1e-11,
        scaled_divergence_tolerance: 1e-11,
    }
}

fn manufactured_predictor(fields: &PressureFields) -> FaceValues {
    let grid = fields.grid();
    let known_pressure: Vec<f64> = (0..grid.cells())
        .map(|cell| ((cell * 7 % 11) as f64 - 5.0) * 0.2)
        .collect();
    let gradient = pressure_gradient_pa_per_m(fields, &known_pressure).unwrap();
    let assembly = PressureAssembly::new(fields).unwrap();
    FaceValues {
        u: gradient
            .u
            .iter()
            .zip(&assembly.inverse_face_density_m3_kg().u)
            .map(|(gradient, beta)| DT_S * beta * gradient)
            .collect(),
        v: gradient
            .v
            .iter()
            .zip(&assembly.inverse_face_density_m3_kg().v)
            .map(|(gradient, beta)| DT_S * beta * gradient)
            .collect(),
    }
}

fn assert_near(actual: f64, expected: f64) {
    let tolerance = 1e-10 * actual.abs().max(expected.abs()).max(1.0);
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected:e}, got {actual:e}"
    );
}

fn assert_faces_near(actual: &FaceValues, expected: &FaceValues) {
    for (actual, expected) in actual.u.iter().zip(&expected.u) {
        assert_near(*actual, *expected);
    }
    for (actual, expected) in actual.v.iter().zip(&expected.v) {
        assert_near(*actual, *expected);
    }
}

fn assert_cache_matches_rebuild(session: &mut ReferenceSession) -> Projection {
    let fields = session.pressure_fields().unwrap();
    let predictor = manufactured_predictor(fields);
    let source = vec![0.0; session.grid().cells()];
    let cached = session
        .project_pressure_cached(&predictor, &source, DT_S, strict())
        .unwrap();
    let rebuilt = session
        .project_pressure_forced_rebuild(&predictor, &source, DT_S, strict())
        .unwrap();
    assert_faces_near(&cached.velocity_m_s, &rebuilt.velocity_m_s);
    let fields = session.pressure_fields().unwrap();
    let cached_gradient = pressure_gradient_pa_per_m(fields, &cached.pressure_pa).unwrap();
    let rebuilt_gradient = pressure_gradient_pa_per_m(fields, &rebuilt.pressure_pa).unwrap();
    assert_faces_near(&cached_gradient, &rebuilt_gradient);
    assert!(
        cached
            .velocity_m_s
            .u
            .iter()
            .chain(&cached.velocity_m_s.v)
            .all(|speed| speed.abs() < 1e-9)
    );
    cached
}

#[test]
fn cache_reuses_velocity_and_pressure_only_replacements_and_rebuilds_changed_coefficients() {
    let grid = Grid::new(3.0, 2.0).unwrap();
    let mut session = ReferenceSession::new(grid);
    assert_eq!(
        session.pressure_versions(),
        PressureInputVersions::default()
    );
    assert_eq!(session.pressure_assembly_builds(), 0);

    session
        .set_pressure_fields(fields(
            grid,
            [Boundary::Closed; 4],
            vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
            false,
            0.0,
            0.0,
        ))
        .unwrap();
    let initial = session.pressure_versions();
    assert_eq!(
        (initial.geometry, initial.aperture, initial.density),
        (1, 1, 1)
    );
    assert_cache_matches_rebuild(&mut session);
    assert_eq!(session.pressure_assembly_builds(), 1);
    assert_cache_matches_rebuild(&mut session);
    assert_eq!(session.pressure_assembly_builds(), 1);

    session
        .set_pressure_fields(fields(
            grid,
            [Boundary::Closed; 4],
            vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
            false,
            42.0,
            0.05,
        ))
        .unwrap();
    assert_eq!(session.pressure_versions(), initial);
    assert_cache_matches_rebuild(&mut session);
    assert_eq!(session.pressure_assembly_builds(), 1);

    session
        .set_pressure_fields(fields(
            grid,
            [Boundary::Closed; 4],
            vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
            true,
            42.0,
            0.0,
        ))
        .unwrap();
    let blocked = session.pressure_versions();
    assert_eq!(blocked.geometry, initial.geometry);
    assert_eq!(blocked.aperture, initial.aperture + 1);
    assert_eq!(blocked.density, initial.density);
    assert_cache_matches_rebuild(&mut session);
    assert_eq!(session.pressure_assembly_builds(), 2);

    session
        .set_pressure_fields(fields(
            grid,
            [Boundary::Closed; 4],
            vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
            false,
            42.0,
            0.0,
        ))
        .unwrap();
    let reopened = session.pressure_versions();
    assert_eq!(reopened.aperture, blocked.aperture + 1);
    assert_eq!(reopened.density, blocked.density);
    assert_cache_matches_rebuild(&mut session);
    assert_eq!(session.pressure_assembly_builds(), 3);

    session
        .set_pressure_fields(fields(
            grid,
            [Boundary::Closed; 4],
            vec![1.0, 2.0, 3.0, 4.0, 5.0, 12.0],
            false,
            42.0,
            0.0,
        ))
        .unwrap();
    let denser = session.pressure_versions();
    assert_eq!(denser.geometry, reopened.geometry);
    assert_eq!(denser.aperture, reopened.aperture);
    assert_eq!(denser.density, reopened.density + 1);
    assert_cache_matches_rebuild(&mut session);
    assert_eq!(session.pressure_assembly_builds(), 4);
}

#[test]
fn rejected_grid_mismatch_preserves_fields_versions_and_cached_assembly() {
    let grid = Grid::new(3.0, 2.0).unwrap();
    let mut session = ReferenceSession::new(grid);
    session
        .set_pressure_fields(fields(
            grid,
            [Boundary::Closed; 4],
            vec![1.0; grid.cells()],
            false,
            0.0,
            0.0,
        ))
        .unwrap();
    assert_cache_matches_rebuild(&mut session);
    let before = session.pressure_versions();
    let builds = session.pressure_assembly_builds();

    let other_grid = Grid::new(2.0, 2.0).unwrap();
    assert_eq!(
        session.set_pressure_fields(fields(
            other_grid,
            [Boundary::Closed; 4],
            vec![8.0; other_grid.cells()],
            false,
            99.0,
            0.0,
        )),
        Err(PressureFieldError::GridMismatch)
    );
    assert_eq!(session.pressure_versions(), before);
    assert_eq!(session.pressure_assembly_builds(), builds);
    assert_eq!(session.pressure_fields().unwrap().grid(), grid);
    assert_eq!(
        session.pressure_fields().unwrap().density_kg_m3(),
        &[1.0; 6]
    );
    assert_cache_matches_rebuild(&mut session);
    assert_eq!(session.pressure_assembly_builds(), builds);
}

#[test]
fn boundary_change_invalidates_closed_assembly_before_open_boundary_rejection() {
    let grid = Grid::new(3.0, 2.0).unwrap();
    let mut session = ReferenceSession::new(grid);
    session
        .set_pressure_fields(fields(
            grid,
            [Boundary::Closed; 4],
            vec![1.0; grid.cells()],
            false,
            0.0,
            0.0,
        ))
        .unwrap();
    assert_cache_matches_rebuild(&mut session);
    let closed_versions = session.pressure_versions();
    assert_eq!(session.pressure_assembly_builds(), 1);

    let open_left = [
        Boundary::Open {
            reservoir: NonZeroU32::new(1).unwrap(),
        },
        Boundary::Closed,
        Boundary::Closed,
        Boundary::Closed,
    ];
    session
        .set_pressure_fields(fields(
            grid,
            open_left,
            vec![1.0; grid.cells()],
            false,
            0.0,
            0.0,
        ))
        .unwrap();
    let open_versions = session.pressure_versions();
    assert_eq!(open_versions.geometry, closed_versions.geometry + 1);
    assert_eq!(open_versions.aperture, closed_versions.aperture);
    assert_eq!(open_versions.density, closed_versions.density);
    assert!(matches!(
        session.project_pressure_cached(&zeros(grid), &[0.0; 6], DT_S, strict()),
        Err(PressureSessionError::Assembly(
            PressureAssemblyError::OpenBoundaryRequiresReservoir { side: "left" }
        ))
    ));
    assert!(matches!(
        session.project_pressure_forced_rebuild(&zeros(grid), &[0.0; 6], DT_S, strict()),
        Err(PressureSessionError::Assembly(
            PressureAssemblyError::OpenBoundaryRequiresReservoir { side: "left" }
        ))
    ));
    assert_eq!(session.pressure_assembly_builds(), 1);

    session
        .set_pressure_fields(fields(
            grid,
            [Boundary::Closed; 4],
            vec![1.0; grid.cells()],
            false,
            0.0,
            0.0,
        ))
        .unwrap();
    assert_eq!(
        session.pressure_versions().geometry,
        open_versions.geometry + 1
    );
    assert_cache_matches_rebuild(&mut session);
    assert_eq!(session.pressure_assembly_builds(), 2);
}
