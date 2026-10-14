use super::{tests::*, *};
use crate::{GpuContext, scene::GpuSceneSeed};

fn open_seed() -> GpuSceneSeed {
    let grid = Grid::new(64.0, 32.0).unwrap();
    let cells = grid.cells();
    let mut u_aperture = vec![1.0; grid.u_faces()];
    for y in 0..grid.height() as usize {
        u_aperture[y * 65] = 0.0;
        u_aperture[y * 65 + 64] = 0.0;
    }
    let mut v_aperture = vec![1.0; grid.v_faces()];
    v_aperture[..64].fill(0.0);
    v_aperture[32 * 64..].fill(0.0);
    let mut u_velocity_m_s = vec![0.0; grid.u_faces()];
    u_velocity_m_s[2] = 0.01;
    GpuSceneSeed {
        grid,
        phase_density_kg_m3: [1000.0, 1.2],
        mass_kg: vec![0.0; cells]
            .into_iter()
            .chain(vec![1.2 * grid.cell_volume_m3(); cells])
            .collect(),
        marker: vec![0.0; cells]
            .into_iter()
            .chain(vec![0.05; cells])
            .collect(),
        energy_j: vec![0.0; cells],
        fixed_wall: vec![0; cells],
        u_velocity_m_s,
        v_velocity_m_s: vec![0.0; grid.v_faces()],
        u_aperture,
        v_aperture,
        epoch: 2,
        tick: 7,
        accepted_time_s: 7.0 / 60.0,
    }
}

fn fields(scene: &GpuScene) -> Vec<Vec<f32>> {
    let committed = scene.committed();
    [
        &committed.mass_kg,
        &committed.marker,
        &committed.density_kg_m3,
        &committed.u_velocity_m_s,
        &committed.v_velocity_m_s,
    ]
    .map(|field| read_f32(scene, field))
    .to_vec()
}

fn stamp(scene: &GpuScene) -> (u64, u64, f64, usize, u64) {
    (
        scene.epoch,
        scene.tick,
        scene.accepted_time_s,
        scene.committed_index,
        scene.state_revision,
    )
}

#[test]
fn stroke_command_validates_both_endpoints_and_radius() {
    let grid = Grid::new(64.0, 32.0).unwrap();
    for (from, to, radius) in [
        ([64, 1], [1, 1], 4),
        ([1, 32], [1, 1], 4),
        ([1, 1], [64, 1], 4),
        ([1, 1], [1, 32], 4),
        ([1, 1], [2, 2], 17),
    ] {
        assert!(
            PaintCommand::stroke(
                grid,
                [1000.0, 1.2],
                from,
                to,
                radius,
                GpuPaintMaterial::Water
            )
            .is_err()
        );
    }
    assert!(
        PaintCommand::stroke(
            grid,
            [1000.0, 1.2],
            [0, 0],
            [63, 31],
            16,
            GpuPaintMaterial::Air
        )
        .is_ok()
    );
}

#[test]
#[ignore = "requires a native GPU compute adapter"]
fn resident_stroke_fills_horizontal_span_without_stamp_scallops() {
    block_on(async {
        let context = GpuContext::new().await.unwrap();
        let mut scene = GpuScene::new(&context, open_seed()).unwrap();
        let stage = GpuPaintStage::new(&context.device, scene.grid).unwrap();
        let original = fields(&scene);
        stage
            .apply_stroke(&mut scene, [10, 10], [54, 10], 4, GpuPaintMaterial::Water)
            .unwrap();
        let painted = fields(&scene);
        let cells = scene.grid.cells();
        let water_mass = 1000.0 * scene.grid.cell_volume_m3() as f32;
        for y in 0..32_i32 {
            for x in 0..64_i32 {
                let cell = (y * 64 + x) as usize;
                let nearest_x = x.clamp(10, 54);
                let selected = (x - nearest_x).pow(2) + (y - 10).pow(2) <= 16;
                for phase in 0..2 {
                    let index = phase * cells + cell;
                    assert_eq!(
                        painted[0][index],
                        if selected {
                            if phase == 0 { water_mass } else { 0.0 }
                        } else {
                            original[0][index]
                        },
                        "mass at ({x}, {y}), phase {phase}"
                    );
                    assert_eq!(
                        painted[1][index],
                        if selected { 0.0 } else { original[1][index] }
                    );
                }
                assert_eq!(
                    painted[2][cell],
                    if selected {
                        water_mass / scene.grid.cell_volume_m3() as f32
                    } else {
                        original[2][cell]
                    }
                );
            }
        }
        assert_eq!(painted[3..], original[3..]);
        assert_eq!(stamp(&scene), (2, 7, 7.0 / 60.0, 1, 1));
    });
}

#[test]
#[ignore = "requires a native GPU compute adapter"]
fn resident_stroke_matches_diagonal_capsule_and_reverse_direction() {
    block_on(async {
        let context = GpuContext::new().await.unwrap();
        let mut forward = GpuScene::new(&context, open_seed()).unwrap();
        let mut reverse = GpuScene::new(&context, open_seed()).unwrap();
        let stage = GpuPaintStage::new(&context.device, forward.grid).unwrap();
        stage
            .apply_stroke(&mut forward, [8, 5], [49, 25], 3, GpuPaintMaterial::Water)
            .unwrap();
        stage
            .apply_stroke(&mut reverse, [49, 25], [8, 5], 3, GpuPaintMaterial::Water)
            .unwrap();
        let painted = fields(&forward);
        assert_eq!(painted, fields(&reverse));
        for y in 0..32 {
            for x in 0..64 {
                let dx = f64::from(x) - 8.0;
                let dy = f64::from(y) - 5.0;
                let t = ((dx * 41.0 + dy * 20.0) / (41.0_f64.powi(2) + 20.0_f64.powi(2)))
                    .clamp(0.0, 1.0);
                let distance_squared = (dx - t * 41.0).powi(2) + (dy - t * 20.0).powi(2);
                assert_eq!(
                    painted[0][y as usize * 64 + x as usize] > 0.0,
                    distance_squared <= 9.0,
                    "capsule at ({x}, {y})"
                );
            }
        }
    });
}

#[test]
#[ignore = "requires a native GPU compute adapter"]
fn resident_stroke_degenerate_segment_matches_point_paint() {
    block_on(async {
        let context = GpuContext::new().await.unwrap();
        for radius in [0, 4, 16] {
            let mut point = GpuScene::new(&context, open_seed()).unwrap();
            let mut stroke = GpuScene::new(&context, open_seed()).unwrap();
            let stage = GpuPaintStage::new(&context.device, point.grid).unwrap();
            stage
                .apply(&mut point, 20, 12, radius, GpuPaintMaterial::Water)
                .unwrap();
            stage
                .apply_stroke(
                    &mut stroke,
                    [20, 12],
                    [20, 12],
                    radius,
                    GpuPaintMaterial::Water,
                )
                .unwrap();
            assert_eq!(fields(&point), fields(&stroke));
            assert_eq!(stamp(&point), stamp(&stroke));
        }
    });
}

#[test]
#[ignore = "requires a native GPU compute adapter"]
fn resident_zero_radius_stroke_is_connected_and_direction_independent() {
    block_on(async {
        let context = GpuContext::new().await.unwrap();
        for (from, to, expected) in [
            (
                [0, 0],
                [3, 2],
                vec![[0, 0], [1, 0], [1, 1], [2, 1], [2, 2], [3, 2]],
            ),
            (
                [0, 2],
                [3, 0],
                vec![[0, 2], [1, 2], [1, 1], [2, 1], [2, 0], [3, 0]],
            ),
            ([0, 0], [3, 0], vec![[0, 0], [1, 0], [2, 0], [3, 0]]),
            ([0, 0], [0, 3], vec![[0, 0], [0, 1], [0, 2], [0, 3]]),
            ([0, 0], [3, 3], vec![[0, 0], [1, 1], [2, 2], [3, 3]]),
        ] {
            let mut forward = GpuScene::new(&context, open_seed()).unwrap();
            let mut reverse = GpuScene::new(&context, open_seed()).unwrap();
            let stage = GpuPaintStage::new(&context.device, forward.grid).unwrap();
            stage
                .apply_stroke(&mut forward, from, to, 0, GpuPaintMaterial::Water)
                .unwrap();
            stage
                .apply_stroke(&mut reverse, to, from, 0, GpuPaintMaterial::Water)
                .unwrap();
            let painted = fields(&forward);
            assert_eq!(painted, fields(&reverse));
            for y in 0..32 {
                for x in 0..64 {
                    assert_eq!(
                        painted[0][y as usize * 64 + x as usize] > 0.0,
                        expected.contains(&[x, y]),
                        "thin stroke from {from:?} to {to:?} at ({x}, {y})"
                    );
                }
            }
        }
        let mut walled = GpuScene::new(&context, seed()).unwrap();
        let stage = GpuPaintStage::new(&context.device, walled.grid).unwrap();
        stage
            .apply_stroke(&mut walled, [0, 0], [3, 0], 0, GpuPaintMaterial::Water)
            .unwrap();
        let painted = fields(&walled);
        for (cell, &mass) in painted[0][..walled.grid.cells()].iter().enumerate() {
            assert_eq!(mass > 0.0, [1, 2].contains(&cell));
        }
    });
}

#[test]
#[ignore = "requires a native GPU compute adapter"]
fn resident_stroke_preserves_walls_and_rejects_invalid_edits_atomically() {
    block_on(async {
        let context = GpuContext::new().await.unwrap();
        let mut scene = GpuScene::new(&context, seed()).unwrap();
        let stage = GpuPaintStage::new(&context.device, scene.grid).unwrap();
        let original = fields(&scene);
        stage
            .apply_stroke(&mut scene, [0, 0], [3, 0], 1, GpuPaintMaterial::Water)
            .unwrap();
        let painted = fields(&scene);
        for cell in [0, 3, 4, 7, 8, 9, 10, 11] {
            assert_eq!(painted[0][cell], original[0][cell]);
            assert_eq!(painted[0][12 + cell], original[0][12 + cell]);
            assert_eq!(painted[1][cell], original[1][cell]);
            assert_eq!(painted[1][12 + cell], original[1][12 + cell]);
            assert_eq!(painted[2][cell], original[2][cell]);
        }
        for cell in [1, 2, 5, 6] {
            assert!(painted[0][cell] > 0.0);
            assert_eq!(painted[0][12 + cell], 0.0);
        }
        assert_eq!(painted[3..], original[3..]);
        let painted_stamp = stamp(&scene);
        for (from, to, radius) in [
            ([4, 0], [1, 0], 1),
            ([1, 0], [1, 3], 1),
            ([1, 0], [2, 0], 17),
        ] {
            assert!(
                stage
                    .apply_stroke(&mut scene, from, to, radius, GpuPaintMaterial::Air)
                    .is_err()
            );
            assert_eq!(stamp(&scene), painted_stamp);
            assert_eq!(fields(&scene), painted);
        }
        stage
            .apply_stroke(&mut scene, [3, 0], [0, 0], 1, GpuPaintMaterial::Air)
            .unwrap();
        let erased = fields(&scene);
        for (actual, expected) in erased[0].iter().zip(&original[0]) {
            assert!((actual - expected).abs() <= 1e-6 * expected.abs());
        }
        for (actual, expected) in erased[2].iter().zip(&original[2]) {
            assert!((actual - expected).abs() <= 1e-6 * expected.abs());
        }
        assert_eq!(erased[3..], original[3..]);
        assert_eq!(stamp(&scene), (2, 7, 7.0 / 60.0, 0, 2));
    });
}
