//! Compact GPU CFL observation for detached closed-grid substeps.
//!
//! The device reduces the largest signed incoming or outgoing speed sum per
//! cell. It returns eight bytes: a nonnegative f32 rate and an error status.
//! Host selection rounds both the rate bound and f32 duration conservatively.
//! Neither measurement nor selection advances accepted physical time.

use std::{
    future::poll_fn,
    sync::{Arc, Mutex},
    task::{Poll, Waker},
};

use particle_sim::{Grid, gpu_layout::GpuGridParams};
use wgpu::util::DeviceExt;

const WORKGROUP_SIZE: u32 = 64;
const RESULT_BYTES: u64 = 8;
const MAX_CELL_CFL: f64 = 0.5;
/// Three f32 additions build a four-face rate. This multiplier exceeds their
/// worst-case normal-value rounding loss and includes room for f64 arithmetic.
const RATE_HEADROOM: f64 = 1.0 + 4.0 * f32::EPSILON as f64;

/// A face is NaN, infinite, or subnormal. Subnormals cannot be trusted across
/// GPUs that flush them to zero during arithmetic.
#[cfg(test)]
pub(crate) const STATUS_INVALID_SPEED: u32 = 1;
/// A sum of finite face speeds overflowed f32.
#[cfg(test)]
pub(crate) const STATUS_INVALID_RATE: u32 = 2;

/// A checked, compact observation of one resident MAC velocity generation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GpuCflMeasurement {
    grid: Grid,
    /// Largest device-computed f32 signed incoming or outgoing sum in m/s.
    pub(crate) max_cell_rate_m_s: f32,
}

/// A proposed f32 duration and its conservative host limits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GpuCflSelection {
    /// Duration to pass to the f32 GPU stages. Time is not accepted yet.
    pub(crate) dt_s: f32,
    /// Limit after inflating the device rate for f32 rounding.
    pub(crate) max_cfl_dt_s: Option<f64>,
    /// Explicit diffusion limit, if viscosity is positive.
    pub(crate) max_diffusion_dt_s: Option<f64>,
}

impl GpuCflMeasurement {
    /// Selects one detached duration from remaining time and a conservative
    /// maximum kinematic viscosity. The owner accepts time only after all
    /// later GPU stages and their status checks succeed.
    pub(crate) fn select_duration(
        self,
        remaining_s: f64,
        max_kinematic_viscosity_m2_s: f64,
    ) -> Result<GpuCflSelection, String> {
        if !remaining_s.is_finite() || remaining_s <= 0.0 {
            return Err("GPU CFL remaining time must be finite and positive".into());
        }
        if !max_kinematic_viscosity_m2_s.is_finite() || max_kinematic_viscosity_m2_s < 0.0 {
            return Err("GPU CFL kinematic viscosity bound must be finite and nonnegative".into());
        }
        if !self.max_cell_rate_m_s.is_finite() || self.max_cell_rate_m_s < 0.0 {
            return Err("GPU CFL measured rate is invalid".into());
        }
        let dx_m = self.grid.cell_width_m();
        let max_cfl_dt_s = if self.max_cell_rate_m_s == 0.0 {
            None
        } else {
            let rate_upper_m_s = self.max_cell_rate_m_s as f64 * RATE_HEADROOM;
            let limit = MAX_CELL_CFL * dx_m / rate_upper_m_s;
            if !limit.is_finite() || limit <= 0.0 {
                return Err("GPU CFL duration cannot be represented".into());
            }
            Some(limit)
        };
        let max_diffusion_dt_s = if max_kinematic_viscosity_m2_s == 0.0 {
            None
        } else {
            let limit = dx_m * dx_m / (4.0 * max_kinematic_viscosity_m2_s);
            if limit.is_infinite() {
                None
            } else if !limit.is_finite() || limit <= 0.0 {
                return Err("GPU diffusion duration cannot be represented".into());
            } else {
                Some(limit)
            }
        };
        let mut allowed_s = remaining_s;
        if let Some(limit) = max_cfl_dt_s {
            allowed_s = allowed_s.min(limit);
        }
        if let Some(limit) = max_diffusion_dt_s {
            allowed_s = allowed_s.min(limit);
        }
        let mut dt_s = allowed_s.min(f32::MAX as f64) as f32;
        if dt_s as f64 > allowed_s {
            dt_s = f32::from_bits(dt_s.to_bits() - 1);
        }
        if !dt_s.is_finite() || dt_s <= 0.0 || dt_s as f64 > allowed_s {
            return Err("GPU CFL duration cannot be represented in positive f32".into());
        }
        Ok(GpuCflSelection {
            dt_s,
            max_cfl_dt_s,
            max_diffusion_dt_s,
        })
    }
}

/// Reusable reduction for one base-spacing grid. Each measurement owns its
/// eight-byte staging buffer, so a cancelled map cannot affect a later call.
pub(crate) struct GpuCflStage {
    grid: Grid,
    pipeline: wgpu::ComputePipeline,
    result: wgpu::Buffer,
}

impl GpuCflStage {
    /// Checks device limits before compiling or allocating the reduction.
    pub(crate) fn new(device: &wgpu::Device, grid: Grid) -> Result<Self, String> {
        preflight(grid, &device.limits())?;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("cellwise CFL rate"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/cfl_rate.wgsl").into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("cellwise CFL reduction"),
            layout: None,
            module: &shader,
            entry_point: Some("reduce_rate"),
            compilation_options: Default::default(),
            cache: None,
        });
        let result = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("CFL max rate and status"),
            size: RESULT_BYTES,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Ok(Self {
            grid,
            pipeline,
            result,
        })
    }

    /// Measures resident face speeds. The mutable receiver orders submissions;
    /// each call allocates its own compact staging buffer for cancellation safety.
    /// No face buffer or clock changes.
    pub(crate) async fn measure(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        u_velocity_m_s: &wgpu::Buffer,
        v_velocity_m_s: &wgpu::Buffer,
    ) -> Result<GpuCflMeasurement, String> {
        let staging = self.submit_measurement(device, queue, u_velocity_m_s, v_velocity_m_s)?;
        wait_for_map(device, &staging).await?;
        let mapped = staging.get_mapped_range(..);
        let rate_bits = u32::from_le_bytes(mapped[..4].try_into().expect("rate word"));
        let status = u32::from_le_bytes(mapped[4..8].try_into().expect("status word"));
        drop(mapped);
        staging.unmap();
        if status != 0 {
            return Err(format!(
                "GPU CFL rate rejected resident face data: status {status}"
            ));
        }
        let max_cell_rate_m_s = f32::from_bits(rate_bits);
        if !max_cell_rate_m_s.is_finite() || max_cell_rate_m_s < 0.0 {
            return Err("GPU CFL reduction returned an invalid rate".into());
        }
        Ok(GpuCflMeasurement {
            grid: self.grid,
            max_cell_rate_m_s,
        })
    }

    fn submit_measurement(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        u_velocity_m_s: &wgpu::Buffer,
        v_velocity_m_s: &wgpu::Buffer,
    ) -> Result<wgpu::Buffer, String> {
        let u_bytes = field_bytes(self.grid.u_faces());
        let v_bytes = field_bytes(self.grid.v_faces());
        if u_velocity_m_s.size() < u_bytes || v_velocity_m_s.size() < v_bytes {
            return Err("GPU CFL face buffer is shorter than its grid layout".into());
        }
        // The eight-byte staging allocation is covered by `preflight` and is
        // local to this submission. Dropping a pending map drops this handle.
        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("CFL compact readback"),
            size: RESULT_BYTES,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let params = GpuGridParams::new(self.grid, 2)
            .map_err(|error| error.to_string())?
            .to_le_bytes();
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("CFL grid parameters"),
            contents: &params,
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("CFL resident faces"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                bind(0, &uniform),
                bind(1, u_velocity_m_s),
                bind(2, v_velocity_m_s),
                bind(3, &self.result),
            ],
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("CFL rate reduction"),
        });
        encoder.clear_buffer(&self.result, 0, None);
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("CFL signed cell rate"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups((self.grid.cells() as u32).div_ceil(WORKGROUP_SIZE), 1, 1);
        }
        encoder.copy_buffer_to_buffer(&self.result, 0, &staging, 0, RESULT_BYTES);
        queue.submit([encoder.finish()]);
        Ok(staging)
    }
}

fn bind(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
    }
}

fn field_bytes(entries: usize) -> u64 {
    entries as u64 * size_of::<f32>() as u64
}

fn preflight(grid: Grid, limits: &wgpu::Limits) -> Result<(), String> {
    GpuGridParams::new(grid, 2).map_err(|error| error.to_string())?;
    if limits.max_compute_invocations_per_workgroup < WORKGROUP_SIZE
        || limits.max_compute_workgroup_size_x < WORKGROUP_SIZE
        || limits.max_compute_workgroups_per_dimension
            < (grid.cells() as u32).div_ceil(WORKGROUP_SIZE)
        || limits.max_bindings_per_bind_group < 4
        || limits.max_storage_buffers_per_shader_stage < 3
        || limits.max_uniform_buffer_binding_size < 16
    {
        return Err("GPU adapter limits cannot run CFL reduction".into());
    }
    for bytes in [
        field_bytes(grid.u_faces()),
        field_bytes(grid.v_faces()),
        RESULT_BYTES,
    ] {
        if bytes > limits.max_storage_buffer_binding_size || bytes > limits.max_buffer_size {
            return Err("GPU CFL field exceeds effective buffer limit".into());
        }
    }
    Ok(())
}

#[derive(Default)]
struct MapCompletion {
    result: Option<Result<(), String>>,
    waker: Option<Waker>,
}

async fn wait_for_map(device: &wgpu::Device, staging: &wgpu::Buffer) -> Result<(), String> {
    let completion = Arc::new(Mutex::new(MapCompletion::default()));
    let callback_completion = Arc::clone(&completion);
    staging.map_async(wgpu::MapMode::Read, .., move |result| {
        let waker = {
            let mut state = callback_completion.lock().expect("CFL map completion lock");
            state.result =
                Some(result.map_err(|error| format!("GPU CFL staging map failed: {error}")));
            state.waker.take()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    });
    #[cfg(not(target_arch = "wasm32"))]
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(10)),
        })
        .map_err(|error| format!("GPU CFL submission did not complete: {error}"))?;
    #[cfg(target_arch = "wasm32")]
    let _ = device;
    poll_fn(|cx| {
        let mut state = completion.lock().expect("CFL map completion lock");
        if let Some(result) = state.result.take() {
            Poll::Ready(result)
        } else {
            state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    })
    .await
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        sync::Arc,
        task::{Context, Poll, Wake, Waker},
        time::{Duration, Instant},
    };

    use particle_sim::OUTER_DT_S;
    use particle_sim_cpu::{
        fluid::FaceValues,
        substep::{SubstepClock, SubstepSelection},
    };

    use super::*;

    #[test]
    fn selected_f32_duration_never_exceeds_cpu_geometric_or_diffusion_bound() {
        let grid = Grid::new(1.0, 1.0).unwrap();
        let measurement = GpuCflMeasurement {
            grid,
            max_cell_rate_m_s: 7.0,
        };
        let choice = measurement.select_duration(OUTER_DT_S, 0.0).unwrap();
        assert!(choice.dt_s as f64 <= 0.005 / 7.0);
        assert!(choice.max_cfl_dt_s.unwrap() < 0.005 / 7.0);
        assert_eq!(choice.max_diffusion_dt_s, None);

        let zero = GpuCflMeasurement {
            grid,
            max_cell_rate_m_s: 0.0,
        };
        let choice = zero.select_duration(0.002, 0.05).unwrap();
        assert_eq!(choice.max_cfl_dt_s, None);
        assert_eq!(choice.max_diffusion_dt_s, Some(0.0005));
        assert!(choice.dt_s as f64 <= 0.0005);
        assert!(zero.select_duration(f64::MIN_POSITIVE, 0.0).is_err());
    }

    #[test]
    fn preflight_covers_full_scene_dispatch_and_effective_limits() {
        let grid = Grid::new(480.0, 270.0).unwrap();
        assert!(preflight(grid, &wgpu::Limits::default()).is_ok());
        let limits = wgpu::Limits {
            max_compute_workgroups_per_dimension: 2024,
            ..wgpu::Limits::default()
        };
        assert!(preflight(grid, &limits).is_err());
        let refined = Grid::with_cell_width(1.0, 1.0, 0.005).unwrap();
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
                    assert!(Instant::now() < deadline, "GPU CFL fixture timed out");
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
            usage: wgpu::BufferUsages::STORAGE,
        })
    }

    fn cpu_limit(grid: Grid, u: &[f32], v: &[f32], nu: f64) -> f64 {
        let clock = SubstepClock::new(OUTER_DT_S, 32).unwrap();
        let velocity = FaceValues {
            u: u.iter().map(|&value| value as f64).collect(),
            v: v.iter().map(|&value| value as f64).collect(),
        };
        let selection = clock.select(grid, &velocity, nu).unwrap();
        assert_eq!(clock.accepted_time_s(), 0.0);
        match selection {
            SubstepSelection::Candidate(step) => step.dt_s(),
            other => panic!("expected CPU candidate, got {other:?}"),
        }
    }

    #[test]
    #[ignore = "requires a native GPU compute adapter"]
    fn native_gpu_rate_matches_cpu_and_rejects_invalid_arithmetic() {
        let gpu = block_on(crate::GpuContext::new()).expect("native GPU adapter for E07 fixture");
        let grid = Grid::new(1.0, 1.0).unwrap();
        let mut stage = GpuCflStage::new(&gpu.device, grid).unwrap();
        for (u, v, rate, nu) in [
            ([-3.0, 3.0], [0.0, 0.0], 6.0, 0.0),
            ([3.0, 3.0], [4.0, 4.0], 7.0, 0.0),
            ([0.0, 0.0], [0.0, 0.0], 0.0, 0.05),
        ] {
            let u_buffer = upload(&gpu.device, "CFL fixture u", &u);
            let v_buffer = upload(&gpu.device, "CFL fixture v", &v);
            let measured =
                block_on(stage.measure(&gpu.device, &gpu.queue, &u_buffer, &v_buffer)).unwrap();
            assert_eq!(measured.max_cell_rate_m_s, rate);
            let gpu_choice = measured.select_duration(OUTER_DT_S, nu).unwrap();
            let cpu_dt = cpu_limit(grid, &u, &v, nu);
            assert!(gpu_choice.dt_s as f64 <= cpu_dt);
            assert!((gpu_choice.dt_s as f64 - cpu_dt).abs() <= cpu_dt * 2e-6);
        }

        let two_cells = Grid::new(2.0, 1.0).unwrap();
        let mut signed_zero_stage = GpuCflStage::new(&gpu.device, two_cells).unwrap();
        let u = upload(&gpu.device, "CFL signed zero u", &[-0.0, 0.0, 3.0]);
        let v = upload(&gpu.device, "CFL signed zero v", &[0.0; 4]);
        let measured =
            block_on(signed_zero_stage.measure(&gpu.device, &gpu.queue, &u, &v)).unwrap();
        assert_eq!(measured.max_cell_rate_m_s, 3.0);

        // Cancellation after submission can leave the old map pending. Its
        // staging buffer must never be reused by the next measurement.
        let u_one = upload(&gpu.device, "CFL pending-map u", &[-3.0, 3.0]);
        let v_one = upload(&gpu.device, "CFL pending-map v", &[0.0, 0.0]);
        let orphan = stage
            .submit_measurement(&gpu.device, &gpu.queue, &u_one, &v_one)
            .unwrap();
        orphan.map_async(wgpu::MapMode::Read, .., |_| {});
        drop(orphan);
        let next = block_on(stage.measure(&gpu.device, &gpu.queue, &u_one, &v_one)).unwrap();
        assert_eq!(next.max_cell_rate_m_s, 6.0);

        for (u, v, expected_status) in [
            ([f32::NAN, 0.0], [0.0, 0.0], STATUS_INVALID_SPEED),
            ([0.0, f32::INFINITY], [0.0, 0.0], STATUS_INVALID_SPEED),
            ([f32::from_bits(1), 0.0], [0.0, 0.0], STATUS_INVALID_SPEED),
            (
                [-f32::MAX, f32::MAX],
                [-f32::MAX, f32::MAX],
                STATUS_INVALID_RATE,
            ),
        ] {
            let u_buffer = upload(&gpu.device, "CFL invalid u", &u);
            let v_buffer = upload(&gpu.device, "CFL invalid v", &v);
            let error =
                block_on(stage.measure(&gpu.device, &gpu.queue, &u_buffer, &v_buffer)).unwrap_err();
            assert!(
                error.contains(&format!("status {expected_status}")),
                "{error}"
            );
        }

        let full_grid = Grid::new(480.0, 270.0).unwrap();
        let mut full_stage = GpuCflStage::new(&gpu.device, full_grid).unwrap();
        let u = upload(
            &gpu.device,
            "CFL full-grid u",
            &vec![0.0; full_grid.u_faces()],
        );
        let v = upload(
            &gpu.device,
            "CFL full-grid v",
            &vec![0.0; full_grid.v_faces()],
        );
        let full_rate = block_on(full_stage.measure(&gpu.device, &gpu.queue, &u, &v)).unwrap();
        assert_eq!(full_rate.max_cell_rate_m_s, 0.0);
        gpu.dispose();
    }
}
