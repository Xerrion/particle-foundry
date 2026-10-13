//! Coarse corrections must preserve symmetry and component isolation.

use super::*;

async fn apply_cycle(
    context: &crate::GpuContext,
    grid: Grid,
    fields: PressureFields<'_>,
    values: &[f32],
) -> Vec<f32> {
    let residual = initial_buffer(&context.device, "coarse test residual", values);
    let output = candidate_buffer(&context.device, "coarse test correction", grid.cells());
    let status = initial_buffer(&context.device, "coarse test active status", &[0.0; 8]);
    let owner = super::super::coarse::CoarsePressure::new(
        &context.device,
        grid,
        &status,
        &residual,
        &output,
    )
    .unwrap();
    let mut bytes = Vec::new();
    for value in [
        grid.width(),
        grid.height(),
        grid.cells() as u32,
        (grid.cells() as u32).div_ceil(WORKGROUP_SIZE),
    ] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for value in [0.01_f32, 1.0 / 60.0, 1e-5, 1e-5, 0.0, 0.0, 0.0, 2.0] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    let uniform = context
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("coarse test fine geometry"),
            contents: &bytes,
            usage: wgpu::BufferUsages::UNIFORM,
        });
    let binding = owner.bind_fields(&context.device, fields, &uniform);
    let mut encoder = context.device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        owner.prepare(&mut pass, &binding);
        owner.apply(&mut pass);
    }
    context.queue.submit([encoder.finish()]);
    read_f32(&context.device, &context.queue, &output).await
}

#[test]
#[ignore = "requires a native compute adapter"]
fn partial_aggregates_keep_symmetry_positive_work_and_cross_wall_isolation() {
    block_on(async {
        let context = crate::GpuContext::new().await.unwrap();
        let grid = Grid::new(9.0, 5.0).unwrap();
        let mut u = vec![0.0; grid.u_faces()];
        let mut v = vec![0.0; grid.v_faces()];
        let mut roots = vec![0; grid.cells()];
        let mut density = vec![1.2; grid.cells()];
        let mut first = vec![0.0; grid.cells()];
        let mut second = vec![0.0; grid.cells()];
        for y in 0..5 {
            for x in 0..9 {
                let cell = y * 9 + x;
                roots[cell] = if x < 4 {
                    0
                } else if x > 4 {
                    5
                } else {
                    cell as u32
                };
                density[cell] = if x > 4 || y > 2 { 1000.0 } else { 1.2 };
                if x != 4 {
                    first[cell] = (cell % 7) as f32 - 3.0;
                    if x < 4 {
                        second[cell] = (cell % 5) as f32 - 2.0;
                    }
                }
                if x > 0 && x != 4 && x != 5 {
                    u[y * 10 + x] = 1.0;
                }
                if y > 0 && x != 4 {
                    v[y * 9 + x] = 1.0;
                }
            }
        }
        for (columns, last) in [(0..4, 39), (5..9, 44)] {
            for values in [&mut first, &mut second] {
                values[last] = 0.0;
                let sum: f64 = (0..5)
                    .flat_map(|y| columns.clone().map(move |x| y * 9 + x))
                    .map(|cell| f64::from(values[cell]))
                    .sum();
                values[last] = -sum as f32;
            }
        }
        let density = initial_buffer(&context.device, "coarse test density", &density);
        let roots = root_buffer(&context.device, &roots);
        let u = initial_buffer(&context.device, "coarse test u aperture", &u);
        let v = initial_buffer(&context.device, "coarse test v aperture", &v);
        let predictor_u =
            candidate_buffer(&context.device, "coarse test u predictor", grid.u_faces());
        let predictor_v =
            candidate_buffer(&context.device, "coarse test v predictor", grid.v_faces());
        let corrected_u = candidate_buffer(&context.device, "coarse test u output", grid.u_faces());
        let corrected_v = candidate_buffer(&context.device, "coarse test v output", grid.v_faces());
        let fields = PressureFields {
            density: &density,
            root_cell: &roots,
            aperture_u: &u,
            aperture_v: &v,
            predictor_u: &predictor_u,
            predictor_v: &predictor_v,
            corrected_u: &corrected_u,
            corrected_v: &corrected_v,
        };
        let first_z = apply_cycle(&context, grid, fields, &first).await;
        let second_z = apply_cycle(&context, grid, fields, &second).await;
        let zero = apply_cycle(&context, grid, fields, &vec![0.0; grid.cells()]).await;
        let dot = |left: &[f32], right: &[f32]| {
            left.iter()
                .zip(right)
                .map(|(a, b)| f64::from(*a) * f64::from(*b))
                .sum::<f64>()
        };
        let forward = dot(&first, &second_z);
        let transpose = dot(&second, &first_z);
        let scale = forward.abs().max(transpose.abs()).max(1e-12);
        assert!(
            (forward - transpose).abs() <= 1e-3 * scale,
            "coarse symmetry: forward={forward}, transpose={transpose}"
        );
        assert!(dot(&first, &first_z) > 0.0);
        assert!(
            first_z
                .iter()
                .chain(&second_z)
                .all(|value| value.is_finite())
        );
        assert!(zero.iter().all(|value| *value == 0.0));
        for y in 0..5 {
            for x in 4..9 {
                assert_eq!(second_z[y * 9 + x], 0.0, "cross-wall correction");
            }
            assert_eq!(first_z[y * 9 + 4], 0.0, "isolated wall correction");
        }
        context.dispose();
    });
}
