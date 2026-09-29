//! Detached f32 gravity predictor for closed MAC faces.
//!
//! Open horizontal speeds are copied. Open interior vertical speeds gain the
//! same downward acceleration as the E06 f64 reference. Blocked and outer
//! faces remain zero. The caller accepts neither output while status is set.

// The private E07 coupled stage graph will consume this verified stage.
#![allow(dead_code)]

use particle_sim::{Grid, gpu_layout::GpuGridParams};
use wgpu::util::DeviceExt;

const WORKGROUP_SIZE: u32 = 64;
const PARAM_BYTES: u64 = 32;

/// A face speed, aperture, or closed-wall constraint is invalid.
pub(crate) const STATUS_INVALID_FACE: u32 = 1;
/// Finite inputs produced an unrepresentable f32 predictor speed.
pub(crate) const STATUS_NONFINITE_RESULT: u32 = 2;

/// Positive gravity points down along the vertical MAC axis.
#[derive(Clone, Copy, Debug)]
pub(crate) struct GpuGravityStep {
    pub dt_s: f32,
    pub gravity_m_s2: f32,
}

/// Immutable resident speeds and apertures for one proposed predictor.
#[derive(Clone, Copy)]
pub(crate) struct GpuGravityInput<'a> {
    pub u_velocity_m_s: &'a wgpu::Buffer,
    pub v_velocity_m_s: &'a wgpu::Buffer,
    pub u_aperture: &'a wgpu::Buffer,
    pub v_aperture: &'a wgpu::Buffer,
}

/// Detached pressure predictor and one four-byte atomic failure word.
pub(crate) struct GpuGravityCandidate {
    pub u_velocity_m_s: wgpu::Buffer,
    pub v_velocity_m_s: wgpu::Buffer,
    pub status: wgpu::Buffer,
}

impl GpuGravityCandidate {
    pub(crate) fn new(device: &wgpu::Device, grid: Grid) -> Result<Self, String> {
        preflight(grid, &device.limits())?;
        let field = |label, faces| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: field_bytes(faces),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            })
        };
        Ok(Self {
            u_velocity_m_s: field("gravity candidate u", grid.u_faces()),
            v_velocity_m_s: field("gravity candidate v", grid.v_faces()),
            status: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("gravity candidate status"),
                size: size_of::<u32>() as u64,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_SRC
                    | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        })
    }
}

/// Reusable closed-grid predictor pipelines.
pub(crate) struct GpuGravityStage {
    grid: Grid,
    u_faces: wgpu::ComputePipeline,
    v_faces: wgpu::ComputePipeline,
}

impl GpuGravityStage {
    pub(crate) fn new(device: &wgpu::Device, grid: Grid) -> Result<Self, String> {
        preflight(grid, &device.limits())?;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gravity predictor"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/gravity_predictor.wgsl").into(),
            ),
        });
        let pipeline = |label, entry_point| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(label),
                layout: None,
                module: &shader,
                entry_point: Some(entry_point),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        Ok(Self {
            grid,
            u_faces: pipeline("gravity horizontal faces", "u_faces"),
            v_faces: pipeline("gravity vertical faces", "v_faces"),
        })
    }

    /// Writes a detached predictor. The caller checks status after submission.
    pub(crate) fn encode(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        input: &GpuGravityInput<'_>,
        candidate: &GpuGravityCandidate,
        step: GpuGravityStep,
    ) -> Result<(), String> {
        validate_step(step)?;
        check_buffer_sizes(self.grid, input, candidate)?;
        let mut bytes = [0_u8; PARAM_BYTES as usize];
        for (index, word) in [
            self.grid.width(),
            self.grid.height(),
            self.grid.cells() as u32,
            0,
        ]
        .into_iter()
        .enumerate()
        {
            bytes[index * 4..index * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
        for (index, word) in [step.dt_s, step.gravity_m_s2].into_iter().enumerate() {
            bytes[16 + index * 4..20 + index * 4].copy_from_slice(&word.to_le_bytes());
        }
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("gravity predictor parameters"),
            contents: &bytes,
            usage: wgpu::BufferUsages::UNIFORM,
        });
        encoder.clear_buffer(&candidate.status, 0, None);
        for (pipeline, source, aperture, output, faces) in [
            (
                &self.u_faces,
                input.u_velocity_m_s,
                input.u_aperture,
                &candidate.u_velocity_m_s,
                self.grid.u_faces(),
            ),
            (
                &self.v_faces,
                input.v_velocity_m_s,
                input.v_aperture,
                &candidate.v_velocity_m_s,
                self.grid.v_faces(),
            ),
        ] {
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("gravity predictor bindings"),
                layout: &pipeline.get_bind_group_layout(0),
                entries: &[
                    bind(0, &uniform),
                    bind(1, source),
                    bind(2, aperture),
                    bind(3, output),
                    bind(4, &candidate.status),
                ],
            });
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gravity predictor pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups((faces as u32).div_ceil(WORKGROUP_SIZE), 1, 1);
        }
        Ok(())
    }
}

fn bind(index: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding: index,
        resource: buffer.as_entire_binding(),
    }
}

fn field_bytes(faces: usize) -> u64 {
    faces as u64 * size_of::<f32>() as u64
}

fn validate_step(step: GpuGravityStep) -> Result<(), String> {
    if !step.dt_s.is_finite() || step.dt_s <= 0.0 {
        return Err("gravity predictor dt must be finite and positive".into());
    }
    if !step.gravity_m_s2.is_finite() {
        return Err("gravity predictor acceleration must be finite".into());
    }
    Ok(())
}

fn preflight(grid: Grid, limits: &wgpu::Limits) -> Result<(), String> {
    GpuGridParams::new(grid, 2).map_err(|error| error.to_string())?;
    if limits.max_compute_invocations_per_workgroup < WORKGROUP_SIZE
        || limits.max_compute_workgroup_size_x < WORKGROUP_SIZE
        || limits.max_bindings_per_bind_group < 5
        || limits.max_storage_buffers_per_shader_stage < 4
        || limits.max_uniform_buffer_binding_size < PARAM_BYTES
        || limits.max_buffer_size < PARAM_BYTES
    {
        return Err("GPU adapter limits cannot run gravity predictor".into());
    }
    for faces in [grid.u_faces(), grid.v_faces()] {
        if (faces as u32).div_ceil(WORKGROUP_SIZE) > limits.max_compute_workgroups_per_dimension
            || field_bytes(faces) > limits.max_storage_buffer_binding_size
            || field_bytes(faces) > limits.max_buffer_size
        {
            return Err("GPU gravity predictor field exceeds effective adapter limits".into());
        }
    }
    Ok(())
}

fn check_buffer_sizes(
    grid: Grid,
    input: &GpuGravityInput<'_>,
    candidate: &GpuGravityCandidate,
) -> Result<(), String> {
    for (label, buffer, bytes) in [
        (
            "source u",
            input.u_velocity_m_s,
            field_bytes(grid.u_faces()),
        ),
        (
            "source v",
            input.v_velocity_m_s,
            field_bytes(grid.v_faces()),
        ),
        ("u aperture", input.u_aperture, field_bytes(grid.u_faces())),
        ("v aperture", input.v_aperture, field_bytes(grid.v_faces())),
        (
            "candidate u",
            &candidate.u_velocity_m_s,
            field_bytes(grid.u_faces()),
        ),
        (
            "candidate v",
            &candidate.v_velocity_m_s,
            field_bytes(grid.v_faces()),
        ),
        ("candidate status", &candidate.status, 4),
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
    fn host_rejects_invalid_step_and_adapter_limits() {
        let grid = Grid::new(3.0, 3.0).unwrap();
        assert!(preflight(grid, &wgpu::Limits::default()).is_ok());
        let limits = wgpu::Limits {
            max_storage_buffers_per_shader_stage: 3,
            ..wgpu::Limits::default()
        };
        assert!(preflight(grid, &limits).is_err());
        for step in [
            GpuGravityStep {
                dt_s: 0.0,
                gravity_m_s2: 1.0,
            },
            GpuGravityStep {
                dt_s: f32::INFINITY,
                gravity_m_s2: 1.0,
            },
            GpuGravityStep {
                dt_s: 0.01,
                gravity_m_s2: f32::NAN,
            },
        ] {
            assert!(validate_step(step).is_err());
        }
        assert!(
            validate_step(GpuGravityStep {
                dt_s: 0.01,
                gravity_m_s2: 0.0
            })
            .is_ok()
        );
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
            solver::gravity_predictor_closed,
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
                        assert!(Instant::now() < deadline, "gravity GPU fixture timed out");
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

        fn upload(device: &wgpu::Device, label: &'static str, values: &[f32]) -> wgpu::Buffer {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: &f32_bytes(values.iter().copied()),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            })
        }

        fn read_bytes(
            device: &wgpu::Device,
            queue: &wgpu::Queue,
            source: &wgpu::Buffer,
        ) -> Vec<u8> {
            let staging = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("gravity fixture readback"),
                size: source.size(),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gravity fixture copy"),
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
                .expect("gravity fixture GPU submission");
            receiver
                .recv_timeout(Duration::from_secs(10))
                .expect("gravity fixture map callback")
                .expect("gravity fixture mapped result");
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

        fn run(
            gpu: &crate::GpuContext,
            stage: &GpuGravityStage,
            grid: Grid,
            input: &GpuGravityInput<'_>,
            step: GpuGravityStep,
        ) -> GpuGravityCandidate {
            let candidate = GpuGravityCandidate::new(&gpu.device, grid).unwrap();
            let mut encoder = gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("gravity fixture candidate"),
                });
            stage
                .encode(&gpu.device, &mut encoder, input, &candidate, step)
                .unwrap();
            gpu.queue.submit([encoder.finish()]);
            candidate
        }

        #[test]
        #[ignore = "requires a native GPU adapter"]
        fn closed_predictor_matches_cpu_and_rejects_bad_faces_or_f32_overflow() {
            let gpu =
                block_on(crate::GpuContext::new()).expect("native GPU adapter for gravity fixture");
            let grid = Grid::new(3.0, 3.0).unwrap();
            let mut u = vec![0.0; grid.u_faces()];
            let mut v = vec![0.0; grid.v_faces()];
            let mut u_aperture = vec![0.0; grid.u_faces()];
            let mut v_aperture = vec![0.0; grid.v_faces()];
            for y in 0..grid.height() {
                for x in 1..grid.width() {
                    u_aperture[grid.u_face_index(x, y).unwrap()] = 1.0;
                }
            }
            for y in 1..grid.height() {
                for x in 0..grid.width() {
                    v_aperture[grid.v_face_index(x, y).unwrap()] = 1.0;
                }
            }
            let u_fractional = grid.u_face_index(2, 1).unwrap();
            u_aperture[u_fractional] = 0.5;
            let u_blocked = grid.u_face_index(1, 1).unwrap();
            u_aperture[u_blocked] = 0.0;
            u[grid.u_face_index(1, 0).unwrap()] = 0.2;
            u[u_fractional] = -0.3;
            let v_fractional = grid.v_face_index(0, 1).unwrap();
            v_aperture[v_fractional] = 0.25;
            let v_blocked = grid.v_face_index(1, 2).unwrap();
            v_aperture[v_blocked] = 0.0;
            for (x, y, speed) in [
                (0, 1, 0.4),
                (1, 1, -0.2),
                (2, 1, 0.1),
                (0, 2, -0.3),
                (2, 2, 0.5),
            ] {
                v[grid.v_face_index(x, y).unwrap()] = speed;
            }
            let fields = PressureFields::new(
                grid,
                [Boundary::Closed; 4],
                vec![1.0; grid.cells()],
                vec![0.0; grid.cells()],
                FaceValues {
                    u: u.clone(),
                    v: v.clone(),
                },
                FaceValues {
                    u: u_aperture.clone(),
                    v: v_aperture.clone(),
                },
            )
            .unwrap();
            let cpu = gravity_predictor_closed(&fields, 0.02, 9.81).unwrap();
            let u_values = u.iter().map(|value| *value as f32).collect::<Vec<_>>();
            let v_values = v.iter().map(|value| *value as f32).collect::<Vec<_>>();
            let u_open = u_aperture
                .iter()
                .map(|value| *value as f32)
                .collect::<Vec<_>>();
            let v_open = v_aperture
                .iter()
                .map(|value| *value as f32)
                .collect::<Vec<_>>();
            let u_buffer = upload(&gpu.device, "gravity source u", &u_values);
            let v_buffer = upload(&gpu.device, "gravity source v", &v_values);
            let u_open_buffer = upload(&gpu.device, "gravity u aperture", &u_open);
            let v_open_buffer = upload(&gpu.device, "gravity v aperture", &v_open);
            let input = GpuGravityInput {
                u_velocity_m_s: &u_buffer,
                v_velocity_m_s: &v_buffer,
                u_aperture: &u_open_buffer,
                v_aperture: &v_open_buffer,
            };
            let stage = GpuGravityStage::new(&gpu.device, grid).unwrap();
            let candidate = run(
                &gpu,
                &stage,
                grid,
                &input,
                GpuGravityStep {
                    dt_s: 0.02,
                    gravity_m_s2: 9.81,
                },
            );
            let status = read_bytes(&gpu.device, &gpu.queue, &candidate.status);
            assert_eq!(u32::from_le_bytes(status.try_into().unwrap()), 0);
            let actual_u = read_f32(&gpu.device, &gpu.queue, &candidate.u_velocity_m_s);
            let actual_v = read_f32(&gpu.device, &gpu.queue, &candidate.v_velocity_m_s);
            for (actual, expected) in actual_u.iter().zip(&cpu.u) {
                assert!((f64::from(*actual) - expected).abs() <= 1e-6);
            }
            for (actual, expected) in actual_v.iter().zip(&cpu.v) {
                assert!((f64::from(*actual) - expected).abs() <= 1e-6);
            }
            assert_eq!(actual_u[u_blocked], 0.0);
            assert_eq!(actual_v[v_blocked], 0.0);
            assert!((actual_v[v_fractional] - v_values[v_fractional] - 0.02 * 9.81).abs() <= 1e-6);
            assert_eq!(read_f32(&gpu.device, &gpu.queue, &u_buffer), u_values);
            assert_eq!(read_f32(&gpu.device, &gpu.queue, &v_buffer), v_values);

            let zero = run(
                &gpu,
                &stage,
                grid,
                &input,
                GpuGravityStep {
                    dt_s: 0.02,
                    gravity_m_s2: 0.0,
                },
            );
            let zero_status = read_bytes(&gpu.device, &gpu.queue, &zero.status);
            assert_eq!(u32::from_le_bytes(zero_status.try_into().unwrap()), 0);
            assert_eq!(
                read_f32(&gpu.device, &gpu.queue, &zero.u_velocity_m_s),
                u_values
            );
            assert_eq!(
                read_f32(&gpu.device, &gpu.queue, &zero.v_velocity_m_s),
                v_values
            );

            let mut bad_aperture = v_open.clone();
            bad_aperture[v_fractional] = 1.2;
            let bad_aperture_buffer =
                upload(&gpu.device, "gravity invalid aperture", &bad_aperture);
            let bad_aperture_input = GpuGravityInput {
                v_aperture: &bad_aperture_buffer,
                ..input
            };
            let rejected = run(
                &gpu,
                &stage,
                grid,
                &bad_aperture_input,
                GpuGravityStep {
                    dt_s: 0.02,
                    gravity_m_s2: 9.81,
                },
            );
            let rejected_status = read_bytes(&gpu.device, &gpu.queue, &rejected.status);
            assert_ne!(
                u32::from_le_bytes(rejected_status.try_into().unwrap()) & STATUS_INVALID_FACE,
                0
            );

            let mut bad_speed = v_values.clone();
            bad_speed[v_fractional] = f32::NAN;
            let bad_speed_buffer = upload(&gpu.device, "gravity nonfinite speed", &bad_speed);
            let bad_speed_input = GpuGravityInput {
                v_velocity_m_s: &bad_speed_buffer,
                ..input
            };
            let rejected = run(
                &gpu,
                &stage,
                grid,
                &bad_speed_input,
                GpuGravityStep {
                    dt_s: 0.02,
                    gravity_m_s2: 9.81,
                },
            );
            let rejected_status = read_bytes(&gpu.device, &gpu.queue, &rejected.status);
            assert_ne!(
                u32::from_le_bytes(rejected_status.try_into().unwrap()) & STATUS_INVALID_FACE,
                0
            );

            let overflow = run(
                &gpu,
                &stage,
                grid,
                &input,
                GpuGravityStep {
                    dt_s: 1e30,
                    gravity_m_s2: 1e30,
                },
            );
            let overflow_status = read_bytes(&gpu.device, &gpu.queue, &overflow.status);
            assert_ne!(
                u32::from_le_bytes(overflow_status.try_into().unwrap()) & STATUS_NONFINITE_RESULT,
                0
            );
            println!(
                "GPU gravity: 3×3 closed fixture matches E06 CPU within 1e-6 m/s; fractional/blocked faces, zero gravity, invalid aperture/speed, and f32 overflow checked"
            );
            gpu.dispose();
        }
    }
}
