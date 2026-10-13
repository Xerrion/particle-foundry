//! Small dynamic corrections must survive repeated f32 accumulation.

use super::*;

#[test]
#[ignore = "requires a native compute adapter"]
fn four_sub_ulp_updates_preserve_their_accumulated_pressure() {
    block_on(async {
        let context = crate::GpuContext::new().await.unwrap();
        let grid = Grid::new(2.0, 1.0).unwrap();
        let projector = GpuPressureProjector::new(&context.device, grid).unwrap();
        for method in [PressureMethod::Reference, PressureMethod::Compensated] {
            let mut params = Vec::new();
            for value in [2_u32, 1, 2, 1] {
                params.extend_from_slice(&value.to_le_bytes());
            }
            for value in [
                0.01_f32,
                1.0 / 60.0,
                1e-5,
                1e-5,
                0.0,
                0.0,
                0.0,
                method as u32 as f32,
            ] {
                params.extend_from_slice(&value.to_le_bytes());
            }
            let uniform = context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("compensated pressure regression parameters"),
                    contents: &params,
                    usage: wgpu::BufferUsages::UNIFORM,
                });
            let bytes = [1.0_f32, 1.0, 0.0, 0.0]
                .iter()
                .flat_map(|value| value.to_le_bytes())
                .collect::<Vec<_>>();
            let pressure = context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("compensated pressure regression field"),
                    contents: &bytes,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                });
            let residual = initial_buffer(&context.device, "residual", &[0.0; 2]);
            let search = initial_buffer(
                &context.device,
                "quarter ulp pressure",
                &[0.0, f32::EPSILON * 0.25],
            );
            let applied = initial_buffer(&context.device, "applied search", &[0.0; 2]);
            let status = initial_buffer(
                &context.device,
                "status",
                &[0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            );
            let bindings = bind(
                &context.device,
                &projector.pipelines.update_solution,
                &[
                    (0, &uniform),
                    (5, &pressure),
                    (6, &residual),
                    (8, &search),
                    (9, &applied),
                    (11, &status),
                ],
            );
            let mut encoder = context.device.create_command_encoder(&Default::default());
            for _ in 0..4 {
                dispatch(
                    &mut encoder,
                    &projector.pipelines.update_solution,
                    &bindings,
                    1,
                );
            }
            context.queue.submit([encoder.finish()]);
            let values = read_f32(&context.device, &context.queue, &pressure).await;
            eprintln!("sub-ulp accumulation {method:?}: {values:?}");
            let expected = match method {
                PressureMethod::Reference => 1.0,
                PressureMethod::Compensated => 1.0 + f32::EPSILON,
                PressureMethod::Coarse => unreachable!(),
            };
            assert_eq!(values[0], 1.0);
            assert_eq!(values[1], expected, "{method:?}: {values:?}");
            assert_eq!(values[2], 0.0);
            assert_eq!(values[3], 0.0);
        }
        context.dispose();
    });
}
