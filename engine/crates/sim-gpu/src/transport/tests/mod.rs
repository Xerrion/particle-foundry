mod budget_helpers;
mod closure;
mod donor_budget;
mod gather;
mod gradient;
mod thin_phase;
mod trace_flux;
mod vortex;
mod y_budget;

use std::{
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
    time::{Duration, Instant},
};

use particle_sim::{contracts::Boundary, gpu_layout::GpuCellHeader};
use particle_sim_cpu::{
    fluid::{FaceValues, PressureFields},
    transport::TransportInventory,
};

use particle_sim::CELL_WIDTH_M;
use wgpu::util::DeviceExt;

use super::bindings::{budget_group, face_group, gather_group};
use super::*;
use budget_helpers::assert_two_outlet_budget;

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
                assert!(Instant::now() < deadline, "GPU adapter timed out");
                std::thread::park_timeout(Duration::from_millis(100));
            }
        }
    }
}

fn buffer(device: &wgpu::Device, label: &'static str, bytes: &[u8]) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytes,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
    })
}

fn f32_bytes(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn read_bytes(device: &wgpu::Device, queue: &wgpu::Queue, source: &wgpu::Buffer) -> Vec<u8> {
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("transport fixture readback"),
        size: source.size(),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("transport fixture copy"),
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
        .expect("transport fixture GPU submission");
    receiver
        .recv_timeout(Duration::from_secs(10))
        .expect("transport fixture map callback")
        .expect("transport fixture mapped result");
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

fn run_pass(
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::ComputePipeline,
    bindings: &wgpu::BindGroup,
    entries: usize,
) {
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("transport stage"),
        timestamp_writes: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bindings, &[]);
    pass.dispatch_workgroups((entries as u32).div_ceil(WORKGROUP_SIZE), 1, 1);
}
