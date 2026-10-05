//! Derive a detached f32 density field from two component-major phase masses.
//!
//! The caller publishes the density only after reading a zero status word.
//! Failed candidates never change the committed mass or density buffers.

// The E07 coupled stage graph will call this crate-private validation stage.
#![allow(dead_code)]

use particle_sim::{CELL_WIDTH_M, Grid};
use wgpu::util::DeviceExt;

const WORKGROUP_SIZE: u32 = 64;
const PARAM_BYTES: u64 = 16;
const STATUS_BYTES: u64 = 4;

/// One or more masses are invalid, a wall owns mass, or density is not positive.
pub(crate) const STATUS_INVALID_CELL: u32 = 1;
/// Packed liquid and carrier volumes do not fill the fluid cell.
pub(crate) const STATUS_VOLUME_CLOSURE: u32 = 2;

#[derive(Clone, Copy)]
struct DensityPlan {
    cells: u32,
    workgroups: u32,
    cell_volume_m3: f32,
}

/// Detached density output and its four-byte atomic validation status.
pub(crate) struct GpuDerivedDensityCandidate {
    /// One f32 density per cell. Publish this buffer only when status is zero.
    pub density_kg_m3: wgpu::Buffer,
    /// Four-byte status word. Zero means every cell passed validation.
    pub status: wgpu::Buffer,
}

impl GpuDerivedDensityCandidate {
    /// Allocate a detached candidate within the effective device limits.
    pub(crate) fn new(device: &wgpu::Device, grid: Grid) -> Result<Self, String> {
        let plan = preflight(grid, &device.limits())?;
        let density_kg_m3 = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("derived candidate density"),
            size: u64::from(plan.cells) * 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let status = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("derived density status"),
            size: STATUS_BYTES,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Ok(Self {
            density_kg_m3,
            status,
        })
    }
}

/// GPU validation pipeline for one sealed, base-spacing grid.
pub(crate) struct GpuDensityDeriver {
    grid: Grid,
    plan: DensityPlan,
    pipeline: wgpu::ComputePipeline,
}

impl GpuDensityDeriver {
    /// Create the pipeline after checking dispatch and storage limits.
    pub(crate) fn new(device: &wgpu::Device, grid: Grid) -> Result<Self, String> {
        let plan = preflight(grid, &device.limits())?;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("derived density validation"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/density_derive.wgsl").into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("derived density validation"),
            layout: None,
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        Ok(Self {
            grid,
            plan,
            pipeline,
        })
    }

    /// Encode one validation pass over a detached transport candidate.
    ///
    /// `mass_kg` contains liquid then carrier f32 values, one of each per cell.
    /// `headers` contains eight-byte `GpuCellHeader` records. The fixed phase
    /// densities are in kg/m³. The encoder clears status before dispatch.
    pub(crate) fn encode(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        mass_kg: &wgpu::Buffer,
        headers: &wgpu::Buffer,
        candidate: &GpuDerivedDensityCandidate,
        phase_density_kg_m3: [f32; 2],
    ) -> Result<(), String> {
        let [liquid_density, carrier_density] = phase_density_kg_m3;
        if !liquid_density.is_finite()
            || !carrier_density.is_finite()
            || liquid_density <= 0.0
            || carrier_density <= 0.0
        {
            return Err("derived density needs positive finite phase densities".into());
        }
        preflight(self.grid, &device.limits())?;
        for (name, buffer, bytes, usage) in [
            (
                "phase mass",
                mass_kg,
                u64::from(self.plan.cells) * 8,
                wgpu::BufferUsages::STORAGE,
            ),
            (
                "cell headers",
                headers,
                u64::from(self.plan.cells) * 8,
                wgpu::BufferUsages::STORAGE,
            ),
            (
                "derived density",
                &candidate.density_kg_m3,
                u64::from(self.plan.cells) * 4,
                wgpu::BufferUsages::STORAGE,
            ),
            (
                "derived density status",
                &candidate.status,
                STATUS_BYTES,
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            ),
        ] {
            if buffer.size() != bytes || !buffer.usage().contains(usage) {
                return Err(format!("{name} has an invalid size or GPU usage"));
            }
        }
        let mut bytes = [0_u8; PARAM_BYTES as usize];
        for (slot, word) in [
            self.plan.cells,
            self.plan.cell_volume_m3.to_bits(),
            liquid_density.to_bits(),
            carrier_density.to_bits(),
        ]
        .into_iter()
        .enumerate()
        {
            bytes[slot * 4..slot * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("derived density parameters"),
            contents: &bytes,
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("derived density fields"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: mass_kg.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: headers.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: candidate.density_kg_m3.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: candidate.status.as_entire_binding(),
                },
            ],
        });
        encoder.clear_buffer(&candidate.status, 0, None);
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("derived density validation"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(self.plan.workgroups, 1, 1);
        Ok(())
    }
}

fn preflight(grid: Grid, limits: &wgpu::Limits) -> Result<DensityPlan, String> {
    if grid.cell_width_m() != CELL_WIDTH_M {
        return Err("derived density requires the base cell width".into());
    }
    let cells = u32::try_from(grid.cells()).map_err(|_| "derived density grid is too large")?;
    let workgroups = cells.div_ceil(WORKGROUP_SIZE);
    let cell_volume_m3 = grid.cell_volume_m3() as f32;
    if !cell_volume_m3.is_finite() || cell_volume_m3 <= 0.0 {
        return Err("derived density cell volume is not representable in f32".into());
    }
    if WORKGROUP_SIZE > limits.max_compute_invocations_per_workgroup
        || WORKGROUP_SIZE > limits.max_compute_workgroup_size_x
        || workgroups > limits.max_compute_workgroups_per_dimension
        || limits.max_storage_buffers_per_shader_stage < 4
        || limits.max_bindings_per_bind_group < 5
        || limits.max_uniform_buffer_binding_size < PARAM_BYTES
        || limits.max_buffer_size < PARAM_BYTES
    {
        return Err("derived density exceeds effective GPU limits".into());
    }
    for (name, bytes) in [
        ("phase mass", u64::from(cells) * 8),
        ("cell headers", u64::from(cells) * 8),
        ("derived density", u64::from(cells) * 4),
        ("derived density status", STATUS_BYTES),
    ] {
        if bytes > limits.max_storage_buffer_binding_size || bytes > limits.max_buffer_size {
            return Err(format!("{name} exceeds effective GPU storage limits"));
        }
    }
    Ok(DensityPlan {
        cells,
        workgroups,
        cell_volume_m3,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use particle_sim::gpu_layout::GpuCellHeader;
    use particle_sim_cpu::transport::TransportInventory;
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
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(value) => return value,
                Poll::Pending => {
                    assert!(Instant::now() < deadline, "GPU density fixture timed out");
                    std::thread::park_timeout(Duration::from_millis(100));
                }
            }
        }
    }

    fn read_bytes(device: &wgpu::Device, queue: &wgpu::Queue, source: &wgpu::Buffer) -> Vec<u8> {
        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("density fixture readback"),
            size: source.size(),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("density fixture copy"),
        });
        encoder.copy_buffer_to_buffer(source, 0, &staging, 0, source.size());
        queue.submit([encoder.finish()]);
        let (sender, receiver) = std::sync::mpsc::channel();
        staging.map_async(wgpu::MapMode::Read, .., move |result| {
            let _ = sender.send(result);
        });
        device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(Duration::from_secs(10)),
            })
            .expect("density fixture GPU submission");
        receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("density fixture map callback")
            .expect("density fixture mapped result");
        let bytes = staging.get_mapped_range(..).to_vec();
        staging.unmap();
        bytes
    }

    fn run_fixture(
        gpu: &crate::GpuContext,
        deriver: &GpuDensityDeriver,
        candidate: &GpuDerivedDensityCandidate,
        mass: &[f32],
        wall: &[u8],
        phase_density: [f32; 2],
    ) -> (u32, Vec<f32>) {
        let mass_bytes = mass
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>();
        let header_bytes = wall
            .iter()
            .flat_map(|&flag| GpuCellHeader::new(0.0, flag).unwrap().to_le_bytes())
            .collect::<Vec<_>>();
        let mass_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("density fixture mass"),
                contents: &mass_bytes,
                usage: wgpu::BufferUsages::STORAGE,
            });
        let header_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("density fixture headers"),
                contents: &header_bytes,
                usage: wgpu::BufferUsages::STORAGE,
            });
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("density fixture validation"),
            });
        deriver
            .encode(
                &gpu.device,
                &mut encoder,
                &mass_buffer,
                &header_buffer,
                candidate,
                phase_density,
            )
            .unwrap();
        gpu.queue.submit([encoder.finish()]);
        let status = read_bytes(&gpu.device, &gpu.queue, &candidate.status);
        let density = read_bytes(&gpu.device, &gpu.queue, &candidate.density_kg_m3);
        (
            u32::from_le_bytes(status.try_into().unwrap()),
            density
                .as_chunks::<4>()
                .0
                .iter()
                .map(|word| f32::from_le_bytes(*word))
                .collect(),
        )
    }

    #[test]
    fn preflight_rejects_missing_storage_bindings_and_refined_grid() {
        let grid = Grid::new(4.0, 1.0).unwrap();
        let mut limits = wgpu::Limits::default();
        assert!(preflight(grid, &limits).is_ok());
        limits.max_storage_buffers_per_shader_stage = 3;
        assert!(preflight(grid, &limits).is_err());
        let refined = Grid::with_cell_width(4.0, 1.0, 0.005).unwrap();
        assert!(preflight(refined, &wgpu::Limits::default()).is_err());
    }

    #[test]
    #[ignore = "requires a native compute adapter"]
    fn native_density_matches_cpu_and_rejects_invalid_candidates() {
        let gpu = block_on(crate::GpuContext::new()).unwrap();
        let grid = Grid::new(4.0, 1.0).unwrap();
        let deriver = GpuDensityDeriver::new(&gpu.device, grid).unwrap();
        let candidate = GpuDerivedDensityCandidate::new(&gpu.device, grid).unwrap();
        let phase_density = [1000.0_f32, 2.5_f32];
        let volume = grid.cell_volume_m3();
        let liquid = [0.0, 0.25, 1.0, 0.0].map(|alpha| alpha * volume * 1000.0);
        let carrier = [1.0, 0.75, 0.0, 0.0].map(|alpha| alpha * volume * 2.5);
        let cpu = TransportInventory::new(
            grid,
            1000.0,
            2.5,
            liquid.to_vec(),
            carrier.to_vec(),
            vec![0.0; grid.cells()],
            vec![0.0; grid.cells()],
            vec![false, false, false, true],
        )
        .unwrap();
        let mass = liquid
            .into_iter()
            .chain(carrier)
            .map(|value| value as f32)
            .collect::<Vec<_>>();
        let wall = [0, 0, 0, 1];
        let (status, density) =
            run_fixture(&gpu, &deriver, &candidate, &mass, &wall, phase_density);
        assert_eq!(status, 0);
        for (cell, &gpu_density) in density.iter().take(3).enumerate() {
            let cpu_reference = (cpu.liquid_mass_kg()[cell] + cpu.carrier_mass_kg()[cell]) / volume;
            assert!(
                (f64::from(gpu_density) - cpu_reference).abs() <= 1e-5 * cpu_reference.max(1.0),
                "cell {cell}: GPU {}, CPU {cpu_reference}",
                gpu_density
            );
        }
        assert_eq!(density[3], 2.5);

        // These normal masses produced subnormal physical volumes in the
        // sustained browser scene. Cell-unit validation must retain them.
        let tiny_liquid = [f32::from_bits(79_576_152), f32::from_bits(79_576_312)];
        for value in tiny_liquid {
            assert!(value.is_normal());
            assert!((value / 1000.0).is_subnormal());
            assert!(((value / volume as f32) / 1000.0).is_normal());
        }
        let tiny_mass = [
            tiny_liquid[0],
            tiny_liquid[1],
            0.0,
            0.0,
            1.2e-6,
            1.2e-6,
            1.2e-6,
            0.0,
        ];
        let (status, tiny_density) =
            run_fixture(&gpu, &deriver, &candidate, &tiny_mass, &wall, [1000.0, 1.2]);
        assert_eq!(status, 0, "normal trace phases remain valid");
        for value in tiny_density {
            assert!((value - 1.2).abs() <= 3.0 * f32::EPSILON);
        }
        let mut tiny_carrier = tiny_mass;
        tiny_carrier[2] = 1e-3;
        tiny_carrier[6] = f32::MIN_POSITIVE;
        assert!((tiny_carrier[6] / 1.2).is_subnormal());
        let (status, tiny_density) = run_fixture(
            &gpu,
            &deriver,
            &candidate,
            &tiny_carrier,
            &wall,
            [1000.0, 1.2],
        );
        assert_eq!(status, 0, "normal trace carrier remains valid");
        assert!((tiny_density[2] - 1000.0).abs() <= 3.0 * f32::EPSILON * 1000.0);

        let mut nonclosed = mass.clone();
        nonclosed[4] *= 1.01;
        let (status, _) = run_fixture(&gpu, &deriver, &candidate, &nonclosed, &wall, phase_density);
        assert_ne!(status & STATUS_VOLUME_CLOSURE, 0);

        let mut nonfinite = mass.clone();
        nonfinite[1] = f32::NAN;
        let (status, _) = run_fixture(&gpu, &deriver, &candidate, &nonfinite, &wall, phase_density);
        assert_ne!(status & STATUS_INVALID_CELL, 0);

        let mut wall_mass = mass.clone();
        wall_mass[3] = 1e-8;
        let (status, density) =
            run_fixture(&gpu, &deriver, &candidate, &wall_mass, &wall, phase_density);
        assert_ne!(status & STATUS_INVALID_CELL, 0);
        assert_eq!(density[3], phase_density[1]);

        let mut nonfinite_wall = mass.clone();
        nonfinite_wall[7] = f32::NAN;
        let (status, density) = run_fixture(
            &gpu,
            &deriver,
            &candidate,
            &nonfinite_wall,
            &wall,
            phase_density,
        );
        assert_ne!(status & STATUS_INVALID_CELL, 0);
        assert_eq!(density[3], phase_density[1]);

        let short_mass = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("short density fixture mass"),
            size: 4,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let headers = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("density fixture headers"),
            size: grid.cells() as u64 * 8,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("density fixture preflight"),
            });
        assert!(
            deriver
                .encode(
                    &gpu.device,
                    &mut encoder,
                    &short_mass,
                    &headers,
                    &candidate,
                    phase_density,
                )
                .unwrap_err()
                .contains("phase mass")
        );

        let (status, density) =
            run_fixture(&gpu, &deriver, &candidate, &mass, &wall, phase_density);
        assert_eq!(status, 0, "each encode must clear the prior rejection");
        assert_eq!(density[3], phase_density[1]);
        gpu.dispose();
    }
}
