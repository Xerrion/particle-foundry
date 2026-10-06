//! Accepted pressure guesses must survive rejection and use current fields.

use super::*;

#[test]
#[ignore = "requires a native compute adapter"]
fn accepted_guess_uses_new_density_and_predictor_after_rejected_probe() {
    block_on(async {
        let context = crate::GpuContext::new().await.expect("native GPU device");
        let grid = Grid::new(2.0, 2.0).unwrap();
        let config = PressureConfig::default();
        let density = mutable_buffer(&context.device, "warm-start density", &[1.2; 4]);
        let root_cell = root_buffer(&context.device, &[0; 4]);
        let aperture_u = initial_buffer(
            &context.device,
            "warm-start u aperture",
            &[0.0, 1.0, 0.0, 0.0, 1.0, 0.0],
        );
        let aperture_v = initial_buffer(
            &context.device,
            "warm-start v aperture",
            &[0.0, 0.0, 1.0, 1.0, 0.0, 0.0],
        );
        let predictor_u = mutable_buffer(
            &context.device,
            "warm-start u predictor",
            &[0.0, 0.2, 0.0, 0.0, -0.1, 0.0],
        );
        let predictor_v = mutable_buffer(
            &context.device,
            "warm-start v predictor",
            &[0.0, 0.0, -0.05, 0.15, 0.0, 0.0],
        );
        let corrected_u = candidate_buffer(&context.device, "warm-start corrected u", 6);
        let corrected_v = candidate_buffer(&context.device, "warm-start corrected v", 6);
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
        let first = projector
            .project_closed(&context.device, &context.queue, fields, config)
            .await
            .expect("initial pressure solve");
        assert_current_gates(first, config);
        assert!(
            first.iterations > 0,
            "the initial fixture needs a pressure correction"
        );
        assert!(!projector.has_accepted_guess);
        let pressure = read_f32(&context.device, &context.queue, &projector.pressure).await;
        assert!(pressure.iter().all(|value| value.is_finite()));
        assert!(pressure.iter().any(|value| value.abs() > 1e-5));
        let first_u = read_f32(&context.device, &context.queue, &corrected_u).await;
        let first_v = read_f32(&context.device, &context.queue, &corrected_v).await;
        let mut encoder = context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("test accepted pressure promotion"),
            });
        projector.encode_accepted_guess(&mut encoder);
        context.queue.submit([encoder.finish()]);
        assert!(projector.has_accepted_guess);
        let accepted_bits = cache_bits(&context, &projector).await;
        assert_eq!(
            accepted_bits,
            pressure
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>()
        );

        // The same pressure is an exact solution for these new coefficients.
        // Add a different divergence-free loop so the corrected faces also
        // prove that the predictor changed. A cold zero-budget solve fails,
        // but a retry using the preserved guess needs no PCG iterations.
        let next_density = [2.4, 8.0, 3.6, 5.1];
        let loop_speed = 0.31;
        let (next_u, next_v) =
            predictor_from_pressure(&pressure, next_density, config.dt_s, loop_speed);
        context
            .queue
            .write_buffer(&density, 0, &f32_bytes(&next_density));
        context
            .queue
            .write_buffer(&predictor_u, 0, &f32_bytes(&next_u));
        context
            .queue
            .write_buffer(&predictor_v, 0, &f32_bytes(&next_v));
        let rejected = projector
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
            .expect_err("changed predictor requires a nonzero pressure guess");
        assert!(rejected.is_retryable(), "{rejected}");
        assert!(
            rejected
                .to_string()
                .contains("exhausted its bounded iterations")
        );
        assert!(projector.has_accepted_guess);
        assert_eq!(
            cache_bits(&context, &projector).await,
            accepted_bits,
            "rejected cold probe changed the accepted cache bytes"
        );
        assert!(
            read_f32(&context.device, &context.queue, &projector.pressure)
                .await
                .iter()
                .all(|&value| value == 0.0),
            "zero-budget probe must keep its cold-start semantics"
        );

        let retried = projector
            .project_closed(&context.device, &context.queue, fields, config)
            .await
            .expect("retry from preserved accepted pressure");
        assert_current_gates(retried, config);
        assert_eq!(
            retried.iterations, 0,
            "retry did not reuse the preserved exact pressure guess"
        );
        assert_eq!(
            cache_bits(&context, &projector).await,
            accepted_bits,
            "an unpromoted candidate changed the accepted cache bytes"
        );
        let next_corrected_u = read_f32(&context.device, &context.queue, &corrected_u).await;
        let next_corrected_v = read_f32(&context.device, &context.queue, &corrected_v).await;
        assert_faces(
            &next_corrected_u,
            &[0.0, loop_speed, 0.0, 0.0, -loop_speed, 0.0],
        );
        assert_faces(
            &next_corrected_v,
            &[0.0, 0.0, -loop_speed, loop_speed, 0.0, 0.0],
        );
        assert!(
            first_u
                .iter()
                .zip(&next_corrected_u)
                .chain(first_v.iter().zip(&next_corrected_v))
                .any(|(&before, &after)| (after - before).abs() > 0.1),
            "corrected faces did not respond to the new fields"
        );
        eprintln!(
            "warm-start current fields: first={first:?}, rejected={rejected}, retried={retried:?}, accepted_cache_bytes=preserved"
        );
        context.dispose();
    });
}

fn mutable_buffer(device: &wgpu::Device, label: &str, values: &[f32]) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: &f32_bytes(values),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    })
}

fn f32_bytes(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

async fn cache_bits(context: &crate::GpuContext, projector: &GpuPressureProjector) -> Vec<u32> {
    read_f32(&context.device, &context.queue, &projector.accepted_guess)
        .await
        .into_iter()
        .map(f32::to_bits)
        .collect()
}

fn assert_current_gates(diagnostics: PressureDiagnostics, config: PressureConfig) {
    assert!(
        diagnostics.scaled_residual <= config.scaled_residual_tolerance,
        "{diagnostics:?}"
    );
    assert!(
        diagnostics.scaled_divergence <= config.scaled_divergence_tolerance,
        "{diagnostics:?}"
    );
}

fn predictor_from_pressure(
    pressure: &[f32],
    density: [f32; 4],
    dt_s: f32,
    loop_speed: f32,
) -> ([f32; 6], [f32; 6]) {
    let mut u = [0.0; 6];
    let mut v = [0.0; 6];
    for (face, left, right, circulation) in [(1, 0, 1, loop_speed), (4, 2, 3, -loop_speed)] {
        let inverse_density = 1.0 / (0.5 * density[left] + 0.5 * density[right]);
        u[face] =
            dt_s * inverse_density * ((pressure[right] - pressure[left]) / CELL_WIDTH_M as f32)
                + circulation;
    }
    for (face, top, bottom, circulation) in [(2, 0, 2, -loop_speed), (3, 1, 3, loop_speed)] {
        let inverse_density = 1.0 / (0.5 * density[top] + 0.5 * density[bottom]);
        v[face] =
            dt_s * inverse_density * ((pressure[bottom] - pressure[top]) / CELL_WIDTH_M as f32)
                + circulation;
    }
    (u, v)
}

fn assert_faces(actual: &[f32], expected: &[f32]) {
    for (face, (&actual, &expected)) in actual.iter().zip(expected).enumerate() {
        assert!(
            (actual - expected).abs() <= 1e-5,
            "corrected face {face}: {actual} instead of {expected}"
        );
    }
}
