use super::*;

#[test]
#[ignore = "requires a native compute adapter"]
fn manufactured_closed_gradient_converges_on_gpu_and_exhaustion_rejects() {
    block_on(async {
        let context = crate::GpuContext::new().await.unwrap();
        let grid = Grid::new(4.0, 3.0).unwrap();
        let width = grid.width() as usize;
        let height = grid.height() as usize;
        let density = vec![1.2_f32; grid.cells()];
        let mut aperture_u = vec![0.0_f32; grid.u_faces()];
        let mut aperture_v = vec![0.0_f32; grid.v_faces()];
        let mut predictor_u = vec![0.0_f32; grid.u_faces()];
        let mut predictor_v = vec![0.0_f32; grid.v_faces()];
        let known_pressure = [
            0.03, 0.11, -0.06, 0.07, -0.08, 0.02, 0.17, -0.02, 0.09, -0.13, 0.04, -0.05,
        ];
        let dt_s = 1.0 / 60.0;
        for y in 0..height {
            for x in 1..width {
                let face = y * (width + 1) + x;
                let left = y * width + x - 1;
                aperture_u[face] = 1.0;
                predictor_u[face] =
                    dt_s * (1.0 / 1.2) * (known_pressure[left + 1] - known_pressure[left])
                        / CELL_WIDTH_M as f32;
            }
        }
        for y in 1..height {
            for x in 0..width {
                let face = y * width + x;
                let top = (y - 1) * width + x;
                aperture_v[face] = 1.0;
                predictor_v[face] =
                    dt_s * (1.0 / 1.2) * (known_pressure[top + width] - known_pressure[top])
                        / CELL_WIDTH_M as f32;
            }
        }
        let density = initial_buffer(&context.device, "density", &density);
        let root_cell = root_buffer(&context.device, &vec![0; grid.cells()]);
        let aperture_u = initial_buffer(&context.device, "u aperture", &aperture_u);
        let aperture_v = initial_buffer(&context.device, "v aperture", &aperture_v);
        let predictor_u = initial_buffer(&context.device, "u predictor", &predictor_u);
        let predictor_v = initial_buffer(&context.device, "v predictor", &predictor_v);
        let corrected_u = candidate_buffer(&context.device, "u candidate", grid.u_faces());
        let corrected_v = candidate_buffer(&context.device, "v candidate", grid.v_faces());
        let fields = PressureFields {
            density: &density,
            root_cell: &root_cell,
            aperture_u: &aperture_u,
            aperture_v: &aperture_v,
            predictor_u: &predictor_u,
            predictor_v: &predictor_v,
            corrected_u: &corrected_u,
            corrected_v: &corrected_v,
        };
        let mut projector = GpuPressureProjector::new(&context.device, grid).unwrap();
        let exhausted = projector
            .project_closed(
                &context.device,
                &context.queue,
                PressureFields { ..fields },
                PressureConfig {
                    max_iterations: 0,
                    ..PressureConfig::default()
                },
            )
            .await;
        assert!(
            exhausted.is_err(),
            "zero iterations must reject this predictor"
        );
        let diagnostics = projector
            .project_closed(
                &context.device,
                &context.queue,
                fields,
                PressureConfig::default(),
            )
            .await
            .unwrap();
        eprintln!("manufactured GPU pressure: {diagnostics:?}");
        assert!(diagnostics.iterations > 0);
        assert!(diagnostics.scaled_residual <= 1e-5, "{diagnostics:?}");
        assert!(diagnostics.scaled_divergence <= 1e-5, "{diagnostics:?}");
        let corrected_u = read_f32(&context.device, &context.queue, &corrected_u).await;
        let corrected_v = read_f32(&context.device, &context.queue, &corrected_v).await;
        let max_speed = corrected_u
            .iter()
            .chain(&corrected_v)
            .fold(0.0_f32, |maximum, speed| maximum.max(speed.abs()));
        assert!(max_speed <= 2e-5, "max corrected speed {max_speed}");
        context.dispose();
    });
}

#[test]
#[ignore = "requires a native compute adapter"]
fn disconnected_closed_components_project_without_cross_wall_flux() {
    block_on(async {
        let context = crate::GpuContext::new().await.unwrap();
        let grid = Grid::new(5.0, 1.0).unwrap();
        let density = initial_buffer(
            &context.device,
            "density",
            &[1.2, 1.2, 1000.0, 1000.0, 1000.0],
        );
        let root_cell = root_buffer(&context.device, &[0, 0, 2, 3, 3]);
        let aperture_u = initial_buffer(
            &context.device,
            "u aperture",
            &[0.0, 0.7, 0.0, 0.0, 1.0, 0.0],
        );
        let aperture_v = initial_buffer(&context.device, "v aperture", &[0.0; 10]);
        let predictor_u = initial_buffer(
            &context.device,
            "u predictor",
            &[0.0, 0.1, 0.0, 0.0, -0.2, 0.0],
        );
        let predictor_v = initial_buffer(&context.device, "v predictor", &[0.0; 10]);
        let corrected_u = candidate_buffer(&context.device, "u candidate", grid.u_faces());
        let corrected_v = candidate_buffer(&context.device, "v candidate", grid.v_faces());
        let mut projector = GpuPressureProjector::new(&context.device, grid).unwrap();
        let diagnostics = projector
            .project_closed(
                &context.device,
                &context.queue,
                PressureFields {
                    density: &density,
                    root_cell: &root_cell,
                    aperture_u: &aperture_u,
                    aperture_v: &aperture_v,
                    predictor_u: &predictor_u,
                    predictor_v: &predictor_v,
                    corrected_u: &corrected_u,
                    corrected_v: &corrected_v,
                },
                PressureConfig::default(),
            )
            .await
            .unwrap();
        eprintln!("disconnected GPU pressure: {diagnostics:?}");
        assert!(diagnostics.scaled_residual <= 1e-5, "{diagnostics:?}");
        assert!(diagnostics.scaled_divergence <= 1e-5, "{diagnostics:?}");
        let u_values = read_f32(&context.device, &context.queue, &corrected_u).await;
        let v_values = read_f32(&context.device, &context.queue, &corrected_v).await;
        assert!(u_values[1].abs() <= 1e-5);
        assert!(u_values[4].abs() <= 1e-5);
        assert_eq!(u_values[2], 0.0);
        assert_eq!(u_values[3], 0.0);
        assert!(v_values.iter().all(|value| *value == 0.0));

        let next_u = candidate_buffer(&context.device, "next u candidate", grid.u_faces());
        let next_v = candidate_buffer(&context.device, "next v candidate", grid.v_faces());
        for step in 0..8 {
            let (source_u, source_v, target_u, target_v) = if step % 2 == 0 {
                (&corrected_u, &corrected_v, &next_u, &next_v)
            } else {
                (&next_u, &next_v, &corrected_u, &corrected_v)
            };
            let repeated = projector
                .project_closed(
                    &context.device,
                    &context.queue,
                    PressureFields {
                        density: &density,
                        root_cell: &root_cell,
                        aperture_u: &aperture_u,
                        aperture_v: &aperture_v,
                        predictor_u: source_u,
                        predictor_v: source_v,
                        corrected_u: target_u,
                        corrected_v: target_v,
                    },
                    PressureConfig::default(),
                )
                .await
                .unwrap();
            assert!(repeated.scaled_residual <= 1e-5, "{repeated:?}");
            assert!(repeated.scaled_divergence <= 1e-5, "{repeated:?}");
        }
        let repeated_u = read_f32(&context.device, &context.queue, &corrected_u).await;
        assert!(repeated_u.iter().all(|face| face.abs() <= 1e-5));

        let still_u = initial_buffer(&context.device, "zero u predictor", &[0.0; 6]);
        let still_v = initial_buffer(&context.device, "zero v predictor", &[0.0; 10]);
        let zero = projector
            .project_closed(
                &context.device,
                &context.queue,
                PressureFields {
                    density: &density,
                    root_cell: &root_cell,
                    aperture_u: &aperture_u,
                    aperture_v: &aperture_v,
                    predictor_u: &still_u,
                    predictor_v: &still_v,
                    corrected_u: &corrected_u,
                    corrected_v: &corrected_v,
                },
                PressureConfig {
                    max_iterations: 0,
                    ..PressureConfig::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(zero.iterations, 0);
        assert_eq!(zero.scaled_residual, 0.0);
        assert_eq!(zero.scaled_divergence, 0.0);
        context.dispose();
    });
}

#[test]
#[ignore = "requires a native compute adapter"]
fn nonfinite_interior_predictor_is_rejected_by_gpu_reduction() {
    block_on(async {
        let context = crate::GpuContext::new().await.unwrap();
        let grid = Grid::new(2.0, 1.0).unwrap();
        let density = initial_buffer(&context.device, "density", &[1.2; 2]);
        let root_cell = root_buffer(&context.device, &[0, 0]);
        let aperture_u = initial_buffer(&context.device, "u aperture", &[0.0, 1.0, 0.0]);
        let aperture_v = initial_buffer(&context.device, "v aperture", &[0.0; 4]);
        let predictor_u = initial_buffer(&context.device, "u predictor", &[0.0, f32::NAN, 0.0]);
        let predictor_v = initial_buffer(&context.device, "v predictor", &[0.0; 4]);
        let corrected_u = candidate_buffer(&context.device, "u candidate", grid.u_faces());
        let corrected_v = candidate_buffer(&context.device, "v candidate", grid.v_faces());
        let mut projector = GpuPressureProjector::new(&context.device, grid).unwrap();
        let result = projector
            .project_closed(
                &context.device,
                &context.queue,
                PressureFields {
                    density: &density,
                    root_cell: &root_cell,
                    aperture_u: &aperture_u,
                    aperture_v: &aperture_v,
                    predictor_u: &predictor_u,
                    predictor_v: &predictor_v,
                    corrected_u: &corrected_u,
                    corrected_v: &corrected_v,
                },
                PressureConfig::default(),
            )
            .await;
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("arithmetic rejected the initial fields")
        );
        context.dispose();
    });
}
