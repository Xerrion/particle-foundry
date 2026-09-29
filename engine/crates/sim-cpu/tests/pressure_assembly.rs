//! Manufactured sealed pressure matrices and component compatibility.

use std::num::NonZeroU32;

use particle_sim::{CELL_WIDTH_M, Grid, contracts::Boundary};
use particle_sim_cpu::{
    assembly::{PressureAssembly, PressureAssemblyError},
    fluid::{FaceValues, PressureFields},
    operator::{divergence_per_s, pressure_gradient_pa_per_m},
};

fn closed_aperture(grid: Grid) -> FaceValues {
    let width = grid.width() as usize;
    let height = grid.height() as usize;
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
    aperture
}

fn fields(grid: Grid, density: Vec<f64>, aperture: FaceValues) -> PressureFields {
    PressureFields::new(
        grid,
        [Boundary::Closed; 4],
        density,
        vec![0.0; grid.cells()],
        FaceValues {
            u: vec![0.0; grid.u_faces()],
            v: vec![0.0; grid.v_faces()],
        },
        aperture,
    )
    .unwrap()
}

fn assert_near(actual: f64, expected: f64) {
    let tolerance = 1e-12 * expected.abs().max(1.0);
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected}, got {actual}"
    );
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

#[test]
fn same_shape_at_different_cell_widths_scales_pressure_matrix() {
    let default_grid = Grid::new(2.0, 1.0).unwrap();
    let refined_grid = Grid::with_cell_width(2.0, 1.0, CELL_WIDTH_M / 2.0).unwrap();

    for grid in [default_grid, refined_grid] {
        let fields = fields(grid, vec![1.0, 3.0], closed_aperture(grid));
        let assembly = PressureAssembly::new(&fields).unwrap();
        let face = grid.u_face_index(1, 0).unwrap();
        let weight = 0.5 / grid.cell_width_m().powi(2);
        assert_near(assembly.weights_m_kg().u[face], weight);
        assert_eq!(assembly.diagonal_m_kg(), &[weight, weight]);
        let result = assembly.apply(&[0.0, 2.0]).unwrap();
        assert_near(result[0], -2.0 * weight);
        assert_near(result[1], 2.0 * weight);
    }
}

#[test]
fn two_cell_stencil_uses_arithmetic_face_density_and_expected_units() {
    let grid = Grid::new(2.0, 1.0).unwrap();
    let fields = fields(grid, vec![1.0, 3.0], closed_aperture(grid));
    let assembly = PressureAssembly::new(&fields).unwrap();
    let face = grid.u_face_index(1, 0).unwrap();
    let beta = 1.0 / 2.0;
    let weight = beta / (CELL_WIDTH_M * CELL_WIDTH_M);

    assert_near(assembly.inverse_face_density_m3_kg().u[face], beta);
    assert_near(assembly.weights_m_kg().u[face], weight);
    assert_eq!(assembly.diagonal_m_kg(), &[weight, weight]);
    assert_eq!(assembly.component_of_cell(), &[0, 0]);
    assert_eq!(assembly.component_sizes(), &[2]);

    let result = assembly.apply(&[2.0, 5.0]).unwrap();
    assert_near(result[0], -3.0 * weight);
    assert_near(result[1], 3.0 * weight);
    assert_eq!(assembly.apply(&[7.0, 7.0]).unwrap(), vec![0.0, 0.0]);
}

#[test]
fn variable_density_and_partial_faces_match_negative_divergence_of_beta_gradient() {
    let grid = Grid::new(3.0, 2.0).unwrap();
    let mut aperture = closed_aperture(grid);
    aperture.u[grid.u_face_index(1, 0).unwrap()] = 0.25;
    aperture.u[grid.u_face_index(2, 0).unwrap()] = 0.0;
    aperture.v[grid.v_face_index(0, 1).unwrap()] = 0.6;
    let fields = fields(grid, vec![1.0, 3.0, 8.0, 2.0, 4.0, 16.0], aperture);
    let assembly = PressureAssembly::new(&fields).unwrap();
    let pressure_pa = vec![0.0, 5.0, -2.0, 9.0, 3.0, 11.0];

    let face = grid.u_face_index(1, 0).unwrap();
    assert_near(assembly.inverse_face_density_m3_kg().u[face], 0.5);
    assert_near(
        assembly.weights_m_kg().u[face],
        0.25 * 0.5 / (CELL_WIDTH_M * CELL_WIDTH_M),
    );
    let blocked = grid.u_face_index(2, 0).unwrap();
    assert_eq!(assembly.inverse_face_density_m3_kg().u[blocked], 0.0);
    assert_eq!(assembly.weights_m_kg().u[blocked], 0.0);

    let mut beta_gradient = pressure_gradient_pa_per_m(&fields, &pressure_pa).unwrap();
    for (gradient, beta) in beta_gradient
        .u
        .iter_mut()
        .zip(&assembly.inverse_face_density_m3_kg().u)
    {
        *gradient *= beta;
    }
    for (gradient, beta) in beta_gradient
        .v
        .iter_mut()
        .zip(&assembly.inverse_face_density_m3_kg().v)
    {
        *gradient *= beta;
    }
    let divergence = divergence_per_s(&fields, &beta_gradient).unwrap();
    let actual = assembly.apply(&pressure_pa).unwrap();
    for (assembled, divergence) in actual.iter().zip(divergence) {
        assert_near(*assembled, -divergence);
    }
}

#[test]
fn matrix_is_symmetric_nonnegative_and_has_one_null_mode_per_component() {
    let grid = Grid::new(3.0, 2.0).unwrap();
    let mut aperture = closed_aperture(grid);
    aperture.u[grid.u_face_index(1, 0).unwrap()] = 0.4;
    aperture.v[grid.v_face_index(2, 1).unwrap()] = 0.0;
    let fields = fields(grid, vec![1.0, 3.0, 9.0, 2.0, 4.0, 8.0], aperture);
    let assembly = PressureAssembly::new(&fields).unwrap();
    let p = vec![3.0, -1.0, 2.0, 6.0, 0.0, 5.0];
    let q = vec![-2.0, 4.0, 1.0, 3.0, 7.0, -1.0];
    let ap = assembly.apply(&p).unwrap();
    let aq = assembly.apply(&q).unwrap();

    assert_near(dot(&p, &aq), dot(&q, &ap));
    assert!(dot(&p, &ap) > 0.0);
    assert_eq!(assembly.apply(&[10.0; 6]).unwrap(), vec![0.0; 6]);
    let net: f64 = ap.iter().sum();
    let scale: f64 = ap.iter().map(|value| value.abs()).sum();
    assert!(
        net.abs() <= 1e-12 * scale,
        "net {net} exceeds roundoff at scale {scale}"
    );

    for cell in 0..grid.cells() {
        let mut basis = vec![0.0; grid.cells()];
        basis[cell] = 1.0;
        let row = assembly.apply(&basis).unwrap();
        assert_near(row[cell], assembly.diagonal_m_kg()[cell]);
    }
}

#[test]
fn blocked_faces_split_components_and_isolated_rhs_must_be_zero() {
    let grid = Grid::new(3.0, 1.0).unwrap();
    let mut aperture = closed_aperture(grid);
    aperture.u[grid.u_face_index(2, 0).unwrap()] = 0.0;
    let fields = fields(grid, vec![1.0; 3], aperture);
    let assembly = PressureAssembly::new(&fields).unwrap();

    assert_eq!(assembly.component_of_cell(), &[0, 0, 1]);
    assert_eq!(assembly.component_sizes(), &[2, 1]);
    assert_eq!(assembly.diagonal_m_kg()[2], 0.0);
    assert_eq!(assembly.apply(&[5.0, 5.0, 13.0]).unwrap(), vec![0.0; 3]);
    assembly.check_compatible_rhs(&[1.0, -1.0, 0.0]).unwrap();
    assert!(matches!(
        assembly.check_compatible_rhs(&[1.0, -1.0, 0.1]),
        Err(PressureAssemblyError::IncompatibleRhs { component: 1, .. })
    ));
    assert!(matches!(
        assembly.check_compatible_rhs(&[1.0, 0.0, 0.0]),
        Err(PressureAssemblyError::IncompatibleRhs { component: 0, .. })
    ));
}

#[test]
fn open_boundary_and_unrepresentable_coefficients_are_rejected() {
    let grid = Grid::new(2.0, 1.0).unwrap();
    let mut aperture = closed_aperture(grid);
    aperture.u[grid.u_face_index(0, 0).unwrap()] = 0.5;
    let open_fields = PressureFields::new(
        grid,
        [
            Boundary::Open {
                reservoir: NonZeroU32::new(1).unwrap(),
            },
            Boundary::Closed,
            Boundary::Closed,
            Boundary::Closed,
        ],
        vec![1.0; grid.cells()],
        vec![0.0; grid.cells()],
        FaceValues {
            u: vec![0.0; grid.u_faces()],
            v: vec![0.0; grid.v_faces()],
        },
        aperture,
    )
    .unwrap();
    assert!(matches!(
        PressureAssembly::new(&open_fields),
        Err(PressureAssemblyError::OpenBoundaryRequiresReservoir { side: "left" })
    ));

    let tiny_density = fields(
        grid,
        vec![f64::MIN_POSITIVE / 2.0; 2],
        closed_aperture(grid),
    );
    assert!(matches!(
        PressureAssembly::new(&tiny_density),
        Err(PressureAssemblyError::InvalidFaceCoefficient {
            axis: "u",
            coefficient: "weight_m_kg",
            ..
        })
    ));
}

#[test]
fn invalid_vectors_do_not_leave_partial_results() {
    let grid = Grid::new(2.0, 1.0).unwrap();
    let fields = fields(grid, vec![1.0; 2], closed_aperture(grid));
    let assembly = PressureAssembly::new(&fields).unwrap();
    let mut output = vec![99.0; 2];

    assert_eq!(
        assembly.apply_into(&[0.0], &mut output),
        Err(PressureAssemblyError::Length {
            field: "pressure_pa",
            expected: 2,
            actual: 1,
        })
    );
    assert_eq!(output, vec![99.0; 2]);
    assert_eq!(
        assembly.apply_into(&[0.0, f64::NAN], &mut output),
        Err(PressureAssemblyError::NonFiniteInput {
            field: "pressure_pa",
            cell: 1,
        })
    );
    assert_eq!(output, vec![99.0; 2]);

    assert!(matches!(
        assembly.apply_into(&[f64::MAX, -f64::MAX], &mut output),
        Err(PressureAssemblyError::NonFiniteApply { .. })
    ));
    assert_eq!(output, vec![0.0; 2]);
    assert!(matches!(
        assembly.check_compatible_rhs(&[0.0, f64::INFINITY]),
        Err(PressureAssemblyError::NonFiniteInput {
            field: "rhs_per_s2",
            cell: 1
        })
    ));
}
