//! Validation of the owned CPU pressure-field layout and session isolation.

use std::num::NonZeroU32;

use particle_sim::{
    Grid,
    contracts::{Boundaries, Boundary},
};
use particle_sim_cpu::{
    ReferenceSession,
    fluid::{FaceValues, PressureFieldError, PressureFields},
};

fn closed_inputs(grid: Grid) -> (Boundaries, Vec<f64>, Vec<f64>, FaceValues, FaceValues) {
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
    (
        [Boundary::Closed; 4],
        vec![1.2; grid.cells()],
        vec![0.0; grid.cells()],
        FaceValues {
            u: vec![0.0; grid.u_faces()],
            v: vec![0.0; grid.v_faces()],
        },
        aperture,
    )
}

#[test]
fn owns_si_cell_and_face_layout_without_advancing_a_world() {
    let grid = Grid::new(3.0, 2.0).unwrap();
    let (boundaries, mut density, mut pressure, mut velocity, aperture) = closed_inputs(grid);
    density[grid.cell_index(1, 1).unwrap()] = 1000.0;
    pressure[grid.cell_index(1, 1).unwrap()] = 125.0;
    velocity.u[grid.u_face_index(1, 0).unwrap()] = 0.25;
    velocity.v[grid.v_face_index(2, 1).unwrap()] = -0.5;

    let fields =
        PressureFields::new(grid, boundaries, density, pressure, velocity, aperture).unwrap();
    assert_eq!(fields.grid(), grid);
    assert_eq!(fields.boundaries(), boundaries);
    assert_eq!(
        fields.density_kg_m3()[grid.cell_index(1, 1).unwrap()],
        1000.0
    );
    assert_eq!(
        fields.velocity_m_s().u[grid.u_face_index(1, 0).unwrap()],
        0.25
    );
    assert_eq!(
        fields.velocity_m_s().v[grid.v_face_index(2, 1).unwrap()],
        -0.5
    );
    assert_eq!(
        fields.correction_pressure_pa()[grid.cell_index(1, 1).unwrap()],
        125.0
    );
    assert_eq!(fields.velocity_m_s().u.len(), (3 + 1) * 2);
    assert_eq!(fields.velocity_m_s().v.len(), 3 * (2 + 1));
}

#[test]
fn rejects_bad_geometry_lengths_units_and_blocked_face_motion() {
    assert!(Grid::new(f64::NAN, 2.0).is_err());
    assert!(Grid::new(2.5, 2.0).is_err());
    let grid = Grid::new(2.0, 2.0).unwrap();

    let (boundaries, mut density, pressure, velocity, aperture) = closed_inputs(grid);
    density.pop();
    assert_eq!(
        PressureFields::new(grid, boundaries, density, pressure, velocity, aperture).unwrap_err(),
        PressureFieldError::Length {
            field: "density_kg_m3",
            expected: 4,
            actual: 3
        }
    );

    let (boundaries, density, mut pressure, velocity, aperture) = closed_inputs(grid);
    pressure.pop();
    assert_eq!(
        PressureFields::new(grid, boundaries, density, pressure, velocity, aperture).unwrap_err(),
        PressureFieldError::Length {
            field: "correction_pressure_pa",
            expected: 4,
            actual: 3
        }
    );

    let (boundaries, density, mut pressure, velocity, aperture) = closed_inputs(grid);
    pressure[1] = f64::INFINITY;
    assert_eq!(
        PressureFields::new(grid, boundaries, density, pressure, velocity, aperture).unwrap_err(),
        PressureFieldError::NonFinite {
            field: "correction_pressure_pa",
            index: 1
        }
    );

    let (boundaries, mut density, pressure, velocity, aperture) = closed_inputs(grid);
    density[0] = f64::NAN;
    assert_eq!(
        PressureFields::new(grid, boundaries, density, pressure, velocity, aperture).unwrap_err(),
        PressureFieldError::NonFinite {
            field: "density_kg_m3",
            index: 0
        }
    );

    let (boundaries, mut density, pressure, velocity, aperture) = closed_inputs(grid);
    density[0] = 0.0;
    assert_eq!(
        PressureFields::new(grid, boundaries, density, pressure, velocity, aperture).unwrap_err(),
        PressureFieldError::NonPositiveDensity { index: 0 }
    );

    let (boundaries, density, pressure, mut velocity, mut aperture) = closed_inputs(grid);
    let interior = grid.u_face_index(1, 0).unwrap();
    aperture.u[interior] = 0.0;
    velocity.u[interior] = 0.1;
    assert_eq!(
        PressureFields::new(grid, boundaries, density, pressure, velocity, aperture).unwrap_err(),
        PressureFieldError::BlockedFaceVelocity {
            axis: "u",
            index: interior
        }
    );

    let (boundaries, density, pressure, velocity, mut aperture) = closed_inputs(grid);
    aperture.v[grid.v_face_index(1, 1).unwrap()] = 1.1;
    assert_eq!(
        PressureFields::new(grid, boundaries, density, pressure, velocity, aperture).unwrap_err(),
        PressureFieldError::InvalidAperture {
            axis: "v",
            index: 3
        }
    );

    let (boundaries, density, pressure, mut velocity, aperture) = closed_inputs(grid);
    velocity.v[grid.v_face_index(0, 1).unwrap()] = f64::INFINITY;
    assert_eq!(
        PressureFields::new(grid, boundaries, density, pressure, velocity, aperture).unwrap_err(),
        PressureFieldError::NonFinite {
            field: "velocity.v_m_s",
            index: 2
        }
    );
}

#[test]
fn distinguishes_closed_walls_from_declared_open_boundaries() {
    let grid = Grid::new(2.0, 1.0).unwrap();
    let (boundaries, density, pressure, velocity, mut aperture) = closed_inputs(grid);
    aperture.u[0] = 1.0;
    assert_eq!(
        PressureFields::new(grid, boundaries, density, pressure, velocity, aperture).unwrap_err(),
        PressureFieldError::ClosedBoundaryFace {
            side: "left",
            index: 0
        }
    );

    let (mut boundaries, density, pressure, mut velocity, mut aperture) = closed_inputs(grid);
    boundaries[0] = Boundary::Open {
        reservoir: NonZeroU32::new(1).unwrap(),
    };
    aperture.u[0] = 0.5;
    velocity.u[0] = 0.2;
    let fields =
        PressureFields::new(grid, boundaries, density, pressure, velocity, aperture).unwrap();
    assert_eq!(fields.boundaries()[0], boundaries[0]);
    assert_eq!(fields.aperture().u[0], 0.5);
    assert_eq!(fields.velocity_m_s().u[0], 0.2);
}

#[test]
fn sessions_own_independent_pressure_inputs_and_reject_other_geometry() {
    let grid = Grid::new(2.0, 1.0).unwrap();
    let (boundaries, density, pressure, velocity, aperture) = closed_inputs(grid);
    let mut first = ReferenceSession::new(grid);
    let mut second = ReferenceSession::new(grid);
    first
        .set_pressure_fields(
            PressureFields::new(grid, boundaries, density, pressure, velocity, aperture).unwrap(),
        )
        .unwrap();

    let (boundaries, mut density, pressure, velocity, aperture) = closed_inputs(grid);
    density[0] = 1000.0;
    second
        .set_pressure_fields(
            PressureFields::new(grid, boundaries, density, pressure, velocity, aperture).unwrap(),
        )
        .unwrap();
    assert_eq!(first.pressure_fields().unwrap().density_kg_m3()[0], 1.2);
    assert_eq!(second.pressure_fields().unwrap().density_kg_m3()[0], 1000.0);

    let other_grid = Grid::new(1.0, 1.0).unwrap();
    let (boundaries, density, pressure, velocity, aperture) = closed_inputs(other_grid);
    let other = PressureFields::new(
        other_grid, boundaries, density, pressure, velocity, aperture,
    )
    .unwrap();
    assert_eq!(
        first.set_pressure_fields(other),
        Err(PressureFieldError::GridMismatch)
    );
    assert_eq!(first.pressure_fields().unwrap().grid(), grid);
}
