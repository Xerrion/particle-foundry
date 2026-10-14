use super::*;

#[derive(Default)]
struct MapCompletion {
    result: Option<Result<(), String>>,
    waker: Option<Waker>,
}

pub(super) async fn read_stage_statuses(
    device: &wgpu::Device,
    staging: &wgpu::Buffer,
) -> Result<[u32; STAGE_COUNT], String> {
    wait_for_scene_map(device, staging).await?;
    let mapped = staging.get_mapped_range(..);
    let mut statuses = [0_u32; STAGE_COUNT];
    for (slot, status) in statuses.iter_mut().enumerate() {
        let start = slot * 4;
        *status = u32::from_le_bytes(mapped[start..start + 4].try_into().expect("status word"));
    }
    drop(mapped);
    staging.unmap();
    Ok(statuses)
}

// Only the public qualification fixture and native regressions download full
// fields. The live candidate path never calls this function.
pub(super) async fn read_fixture_committed_state(scene: &GpuScene) -> Result<[Vec<u8>; 5], String> {
    let committed = scene.committed();
    let fields = [
        &committed.mass_kg,
        &committed.marker,
        &committed.density_kg_m3,
        &committed.u_velocity_m_s,
        &committed.v_velocity_m_s,
    ];
    let mut offsets = [0_u64; 5];
    let mut total = 0_u64;
    for (slot, field) in fields.iter().enumerate() {
        offsets[slot] = total;
        total = total
            .checked_add(field.size())
            .ok_or("GPU fixture readback size overflow")?;
    }
    if total > scene.context.device.limits().max_buffer_size {
        return Err("GPU fixture readback exceeds adapter limit".into());
    }
    let staging = scene.context.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("scene qualification committed-state readback"),
        size: total,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder =
        scene
            .context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("scene qualification committed-state copy"),
            });
    for (slot, field) in fields.iter().enumerate() {
        encoder.copy_buffer_to_buffer(field, 0, &staging, offsets[slot], field.size());
    }
    scene.context.queue.submit([encoder.finish()]);
    wait_for_scene_map(&scene.context.device, &staging).await?;
    let mapped = staging.get_mapped_range(..);
    let bytes = std::array::from_fn(|slot| {
        let start = offsets[slot] as usize;
        let end = (offsets[slot] + fields[slot].size()) as usize;
        mapped[start..end].to_vec()
    });
    drop(mapped);
    staging.unmap();
    Ok(bytes)
}

async fn wait_for_scene_map(device: &wgpu::Device, staging: &wgpu::Buffer) -> Result<(), String> {
    let completion = Arc::new(Mutex::new(MapCompletion::default()));
    let callback_completion = Arc::clone(&completion);
    staging.map_async(wgpu::MapMode::Read, .., move |result| {
        let waker = {
            let mut state = callback_completion.lock().expect("scene staging map lock");
            state.result =
                Some(result.map_err(|error| format!("scene staging map failed: {error}")));
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
        .map_err(|error| format!("GPU scene submission did not complete: {error}"))?;
    #[cfg(target_arch = "wasm32")]
    let _ = device;
    poll_fn(|cx| {
        let mut state = completion.lock().expect("scene staging map lock");
        if let Some(result) = state.result.take() {
            Poll::Ready(result)
        } else {
            state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    })
    .await
}
