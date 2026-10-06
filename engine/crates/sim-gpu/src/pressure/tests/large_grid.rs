use super::*;

#[test]
#[ignore = "requires a native compute adapter"]
fn smooth_480x270_closed_predictor_meets_both_gates() {
    block_on(async {
        let context = crate::GpuContext::new().await.unwrap();
        let grid = Grid::new(480.0, 270.0).unwrap();
        let width = grid.width() as usize;
        let height = grid.height() as usize;
        let density = vec![1.2_f32; grid.cells()];
        let mut aperture_u = vec![0.0_f32; grid.u_faces()];
        let mut aperture_v = vec![0.0_f32; grid.v_faces()];
        let mut predictor_u = vec![0.0_f32; grid.u_faces()];
        for y in 0..height {
            for x in 1..width {
                let face = y * (width + 1) + x;
                aperture_u[face] = 1.0;
                predictor_u[face] = 0.1 * (std::f32::consts::PI * x as f32 / width as f32).sin();
            }
        }
        for y in 1..height {
            for x in 0..width {
                aperture_v[y * width + x] = 1.0;
            }
        }
        let density = initial_buffer(&context.device, "density", &density);
        let root_cell = root_buffer(&context.device, &vec![0; grid.cells()]);
        let aperture_u = initial_buffer(&context.device, "u aperture", &aperture_u);
        let aperture_v = initial_buffer(&context.device, "v aperture", &aperture_v);
        let predictor_u = initial_buffer(&context.device, "u predictor", &predictor_u);
        let predictor_v =
            initial_buffer(&context.device, "v predictor", &vec![0.0; grid.v_faces()]);
        let corrected_u = candidate_buffer(&context.device, "u candidate", grid.u_faces());
        let corrected_v = candidate_buffer(&context.device, "v candidate", grid.v_faces());
        let mut projector = GpuPressureProjector::new(&context.device, grid).unwrap();
        let start = Instant::now();
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
                PressureConfig {
                    max_iterations: 1024,
                    ..PressureConfig::default()
                },
            )
            .await;
        eprintln!(
            "480x270 sealed GPU pressure: {result:?}, elapsed {:?}",
            start.elapsed()
        );
        let diagnostics = result.unwrap();
        assert!(diagnostics.iterations <= 1024, "{diagnostics:?}");
        assert!(diagnostics.scaled_residual <= 1e-5, "{diagnostics:?}");
        assert!(diagnostics.scaled_divergence <= 1e-5, "{diagnostics:?}");
        context.dispose();
    });
}
