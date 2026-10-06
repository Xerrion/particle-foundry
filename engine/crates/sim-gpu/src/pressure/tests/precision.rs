use super::*;

#[test]
#[ignore = "requires a native compute adapter"]
fn sub_ulp_dynamic_pressure_survives_residual_and_face_correction() {
    block_on(async {
        let context = crate::GpuContext::new().await.unwrap();
        let grid = Grid::new(2.0, 1.0).unwrap();
        let dx_m = CELL_WIDTH_M as f32;
        let dt_s = 1.0 / 60.0_f32;
        let density_value = 1.2_f32;
        let base_value = 28.243_122_f32;
        let dynamic_step = 0.5e-6_f32;
        assert_eq!(base_value + dynamic_step, base_value);

        let mut params = Vec::with_capacity(PARAM_BYTES as usize);
        for value in [grid.width(), grid.height(), grid.cells() as u32, 1] {
            params.extend_from_slice(&value.to_le_bytes());
        }
        for value in [dx_m, dt_s, 1e-8, 1e-8, 0.0, 0.0, 0.0, 0.0] {
            params.extend_from_slice(&value.to_le_bytes());
        }
        let uniform = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("split pressure test parameters"),
                contents: &params,
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let density = initial_buffer(&context.device, "density", &[density_value; 2]);
        let aperture_u = initial_buffer(&context.device, "u aperture", &[0.0, 1.0, 0.0]);
        let aperture_v = initial_buffer(&context.device, "v aperture", &[0.0; 4]);
        let base = initial_buffer(&context.device, "hydrostatic base", &[base_value; 2]);
        let dynamic = initial_buffer(&context.device, "dynamic pressure", &[0.0, dynamic_step]);
        let edge_weight = 1.0 / density_value / (dx_m * dx_m);
        let rhs = initial_buffer(
            &context.device,
            "rhs",
            &[-edge_weight * dynamic_step, edge_weight * dynamic_step],
        );
        let residual = initial_buffer(&context.device, "recursive residual", &[0.0; 2]);
        let partials = candidate_buffer(&context.device, "true residual partial", 4);
        let predictor = initial_buffer(
            &context.device,
            "u predictor",
            &[0.0, dt_s * dynamic_step / (density_value * dx_m), 0.0],
        );
        let corrected = candidate_buffer(&context.device, "u corrected", grid.u_faces());
        let pcg_shader = shader(
            &context.device,
            "split pressure PCG regression",
            include_str!("../../../shaders/pressure_pcg.wgsl"),
        );
        let reduction = pipeline(&context.device, &pcg_shader, "reduce_updated");
        let reduction_bind = bind(
            &context.device,
            &reduction,
            &[
                (0, &uniform),
                (1, &density),
                (2, &aperture_u),
                (3, &aperture_v),
                (4, &rhs),
                (5, &dynamic),
                (6, &residual),
                (10, &partials),
                (12, &base),
            ],
        );
        let correction_shader = shader(
            &context.device,
            "split pressure face regression",
            include_str!("../../../shaders/pressure_correct.wgsl"),
        );
        let correction = pipeline(&context.device, &correction_shader, "u");
        let correction_bind = bind(
            &context.device,
            &correction,
            &[
                (0, &uniform),
                (1, &density),
                (2, &aperture_u),
                (4, &predictor),
                (6, &dynamic),
                (7, &corrected),
                (9, &base),
            ],
        );
        let mut encoder = context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("split pressure arithmetic regression"),
            });
        dispatch(&mut encoder, &reduction, &reduction_bind, 1);
        dispatch(&mut encoder, &correction, &correction_bind, 1);
        context.queue.submit([encoder.finish()]);

        let reduced = read_f32(&context.device, &context.queue, &partials).await;
        let corrected = read_f32(&context.device, &context.queue, &corrected).await;
        assert!(reduced[1] * dt_s * dt_s <= 1e-8, "{reduced:?}");
        assert!(corrected[1].abs() <= 1e-8, "{corrected:?}");
        context.dispose();
    });
}

#[test]
#[ignore = "requires a native compute adapter"]
fn mapped_prior_status_staging_cannot_poison_reused_projector() {
    block_on(async {
        let context = crate::GpuContext::new().await.unwrap();
        let grid = Grid::new(1.0, 1.0).unwrap();
        let density = initial_buffer(&context.device, "density", &[1.2]);
        let root_cell = root_buffer(&context.device, &[0]);
        let aperture_u = initial_buffer(&context.device, "u aperture", &[0.0; 2]);
        let aperture_v = initial_buffer(&context.device, "v aperture", &[0.0; 2]);
        let predictor_u = initial_buffer(&context.device, "u predictor", &[0.0; 2]);
        let predictor_v = initial_buffer(&context.device, "v predictor", &[0.0; 2]);
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

        // Native map polling completes synchronously. Keep an older status
        // staging buffer mapped to model the state left by a canceled map.
        let mut encoder = context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("abandoned pressure status readback"),
            });
        let abandoned = stage_status_readback(&context.device, &mut encoder, &projector.status);
        context.queue.submit([encoder.finish()]);
        wait_for_map(&context.device, &abandoned).await.unwrap();
        let stale_map = abandoned.get_mapped_range(..);

        for _ in 0..2 {
            let result = projector
                .project_closed(
                    &context.device,
                    &context.queue,
                    fields,
                    PressureConfig {
                        max_iterations: 0,
                        ..PressureConfig::default()
                    },
                )
                .await
                .unwrap();
            assert_eq!(result.iterations, 0);
            assert_eq!(result.scaled_residual, 0.0);
            assert_eq!(result.scaled_divergence, 0.0);
        }
        drop(stale_map);
        abandoned.unmap();
        context.dispose();
    });
}
