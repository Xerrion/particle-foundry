use std::{
    future::poll_fn,
    sync::{Arc, Mutex},
    task::{Poll, Waker},
};

use super::*;

pub(super) fn stage_status_readback(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    status: &wgpu::Buffer,
) -> wgpu::Buffer {
    // The map future owns this buffer. Dropping that future cannot leave a
    // shared staging buffer mapped for the next projection or PCG batch.
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("pressure completion staging"),
        size: STATUS_BYTES,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_buffer_to_buffer(status, 0, &staging, 0, STATUS_BYTES);
    staging
}

pub(super) async fn read_status(
    device: &wgpu::Device,
    staging: wgpu::Buffer,
    journal: Option<&mut journal::PressureJournal>,
    phase: PressurePhase,
    batch_index: u32,
    requested_iterations: u32,
) -> Result<SolveStatus, String> {
    let map_started_ms = journal.as_ref().map(|journal| journal.now_ms());
    wait_for_map(device, &staging).await?;
    let map_completed_ms = journal.as_ref().map(|journal| journal.now_ms());
    let mapped = staging
        .get_mapped_range(..)
        .map_err(|error| format!("GPU mapped range failed: {error}"))?;
    let words = mapped.as_chunks::<4>().0;
    let status_words = std::array::from_fn(|index| u32::from_le_bytes(words[index]));
    let status = SolveStatus {
        scaled_residual: f32::from_bits(status_words[3]),
        scaled_divergence: f32::from_bits(status_words[4]),
        iterations: status_words[5],
        converged: status_words[6],
        invalid: status_words[7],
    };
    drop(mapped);
    staging.unmap();
    if let Some(journal) = journal {
        journal.pressure_status(
            phase,
            batch_index,
            requested_iterations,
            map_started_ms.expect("enabled diagnostic clock"),
            map_completed_ms.expect("enabled diagnostic clock"),
            status_words,
        );
    }
    Ok(status)
}

#[derive(Default)]
struct MapCompletion {
    result: Option<Result<(), String>>,
    waker: Option<Waker>,
}

pub(super) async fn wait_for_map(
    device: &wgpu::Device,
    staging: &wgpu::Buffer,
) -> Result<(), String> {
    let completion = Arc::new(Mutex::new(MapCompletion::default()));
    let callback_completion = Arc::clone(&completion);
    staging.map_async(wgpu::MapMode::Read, .., move |result| {
        let waker = {
            let mut state = callback_completion.lock().expect("map completion lock");
            state.result =
                Some(result.map_err(|error| format!("GPU pressure map failed: {error}")));
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
        .map_err(|error| format!("GPU pressure submission did not complete: {error}"))?;
    #[cfg(target_arch = "wasm32")]
    let _ = device;
    poll_fn(|cx| {
        let mut state = completion.lock().expect("map completion lock");
        if let Some(result) = state.result.take() {
            Poll::Ready(result)
        } else {
            state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    })
    .await
}
