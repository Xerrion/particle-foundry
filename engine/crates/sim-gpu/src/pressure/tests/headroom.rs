use super::*;

#[test]
#[ignore = "requires a native compute adapter"]
fn pcg_stop_reserves_headroom_without_changing_final_gates() {
    block_on(async {
        let context = crate::GpuContext::new().await.unwrap();
        let mut params = Vec::with_capacity(PARAM_BYTES as usize);
        for value in [1_u32, 1, 1, 1] {
            params.extend_from_slice(&value.to_le_bytes());
        }
        for value in [0.01_f32, 1.0, 1e-5, 1e-5, 0.0, 0.0, 0.0, 0.0] {
            params.extend_from_slice(&value.to_le_bytes());
        }
        let uniform = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("pressure headroom test parameters"),
                contents: &params,
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let partials = context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pressure headroom test partial"),
            size: 16,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let status_buffer = context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pressure headroom test status"),
            size: STATUS_BYTES,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        context
            .queue
            .write_buffer(&status_buffer, 0, &[0; STATUS_BYTES as usize]);
        let finish_shader = shader(
            &context.device,
            "pressure headroom test finish",
            include_str!("../../../shaders/pressure_finish.wgsl"),
        );
        let initial = pipeline(&context.device, &finish_shader, "initial");
        let final_residual = pipeline(&context.device, &finish_shader, "final_residual");
        let completion = pipeline(&context.device, &finish_shader, "completion");
        let stage_bind = |pipeline: &wgpu::ComputePipeline| {
            bind(
                &context.device,
                pipeline,
                &[(0, &uniform), (1, &partials), (2, &status_buffer)],
            )
        };
        let initial_bind = stage_bind(&initial);
        let final_bind = stage_bind(&final_residual);
        let completion_bind = stage_bind(&completion);
        let write_partial = |maximum: f32| {
            let mut bytes = [0_u8; 16];
            bytes[..4].copy_from_slice(&1.0_f32.to_le_bytes());
            bytes[4..8].copy_from_slice(&maximum.to_le_bytes());
            context.queue.write_buffer(&partials, 0, &bytes);
        };

        write_partial(6.5e-6);
        let mut encoder = context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("pressure headroom internal stop"),
            });
        dispatch(&mut encoder, &initial, &initial_bind, 1);
        let staging = stage_status_readback(&context.device, &mut encoder, &status_buffer);
        context.queue.submit([encoder.finish()]);
        let status = read_status(&context.device, staging).await.unwrap();
        assert_eq!(status.converged, 0, "{status:?}");
        assert!(status.scaled_residual < 1e-5, "{status:?}");

        write_partial(4.5e-6);
        let mut encoder = context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("pressure headroom converged stop"),
            });
        dispatch(&mut encoder, &initial, &initial_bind, 1);
        context.queue.submit([encoder.finish()]);

        write_partial(9.5e-6);
        let mut encoder = context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("pressure headroom final gates"),
            });
        dispatch(&mut encoder, &final_residual, &final_bind, 1);
        dispatch(&mut encoder, &completion, &completion_bind, 1);
        let staging = stage_status_readback(&context.device, &mut encoder, &status_buffer);
        context.queue.submit([encoder.finish()]);
        let status = read_status(&context.device, staging).await.unwrap();
        assert_eq!(status.converged, 1, "{status:?}");
        assert!(status.scaled_residual > 9e-6, "{status:?}");
        assert!(status.scaled_divergence > 9e-6, "{status:?}");

        write_partial(1.0007e-5);
        let mut encoder = context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("pressure headroom rejected divergence"),
            });
        dispatch(&mut encoder, &completion, &completion_bind, 1);
        let staging = stage_status_readback(&context.device, &mut encoder, &status_buffer);
        context.queue.submit([encoder.finish()]);
        let status = read_status(&context.device, staging).await.unwrap();
        assert_eq!(status.converged, 0, "{status:?}");
        assert!(status.scaled_divergence > 1e-5, "{status:?}");
        context.dispose();
    });
}
