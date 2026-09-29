//! One bounded observation from the selected resident scene generation.
//!
//! The owner captures the stamp and committed mass buffer before submission.
//! It must compare the returned stamp with current scene progress after the
//! asynchronous read completes. A different revision makes the result stale,
//! including paint edits that accept no physical time.

use std::{
    future::poll_fn,
    sync::{Arc, Mutex},
    task::{Poll, Waker},
};

use particle_sim::Grid;
use wgpu::util::DeviceExt;

const PROBE_BYTES: u64 = 16;

/// Scene identity captured when the probe enters the GPU queue.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ProbeStamp {
    pub epoch: u64,
    pub tick: u64,
    pub accepted_time_s: f64,
    pub committed_generation: usize,
    pub state_revision: u64,
}

impl ProbeStamp {
    /// Rejects values that cannot identify an accepted resident scene state.
    pub fn validate(self) -> Result<Self, String> {
        if self.epoch == 0
            || !self.accepted_time_s.is_finite()
            || self.accepted_time_s < 0.0
            || self.committed_generation > 1
        {
            return Err("GPU probe stamp is invalid".into());
        }
        Ok(self)
    }
}

/// Exactly one cell from the committed two-phase mass plane and fixed headers.
pub(crate) struct ProbeSource<'a> {
    pub grid: Grid,
    pub headers: &'a wgpu::Buffer,
    pub committed_mass_kg: &'a wgpu::Buffer,
    pub stamp: ProbeStamp,
}

/// A compact observation. Masses are resident f32 values, in kilograms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ProbeSample {
    pub cell: [u32; 2],
    pub mass_kg: [f32; 2],
    pub fixed_wall: bool,
    pub stamp: ProbeStamp,
}

impl ProbeSample {
    /// Call after the readback completes with the owner's current stamp.
    pub fn is_current(self, current: ProbeStamp) -> bool {
        self.stamp == current
    }
}

/// Reusable single-cell sampling pipeline. It owns no scene data.
pub(crate) struct GpuCellProbe {
    pipeline: wgpu::ComputePipeline,
}

/// A submitted four-word readback with its immutable submission stamp.
pub(crate) struct ProbePending {
    staging: wgpu::Buffer,
    cell: [u32; 2],
    stamp: ProbeStamp,
}

impl GpuCellProbe {
    pub fn new(device: &wgpu::Device) -> Result<Self, String> {
        let limits = device.limits();
        if limits.max_uniform_buffer_binding_size < PROBE_BYTES
            || limits.max_storage_buffer_binding_size < PROBE_BYTES
            || limits.max_buffer_size < PROBE_BYTES
            || limits.max_storage_buffers_per_shader_stage < 3
            || limits.max_bindings_per_bind_group < 4
            || limits.max_compute_workgroups_per_dimension == 0
        {
            return Err("GPU adapter limits cannot sample a cell".into());
        }
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("single-cell probe shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/probe_cell.wgsl").into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("single-cell probe"),
            layout: None,
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        Ok(Self { pipeline })
    }

    /// Submits a read of one checked cell from the provided committed buffer.
    /// The caller must compare the result's stamp after awaiting `resolve`.
    pub fn submit(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        source: ProbeSource<'_>,
        cell: [u32; 2],
    ) -> Result<ProbePending, String> {
        let stamp = source.stamp.validate()?;
        let index = source
            .grid
            .cell_index(cell[0], cell[1])
            .ok_or("GPU probe cell is outside the scene grid")?;
        let cells = source.grid.cells() as u64;
        let header_bytes = cells
            .checked_mul(8)
            .ok_or("GPU probe header size overflow")?;
        let mass_bytes = cells.checked_mul(8).ok_or("GPU probe mass size overflow")?;
        let limits = device.limits();
        if header_bytes > limits.max_storage_buffer_binding_size
            || mass_bytes > limits.max_storage_buffer_binding_size
            || source.headers.size() < header_bytes
            || source.committed_mass_kg.size() < mass_bytes
            || !source.headers.usage().contains(wgpu::BufferUsages::STORAGE)
            || !source
                .committed_mass_kg
                .usage()
                .contains(wgpu::BufferUsages::STORAGE)
        {
            return Err("GPU probe source buffers do not match the scene grid".into());
        }

        let mut params = [0_u8; PROBE_BYTES as usize];
        params[..4].copy_from_slice(&(cells as u32).to_le_bytes());
        params[4..8].copy_from_slice(&(index as u32).to_le_bytes());
        let params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("single-cell probe parameters"),
            contents: &params,
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let output = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("single-cell probe output"),
            size: PROBE_BYTES,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("single-cell probe readback"),
            size: PROBE_BYTES,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("single-cell probe inputs"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: source.headers.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: source.committed_mass_kg.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: output.as_entire_binding(),
                },
            ],
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("single-cell probe submission"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("single-cell probe dispatch"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bindings, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&output, 0, &staging, 0, PROBE_BYTES);
        queue.submit([encoder.finish()]);
        Ok(ProbePending {
            staging,
            cell,
            stamp,
        })
    }
}

impl ProbePending {
    /// Resolves only the submitted cell. The scene owner must reject stale data.
    pub async fn resolve(self, device: &wgpu::Device) -> Result<ProbeSample, String> {
        wait_for_map(device, &self.staging).await?;
        let mapped = self.staging.get_mapped_range(..);
        let words = mapped
            .as_chunks::<4>()
            .0
            .iter()
            .map(|bytes| u32::from_le_bytes(*bytes))
            .collect::<Vec<_>>();
        drop(mapped);
        self.staging.unmap();
        let mass_kg = [f32::from_bits(words[0]), f32::from_bits(words[1])];
        if mass_kg.iter().any(|mass| !mass.is_finite() || *mass < 0.0)
            || words[2] > 1
            || words[3] != 0
        {
            return Err("GPU probe read invalid resident cell data".into());
        }
        Ok(ProbeSample {
            cell: self.cell,
            mass_kg,
            fixed_wall: words[2] == 1,
            stamp: self.stamp,
        })
    }
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
            let mut state = callback_completion
                .lock()
                .expect("probe map completion lock");
            state.result = Some(result.map_err(|error| format!("GPU probe map failed: {error}")));
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
            timeout: Some(std::time::Duration::from_secs(30)),
        })
        .map_err(|error| format!("GPU probe submission did not complete: {error}"))?;
    #[cfg(target_arch = "wasm32")]
    let _ = device;
    poll_fn(|cx| {
        let mut state = completion.lock().expect("probe map completion lock");
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
    use super::*;
    use particle_sim::gpu_layout::GpuCellHeader;
    use std::{
        future::Future,
        task::{Context, Wake},
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
                    assert!(Instant::now() < deadline, "GPU probe test timed out");
                    std::thread::park_timeout(Duration::from_millis(100));
                }
            }
        }
    }

    fn stamp() -> ProbeStamp {
        ProbeStamp {
            epoch: 2,
            tick: 4,
            accepted_time_s: 0.05,
            committed_generation: 1,
            state_revision: 9,
        }
    }

    #[test]
    fn stamp_detects_revision_even_without_clock_change() {
        let sample = ProbeSample {
            cell: [0, 0],
            mass_kg: [1.0, 0.0],
            fixed_wall: false,
            stamp: stamp(),
        };
        assert!(sample.is_current(stamp()));
        assert!(!sample.is_current(ProbeStamp {
            state_revision: 10,
            ..stamp()
        }));
        assert!(!sample.is_current(ProbeStamp {
            accepted_time_s: 0.06,
            ..stamp()
        }));
        assert!(!sample.is_current(ProbeStamp {
            epoch: 3,
            ..stamp()
        }));
        assert!(!sample.is_current(ProbeStamp {
            committed_generation: 0,
            ..stamp()
        }));
    }

    #[test]
    fn invalid_stamp_and_coordinates_are_rejected() {
        assert!(
            ProbeStamp {
                epoch: 0,
                ..stamp()
            }
            .validate()
            .is_err()
        );
        assert!(
            ProbeStamp {
                accepted_time_s: f64::NAN,
                ..stamp()
            }
            .validate()
            .is_err()
        );
        assert!(
            ProbeStamp {
                committed_generation: 2,
                ..stamp()
            }
            .validate()
            .is_err()
        );
        let grid = Grid::new(2.0, 2.0).unwrap();
        assert_eq!(grid.cell_index(2, 0), None);
        assert_eq!(grid.cell_index(0, 2), None);
    }

    #[test]
    #[ignore = "requires a native GPU compute adapter"]
    fn native_probe_samples_only_requested_committed_cell() {
        block_on(async {
            let instance = wgpu::Instance::default();
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions::default())
                .await
                .unwrap();
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("single-cell probe test"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    ..Default::default()
                })
                .await
                .unwrap();
            let probe = GpuCellProbe::new(&device).unwrap();
            let grid = Grid::new(3.0, 2.0).unwrap();
            let wall = [0_u8, 1, 0, 0, 0, 1];
            let header_bytes = wall
                .into_iter()
                .flat_map(|flag| GpuCellHeader::new(0.0, flag).unwrap().to_le_bytes())
                .collect::<Vec<_>>();
            let masses = [
                1.0_f32, 2.0, 3.0, 4.0, 5.0, 6.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0,
            ];
            let mass_bytes = masses
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect::<Vec<_>>();
            let headers = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("probe test headers"),
                contents: &header_bytes,
                usage: wgpu::BufferUsages::STORAGE,
            });
            let committed_mass_kg = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("probe test committed mass"),
                contents: &mass_bytes,
                usage: wgpu::BufferUsages::STORAGE,
            });
            let source = || ProbeSource {
                grid,
                headers: &headers,
                committed_mass_kg: &committed_mass_kg,
                stamp: stamp(),
            };
            assert!(probe.submit(&device, &queue, source(), [3, 0]).is_err());
            assert!(probe.submit(&device, &queue, source(), [0, 2]).is_err());
            let first = probe.submit(&device, &queue, source(), [1, 0]).unwrap();
            let last = probe.submit(&device, &queue, source(), [2, 1]).unwrap();
            let first = first.resolve(&device).await.unwrap();
            let last = last.resolve(&device).await.unwrap();
            assert_eq!(first.cell, [1, 0]);
            assert_eq!(first.mass_kg, [2.0, 12.0]);
            assert!(first.fixed_wall);
            assert_eq!(last.cell, [2, 1]);
            assert_eq!(last.mass_kg, [6.0, 16.0]);
            assert!(last.fixed_wall);
            assert!(last.is_current(stamp()));
        });
    }
}
