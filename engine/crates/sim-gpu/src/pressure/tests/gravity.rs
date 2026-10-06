use super::*;

#[test]
#[ignore = "requires a native compute adapter"]
fn mixed_density_hydrostatic_guess_closes_vertical_gravity_faces() {
    block_on(async {
        let context = crate::GpuContext::new().await.unwrap();
        let grid = Grid::new(2.0, 2.0).unwrap();
        let dt_s = 1.0 / 60.0;
        let gravity_m_s2 = 9.80665;
        let density = initial_buffer(&context.device, "density", &[1.2, 1.2, 1000.0, 1000.0]);
        let root_cell = root_buffer(&context.device, &[0, 0, 0, 0]);
        let aperture_u = initial_buffer(
            &context.device,
            "u aperture",
            &[0.0, 1.0, 0.0, 0.0, 1.0, 0.0],
        );
        let aperture_v = initial_buffer(
            &context.device,
            "v aperture",
            &[0.0, 0.0, 1.0, 1.0, 0.0, 0.0],
        );
        let predictor_u = initial_buffer(&context.device, "u predictor", &[0.0; 6]);
        let predictor_v = initial_buffer(
            &context.device,
            "v predictor",
            &[0.0, 0.0, dt_s * gravity_m_s2, dt_s * gravity_m_s2, 0.0, 0.0],
        );
        let corrected_u = candidate_buffer(&context.device, "u candidate", 6);
        let corrected_v = candidate_buffer(&context.device, "v candidate", 6);
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
                PressureConfig {
                    dt_s,
                    gravity_m_s2,
                    max_iterations: 0,
                    ..PressureConfig::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(diagnostics.iterations, 0);
        assert!(diagnostics.scaled_residual <= 1e-5, "{diagnostics:?}");
        assert!(diagnostics.scaled_divergence <= 1e-5, "{diagnostics:?}");
        let corrected_v = read_f32(&context.device, &context.queue, &corrected_v).await;
        assert!(corrected_v.iter().all(|velocity| velocity.abs() <= 1e-5));
        context.dispose();
    });
}

#[test]
#[ignore = "requires a native compute adapter"]
fn horizontal_hydrostatic_variation_converges_with_gravity() {
    block_on(async {
        let context = crate::GpuContext::new().await.unwrap();
        let grid = Grid::new(4.0, 3.0).unwrap();
        let width = grid.width() as usize;
        let height = grid.height() as usize;
        let dt_s = 1.0 / 60.0;
        let gravity_m_s2 = 9.80665;
        let density = [
            950.0, 1000.0, 1050.0, 1100.0, 925.0, 1025.0, 1075.0, 1125.0, 975.0, 995.0, 1055.0,
            1085.0,
        ];
        let mut aperture_u = vec![0.0_f32; grid.u_faces()];
        let mut aperture_v = vec![0.0_f32; grid.v_faces()];
        let predictor_u = vec![0.0_f32; grid.u_faces()];
        let mut predictor_v = vec![0.0_f32; grid.v_faces()];
        for y in 0..height {
            for x in 1..width {
                aperture_u[y * (width + 1) + x] = 1.0;
            }
        }
        for y in 1..height {
            for x in 0..width {
                let face = y * width + x;
                aperture_v[face] = 1.0;
                predictor_v[face] = dt_s * gravity_m_s2;
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
        let config = PressureConfig {
            dt_s,
            gravity_m_s2,
            ..PressureConfig::default()
        };
        let exhausted = projector
            .project_closed(
                &context.device,
                &context.queue,
                fields,
                PressureConfig {
                    max_iterations: 0,
                    ..config
                },
            )
            .await
            .unwrap_err();
        assert!(
            exhausted
                .to_string()
                .contains("exhausted its bounded iterations"),
            "horizontal hydrostatic gradients need a dynamic correction: {exhausted}"
        );
        let diagnostics = projector
            .project_closed(&context.device, &context.queue, fields, config)
            .await
            .unwrap();
        eprintln!("horizontal hydrostatic GPU pressure: {diagnostics:?}");
        assert!(diagnostics.iterations > 0, "{diagnostics:?}");
        assert!(diagnostics.scaled_residual <= 1e-5, "{diagnostics:?}");
        assert!(diagnostics.scaled_divergence <= 1e-5, "{diagnostics:?}");
        let mut encoder = context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("pressure test status readback"),
            });
        let staging = stage_status_readback(&context.device, &mut encoder, &projector.status);
        context.queue.submit([encoder.finish()]);
        let status = read_status(&context.device, staging).await.unwrap();
        assert_eq!(status.invalid, 0, "{status:?}");
        assert_eq!(status.converged, 1, "{status:?}");
        let corrected_u = read_f32(&context.device, &context.queue, &corrected_u).await;
        let corrected_v = read_f32(&context.device, &context.queue, &corrected_v).await;
        assert!(corrected_u.iter().all(|face| face.is_finite()));
        assert!(corrected_v.iter().all(|face| face.is_finite()));
        context.dispose();
    });
}
