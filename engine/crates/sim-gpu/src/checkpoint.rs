//! Explicit, detached readback of one committed GPU generation.
//!
//! This is the M3 capture prototype. It retains the GPU fields and their
//! submission stamp in memory. It is not the versioned PFSN recovery format:
//! static scene configuration, source ledgers and load preflight belong to E13.
//! Normal rendering and stepping never call this module.

use std::{
    future::poll_fn,
    sync::{Arc, Mutex},
    task::{Poll, Waker},
};

use particle_sim::Grid;

const FIELD_COUNT: usize = 5;

/// Identity of the selected state when the copy was submitted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CheckpointStamp {
    pub(crate) epoch: u64,
    pub(crate) tick: u64,
    pub(crate) accepted_time_s: f64,
    pub(crate) remaining_outer_s: f64,
    pub(crate) generation: u32,
    pub(crate) state_revision: u64,
}

/// Borrowed buffers from one selected generation, in a fixed field order.
pub(crate) struct CheckpointSource<'a> {
    pub(crate) grid: Grid,
    pub(crate) stamp: CheckpointStamp,
    /// Mass, passive marker, derived density, horizontal velocity, vertical velocity.
    pub(crate) fields: [&'a wgpu::Buffer; FIELD_COUNT],
}

/// An explicit detached capture. No part of this object remains GPU resident.
pub(crate) struct CheckpointCapture {
    pub(crate) grid: Grid,
    pub(crate) stamp: CheckpointStamp,
    /// Mass, passive marker, derived density, horizontal velocity, vertical velocity.
    pub(crate) fields: [Vec<f32>; FIELD_COUNT],
    pub(crate) readback_bytes: u64,
}

impl CheckpointCapture {
    /// A reset, accepted substep or paint after submission makes this stale.
    pub(crate) fn is_current(&self, current: CheckpointStamp) -> bool {
        self.stamp == current
    }

    /// Encodes a diagnostic M3 capture. `PFCP` version zero is deliberately
    /// distinct from the durable `PFSN` snapshot schema and has no load API.
    pub(crate) fn into_prototype_bytes(self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(108 + self.readback_bytes as usize);
        bytes.extend_from_slice(b"PFCP");
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&self.grid.width().to_le_bytes());
        bytes.extend_from_slice(&self.grid.height().to_le_bytes());
        bytes.extend_from_slice(&self.stamp.epoch.to_le_bytes());
        bytes.extend_from_slice(&self.stamp.tick.to_le_bytes());
        bytes.extend_from_slice(&self.stamp.accepted_time_s.to_le_bytes());
        bytes.extend_from_slice(&self.stamp.remaining_outer_s.to_le_bytes());
        bytes.extend_from_slice(&self.stamp.generation.to_le_bytes());
        bytes.extend_from_slice(&self.stamp.state_revision.to_le_bytes());
        bytes.extend_from_slice(&self.readback_bytes.to_le_bytes());
        for field in &self.fields {
            bytes.extend_from_slice(&(field.len() as u64).to_le_bytes());
        }
        for field in self.fields {
            for value in field {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        bytes
    }
}

/// Copies only on an explicit request. The caller must check the stamp again
/// after the awaited map before presenting or persisting this capture.
pub(crate) async fn capture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    source: CheckpointSource<'_>,
) -> Result<CheckpointCapture, String> {
    let cells = source.grid.cells() as u64;
    let expected = [
        cells * 8,
        cells * 8,
        cells * 4,
        source.grid.u_faces() as u64 * 4,
        source.grid.v_faces() as u64 * 4,
    ];
    if source.stamp.epoch == 0
        || !source.stamp.accepted_time_s.is_finite()
        || source.stamp.accepted_time_s < 0.0
        || !source.stamp.remaining_outer_s.is_finite()
        || source.stamp.remaining_outer_s < 0.0
        || source.stamp.generation > 1
    {
        return Err("GPU checkpoint stamp is invalid".into());
    }
    let mut offsets = [0_u64; FIELD_COUNT];
    let mut total = 0_u64;
    for (slot, (&buffer, &size)) in source.fields.iter().zip(&expected).enumerate() {
        if buffer.size() != size || !buffer.usage().contains(wgpu::BufferUsages::COPY_SRC) {
            return Err(format!(
                "GPU checkpoint field {slot} has invalid size or usage"
            ));
        }
        offsets[slot] = total;
        total = total
            .checked_add(size)
            .ok_or("GPU checkpoint readback size overflow")?;
    }
    if total > device.limits().max_buffer_size || usize::try_from(total).is_err() {
        return Err("GPU checkpoint readback exceeds adapter or host limit".into());
    }
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("explicit GPU checkpoint capture"),
        size: total,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("explicit GPU checkpoint copy"),
    });
    for (slot, buffer) in source.fields.iter().enumerate() {
        encoder.copy_buffer_to_buffer(buffer, 0, &staging, offsets[slot], expected[slot]);
    }
    queue.submit([encoder.finish()]);
    wait_for_map(device, &staging).await?;
    let mapped = staging.get_mapped_range(..);
    let mut fields: [Vec<f32>; FIELD_COUNT] = std::array::from_fn(|_| Vec::new());
    for slot in 0..FIELD_COUNT {
        let start = offsets[slot] as usize;
        let end = (offsets[slot] + expected[slot]) as usize;
        fields[slot] = mapped[start..end]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|bytes| f32::from_le_bytes(*bytes))
            .collect();
    }
    drop(mapped);
    staging.unmap();
    if fields.iter().flatten().any(|value| !value.is_finite())
        || fields[0].iter().chain(&fields[1]).any(|value| *value < 0.0)
        || fields[2].iter().any(|value| *value <= 0.0)
    {
        return Err("GPU checkpoint contains invalid committed values".into());
    }
    Ok(CheckpointCapture {
        grid: source.grid,
        stamp: source.stamp,
        fields,
        readback_bytes: total,
    })
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
            let mut state = callback_completion.lock().expect("checkpoint map lock");
            state.result = Some(result.map_err(|error| format!("checkpoint map failed: {error}")));
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
        .map_err(|error| format!("checkpoint device poll failed: {error}"))?;
    #[cfg(target_arch = "wasm32")]
    let _ = device;
    poll_fn(|context| {
        let mut state = completion.lock().expect("checkpoint map lock");
        if let Some(result) = state.result.take() {
            Poll::Ready(result)
        } else {
            state.waker = Some(context.waker().clone());
            Poll::Pending
        }
    })
    .await
}
