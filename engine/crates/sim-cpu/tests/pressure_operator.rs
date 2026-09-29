//! Manufactured MAC operators on validated closed and open CPU pressure fields.

use std::num::NonZeroU32;

use particle_sim::{CELL_WIDTH_M, Grid, contracts::Boundary};
use particle_sim_cpu::{
    fluid::{FaceValues, PressureFieldError, PressureFields},
    operator::{
        PressureOperatorError, ReservoirPressure, divergence_per_s, pressure_gradient_pa_per_m,
        pressure_gradient_with_reservoirs_pa_per_m,
    },
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

fn zero_velocity(grid: Grid) -> FaceValues {
    FaceValues {
        u: vec![0.0; grid.u_faces()],
        v: vec![0.0; grid.v_faces()],
    }
}

fn closed_fields(grid: Grid, pressure_pa: Vec<f64>, aperture: FaceValues) -> PressureFields {
    PressureFields::new(
        grid,
        [Boundary::Closed; 4],
        vec![1.0; grid.cells()],
        pressure_pa,
        zero_velocity(grid),
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

#[test]
fn constant_pressure_and_zero_velocity_have_zero_operators() {
    let grid = Grid::new(3.0, 2.0).unwrap();
    let fields = closed_fields(grid, vec![12.0; grid.cells()], closed_aperture(grid));
    let gradient = pressure_gradient_pa_per_m(&fields, fields.correction_pressure_pa()).unwrap();
    assert!(gradient.u.iter().all(|value| *value == 0.0));
    assert!(gradient.v.iter().all(|value| *value == 0.0));
    let divergence = divergence_per_s(&fields, fields.velocity_m_s()).unwrap();
    assert!(divergence.iter().all(|value| *value == 0.0));
}

#[test]
fn linear_pressure_and_uniform_interior_flow_match_boundary_cell_stencils() {
    let grid = Grid::new(3.0, 3.0).unwrap();
    let mut pressure_pa = vec![0.0; grid.cells()];
    for y in 0..3 {
        for x in 0..3 {
            pressure_pa[grid.cell_index(x, y).unwrap()] =
                17.0 + 3.0 * f64::from(x) * CELL_WIDTH_M - 4.0 * f64::from(y) * CELL_WIDTH_M;
        }
    }
    let fields = closed_fields(grid, pressure_pa, closed_aperture(grid));
    let gradient = pressure_gradient_pa_per_m(&fields, fields.correction_pressure_pa()).unwrap();

    for y in 0..3 {
        assert_eq!(gradient.u[grid.u_face_index(0, y).unwrap()], 0.0);
        assert_eq!(gradient.u[grid.u_face_index(3, y).unwrap()], 0.0);
        for x in 1..3 {
            assert_near(gradient.u[grid.u_face_index(x, y).unwrap()], 3.0);
        }
    }
    for x in 0..3 {
        assert_eq!(gradient.v[grid.v_face_index(x, 0).unwrap()], 0.0);
        assert_eq!(gradient.v[grid.v_face_index(x, 3).unwrap()], 0.0);
        for y in 1..3 {
            assert_near(gradient.v[grid.v_face_index(x, y).unwrap()], -4.0);
        }
    }

    let mut velocity = zero_velocity(grid);
    for y in 0..3 {
        for x in 1..3 {
            velocity.u[grid.u_face_index(x, y).unwrap()] = 2.0;
        }
    }
    for y in 1..3 {
        for x in 0..3 {
            velocity.v[grid.v_face_index(x, y).unwrap()] = -1.0;
        }
    }
    let divergence = divergence_per_s(&fields, &velocity).unwrap();
    assert_near(divergence[grid.cell_index(1, 1).unwrap()], 0.0);
    assert_near(divergence[grid.cell_index(0, 0).unwrap()], 100.0);
    assert_near(divergence[grid.cell_index(2, 2).unwrap()], -100.0);
    assert_near(divergence.iter().sum::<f64>(), 0.0);
}

#[test]
fn partial_face_scales_divergence_and_blocked_face_stops_both_operators() {
    let grid = Grid::new(3.0, 1.0).unwrap();
    let mut aperture = closed_aperture(grid);
    aperture.u[grid.u_face_index(1, 0).unwrap()] = 0.25;
    aperture.u[grid.u_face_index(2, 0).unwrap()] = 0.0;
    let fields = closed_fields(grid, vec![1.0, 3.0, 5.0], aperture);
    let gradient = pressure_gradient_pa_per_m(&fields, fields.correction_pressure_pa()).unwrap();
    assert_near(gradient.u[grid.u_face_index(1, 0).unwrap()], 200.0);
    assert_eq!(gradient.u[grid.u_face_index(2, 0).unwrap()], 0.0);

    let mut velocity = zero_velocity(grid);
    velocity.u[grid.u_face_index(1, 0).unwrap()] = 2.0;
    let divergence = divergence_per_s(&fields, &velocity).unwrap();
    assert_near(divergence[0], 50.0);
    assert_near(divergence[1], -50.0);
    assert_eq!(divergence[2], 0.0);

    velocity.u[grid.u_face_index(2, 0).unwrap()] = 0.1;
    assert_eq!(
        divergence_per_s(&fields, &velocity),
        Err(PressureOperatorError::Field(
            PressureFieldError::BlockedFaceVelocity {
                axis: "u",
                index: grid.u_face_index(2, 0).unwrap(),
            }
        ))
    );
}

#[test]
fn discrete_integration_by_parts_uses_matching_aperture_weights() {
    let grid = Grid::new(3.0, 2.0).unwrap();
    let mut aperture = closed_aperture(grid);
    aperture.u[grid.u_face_index(1, 0).unwrap()] = 0.25;
    aperture.u[grid.u_face_index(2, 0).unwrap()] = 0.75;
    aperture.u[grid.u_face_index(2, 1).unwrap()] = 0.0;
    aperture.v[grid.v_face_index(0, 1).unwrap()] = 0.3;
    aperture.v[grid.v_face_index(2, 1).unwrap()] = 0.6;
    let pressure_pa = vec![2.0, -1.0, 4.0, 0.0, 3.0, -2.0];
    let fields = closed_fields(grid, pressure_pa.clone(), aperture);

    let mut velocity = zero_velocity(grid);
    velocity.u[grid.u_face_index(1, 0).unwrap()] = 0.3;
    velocity.u[grid.u_face_index(2, 0).unwrap()] = -0.2;
    velocity.u[grid.u_face_index(1, 1).unwrap()] = 0.5;
    velocity.v[grid.v_face_index(0, 1).unwrap()] = 0.4;
    velocity.v[grid.v_face_index(1, 1).unwrap()] = -0.3;
    velocity.v[grid.v_face_index(2, 1).unwrap()] = 0.6;

    let divergence = divergence_per_s(&fields, &velocity).unwrap();
    let gradient = pressure_gradient_pa_per_m(&fields, &pressure_pa).unwrap();
    let cell_inner_product: f64 = pressure_pa
        .iter()
        .zip(&divergence)
        .map(|(pressure, div)| pressure * div)
        .sum();
    let face_inner_product: f64 = fields
        .aperture()
        .u
        .iter()
        .zip(&gradient.u)
        .zip(&velocity.u)
        .map(|((aperture, grad), speed)| aperture * grad * speed)
        .chain(
            fields
                .aperture()
                .v
                .iter()
                .zip(&gradient.v)
                .zip(&velocity.v)
                .map(|((aperture, grad), speed)| aperture * grad * speed),
        )
        .sum();
    assert_near(cell_inner_product, -face_inner_product);

    // D includes aperture, so D(G p) cannot have positive pressure energy.
    let energy: f64 = pressure_pa
        .iter()
        .zip(divergence_per_s(&fields, &gradient).unwrap())
        .map(|(pressure, div)| pressure * div)
        .sum();
    assert!(energy < 0.0);
}

#[test]
fn missing_reservoir_and_malformed_operator_inputs_fail_explicitly() {
    let grid = Grid::new(2.0, 1.0).unwrap();
    let mut aperture = closed_aperture(grid);
    aperture.u[grid.u_face_index(0, 0).unwrap()] = 0.5;
    let fields = PressureFields::new(
        grid,
        [
            Boundary::Open {
                reservoir: NonZeroU32::new(7).unwrap(),
            },
            Boundary::Closed,
            Boundary::Closed,
            Boundary::Closed,
        ],
        vec![1.0; grid.cells()],
        vec![0.0; grid.cells()],
        zero_velocity(grid),
        aperture,
    )
    .unwrap();
    let missing = PressureOperatorError::MissingReservoirPressure {
        reservoir: NonZeroU32::new(7).unwrap(),
    };
    assert_eq!(
        divergence_per_s(&fields, fields.velocity_m_s()).unwrap(),
        vec![0.0; grid.cells()]
    );
    assert_eq!(
        pressure_gradient_pa_per_m(&fields, fields.correction_pressure_pa()).unwrap_err(),
        missing
    );

    let fields = closed_fields(grid, vec![0.0; grid.cells()], closed_aperture(grid));
    assert_eq!(
        pressure_gradient_pa_per_m(&fields, &[0.0]),
        Err(PressureOperatorError::Field(PressureFieldError::Length {
            field: "pressure_pa",
            expected: 2,
            actual: 1,
        }))
    );
    assert_eq!(
        pressure_gradient_pa_per_m(&fields, &[0.0, f64::NAN]),
        Err(PressureOperatorError::Field(
            PressureFieldError::NonFinite {
                field: "pressure_pa",
                index: 1,
            }
        ))
    );
    assert_eq!(
        pressure_gradient_pa_per_m(&fields, &[f64::MAX, -f64::MAX]),
        Err(PressureOperatorError::NonFiniteOutput {
            field: "gradient.u_pa_per_m",
            index: grid.u_face_index(1, 0).unwrap(),
        })
    );
    let mut extreme_velocity = zero_velocity(grid);
    extreme_velocity.u[grid.u_face_index(1, 0).unwrap()] = f64::MAX;
    assert_eq!(
        divergence_per_s(&fields, &extreme_velocity),
        Err(PressureOperatorError::NonFiniteOutput {
            field: "divergence_per_s",
            index: 0,
        })
    );
}

#[test]
fn open_faces_use_supplied_flux_and_half_cell_reservoir_gradients() {
    let grid = Grid::new(1.0, 1.0).unwrap();
    let reservoirs = [1, 2, 3, 4].map(|id| NonZeroU32::new(id).unwrap());
    let boundaries = reservoirs.map(|reservoir| Boundary::Open { reservoir });
    let aperture = FaceValues {
        u: vec![0.5, 0.25],
        v: vec![0.75, 0.4],
    };
    let velocity = FaceValues {
        u: vec![2.0, 3.0],
        v: vec![-4.0, 5.0],
    };
    let fields =
        PressureFields::new(grid, boundaries, vec![1.0], vec![10.0], velocity, aperture).unwrap();
    assert_near(
        divergence_per_s(&fields, fields.velocity_m_s()).unwrap()[0],
        475.0,
    );

    let prescribed = [8.0, 7.0, 12.0, 9.0]
        .into_iter()
        .zip(reservoirs)
        .map(|(pressure_pa, reservoir)| ReservoirPressure {
            reservoir,
            pressure_pa,
        })
        .collect::<Vec<_>>();
    let gradient = pressure_gradient_with_reservoirs_pa_per_m(
        &fields,
        fields.correction_pressure_pa(),
        &prescribed,
    )
    .unwrap();
    assert_near(gradient.u[grid.u_face_index(0, 0).unwrap()], 400.0);
    assert_near(gradient.u[grid.u_face_index(1, 0).unwrap()], -600.0);
    assert_near(gradient.v[grid.v_face_index(0, 0).unwrap()], -400.0);
    assert_near(gradient.v[grid.v_face_index(0, 1).unwrap()], -200.0);

    assert_eq!(
        pressure_gradient_with_reservoirs_pa_per_m(
            &fields,
            fields.correction_pressure_pa(),
            &prescribed[..3],
        ),
        Err(PressureOperatorError::MissingReservoirPressure {
            reservoir: reservoirs[3],
        })
    );
    let mut same_pressure_twice = prescribed.clone();
    same_pressure_twice.push(prescribed[0]);
    assert_eq!(
        pressure_gradient_with_reservoirs_pa_per_m(
            &fields,
            fields.correction_pressure_pa(),
            &same_pressure_twice,
        ),
        Ok(gradient)
    );

    let blocked_top = PressureFields::new(
        grid,
        boundaries,
        vec![1.0],
        vec![10.0],
        zero_velocity(grid),
        FaceValues {
            u: vec![0.5, 0.25],
            v: vec![0.0, 0.4],
        },
    )
    .unwrap();
    let blocked_gradient =
        pressure_gradient_with_reservoirs_pa_per_m(&blocked_top, &[10.0], &prescribed).unwrap();
    assert_eq!(blocked_gradient.v[grid.v_face_index(0, 0).unwrap()], 0.0);
}

#[test]
fn open_reservoir_values_reject_nonfinite_or_conflicting_pressure() {
    let grid = Grid::new(1.0, 1.0).unwrap();
    let reservoir = NonZeroU32::new(7).unwrap();
    let fields = PressureFields::new(
        grid,
        [Boundary::Open { reservoir }; 4],
        vec![1.0],
        vec![10.0],
        zero_velocity(grid),
        FaceValues {
            u: vec![1.0; grid.u_faces()],
            v: vec![1.0; grid.v_faces()],
        },
    )
    .unwrap();
    let one_value = [ReservoirPressure {
        reservoir,
        pressure_pa: 8.0,
    }];
    let gradient =
        pressure_gradient_with_reservoirs_pa_per_m(&fields, &[10.0], &one_value).unwrap();
    assert_near(gradient.u[0], 400.0);
    assert_near(gradient.u[1], -400.0);
    assert_near(gradient.v[0], 400.0);
    assert_near(gradient.v[1], -400.0);

    assert_eq!(
        pressure_gradient_with_reservoirs_pa_per_m(
            &fields,
            &[10.0],
            &[ReservoirPressure {
                reservoir,
                pressure_pa: f64::NAN,
            }],
        ),
        Err(PressureOperatorError::NonFiniteReservoirPressure { reservoir })
    );
    assert_eq!(
        pressure_gradient_with_reservoirs_pa_per_m(
            &fields,
            &[10.0],
            &[
                ReservoirPressure {
                    reservoir,
                    pressure_pa: 8.0,
                },
                ReservoirPressure {
                    reservoir,
                    pressure_pa: 9.0,
                },
            ],
        ),
        Err(PressureOperatorError::ConflictingReservoirPressure { reservoir })
    );
}
