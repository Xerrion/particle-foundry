mod components;
mod gravity;
mod headroom;
mod large_grid;
mod precision;
mod warm_start;

use std::{
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
    time::{Duration, Instant},
};

use super::readback::wait_for_map;
use super::*;
use wgpu::util::DeviceExt;

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
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(result) => return result,
            Poll::Pending => {
                assert!(Instant::now() < deadline, "GPU pressure test timed out");
                std::thread::park_timeout(Duration::from_millis(100));
            }
        }
    }
}

fn initial_buffer(device: &wgpu::Device, label: &str, values: &[f32]) -> wgpu::Buffer {
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

fn root_buffer(device: &wgpu::Device, values: &[u32]) -> wgpu::Buffer {
    let bytes = values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect::<Vec<_>>();
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("pressure component roots"),
        contents: &bytes,
        usage: wgpu::BufferUsages::STORAGE,
    })
}

fn candidate_buffer(device: &wgpu::Device, label: &str, count: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: count as u64 * 4,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}

async fn read_f32(device: &wgpu::Device, queue: &wgpu::Queue, buffer: &wgpu::Buffer) -> Vec<f32> {
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("pressure test-only full-field readback"),
        size: buffer.size(),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("pressure test-only readback"),
    });
    encoder.copy_buffer_to_buffer(buffer, 0, &staging, 0, buffer.size());
    queue.submit([encoder.finish()]);
    wait_for_map(device, &staging).await.unwrap();
    let mapped = staging.get_mapped_range(..);
    let values = mapped
        .as_chunks::<4>()
        .0
        .iter()
        .map(|word| f32::from_le_bytes(*word))
        .collect::<Vec<_>>();
    drop(mapped);
    staging.unmap();
    values
}

fn dispatch(
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::ComputePipeline,
    bindings: &wgpu::BindGroup,
    groups: u32,
) {
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("pressure stage"),
        timestamp_writes: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bindings, &[]);
    pass.dispatch_workgroups(groups, 1, 1);
}

#[test]
fn rejects_unbounded_or_invalid_config_before_gpu_submission() {
    assert!(
        validate_config(PressureConfig {
            max_iterations: MAX_ITERATIONS + 1,
            ..PressureConfig::default()
        })
        .is_err()
    );
    assert!(
        validate_config(PressureConfig {
            dt_s: 0.0,
            ..PressureConfig::default()
        })
        .is_err()
    );
    assert!(
        validate_config(PressureConfig {
            scaled_residual_tolerance: f32::NAN,
            ..PressureConfig::default()
        })
        .is_err()
    );
    assert!(
        validate_config(PressureConfig {
            gravity_m_s2: f32::NAN,
            ..PressureConfig::default()
        })
        .is_err()
    );
}
