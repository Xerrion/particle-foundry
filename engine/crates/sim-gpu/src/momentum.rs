//! Compatible f32 MAC-face momentum advection from the phase transport ledger.
//!
//! Four dual-edge passes own signed mass and momentum transfers. Each edge is
//! written once, then both adjacent faces gather the same record. The X sweep
//! completes before Y builds its edge ledger from X-updated velocities. Fixed
//! faces write explicit wall impulse instead of retaining momentum. Every
//! output is detached until the coupled stage accepts the status word.

// The private E07 coupled stage graph will consume this verified stage.
#![allow(dead_code)]

use particle_sim::{Grid, gpu_layout::GpuGridParams};
use wgpu::util::DeviceExt;

const WORKGROUP_SIZE: u32 = 64;
const PRIMAL_FLUX_BYTES: u64 = 20;
const DUAL_FLUX_BYTES: u64 = 12;

/// A dual mass ledger, aperture, or f32 result is invalid.
pub(crate) const STATUS_INVALID_LEDGER: u32 = 1;
/// A dual cell sends more mass than it owns at the start of a sweep.
pub(crate) const STATUS_DONOR_OVERDRAW: u32 = 2;
/// An open MAC face has no mass from which to derive its velocity.
pub(crate) const STATUS_EMPTY_OPEN_FACE: u32 = 4;

/// Resident inputs for one compatible momentum candidate.
///
/// Mass is component-major. Each primal flux is a 20-byte record with signed
/// volume, liquid mass, carrier mass, liquid marker, and carrier marker in that
/// order. Every other buffer is a row-major f32 MAC or cell array.
pub(crate) struct GpuMomentumInput<'a> {
    pub source_mass_kg: &'a wgpu::Buffer,
    pub after_x_mass_kg: &'a wgpu::Buffer,
    pub final_mass_kg: &'a wgpu::Buffer,
    pub source_u_velocity_m_s: &'a wgpu::Buffer,
    pub source_v_velocity_m_s: &'a wgpu::Buffer,
    pub u_aperture: &'a wgpu::Buffer,
    pub v_aperture: &'a wgpu::Buffer,
    pub u_flux: &'a wgpu::Buffer,
    pub v_flux: &'a wgpu::Buffer,
}

/// Detached face velocities, owned dual-edge ledgers, and wall impulse entries.
///
/// Each dual edge contains three scalar f32 values: its two signed mass
/// subfaces and their already-upwinded total momentum transfer. The 12-byte
/// stride is independent of WGSL vector alignment.
pub(crate) struct GpuMomentumCandidate {
    pub after_x_u_velocity_m_s: wgpu::Buffer,
    pub after_x_v_velocity_m_s: wgpu::Buffer,
    pub u_velocity_m_s: wgpu::Buffer,
    pub v_velocity_m_s: wgpu::Buffer,
    pub ux_dual_flux: wgpu::Buffer,
    pub vx_dual_flux: wgpu::Buffer,
    pub uy_dual_flux: wgpu::Buffer,
    pub vy_dual_flux: wgpu::Buffer,
    pub ux_wall_impulse: wgpu::Buffer,
    pub vx_wall_impulse: wgpu::Buffer,
    pub uy_wall_impulse: wgpu::Buffer,
    pub vy_wall_impulse: wgpu::Buffer,
    pub status: wgpu::Buffer,
}

impl GpuMomentumCandidate {
    pub(crate) fn new(device: &wgpu::Device, grid: Grid) -> Result<Self, String> {
        preflight(grid, &device.limits())?;
        let buffer = |label, size| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            })
        };
        let u_bytes = grid.u_faces() as u64 * size_of::<f32>() as u64;
        let v_bytes = grid.v_faces() as u64 * size_of::<f32>() as u64;
        let w = grid.width() as u64;
        let h = grid.height() as u64;
        Ok(Self {
            after_x_u_velocity_m_s: buffer("momentum after-x u", u_bytes),
            after_x_v_velocity_m_s: buffer("momentum after-x v", v_bytes),
            u_velocity_m_s: buffer("momentum candidate u", u_bytes),
            v_velocity_m_s: buffer("momentum candidate v", v_bytes),
            ux_dual_flux: buffer("momentum u x-edge flux", (w + 2) * h * DUAL_FLUX_BYTES),
            vx_dual_flux: buffer(
                "momentum v x-edge flux",
                (w + 1) * (h + 1) * DUAL_FLUX_BYTES,
            ),
            uy_dual_flux: buffer(
                "momentum u y-edge flux",
                (w + 1) * (h + 1) * DUAL_FLUX_BYTES,
            ),
            vy_dual_flux: buffer("momentum v y-edge flux", w * (h + 2) * DUAL_FLUX_BYTES),
            ux_wall_impulse: buffer("momentum u x-wall impulse", u_bytes),
            vx_wall_impulse: buffer("momentum v x-wall impulse", v_bytes),
            uy_wall_impulse: buffer("momentum u y-wall impulse", u_bytes),
            vy_wall_impulse: buffer("momentum v y-wall impulse", v_bytes),
            status: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("momentum candidate status"),
                size: size_of::<u32>() as u64,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_SRC
                    | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        })
    }
}

/// Reusable pipelines for one closed, base-spacing grid.
pub(crate) struct GpuMomentumStage {
    grid: Grid,
    ux_flux: wgpu::ComputePipeline,
    vx_flux: wgpu::ComputePipeline,
    uy_flux: wgpu::ComputePipeline,
    vy_flux: wgpu::ComputePipeline,
    ux_gather: wgpu::ComputePipeline,
    vx_gather: wgpu::ComputePipeline,
    uy_gather: wgpu::ComputePipeline,
    vy_gather: wgpu::ComputePipeline,
}

impl GpuMomentumStage {
    pub(crate) fn new(device: &wgpu::Device, grid: Grid) -> Result<Self, String> {
        preflight(grid, &device.limits())?;
        let flux_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("compatible momentum dual-edge flux"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/momentum_flux.wgsl").into()),
        });
        let gather_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("compatible momentum dual-face gather"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/momentum_gather.wgsl").into(),
            ),
        });
        let pipeline = |label, module, entry_point| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(label),
                layout: None,
                module,
                entry_point: Some(entry_point),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        Ok(Self {
            grid,
            ux_flux: pipeline("momentum u x flux", &flux_shader, "ux"),
            vx_flux: pipeline("momentum v x flux", &flux_shader, "vx"),
            uy_flux: pipeline("momentum u y flux", &flux_shader, "uy"),
            vy_flux: pipeline("momentum v y flux", &flux_shader, "vy"),
            ux_gather: pipeline("momentum u x gather", &gather_shader, "ux"),
            vx_gather: pipeline("momentum v x gather", &gather_shader, "vx"),
            uy_gather: pipeline("momentum u y gather", &gather_shader, "uy"),
            vy_gather: pipeline("momentum v y gather", &gather_shader, "vy"),
        })
    }

    /// Encodes four owned edge passes and four paired face gathers. The caller
    /// checks the status after submission and publishes no field on failure.
    pub(crate) fn encode(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        input: &GpuMomentumInput<'_>,
        candidate: &GpuMomentumCandidate,
    ) -> Result<(), String> {
        check_buffer_sizes(self.grid, input, candidate)?;
        let mut params = [0_u8; 16];
        for (slot, value) in [
            self.grid.width(),
            self.grid.height(),
            self.grid.cells() as u32,
            0,
        ]
        .into_iter()
        .enumerate()
        {
            params[slot * 4..slot * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("momentum grid parameters"),
            contents: &params,
            usage: wgpu::BufferUsages::UNIFORM,
        });
        encoder.clear_buffer(&candidate.status, 0, None);
        let w = self.grid.width() as usize;
        let h = self.grid.height() as usize;
        self.encode_flux(
            device,
            encoder,
            &self.ux_flux,
            &uniform,
            input.u_flux,
            input.source_u_velocity_m_s,
            &candidate.ux_dual_flux,
            &candidate.status,
            (w + 2) * h,
        );
        self.encode_flux(
            device,
            encoder,
            &self.vx_flux,
            &uniform,
            input.u_flux,
            input.source_v_velocity_m_s,
            &candidate.vx_dual_flux,
            &candidate.status,
            (w + 1) * (h + 1),
        );
        self.encode_gather(
            device,
            encoder,
            &self.ux_gather,
            &uniform,
            input.source_mass_kg,
            input.after_x_mass_kg,
            input.source_u_velocity_m_s,
            input.u_aperture,
            &candidate.ux_dual_flux,
            &candidate.after_x_u_velocity_m_s,
            &candidate.ux_wall_impulse,
            &candidate.status,
            self.grid.u_faces(),
        );
        self.encode_gather(
            device,
            encoder,
            &self.vx_gather,
            &uniform,
            input.source_mass_kg,
            input.after_x_mass_kg,
            input.source_v_velocity_m_s,
            input.v_aperture,
            &candidate.vx_dual_flux,
            &candidate.after_x_v_velocity_m_s,
            &candidate.vx_wall_impulse,
            &candidate.status,
            self.grid.v_faces(),
        );
        self.encode_flux(
            device,
            encoder,
            &self.uy_flux,
            &uniform,
            input.v_flux,
            &candidate.after_x_u_velocity_m_s,
            &candidate.uy_dual_flux,
            &candidate.status,
            (w + 1) * (h + 1),
        );
        self.encode_flux(
            device,
            encoder,
            &self.vy_flux,
            &uniform,
            input.v_flux,
            &candidate.after_x_v_velocity_m_s,
            &candidate.vy_dual_flux,
            &candidate.status,
            w * (h + 2),
        );
        self.encode_gather(
            device,
            encoder,
            &self.uy_gather,
            &uniform,
            input.after_x_mass_kg,
            input.final_mass_kg,
            &candidate.after_x_u_velocity_m_s,
            input.u_aperture,
            &candidate.uy_dual_flux,
            &candidate.u_velocity_m_s,
            &candidate.uy_wall_impulse,
            &candidate.status,
            self.grid.u_faces(),
        );
        self.encode_gather(
            device,
            encoder,
            &self.vy_gather,
            &uniform,
            input.after_x_mass_kg,
            input.final_mass_kg,
            &candidate.after_x_v_velocity_m_s,
            input.v_aperture,
            &candidate.vy_dual_flux,
            &candidate.v_velocity_m_s,
            &candidate.vy_wall_impulse,
            &candidate.status,
            self.grid.v_faces(),
        );
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn encode_flux(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        pipeline: &wgpu::ComputePipeline,
        uniform: &wgpu::Buffer,
        primal: &wgpu::Buffer,
        velocity: &wgpu::Buffer,
        dual: &wgpu::Buffer,
        status: &wgpu::Buffer,
        entries: usize,
    ) {
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("momentum dual-edge bindings"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                bind(0, uniform),
                bind(1, primal),
                bind(2, velocity),
                bind(3, dual),
                bind(4, status),
            ],
        });
        dispatch(encoder, pipeline, &group, entries);
    }

    #[allow(clippy::too_many_arguments)]
    fn encode_gather(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        pipeline: &wgpu::ComputePipeline,
        uniform: &wgpu::Buffer,
        before_mass: &wgpu::Buffer,
        after_mass: &wgpu::Buffer,
        before_velocity: &wgpu::Buffer,
        aperture: &wgpu::Buffer,
        dual: &wgpu::Buffer,
        after_velocity: &wgpu::Buffer,
        impulse: &wgpu::Buffer,
        status: &wgpu::Buffer,
        entries: usize,
    ) {
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("momentum face-gather bindings"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                bind(0, uniform),
                bind(1, before_mass),
                bind(2, after_mass),
                bind(3, before_velocity),
                bind(4, aperture),
                bind(5, dual),
                bind(6, after_velocity),
                bind(7, impulse),
                bind(8, status),
            ],
        });
        dispatch(encoder, pipeline, &group, entries);
    }
}

fn bind(index: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding: index,
        resource: buffer.as_entire_binding(),
    }
}

fn dispatch(
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::ComputePipeline,
    group: &wgpu::BindGroup,
    entries: usize,
) {
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("compatible momentum stage"),
        timestamp_writes: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, group, &[]);
    pass.dispatch_workgroups((entries as u32).div_ceil(WORKGROUP_SIZE), 1, 1);
}

fn preflight(grid: Grid, limits: &wgpu::Limits) -> Result<(), String> {
    GpuGridParams::new(grid, 2).map_err(|error| error.to_string())?;
    let w = grid.width() as u64;
    let h = grid.height() as u64;
    let u = grid.u_faces() as u64;
    let v = grid.v_faces() as u64;
    let ux = (w + 2) * h;
    let transverse = (w + 1) * (h + 1);
    let vy = w * (h + 2);
    if limits.max_bindings_per_bind_group < 9
        || limits.max_storage_buffers_per_shader_stage < 8
        || limits.max_compute_invocations_per_workgroup < WORKGROUP_SIZE
        || limits.max_compute_workgroup_size_x < WORKGROUP_SIZE
        || limits.max_uniform_buffer_binding_size < 16
    {
        return Err("GPU adapter limits cannot run compatible momentum passes".into());
    }
    for entries in [ux, transverse, vy, u, v] {
        if entries.div_ceil(WORKGROUP_SIZE as u64)
            > u64::from(limits.max_compute_workgroups_per_dimension)
        {
            return Err("GPU momentum dispatch exceeds effective workgroup limit".into());
        }
    }
    for bytes in [
        2 * grid.cells() as u64 * 4,
        u * PRIMAL_FLUX_BYTES,
        v * PRIMAL_FLUX_BYTES,
        ux * DUAL_FLUX_BYTES,
        transverse * DUAL_FLUX_BYTES,
        vy * DUAL_FLUX_BYTES,
        u * 4,
        v * 4,
    ] {
        if bytes > limits.max_storage_buffer_binding_size || bytes > limits.max_buffer_size {
            return Err("GPU momentum field exceeds effective buffer limit".into());
        }
    }
    Ok(())
}

fn check_buffer_sizes(
    grid: Grid,
    input: &GpuMomentumInput<'_>,
    candidate: &GpuMomentumCandidate,
) -> Result<(), String> {
    let w = grid.width() as u64;
    let h = grid.height() as u64;
    let mass_bytes = 2 * grid.cells() as u64 * 4;
    let u_bytes = grid.u_faces() as u64 * 4;
    let v_bytes = grid.v_faces() as u64 * 4;
    for (label, buffer, bytes) in [
        ("source mass", input.source_mass_kg, mass_bytes),
        ("after-x mass", input.after_x_mass_kg, mass_bytes),
        ("final mass", input.final_mass_kg, mass_bytes),
        ("source u", input.source_u_velocity_m_s, u_bytes),
        ("source v", input.source_v_velocity_m_s, v_bytes),
        ("u aperture", input.u_aperture, u_bytes),
        ("v aperture", input.v_aperture, v_bytes),
        (
            "u primal flux",
            input.u_flux,
            grid.u_faces() as u64 * PRIMAL_FLUX_BYTES,
        ),
        (
            "v primal flux",
            input.v_flux,
            grid.v_faces() as u64 * PRIMAL_FLUX_BYTES,
        ),
        ("after-x u", &candidate.after_x_u_velocity_m_s, u_bytes),
        ("after-x v", &candidate.after_x_v_velocity_m_s, v_bytes),
        ("candidate u", &candidate.u_velocity_m_s, u_bytes),
        ("candidate v", &candidate.v_velocity_m_s, v_bytes),
        (
            "u x-edge",
            &candidate.ux_dual_flux,
            (w + 2) * h * DUAL_FLUX_BYTES,
        ),
        (
            "v x-edge",
            &candidate.vx_dual_flux,
            (w + 1) * (h + 1) * DUAL_FLUX_BYTES,
        ),
        (
            "u y-edge",
            &candidate.uy_dual_flux,
            (w + 1) * (h + 1) * DUAL_FLUX_BYTES,
        ),
        (
            "v y-edge",
            &candidate.vy_dual_flux,
            w * (h + 2) * DUAL_FLUX_BYTES,
        ),
        ("u x-wall impulse", &candidate.ux_wall_impulse, u_bytes),
        ("v x-wall impulse", &candidate.vx_wall_impulse, v_bytes),
        ("u y-wall impulse", &candidate.uy_wall_impulse, u_bytes),
        ("v y-wall impulse", &candidate.vy_wall_impulse, v_bytes),
        ("momentum status", &candidate.status, 4),
    ] {
        if buffer.size() != bytes {
            return Err(format!("{label} must have exactly {bytes} bytes"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_layout_uses_scalar_dual_flux_stride() {
        assert_eq!(PRIMAL_FLUX_BYTES, 20);
        assert_eq!(DUAL_FLUX_BYTES, 12);
        let grid = Grid::new(2.0, 2.0).unwrap();
        assert!(preflight(grid, &wgpu::Limits::default()).is_ok());
        let limits = wgpu::Limits {
            max_storage_buffers_per_shader_stage: 7,
            ..wgpu::Limits::default()
        };
        assert!(preflight(grid, &limits).is_err());
    }

    #[cfg(not(target_arch = "wasm32"))]
    mod native {
        use std::{
            future::Future,
            sync::Arc,
            task::{Context, Poll, Wake, Waker},
            time::{Duration, Instant},
        };

        use particle_sim::contracts::Boundary;
        use particle_sim_cpu::{
            fluid::{FaceValues, PressureFields},
            momentum::momentum_candidate,
            transport::{TransportFaceFluxes, TransportInventory},
        };

        use super::*;

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
                        assert!(Instant::now() < deadline, "momentum GPU fixture timed out");
                        std::thread::park_timeout(Duration::from_millis(100));
                    }
                }
            }
        }

        fn f32_bytes(values: impl IntoIterator<Item = f32>) -> Vec<u8> {
            values
                .into_iter()
                .flat_map(|value| value.to_le_bytes())
                .collect()
        }

        fn upload(device: &wgpu::Device, label: &'static str, bytes: &[u8]) -> wgpu::Buffer {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytes,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            })
        }

        fn read_bytes(
            device: &wgpu::Device,
            queue: &wgpu::Queue,
            source: &wgpu::Buffer,
        ) -> Vec<u8> {
            let staging = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("momentum fixture readback"),
                size: source.size(),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("momentum fixture copy"),
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
                .expect("momentum fixture GPU submission");
            receiver
                .recv_timeout(Duration::from_secs(10))
                .expect("momentum fixture map callback")
                .expect("momentum fixture mapped result");
            let bytes = staging.get_mapped_range(..).to_vec();
            staging.unmap();
            bytes
        }

        fn read_f32(device: &wgpu::Device, queue: &wgpu::Queue, source: &wgpu::Buffer) -> Vec<f32> {
            read_bytes(device, queue, source)
                .as_chunks::<4>()
                .0
                .iter()
                .map(|chunk| f32::from_le_bytes(*chunk))
                .collect()
        }

        fn pack_primal(flux: &TransportFaceFluxes, axis: char) -> Vec<u8> {
            fn select(values: &FaceValues, axis: char) -> &[f64] {
                if axis == 'u' { &values.u } else { &values.v }
            }
            let mut bytes = Vec::new();
            for face in 0..select(&flux.volume_m3, axis).len() {
                for value in [
                    select(&flux.volume_m3, axis)[face],
                    select(&flux.liquid_mass_kg, axis)[face],
                    select(&flux.carrier_mass_kg, axis)[face],
                    select(&flux.liquid_marker, axis)[face],
                    select(&flux.carrier_marker, axis)[face],
                ] {
                    bytes.extend_from_slice(&(value as f32).to_le_bytes());
                }
            }
            bytes
        }

        fn dual_mass(grid: Grid, cell_mass: &[f64], component: char) -> Vec<f64> {
            let w = grid.width() as usize;
            let h = grid.height() as usize;
            if component == 'u' {
                (0..grid.u_faces())
                    .map(|face| {
                        let x = face % (w + 1);
                        let y = face / (w + 1);
                        let mut mass = 0.0;
                        if x > 0 {
                            mass += 0.5 * cell_mass[y * w + x - 1];
                        }
                        if x < w {
                            mass += 0.5 * cell_mass[y * w + x];
                        }
                        mass
                    })
                    .collect()
            } else {
                (0..grid.v_faces())
                    .map(|face| {
                        let x = face % w;
                        let y = face / w;
                        let mut mass = 0.0;
                        if y > 0 {
                            mass += 0.5 * cell_mass[(y - 1) * w + x];
                        }
                        if y < h {
                            mass += 0.5 * cell_mass[y * w + x];
                        }
                        mass
                    })
                    .collect()
            }
        }

        #[test]
        #[ignore = "requires a native GPU adapter"]
        fn closed_vortex_matches_cpu_momentum_and_paired_wall_ledger() {
            let gpu = block_on(crate::GpuContext::new())
                .expect("native GPU adapter for momentum fixture");
            let grid = Grid::new(2.0, 2.0).unwrap();
            let cell_volume = grid.cell_volume_m3();
            let fractions = [0.1, 0.45, 0.8, 0.3];
            let source = TransportInventory::new(
                grid,
                1000.0,
                1.2,
                fractions
                    .iter()
                    .map(|alpha| alpha * 1000.0 * cell_volume)
                    .collect(),
                fractions
                    .iter()
                    .map(|alpha| (1.0 - alpha) * 1.2 * cell_volume)
                    .collect(),
                vec![0.0; grid.cells()],
                vec![0.0; grid.cells()],
                vec![false; grid.cells()],
            )
            .unwrap();
            let mut velocity = FaceValues {
                u: vec![0.0; grid.u_faces()],
                v: vec![0.0; grid.v_faces()],
            };
            velocity.u[grid.u_face_index(1, 0).unwrap()] = 1.0;
            velocity.u[grid.u_face_index(1, 1).unwrap()] = -1.0;
            velocity.v[grid.v_face_index(0, 1).unwrap()] = -1.0;
            velocity.v[grid.v_face_index(1, 1).unwrap()] = 1.0;
            let mut aperture = FaceValues {
                u: vec![0.0; grid.u_faces()],
                v: vec![0.0; grid.v_faces()],
            };
            for y in 0..grid.height() {
                aperture.u[grid.u_face_index(1, y).unwrap()] = 1.0;
            }
            for x in 0..grid.width() {
                aperture.v[grid.v_face_index(x, 1).unwrap()] = 1.0;
            }
            let fields = PressureFields::new(
                grid,
                [Boundary::Closed; 4],
                vec![1.0; grid.cells()],
                vec![0.0; grid.cells()],
                FaceValues {
                    u: velocity.u.clone(),
                    v: velocity.v.clone(),
                },
                FaceValues {
                    u: aperture.u.clone(),
                    v: aperture.v.clone(),
                },
            )
            .unwrap();
            let transport = source.candidate(&fields, &velocity, 0.001).unwrap();
            let cpu = momentum_candidate(&source, &transport, &fields, &velocity, 0.001).unwrap();

            let cells = grid.cells();
            let source_mass = source
                .liquid_mass_kg()
                .iter()
                .chain(source.carrier_mass_kg())
                .map(|value| *value as f32)
                .collect::<Vec<_>>();
            let mut after_x_mass = vec![0.0_f32; 2 * cells];
            for (component, flux) in [
                &transport.fluxes.liquid_mass_kg.u,
                &transport.fluxes.carrier_mass_kg.u,
            ]
            .into_iter()
            .enumerate()
            {
                for cell in 0..cells {
                    let x = cell % grid.width() as usize;
                    let y = cell / grid.width() as usize;
                    let left = y * (grid.width() as usize + 1) + x;
                    let before = if component == 0 {
                        source.liquid_mass_kg()[cell]
                    } else {
                        source.carrier_mass_kg()[cell]
                    };
                    after_x_mass[component * cells + cell] =
                        (before + flux[left] - flux[left + 1]) as f32;
                }
            }
            let final_mass = transport
                .inventory
                .liquid_mass_kg()
                .iter()
                .chain(transport.inventory.carrier_mass_kg())
                .map(|value| *value as f32)
                .collect::<Vec<_>>();
            let source_mass_buffer =
                upload(&gpu.device, "momentum source mass", &f32_bytes(source_mass));
            let after_x_mass_buffer =
                upload(&gpu.device, "momentum x mass", &f32_bytes(after_x_mass));
            let final_mass_buffer = upload(
                &gpu.device,
                "momentum final mass",
                &f32_bytes(final_mass.iter().copied()),
            );
            let u_buffer = upload(
                &gpu.device,
                "momentum source u",
                &f32_bytes(velocity.u.iter().map(|value| *value as f32)),
            );
            let v_buffer = upload(
                &gpu.device,
                "momentum source v",
                &f32_bytes(velocity.v.iter().map(|value| *value as f32)),
            );
            let u_aperture_buffer = upload(
                &gpu.device,
                "momentum u aperture",
                &f32_bytes(aperture.u.iter().map(|value| *value as f32)),
            );
            let v_aperture_buffer = upload(
                &gpu.device,
                "momentum v aperture",
                &f32_bytes(aperture.v.iter().map(|value| *value as f32)),
            );
            let u_flux_buffer = upload(
                &gpu.device,
                "momentum primal u",
                &pack_primal(&transport.fluxes, 'u'),
            );
            let v_flux_buffer = upload(
                &gpu.device,
                "momentum primal v",
                &pack_primal(&transport.fluxes, 'v'),
            );
            let input = GpuMomentumInput {
                source_mass_kg: &source_mass_buffer,
                after_x_mass_kg: &after_x_mass_buffer,
                final_mass_kg: &final_mass_buffer,
                source_u_velocity_m_s: &u_buffer,
                source_v_velocity_m_s: &v_buffer,
                u_aperture: &u_aperture_buffer,
                v_aperture: &v_aperture_buffer,
                u_flux: &u_flux_buffer,
                v_flux: &v_flux_buffer,
            };
            let stage = GpuMomentumStage::new(&gpu.device, grid).unwrap();
            let candidate = GpuMomentumCandidate::new(&gpu.device, grid).unwrap();
            let mut encoder = gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("momentum vortex fixture"),
                });
            stage
                .encode(&gpu.device, &mut encoder, &input, &candidate)
                .unwrap();
            gpu.queue.submit([encoder.finish()]);
            let status = read_bytes(&gpu.device, &gpu.queue, &candidate.status);
            assert_eq!(u32::from_le_bytes(status.try_into().unwrap()), 0);

            let gpu_u = read_f32(&gpu.device, &gpu.queue, &candidate.u_velocity_m_s);
            let gpu_v = read_f32(&gpu.device, &gpu.queue, &candidate.v_velocity_m_s);
            for (actual, expected) in gpu_u.iter().zip(&cpu.velocity_m_s.u) {
                assert!((*actual as f64 - expected).abs() <= 2e-5);
            }
            for (actual, expected) in gpu_v.iter().zip(&cpu.velocity_m_s.v) {
                assert!((*actual as f64 - expected).abs() <= 2e-5);
            }
            let impulses = [
                read_f32(&gpu.device, &gpu.queue, &candidate.ux_wall_impulse),
                read_f32(&gpu.device, &gpu.queue, &candidate.vx_wall_impulse),
                read_f32(&gpu.device, &gpu.queue, &candidate.uy_wall_impulse),
                read_f32(&gpu.device, &gpu.queue, &candidate.vy_wall_impulse),
            ];
            let u_impulse = impulses[0]
                .iter()
                .chain(&impulses[2])
                .map(|value| *value as f64)
                .sum::<f64>();
            let v_impulse = impulses[1]
                .iter()
                .chain(&impulses[3])
                .map(|value| *value as f64)
                .sum::<f64>();
            assert!(
                v_impulse.abs() > 1e-10,
                "fixture must exercise wall impulse"
            );
            assert!((u_impulse - cpu.wall_impulse_kg_m_s[0]).abs() <= 2e-11);
            assert!((v_impulse - cpu.wall_impulse_kg_m_s[1]).abs() <= 2e-11);

            let total_cell_mass = |inventory: &TransportInventory| {
                inventory
                    .liquid_mass_kg()
                    .iter()
                    .zip(inventory.carrier_mass_kg())
                    .map(|(liquid, carrier)| liquid + carrier)
                    .collect::<Vec<_>>()
            };
            for (component, before_speed, after_speed, impulse) in [
                ('u', &velocity.u, &gpu_u, u_impulse),
                ('v', &velocity.v, &gpu_v, v_impulse),
            ] {
                let before_mass = dual_mass(grid, &total_cell_mass(&source), component);
                let after_mass = dual_mass(grid, &total_cell_mass(&transport.inventory), component);
                let before = before_mass
                    .iter()
                    .zip(before_speed)
                    .map(|(mass, speed)| mass * speed)
                    .sum::<f64>();
                let after = after_mass
                    .iter()
                    .zip(after_speed)
                    .map(|(mass, speed)| mass * f64::from(*speed))
                    .sum::<f64>();
                let scale = before_mass
                    .iter()
                    .zip(before_speed)
                    .map(|(mass, speed)| (mass * speed).abs())
                    .sum::<f64>()
                    + after_mass
                        .iter()
                        .zip(after_speed)
                        .map(|(mass, speed)| (mass * f64::from(*speed)).abs())
                        .sum::<f64>()
                    + impulse.abs();
                assert!((after - before - impulse).abs() <= 2e-5 * scale.max(1e-12));
            }

            let transverse = read_f32(&gpu.device, &gpu.queue, &candidate.uy_dual_flux);
            assert_eq!(
                transverse.len(),
                (grid.width() as usize + 1) * (grid.height() as usize + 1) * 3
            );
            let center = (grid.width() as usize + 1) + 1;
            assert!(transverse[center * 3] * transverse[center * 3 + 1] < 0.0);
            assert!(transverse[center * 3 + 2] != 0.0);

            let mut invalid_final_mass = final_mass;
            invalid_final_mass[cells] *= 0.5;
            let invalid_final_buffer = upload(
                &gpu.device,
                "momentum inconsistent final mass",
                &f32_bytes(invalid_final_mass),
            );
            let invalid_input = GpuMomentumInput {
                final_mass_kg: &invalid_final_buffer,
                ..input
            };
            let rejected = GpuMomentumCandidate::new(&gpu.device, grid).unwrap();
            let mut encoder = gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("momentum inconsistent ledger fixture"),
                });
            stage
                .encode(&gpu.device, &mut encoder, &invalid_input, &rejected)
                .unwrap();
            gpu.queue.submit([encoder.finish()]);
            let rejected_status = read_bytes(&gpu.device, &gpu.queue, &rejected.status);
            assert_ne!(
                u32::from_le_bytes(rejected_status.try_into().unwrap()) & STATUS_INVALID_LEDGER,
                0
            );
            println!(
                "GPU momentum: u/v wall impulse {u_impulse:.6e}/{v_impulse:.6e} kg m/s; paired dual-edge stride 12 bytes; CPU face parity within 2e-5 m/s"
            );
            gpu.dispose();
        }
    }
}
