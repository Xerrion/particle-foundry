//! Detached f32 tangential shear for a closed MAC grid.
//!
//! One corner owns each stress. Separate face passes gather its paired
//! impulses without floating-point atomics. The caller reads `status` before
//! publishing either candidate velocity buffer. This stage does not deposit
//! the lost kinetic energy into a temperature or energy field.

use particle_sim::{Grid, gpu_layout::GpuGridParams};
use wgpu::util::DeviceExt;

/// A cell density or viscosity is invalid or violates its supplied bound.
pub const STATUS_INVALID_CELL: u32 = 1;
/// A face speed or aperture violates the closed-wall MAC contract.
pub const STATUS_INVALID_FACE: u32 = 2;
/// A finite input produced a nonfinite stress or candidate velocity.
pub const STATUS_NONFINITE_RESULT: u32 = 4;

const WORKGROUP_SIZE: u32 = 64;

/// Physical coefficients and a conservative bound for one explicit substep.
///
/// `minimum_density_kg_m3` must not exceed any resident cell density.
/// `maximum_dynamic_viscosity_pa_s` must not be below any resident cell
/// viscosity. The corner pass checks both claims on the device and rejects
/// the candidate through `status` when they are false.
#[derive(Clone, Copy, Debug)]
pub struct GpuViscosityStep {
    /// Proposed substep duration in seconds.
    pub dt_s: f32,
    /// Conservative lower density bound in kg/m³.
    pub minimum_density_kg_m3: f32,
    /// Conservative upper dynamic-viscosity bound in Pa s.
    pub maximum_dynamic_viscosity_pa_s: f32,
}

/// Resident cell and MAC face fields for a closed-grid shear step.
pub struct GpuViscosityInput<'a> {
    /// Derived cell density in kg/m³.
    pub density_kg_m3: &'a wgpu::Buffer,
    /// Positive dynamic viscosity for each cell, in Pa s.
    pub cell_dynamic_viscosity_pa_s: &'a wgpu::Buffer,
    /// Horizontal face velocity in m/s.
    pub u_velocity_m_s: &'a wgpu::Buffer,
    /// Vertical face velocity in m/s.
    pub v_velocity_m_s: &'a wgpu::Buffer,
    /// Horizontal aperture in zero to one.
    pub u_aperture: &'a wgpu::Buffer,
    /// Vertical aperture in zero to one.
    pub v_aperture: &'a wgpu::Buffer,
}

/// Detached face velocities and scratch for one proposed shear substep.
pub struct GpuViscosityCandidate {
    /// Horizontal candidate velocities. Publish only when `status` is zero.
    pub u_velocity_m_s: wgpu::Buffer,
    /// Vertical candidate velocities. Publish only when `status` is zero.
    pub v_velocity_m_s: wgpu::Buffer,
    /// One stress per grid cell index; only interior corner entries are active.
    pub corner_stress_pa: wgpu::Buffer,
    /// Atomic completion status. Zero means the candidate passed stage checks.
    pub status: wgpu::Buffer,
}

impl GpuViscosityCandidate {
    /// Allocates candidate and scratch buffers after checking effective limits.
    pub fn new(device: &wgpu::Device, grid: Grid) -> Result<Self, String> {
        preflight(grid, &device.limits())?;
        let make = |label, size, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        let candidate_usage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST;
        Ok(Self {
            u_velocity_m_s: make(
                "viscosity candidate horizontal velocity",
                field_bytes(grid.u_faces()),
                candidate_usage,
            ),
            v_velocity_m_s: make(
                "viscosity candidate vertical velocity",
                field_bytes(grid.v_faces()),
                candidate_usage,
            ),
            corner_stress_pa: make(
                "viscosity corner stress",
                field_bytes(grid.cells()),
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            ),
            status: make(
                "viscosity candidate status",
                size_of::<u32>() as u64,
                wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_SRC
                    | wgpu::BufferUsages::COPY_DST,
            ),
        })
    }
}

/// Reusable corner-stress and owned-face gather pipelines for one grid.
pub struct GpuViscosityStage {
    grid: Grid,
    corners: wgpu::ComputePipeline,
    u_faces: wgpu::ComputePipeline,
    v_faces: wgpu::ComputePipeline,
}

impl GpuViscosityStage {
    /// Compiles closed-grid shear pipelines after checking effective limits.
    pub fn new(device: &wgpu::Device, grid: Grid) -> Result<Self, String> {
        preflight(grid, &device.limits())?;
        let corner_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("viscosity corner stress"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/viscosity_corner.wgsl").into(),
            ),
        });
        let face_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("viscosity face gather"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/viscosity_face.wgsl").into()),
        });
        let pipeline = |label, shader, entry_point| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(label),
                layout: None,
                module: shader,
                entry_point: Some(entry_point),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        Ok(Self {
            grid,
            corners: pipeline("viscosity corners", &corner_shader, "corners"),
            u_faces: pipeline("viscosity horizontal faces", &face_shader, "u_faces"),
            v_faces: pipeline("viscosity vertical faces", &face_shader, "v_faces"),
        })
    }

    /// Encodes a detached candidate. The caller submits the encoder and reads
    /// `candidate.status`. A nonzero value rejects both candidate face buffers.
    /// No input buffer is changed by this stage.
    pub fn encode(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        input: &GpuViscosityInput<'_>,
        candidate: &GpuViscosityCandidate,
        step: GpuViscosityStep,
    ) -> Result<(), String> {
        let dx_m = self.grid.cell_width_m() as f32;
        if !step.dt_s.is_finite() || step.dt_s <= 0.0 {
            return Err("viscosity dt must be finite and positive".into());
        }
        if !step.minimum_density_kg_m3.is_finite()
            || step.minimum_density_kg_m3 <= 0.0
            || !step.maximum_dynamic_viscosity_pa_s.is_finite()
            || step.maximum_dynamic_viscosity_pa_s <= 0.0
        {
            return Err("viscosity density and coefficient bounds must be positive".into());
        }
        let max_stable_dt_s = (dx_m as f64 * dx_m as f64) * step.minimum_density_kg_m3 as f64
            / (4.0 * step.maximum_dynamic_viscosity_pa_s as f64);
        if !max_stable_dt_s.is_finite() || max_stable_dt_s <= 0.0 {
            return Err("viscosity explicit step bound is invalid".into());
        }
        if step.dt_s as f64 > max_stable_dt_s {
            return Err(format!(
                "viscosity dt {} exceeds explicit bound {max_stable_dt_s}",
                step.dt_s
            ));
        }
        check_buffer_sizes(self.grid, input, candidate)?;

        let mut params = [0_u8; 32];
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
        for (slot, value) in [
            step.dt_s,
            step.minimum_density_kg_m3,
            step.maximum_dynamic_viscosity_pa_s,
            dx_m,
        ]
        .into_iter()
        .enumerate()
        {
            params[16 + slot * 4..20 + slot * 4].copy_from_slice(&value.to_le_bytes());
        }
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("viscosity step parameters"),
            contents: &params,
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let corners = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viscosity corner inputs"),
            layout: &self.corners.get_bind_group_layout(0),
            entries: &[
                entry(0, &uniform),
                entry(1, input.density_kg_m3),
                entry(2, input.cell_dynamic_viscosity_pa_s),
                entry(3, input.u_velocity_m_s),
                entry(4, input.v_velocity_m_s),
                entry(5, input.u_aperture),
                entry(6, input.v_aperture),
                entry(7, &candidate.corner_stress_pa),
                entry(8, &candidate.status),
            ],
        });
        let face_group = |pipeline: &wgpu::ComputePipeline,
                          velocity: &wgpu::Buffer,
                          aperture: &wgpu::Buffer,
                          output: &wgpu::Buffer| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("viscosity face inputs"),
                layout: &pipeline.get_bind_group_layout(0),
                entries: &[
                    entry(0, &uniform),
                    entry(1, input.density_kg_m3),
                    entry(2, velocity),
                    entry(3, aperture),
                    entry(4, &candidate.corner_stress_pa),
                    entry(5, output),
                    entry(6, &candidate.status),
                ],
            })
        };
        let u_faces = face_group(
            &self.u_faces,
            input.u_velocity_m_s,
            input.u_aperture,
            &candidate.u_velocity_m_s,
        );
        let v_faces = face_group(
            &self.v_faces,
            input.v_velocity_m_s,
            input.v_aperture,
            &candidate.v_velocity_m_s,
        );
        encoder.clear_buffer(&candidate.status, 0, None);
        dispatch(encoder, &self.corners, &corners, self.grid.cells());
        dispatch(encoder, &self.u_faces, &u_faces, self.grid.u_faces());
        dispatch(encoder, &self.v_faces, &v_faces, self.grid.v_faces());
        Ok(())
    }
}

fn field_bytes(entries: usize) -> u64 {
    entries as u64 * size_of::<f32>() as u64
}

fn preflight(grid: Grid, limits: &wgpu::Limits) -> Result<(), String> {
    GpuGridParams::new(grid, 2).map_err(|error| error.to_string())?;
    if limits.max_compute_invocations_per_workgroup < WORKGROUP_SIZE
        || limits.max_compute_workgroup_size_x < WORKGROUP_SIZE
        || limits.max_bindings_per_bind_group < 9
        || limits.max_storage_buffers_per_shader_stage < 8
        || limits.max_uniform_buffer_binding_size < 32
    {
        return Err("GPU adapter limits cannot run viscosity passes".into());
    }
    for entries in [grid.cells(), grid.u_faces(), grid.v_faces()] {
        if (entries as u32).div_ceil(WORKGROUP_SIZE) > limits.max_compute_workgroups_per_dimension {
            return Err("GPU viscosity dispatch exceeds effective workgroup limit".into());
        }
    }
    for bytes in [
        field_bytes(grid.cells()),
        field_bytes(grid.u_faces()),
        field_bytes(grid.v_faces()),
    ] {
        if bytes > limits.max_storage_buffer_binding_size || bytes > limits.max_buffer_size {
            return Err("GPU viscosity field exceeds effective storage limit".into());
        }
    }
    Ok(())
}

fn check_buffer_sizes(
    grid: Grid,
    input: &GpuViscosityInput<'_>,
    candidate: &GpuViscosityCandidate,
) -> Result<(), String> {
    for (label, buffer, needed) in [
        ("density", input.density_kg_m3, field_bytes(grid.cells())),
        (
            "dynamic viscosity",
            input.cell_dynamic_viscosity_pa_s,
            field_bytes(grid.cells()),
        ),
        (
            "horizontal velocity",
            input.u_velocity_m_s,
            field_bytes(grid.u_faces()),
        ),
        (
            "vertical velocity",
            input.v_velocity_m_s,
            field_bytes(grid.v_faces()),
        ),
        (
            "horizontal aperture",
            input.u_aperture,
            field_bytes(grid.u_faces()),
        ),
        (
            "vertical aperture",
            input.v_aperture,
            field_bytes(grid.v_faces()),
        ),
        (
            "candidate horizontal velocity",
            &candidate.u_velocity_m_s,
            field_bytes(grid.u_faces()),
        ),
        (
            "candidate vertical velocity",
            &candidate.v_velocity_m_s,
            field_bytes(grid.v_faces()),
        ),
        (
            "candidate corner stress",
            &candidate.corner_stress_pa,
            field_bytes(grid.cells()),
        ),
        (
            "candidate status",
            &candidate.status,
            size_of::<u32>() as u64,
        ),
    ] {
        if buffer.size() < needed {
            return Err(format!(
                "GPU viscosity {label} needs {needed} bytes, got {}",
                buffer.size()
            ));
        }
    }
    Ok(())
}

fn entry(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
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
        label: Some("viscosity candidate pass"),
        timestamp_writes: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, group, &[]);
    pass.dispatch_workgroups((entries as u32).div_ceil(WORKGROUP_SIZE), 1, 1);
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        sync::Arc,
        task::{Context, Poll, Wake, Waker},
        time::{Duration, Instant},
    };

    use particle_sim::contracts::Boundary;
    use particle_sim_cpu::{
        fluid::{FaceValues, PressureFields},
        viscosity::shear_viscosity_candidate,
    };

    use super::*;

    #[test]
    fn preflight_rejects_insufficient_storage_bindings_and_refined_spacing() {
        let grid = Grid::new(4.0, 4.0).unwrap();
        let limits = wgpu::Limits {
            max_storage_buffers_per_shader_stage: 7,
            ..wgpu::Limits::default()
        };
        assert!(preflight(grid, &limits).is_err());
        assert!(preflight(grid, &wgpu::Limits::default()).is_ok());
        let refined = Grid::with_cell_width(4.0, 4.0, 0.005).unwrap();
        assert!(preflight(refined, &wgpu::Limits::default()).is_err());
    }

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
                    assert!(Instant::now() < deadline, "GPU viscosity fixture timed out");
                    std::thread::park_timeout(Duration::from_millis(100));
                }
            }
        }
    }

    fn upload(device: &wgpu::Device, label: &'static str, values: &[f32]) -> wgpu::Buffer {
        let bytes = values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>();
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: &bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        })
    }

    fn read_bytes(device: &wgpu::Device, queue: &wgpu::Queue, source: &wgpu::Buffer) -> Vec<u8> {
        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("viscosity fixture readback"),
            size: source.size(),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("viscosity fixture readback"),
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
            .expect("viscosity fixture GPU submission");
        receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("viscosity fixture map callback")
            .expect("viscosity fixture mapped result");
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

    #[test]
    #[ignore = "requires a native GPU compute adapter"]
    fn closed_two_density_shear_matches_cpu_and_rejects_false_bounds() {
        let gpu = block_on(crate::GpuContext::new()).expect("native GPU adapter for E07 fixture");
        let grid = Grid::new(4.0, 4.0).unwrap();
        let density = (0..grid.cells())
            .map(|cell| if cell / 4 < 2 { 1.2 } else { 1200.0 })
            .collect::<Vec<_>>();
        let viscosity = (0..grid.cells())
            .map(|cell| if cell % 4 < 2 { 0.001 } else { 0.002 })
            .collect::<Vec<_>>();
        let mut u_velocity = vec![0.0; grid.u_faces()];
        let mut v_velocity = vec![0.0; grid.v_faces()];
        u_velocity[grid.u_face_index(1, 1).unwrap()] = 0.2;
        u_velocity[grid.u_face_index(2, 2).unwrap()] = -0.1;
        v_velocity[grid.v_face_index(1, 1).unwrap()] = 0.15;
        v_velocity[grid.v_face_index(2, 2).unwrap()] = -0.05;
        let mut u_aperture = vec![1.0; grid.u_faces()];
        let mut v_aperture = vec![1.0; grid.v_faces()];
        for y in 0..4 {
            u_aperture[grid.u_face_index(0, y).unwrap()] = 0.0;
            u_aperture[grid.u_face_index(4, y).unwrap()] = 0.0;
        }
        for x in 0..4 {
            v_aperture[grid.v_face_index(x, 0).unwrap()] = 0.0;
            v_aperture[grid.v_face_index(x, 4).unwrap()] = 0.0;
        }
        u_aperture[grid.u_face_index(1, 2).unwrap()] = 0.6;
        u_aperture[grid.u_face_index(3, 1).unwrap()] = 0.0;
        let fields = PressureFields::new(
            grid,
            [Boundary::Closed; 4],
            density.clone(),
            vec![0.0; grid.cells()],
            FaceValues {
                u: u_velocity.clone(),
                v: v_velocity.clone(),
            },
            FaceValues {
                u: u_aperture.clone(),
                v: v_aperture.clone(),
            },
        )
        .unwrap();
        let cpu = shear_viscosity_candidate(
            &fields,
            &FaceValues {
                u: u_velocity.clone(),
                v: v_velocity.clone(),
            },
            &viscosity,
            0.001,
        )
        .unwrap();
        let packed = |values: &[f64]| values.iter().map(|value| *value as f32).collect::<Vec<_>>();
        let density_buffer = upload(&gpu.device, "shear density", &packed(&density));
        let viscosity_buffer = upload(&gpu.device, "shear viscosity", &packed(&viscosity));
        let u_buffer = upload(&gpu.device, "shear u", &packed(&u_velocity));
        let v_buffer = upload(&gpu.device, "shear v", &packed(&v_velocity));
        let u_open_buffer = upload(&gpu.device, "shear u opening", &packed(&u_aperture));
        let v_open_buffer = upload(&gpu.device, "shear v opening", &packed(&v_aperture));
        let input = GpuViscosityInput {
            density_kg_m3: &density_buffer,
            cell_dynamic_viscosity_pa_s: &viscosity_buffer,
            u_velocity_m_s: &u_buffer,
            v_velocity_m_s: &v_buffer,
            u_aperture: &u_open_buffer,
            v_aperture: &v_open_buffer,
        };
        let stage = GpuViscosityStage::new(&gpu.device, grid).unwrap();
        let candidate = GpuViscosityCandidate::new(&gpu.device, grid).unwrap();
        let step = GpuViscosityStep {
            dt_s: 0.001,
            minimum_density_kg_m3: 1.2,
            maximum_dynamic_viscosity_pa_s: 0.002,
        };
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("viscosity parity fixture"),
            });
        stage
            .encode(&gpu.device, &mut encoder, &input, &candidate, step)
            .unwrap();
        gpu.queue.submit([encoder.finish()]);
        assert_eq!(
            u32::from_le_bytes(
                read_bytes(&gpu.device, &gpu.queue, &candidate.status)
                    .try_into()
                    .unwrap()
            ),
            0
        );
        for (actual, expected) in read_f32(&gpu.device, &gpu.queue, &candidate.u_velocity_m_s)
            .into_iter()
            .zip(&cpu.velocity_m_s.u)
        {
            assert!(
                (actual as f64 - expected).abs() <= 5e-7,
                "u {actual} != {expected}"
            );
        }
        for (actual, expected) in read_f32(&gpu.device, &gpu.queue, &candidate.v_velocity_m_s)
            .into_iter()
            .zip(&cpu.velocity_m_s.v)
        {
            assert!(
                (actual as f64 - expected).abs() <= 5e-7,
                "v {actual} != {expected}"
            );
        }
        assert_eq!(
            read_f32(&gpu.device, &gpu.queue, &candidate.u_velocity_m_s)
                [grid.u_face_index(3, 1).unwrap()],
            0.0
        );

        let rejected = GpuViscosityCandidate::new(&gpu.device, grid).unwrap();
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("viscosity false bound fixture"),
            });
        stage
            .encode(
                &gpu.device,
                &mut encoder,
                &input,
                &rejected,
                GpuViscosityStep {
                    minimum_density_kg_m3: 1.21,
                    ..step
                },
            )
            .unwrap();
        gpu.queue.submit([encoder.finish()]);
        let rejected_status = u32::from_le_bytes(
            read_bytes(&gpu.device, &gpu.queue, &rejected.status)
                .try_into()
                .unwrap(),
        );
        assert_ne!(rejected_status & STATUS_INVALID_CELL, 0);

        // Density derivation accepts a 1e-5 relative volume deficit. Its
        // lower density bound must allow that same interval, but no more.
        for (label, corner_density, expected_status) in [
            ("accepted closure deficit", 1.199_989_4, 0),
            ("rejected material void", 1.19, STATUS_INVALID_CELL),
        ] {
            let mut perturbed_density = packed(&density);
            perturbed_density[0] = corner_density;
            let perturbed_buffer = upload(&gpu.device, "shear closure density", &perturbed_density);
            let perturbed_input = GpuViscosityInput {
                density_kg_m3: &perturbed_buffer,
                cell_dynamic_viscosity_pa_s: &viscosity_buffer,
                u_velocity_m_s: &u_buffer,
                v_velocity_m_s: &v_buffer,
                u_aperture: &u_open_buffer,
                v_aperture: &v_open_buffer,
            };
            let mut encoder = gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("viscosity closure bound fixture"),
                });
            stage
                .encode(
                    &gpu.device,
                    &mut encoder,
                    &perturbed_input,
                    &rejected,
                    GpuViscosityStep {
                        minimum_density_kg_m3: 1.2 * (1.0 - 1e-5),
                        ..step
                    },
                )
                .unwrap();
            gpu.queue.submit([encoder.finish()]);
            let status = u32::from_le_bytes(
                read_bytes(&gpu.device, &gpu.queue, &rejected.status)
                    .try_into()
                    .unwrap(),
            );
            assert_eq!(status & STATUS_INVALID_CELL, expected_status, "{label}");
        }
        assert!(
            stage
                .encode(
                    &gpu.device,
                    &mut gpu
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("viscosity excessive dt fixture"),
                        }),
                    &input,
                    &rejected,
                    GpuViscosityStep { dt_s: 1.0, ..step },
                )
                .is_err()
        );
        gpu.dispose();
    }
}
