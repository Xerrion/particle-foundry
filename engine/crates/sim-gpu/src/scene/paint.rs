//! Bounded zero-time paint on the resident scene's detached generation.
//!
//! Host input selects only a circle, one phase, and one validated cell mass.
//! The compute pass preserves fixed walls and all cells outside that circle.
//! It never downloads the world to the browser.

use particle_sim::Grid;
use wgpu::util::DeviceExt;

use super::GpuScene;

const WORKGROUP_SIZE: u32 = 64;
pub(crate) const MAX_RADIUS_CELLS: u32 = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GpuPaintMaterial {
    Water,
    Air,
}

#[derive(Clone, Copy)]
struct PaintCommand {
    center_x: u32,
    center_y: u32,
    radius: u32,
    phase: u32,
    selected_mass_kg: f32,
    selected_density_kg_m3: f32,
}

impl PaintCommand {
    fn new(
        grid: Grid,
        phase_density_kg_m3: [f32; 2],
        center_x: u32,
        center_y: u32,
        radius: u32,
        material: GpuPaintMaterial,
    ) -> Result<Self, String> {
        if grid.cell_index(center_x, center_y).is_none() {
            return Err("GPU paint center must be inside the scene grid".into());
        }
        if radius > MAX_RADIUS_CELLS {
            return Err(format!(
                "GPU paint radius must be at most {MAX_RADIUS_CELLS} cells"
            ));
        }
        let phase = match material {
            GpuPaintMaterial::Water => 0,
            GpuPaintMaterial::Air => 1,
        };
        let density = phase_density_kg_m3[phase as usize];
        let volume = grid.cell_volume_m3() as f32;
        let selected_mass_kg = density * volume;
        let selected_density_kg_m3 = selected_mass_kg / volume;
        if !density.is_finite()
            || density <= 0.0
            || !volume.is_finite()
            || volume <= 0.0
            || !selected_mass_kg.is_finite()
            || selected_mass_kg <= 0.0
            || !selected_density_kg_m3.is_finite()
            || selected_density_kg_m3 <= 0.0
            || ((selected_mass_kg / density - volume) / volume).abs() > 1e-5
        {
            return Err("GPU paint mass does not close the selected cell volume".into());
        }
        Ok(Self {
            center_x,
            center_y,
            radius,
            phase,
            selected_mass_kg,
            selected_density_kg_m3,
        })
    }

    fn bytes(self, grid: Grid) -> [u8; 32] {
        let words = [
            grid.width(),
            grid.height(),
            self.center_x,
            self.center_y,
            self.radius,
            self.phase,
            self.selected_mass_kg.to_bits(),
            self.selected_density_kg_m3.to_bits(),
        ];
        let mut bytes = [0; 32];
        for (slot, word) in words.into_iter().enumerate() {
            bytes[slot * 4..slot * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
        bytes
    }
}

pub(super) struct GpuPaintStage {
    grid: Grid,
    workgroups: u32,
    pipeline: wgpu::ComputePipeline,
}

impl GpuPaintStage {
    pub(super) fn new(device: &wgpu::Device, grid: Grid) -> Result<Self, String> {
        let cells = u32::try_from(grid.cells()).map_err(|_| "GPU paint cell count exceeds u32")?;
        let workgroups = cells.div_ceil(WORKGROUP_SIZE);
        if workgroups > device.limits().max_compute_workgroups_per_dimension {
            return Err("GPU paint dispatch exceeds adapter limit".into());
        }
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("resident scene paint"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../shaders/scene_paint.wgsl").into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("resident scene paint"),
            layout: None,
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        Ok(Self {
            grid,
            workgroups,
            pipeline,
        })
    }

    pub(super) fn apply(
        &self,
        scene: &mut GpuScene,
        center_x: u32,
        center_y: u32,
        radius: u32,
        material: GpuPaintMaterial,
    ) -> Result<(), String> {
        if scene.grid != self.grid {
            return Err("GPU paint stage does not match the scene grid".into());
        }
        let command = PaintCommand::new(
            self.grid,
            scene.phase_density_kg_m3,
            center_x,
            center_y,
            radius,
            material,
        )?;
        let next_revision = scene
            .state_revision
            .checked_add(1)
            .ok_or("GPU scene state revision exhausted")?;
        let device = &scene.context.device;
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("resident scene paint command"),
            contents: &command.bytes(self.grid),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let committed = scene.committed();
        let candidate = scene.candidate();
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("resident scene paint fields"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: scene.headers.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: committed.mass_kg.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: committed.marker.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: committed.density_kg_m3.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: candidate.mass_kg.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: candidate.marker.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: candidate.density_kg_m3.as_entire_binding(),
                },
            ],
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("resident scene paint commit"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("resident scene paint"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(self.workgroups, 1, 1);
        }
        for (source, target) in [
            (&committed.u_velocity_m_s, &candidate.u_velocity_m_s),
            (&committed.v_velocity_m_s, &candidate.v_velocity_m_s),
        ] {
            encoder.copy_buffer_to_buffer(source, 0, target, 0, target.size());
        }
        scene.context.queue.submit([encoder.finish()]);
        scene.committed_index = 1 - scene.committed_index;
        scene.state_revision = next_revision;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GpuContext;
    use std::{
        future::Future,
        sync::{Arc, mpsc},
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
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(value) => return value,
                Poll::Pending => {
                    assert!(Instant::now() < deadline, "GPU paint test timed out");
                    std::thread::park_timeout(Duration::from_millis(100));
                }
            }
        }
    }

    fn seed() -> super::super::GpuSceneSeed {
        let grid = Grid::new(4.0, 3.0).unwrap();
        let cells = grid.cells();
        let volume = grid.cell_volume_m3();
        let wall = (0..cells)
            .map(|cell| u8::from(cell % 4 == 0 || cell % 4 == 3 || cell / 4 == 2))
            .collect::<Vec<_>>();
        let mut u_aperture = vec![0.0; grid.u_faces()];
        let mut v_aperture = vec![0.0; grid.v_faces()];
        for y in 0..3 {
            for x in 1..4 {
                if wall[y * 4 + x - 1] == 0 && wall[y * 4 + x] == 0 {
                    u_aperture[y * 5 + x] = 1.0;
                }
            }
        }
        for y in 1..3 {
            for x in 0..4 {
                if wall[(y - 1) * 4 + x] == 0 && wall[y * 4 + x] == 0 {
                    v_aperture[y * 4 + x] = 1.0;
                }
            }
        }
        let mut u_velocity_m_s = vec![0.0; grid.u_faces()];
        u_velocity_m_s[2] = 0.01;
        super::super::GpuSceneSeed {
            grid,
            phase_density_kg_m3: [1000.0, 1.2],
            mass_kg: (0..cells)
                .map(|_| 0.0)
                .chain(
                    wall.iter()
                        .map(|&fixed| if fixed == 0 { 1.2 * volume } else { 0.0 }),
                )
                .collect(),
            marker: (0..cells)
                .map(|_| 0.0)
                .chain(
                    wall.iter()
                        .map(|&fixed| if fixed == 0 { 0.05 } else { 0.0 }),
                )
                .collect(),
            energy_j: vec![0.0; cells],
            fixed_wall: wall,
            u_velocity_m_s,
            v_velocity_m_s: vec![0.0; grid.v_faces()],
            u_aperture,
            v_aperture,
            epoch: 2,
            tick: 7,
            accepted_time_s: 7.0 / 60.0,
        }
    }

    fn read_f32(scene: &GpuScene, field: &wgpu::Buffer) -> Vec<f32> {
        let staging = scene.context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("paint test readback"),
            size: field.size(),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder =
            scene
                .context
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("paint test copy"),
                });
        encoder.copy_buffer_to_buffer(field, 0, &staging, 0, field.size());
        scene.context.queue.submit([encoder.finish()]);
        let (sender, receiver) = mpsc::channel();
        staging.map_async(wgpu::MapMode::Read, .., move |result| {
            sender.send(result).unwrap();
        });
        scene
            .context
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(Duration::from_secs(10)),
            })
            .unwrap();
        receiver
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .unwrap();
        let mapped = staging.get_mapped_range(..);
        let (words, remainder) = mapped.as_chunks::<4>();
        assert!(remainder.is_empty());
        let values = words
            .iter()
            .map(|bytes| f32::from_le_bytes(*bytes))
            .collect();
        drop(mapped);
        staging.unmap();
        values
    }

    #[test]
    fn paint_command_rejects_out_of_bounds_and_bad_radius() {
        let grid = Grid::new(4.0, 3.0).unwrap();
        let density = [1000.0, 1.2];
        assert!(PaintCommand::new(grid, density, 0, 0, 0, GpuPaintMaterial::Water).is_ok());
        assert!(PaintCommand::new(grid, density, 4, 0, 0, GpuPaintMaterial::Water).is_err());
        assert!(PaintCommand::new(grid, density, 0, 3, 0, GpuPaintMaterial::Water).is_err());
        assert!(PaintCommand::new(grid, density, 0, 0, 17, GpuPaintMaterial::Water).is_err());
        assert!(
            PaintCommand::new(grid, [f32::INFINITY, 1.2], 0, 0, 0, GpuPaintMaterial::Water)
                .is_err()
        );
    }

    #[test]
    #[ignore = "requires a native GPU compute adapter"]
    fn resident_paint_preserves_walls_and_clock_across_generation_wrap() {
        block_on(async {
            let context = GpuContext::new().await.unwrap();
            let mut scene = GpuScene::new(&context, seed()).unwrap();
            let stage = GpuPaintStage::new(&context.device, scene.grid).unwrap();
            let original_velocity = read_f32(&scene, &scene.committed().u_velocity_m_s);
            let original_time = scene.accepted_time_s;
            stage
                .apply(&mut scene, 1, 0, 1, GpuPaintMaterial::Water)
                .unwrap();
            assert_eq!(
                (scene.epoch, scene.tick, scene.accepted_time_s),
                (2, 7, original_time)
            );
            assert_eq!((scene.committed_index, scene.state_revision), (1, 1));
            let mass = read_f32(&scene, &scene.committed().mass_kg);
            let marker = read_f32(&scene, &scene.committed().marker);
            let density = read_f32(&scene, &scene.committed().density_kg_m3);
            let water_mass = 1000.0 * scene.grid.cell_volume_m3() as f32;
            for cell in [1, 2, 5] {
                assert_eq!(mass[cell], water_mass);
                assert_eq!(mass[scene.grid.cells() + cell], 0.0);
                assert_eq!(marker[cell], 0.0);
                assert_eq!(marker[scene.grid.cells() + cell], 0.0);
                assert!((density[cell] - 1000.0).abs() < 0.001);
            }
            assert_eq!(mass[0], 0.0);
            assert_eq!(mass[scene.grid.cells()], 0.0);
            assert_eq!(density[0], 1.2);
            assert_eq!(
                read_f32(&scene, &scene.committed().u_velocity_m_s),
                original_velocity
            );
            let before_invalid = (scene.committed_index, scene.state_revision);
            assert!(
                stage
                    .apply(&mut scene, 4, 0, 0, GpuPaintMaterial::Air)
                    .is_err()
            );
            assert_eq!(
                (scene.committed_index, scene.state_revision),
                before_invalid
            );
            assert_eq!(read_f32(&scene, &scene.committed().mass_kg), mass);
            stage
                .apply(&mut scene, 1, 0, 0, GpuPaintMaterial::Air)
                .unwrap();
            assert_eq!((scene.committed_index, scene.state_revision), (0, 2));
            let mass = read_f32(&scene, &scene.committed().mass_kg);
            assert_eq!(mass[1], 0.0);
            assert!(
                (mass[scene.grid.cells() + 1] - 1.2 * scene.grid.cell_volume_m3() as f32).abs()
                    < 1e-11
            );
            assert_eq!(
                (scene.epoch, scene.tick, scene.accepted_time_s),
                (2, 7, original_time)
            );
        });
    }
}
