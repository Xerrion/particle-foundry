//! One bounded, nonreactive M3 scene at the intended browser grid size.

use particle_sim::Grid;

use super::coupled::GpuScenePhysics;
use super::{GpuSceneSeed, coupled::GpuCoupledScene};
use crate::{GpuContext, pressure::PressureConfig};

const WIDTH: usize = 480;
const HEIGHT: usize = 270;
const BASIN_LEFT_WALL: usize = 0;
const BASIN_RIGHT_WALL: usize = 479;
const BASIN_RIM_Y: usize = 220;
const WATER_SURFACE_Y: usize = 242;
const FLOOR_Y: usize = 255;

/// Creates the persistent, single-owner M3 GPU scene used by the browser path.
pub(crate) fn m3_demo_scene(context: &GpuContext, epoch: u64) -> Result<GpuCoupledScene, String> {
    let seed = m3_demo_seed(epoch);
    GpuCoupledScene::new(
        context,
        seed,
        GpuScenePhysics {
            cell_dynamic_viscosity_pa_s: Some(vec![0.001; WIDTH * HEIGHT]),
            gravity_m_s2: 9.80665,
            pressure: PressureConfig::default(),
        },
    )
}

fn m3_demo_seed(epoch: u64) -> GpuSceneSeed {
    let grid = Grid::new(WIDTH as f64, HEIGHT as f64).expect("fixed M3 scene grid");
    let cells = grid.cells();
    let volume = grid.cell_volume_m3();
    let mut fixed_wall = vec![0_u8; cells];
    let mut mass_kg = vec![0.0; cells * 2];
    let mut marker = vec![0.0; cells * 2];

    // The origin is at the top left. Rows 255..270 form a fixed floor.
    // One-cell walls at x=0 and x=479 rise from row 220 to the floor.
    // Pure water occupies x=1..479 and rows 242..255 inside that basin.
    // Every other nonwall cell is pure carrier air. The exterior is sealed.
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let cell = y * WIDTH + x;
            let wall = y >= FLOOR_Y
                || (y >= BASIN_RIM_Y && (x == BASIN_LEFT_WALL || x == BASIN_RIGHT_WALL));
            if wall {
                fixed_wall[cell] = 1;
            } else if x > BASIN_LEFT_WALL && x < BASIN_RIGHT_WALL && y >= WATER_SURFACE_Y {
                mass_kg[cell] = 1000.0 * volume;
                marker[cell] = 0.25;
            } else {
                mass_kg[cells + cell] = 1.2 * volume;
                marker[cells + cell] = 0.05;
            }
        }
    }

    let mut u_aperture = vec![0.0; grid.u_faces()];
    let mut v_aperture = vec![0.0; grid.v_faces()];
    for y in 0..HEIGHT {
        for x in 1..WIDTH {
            let left = y * WIDTH + x - 1;
            let right = y * WIDTH + x;
            if fixed_wall[left] == 0 && fixed_wall[right] == 0 {
                u_aperture[y * (WIDTH + 1) + x] = 1.0;
            }
        }
    }
    for y in 1..HEIGHT {
        for x in 0..WIDTH {
            let above = (y - 1) * WIDTH + x;
            let below = y * WIDTH + x;
            if fixed_wall[above] == 0 && fixed_wall[below] == 0 {
                v_aperture[y * WIDTH + x] = 1.0;
            }
        }
    }

    let mut u_velocity_m_s = vec![0.0; grid.u_faces()];
    let mut v_velocity_m_s = vec![0.0; grid.v_faces()];
    // A 0.002 m/s divergence-free loop around the interior corner (240, 248).
    // All four faces remain inside the water and away from the fixed walls.
    u_velocity_m_s[247 * (WIDTH + 1) + 240] = 0.002;
    u_velocity_m_s[248 * (WIDTH + 1) + 240] = -0.002;
    v_velocity_m_s[248 * WIDTH + 239] = -0.002;
    v_velocity_m_s[248 * WIDTH + 240] = 0.002;

    GpuSceneSeed {
        grid,
        phase_density_kg_m3: [1000.0, 1.2],
        mass_kg,
        marker,
        energy_j: vec![0.0; cells],
        fixed_wall,
        u_velocity_m_s,
        v_velocity_m_s,
        u_aperture,
        v_aperture,
        epoch,
        tick: 0,
        accepted_time_s: 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        render::{RenderViewport, SceneRenderer},
        scene::{PreparedSeed, coupled::GpuTickOutcome},
    };
    use particle_sim::OUTER_DT_S;
    use std::{
        future::Future,
        sync::Arc,
        task::{Context, Poll, Wake, Waker},
        time::{Duration, Instant},
    };

    struct Notify(std::thread::Thread);

    impl Wake for Notify {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }

    fn block_on<F: Future>(future: F) -> F::Output {
        let waker: Waker = Arc::new(Notify(std::thread::current())).into();
        let mut context = Context::from_waker(&waker);
        let mut future = Box::pin(future);
        let deadline = Instant::now() + Duration::from_secs(180);
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(value) => return value,
                Poll::Pending => {
                    assert!(Instant::now() < deadline, "M3 native scene test timed out");
                    std::thread::park_timeout(Duration::from_millis(100));
                }
            }
        }
    }

    #[test]
    fn full_size_pool_seed_prepares_without_gpu() {
        let seed = m3_demo_seed(7);
        assert_eq!(seed.epoch, 7);
        assert_eq!(seed.grid.width(), 480);
        assert_eq!(seed.grid.height(), 270);
        assert_eq!(
            seed.fixed_wall.iter().filter(|&&wall| wall == 1).count(),
            7_270
        );
        assert_eq!(
            seed.mass_kg[..seed.grid.cells()]
                .iter()
                .filter(|&&mass| mass > 0.0)
                .count(),
            6_214
        );
        PreparedSeed::new(seed).expect("M3 pool must pass host and f32 seed validation");
    }

    #[test]
    #[ignore = "requires a native Metal GPU and runs a full 480x270 pressure solve"]
    fn native_metal_full_scene_tick_and_committed_render() {
        block_on(async {
            let context = GpuContext::new().await.expect("native GPU device");
            assert_eq!(context.backend(), "Metal", "this probe requires Metal");
            let mut scene = m3_demo_scene(&context, 7).expect("480x270 scene preflight");
            assert_eq!(scene.progress().epoch, 7);
            let renderer = SceneRenderer::new(&context.device, wgpu::TextureFormat::Rgba8Unorm)
                .expect("M3 committed renderer");
            let target = context.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("M3 committed render probe"),
                size: wgpu::Extent3d {
                    width: 96,
                    height: 54,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let view = target.create_view(&wgpu::TextureViewDescriptor::default());
            let sample_committed = |scene: &GpuCoupledScene| {
                scene
                    .render_committed(
                        &renderer,
                        &view,
                        [96, 54],
                        RenderViewport::full(Grid::new(WIDTH as f64, HEIGHT as f64).unwrap()),
                    )
                    .expect("render committed generation");
                read_render_samples(&context, &target)
            };
            const TARGET_TICKS: u32 = 300;
            let initial_inventory =
                checkpoint_inventory(&scene.checkpoint_prototype().await.unwrap());
            let mut accepted = 0_u32;
            let mut completed = 0_u32;
            let mut step_elapsed = Duration::ZERO;
            for _ in 0..(TARGET_TICKS * 8) {
                let before_progress = scene.progress();
                let before_samples = sample_committed(&scene);
                let started = Instant::now();
                let outcome = scene.advance_outer_tick(1).await;
                step_elapsed += started.elapsed();
                match outcome {
                    Ok(GpuTickOutcome::Paused {
                        accepted_substeps, ..
                    }) => {
                        accepted += accepted_substeps;
                        assert_eq!(scene.progress().tick, before_progress.tick);
                        assert_eq!(
                            scene.progress().state_revision,
                            before_progress.state_revision + u64::from(accepted_substeps)
                        );
                    }
                    Ok(GpuTickOutcome::Complete {
                        accepted_substeps,
                        last_pressure,
                        encoded_time_error_s,
                        ..
                    }) => {
                        accepted += accepted_substeps;
                        assert!(encoded_time_error_s.abs() < 1e-9);
                        let gate = PressureConfig::default();
                        assert!(last_pressure.scaled_residual <= gate.scaled_residual_tolerance);
                        assert!(
                            last_pressure.scaled_divergence <= gate.scaled_divergence_tolerance
                        );
                        let samples = sample_committed(&scene);
                        assert_rgb(samples[0], [8, 18, 27]);
                        assert_rgb(samples[1], [35, 116, 203]);
                        assert_rgb(samples[2], [89, 94, 99]);
                        let progress = scene.progress();
                        assert_eq!(
                            progress.state_revision,
                            before_progress.state_revision + u64::from(accepted_substeps)
                        );
                        assert_eq!(progress.remaining_outer_s, 0.0);
                        eprintln!(
                            "M3 480x270 Metal outer tick accepted: tick={}, accepted_substeps={accepted}, pressure_iterations={}, scaled_residual={}, scaled_divergence={}, elapsed_s={:.3}",
                            scene.progress().tick,
                            last_pressure.iterations,
                            last_pressure.scaled_residual,
                            last_pressure.scaled_divergence,
                            step_elapsed.as_secs_f64(),
                        );
                        completed += 1;
                        assert_eq!(progress.tick, u64::from(completed));
                        assert!(
                            (progress.accepted_time_s - f64::from(completed) * OUTER_DT_S).abs()
                                < 1e-9
                        );
                        if completed.is_multiple_of(8) || completed == TARGET_TICKS {
                            let inventory =
                                checkpoint_inventory(&scene.checkpoint_prototype().await.unwrap());
                            assert_inventory_conserved(initial_inventory, inventory, completed);
                        }
                        if completed == TARGET_TICKS {
                            return;
                        }
                    }
                    Err(error) => {
                        assert_eq!(scene.progress(), before_progress);
                        assert_eq!(sample_committed(&scene), before_samples);
                        assert_rgb(before_samples[0], [8, 18, 27]);
                        assert_rgb(before_samples[1], [35, 116, 203]);
                        assert_rgb(before_samples[2], [89, 94, 99]);
                        panic!(
                            "M3 480x270 Metal tick rejected after {accepted} accepted substeps in {:.3}s: {error}",
                            step_elapsed.as_secs_f64()
                        );
                    }
                }
            }
            panic!(
                "M3 480x270 Metal tick paused after {accepted} accepted substeps in {:.3}s",
                step_elapsed.as_secs_f64(),
            );
        });
    }

    /// Reads the two phase masses and two passive-marker totals from an explicit
    /// checkpoint. This full-field download belongs to the native test only.
    fn checkpoint_inventory(bytes: &[u8]) -> [f64; 4] {
        let cells = WIDTH * HEIGHT;
        let field_bytes = cells * 2 * size_of::<f32>();
        assert_eq!(&bytes[..4], b"PFCP");
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 0);
        assert_eq!(
            u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
            WIDTH as u32
        );
        assert_eq!(
            u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            HEIGHT as u32
        );
        for slot in 0..2 {
            let offset = 68 + slot * size_of::<u64>();
            assert_eq!(
                u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap()),
                (cells * 2) as u64
            );
        }
        assert!(bytes.len() >= 108 + 2 * field_bytes);
        std::array::from_fn(|slot| {
            let start = 108 + slot / 2 * field_bytes + slot % 2 * cells * size_of::<f32>();
            bytes[start..start + cells * size_of::<f32>()]
                .as_chunks::<4>()
                .0
                .iter()
                .map(|value| f64::from(f32::from_le_bytes(*value)))
                .sum()
        })
    }

    fn assert_inventory_conserved(initial: [f64; 4], actual: [f64; 4], tick: u32) {
        for (slot, (before, after)) in initial.into_iter().zip(actual).enumerate() {
            let relative_drift = (after - before).abs() / before;
            assert!(
                relative_drift <= 1e-5,
                "M3 inventory slot {slot} drifted by {relative_drift:.3e} through tick {tick}"
            );
        }
    }

    fn read_render_samples(context: &GpuContext, texture: &wgpu::Texture) -> [[u8; 4]; 3] {
        let staging = context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("M3 three-pixel render readback"),
            size: 3 * 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("M3 render pixel samples"),
            });
        for (index, [x, y]) in [[48, 8], [48, 49], [48, 53]].into_iter().enumerate() {
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x, y, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &staging,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: (index * 256) as u64,
                        bytes_per_row: Some(256),
                        rows_per_image: Some(1),
                    },
                },
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
        }
        context.queue.submit([encoder.finish()]);
        let (sender, receiver) = std::sync::mpsc::channel();
        staging.map_async(wgpu::MapMode::Read, .., move |result| {
            sender.send(result).expect("render map receiver");
        });
        context
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(Duration::from_secs(10)),
            })
            .expect("M3 render submission completed");
        receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("M3 render map callback")
            .expect("M3 render map result");
        let mapped = staging.get_mapped_range(..);
        let samples = std::array::from_fn(|index| {
            let offset = index * 256;
            mapped[offset..offset + 4].try_into().unwrap()
        });
        drop(mapped);
        staging.unmap();
        samples
    }

    fn assert_rgb(actual: [u8; 4], expected: [u8; 3]) {
        for (channel, (actual, expected)) in actual[..3].iter().zip(expected).enumerate() {
            assert!(
                actual.abs_diff(expected) <= 2,
                "channel {channel}: expected {expected}, got {actual}"
            );
        }
        assert_eq!(actual[3], 255);
    }
}
