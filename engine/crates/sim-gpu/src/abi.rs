//! GPU execution check for the portable scalar layout.

use std::{
    future::poll_fn,
    sync::{Arc, Mutex},
    task::{Poll, Waker},
};

use particle_sim::{
    Grid,
    gpu_layout::{GpuCellHeader, GpuGridParams, mass_byte_offset},
};
use wgpu::util::DeviceExt;

use crate::GpuContext;

const RESULT_WORDS: usize = 12;
const RESULT_BYTES: u64 = (RESULT_WORDS * size_of::<u32>()) as u64;
const CORE_WORKGROUP_SIZE: u32 = 64;

/// Minimum field allocation. Solver scratch, rendering, and staging add bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GpuCoreFieldPlan {
    /// Packed cell header bytes.
    pub cell_header_bytes: u64,
    /// Component-major f32 mass bytes.
    pub component_mass_bytes: u64,
    /// Staggered horizontal f32 velocity bytes.
    pub u_face_bytes: u64,
    /// Staggered vertical f32 velocity bytes.
    pub v_face_bytes: u64,
    /// These four fields plus the 16-byte grid uniform.
    pub total_bytes: u64,
    /// Cell dispatch workgroups at 64 cells per group.
    pub cell_workgroups: u32,
}

/// Effective device limits after a successful shader ABI round-trip.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GpuAbiReport {
    /// Selected adapter backend.
    pub backend: String,
    /// Maximum size of one storage binding, in bytes.
    pub max_storage_buffer_binding_size: u64,
    /// Maximum size of one buffer, in bytes.
    pub max_buffer_size: u64,
    /// Maximum dispatch count on one axis.
    pub max_compute_workgroups_per_dimension: u32,
    /// Maximum invocations in one workgroup.
    pub max_compute_invocations_per_workgroup: u32,
    /// Sentinel buffer bytes, including staging.
    pub sentinel_bytes: u64,
    /// Minimum 480x270 two-component core field bytes checked on this device.
    pub core_field_bytes: u64,
}

impl GpuContext {
    /// Checks known grid field sizes against the effective device limits.
    ///
    /// This is a lower bound. E07 solver buffers need their own preflight.
    pub fn preflight_core_grid(
        &self,
        grid: Grid,
        active_count: usize,
    ) -> Result<GpuCoreFieldPlan, String> {
        GpuGridParams::new(grid, active_count).map_err(|error| error.to_string())?;
        let cells = grid.cells() as u64;
        let cell_header_bytes = cells * size_of::<GpuCellHeader>() as u64;
        let component_mass_bytes = cells * active_count as u64 * size_of::<f32>() as u64;
        let u_face_bytes = grid.u_faces() as u64 * size_of::<f32>() as u64;
        let v_face_bytes = grid.v_faces() as u64 * size_of::<f32>() as u64;
        let total_bytes =
            16 + cell_header_bytes + component_mass_bytes + u_face_bytes + v_face_bytes;
        let cell_workgroups = (grid.cells() as u32).div_ceil(CORE_WORKGROUP_SIZE);
        let limits = self.device.limits();
        if CORE_WORKGROUP_SIZE > limits.max_compute_invocations_per_workgroup
            || CORE_WORKGROUP_SIZE > limits.max_compute_workgroup_size_x
            || cell_workgroups > limits.max_compute_workgroups_per_dimension
        {
            return Err("GPU grid exceeds effective compute workgroup limits".into());
        }
        for (label, bytes) in [
            ("cell headers", cell_header_bytes),
            ("component mass", component_mass_bytes),
            ("horizontal faces", u_face_bytes),
            ("vertical faces", v_face_bytes),
        ] {
            check_storage_size(label, bytes, &limits)?;
        }
        Ok(GpuCoreFieldPlan {
            cell_header_bytes,
            component_mass_bytes,
            u_face_bytes,
            v_face_bytes,
            total_bytes,
            cell_workgroups,
        })
    }

    /// Runs one shader to verify Rust-packed grid, cell, and mass values.
    ///
    /// The staging readback contains twelve u32 values. This does not test
    /// fluid operators, parity, or rendering.
    pub async fn validate_abi(&self) -> Result<GpuAbiReport, String> {
        let core_grid = Grid::new(480.0, 270.0).map_err(str::to_string)?;
        let core_field_plan = self.preflight_core_grid(core_grid, 2)?;
        let grid = Grid::new(3.0, 2.0).map_err(str::to_string)?;
        let params = GpuGridParams::new(grid, 2).map_err(|error| error.to_string())?;
        let headers = [
            (1.25, 0),
            (2.25, 0),
            (3.25, 0),
            (4.25, 0),
            (5.25, 0),
            (6.25, 1),
        ]
        .map(|(energy, wall)| GpuCellHeader::new(energy, wall))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
        let header_bytes: Vec<u8> = headers.iter().flat_map(|item| item.to_le_bytes()).collect();
        let masses: Vec<f32> = (0..12).map(|index| 10.5 + index as f32 * 3.0).collect();
        let mass_bytes: Vec<u8> = masses.iter().flat_map(|item| item.to_le_bytes()).collect();
        let limits = self.device.limits();
        preflight_sentinel(&limits, header_bytes.len() as u64, mass_bytes.len() as u64)?;

        let uniform = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("abi grid uniform"),
                contents: &params.to_le_bytes(),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let cell_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("abi cell headers"),
                contents: &header_bytes,
                usage: wgpu::BufferUsages::STORAGE,
            });
        let mass_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("abi component masses"),
                contents: &mass_bytes,
                usage: wgpu::BufferUsages::STORAGE,
            });
        let output = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("abi result"),
            size: RESULT_BYTES,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("abi staging"),
            size: RESULT_BYTES,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("abi sentinel"),
                source: wgpu::ShaderSource::Wgsl(
                    include_str!("../shaders/abi_sentinel.wgsl").into(),
                ),
            });
        let pipeline = self
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("abi sentinel"),
                layout: None,
                module: &shader,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("abi sentinel"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: cell_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: mass_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: output.as_entire_binding(),
                },
            ],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("abi sentinel"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("abi sentinel"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&output, 0, &staging, 0, RESULT_BYTES);
        self.queue.submit([encoder.finish()]);
        wait_for_map(&self.device, &staging).await?;
        let mapped = staging.get_mapped_range(..);
        let words = mapped
            .as_chunks::<4>()
            .0
            .iter()
            .map(|chunk| u32::from_le_bytes(*chunk))
            .collect::<Vec<_>>();
        drop(mapped);
        staging.unmap();

        let last_cell = grid.cells() - 1;
        let second_slot =
            mass_byte_offset(grid, 2, 1, 0).map_err(|error| error.to_string())? / size_of::<f32>();
        let last_mass = mass_byte_offset(grid, 2, 1, last_cell)
            .map_err(|error| error.to_string())?
            / size_of::<f32>();
        let expected = [
            grid.width(),
            grid.height(),
            2,
            0,
            headers[0].energy_j().to_bits(),
            u32::from(headers[0].is_wall()),
            headers[last_cell].energy_j().to_bits(),
            u32::from(headers[last_cell].is_wall()),
            masses[0].to_bits(),
            masses[last_cell].to_bits(),
            masses[second_slot].to_bits(),
            masses[last_mass].to_bits(),
        ];
        if words != expected {
            return Err(format!(
                "GPU ABI sentinel mismatch: expected {expected:?}, got {words:?}"
            ));
        }
        Ok(GpuAbiReport {
            backend: self.backend(),
            max_storage_buffer_binding_size: limits.max_storage_buffer_binding_size,
            max_buffer_size: limits.max_buffer_size,
            max_compute_workgroups_per_dimension: limits.max_compute_workgroups_per_dimension,
            max_compute_invocations_per_workgroup: limits.max_compute_invocations_per_workgroup,
            sentinel_bytes: 16
                + header_bytes.len() as u64
                + mass_bytes.len() as u64
                + RESULT_BYTES * 2,
            core_field_bytes: core_field_plan.total_bytes,
        })
    }
}

fn check_storage_size(label: &str, bytes: u64, limits: &wgpu::Limits) -> Result<(), String> {
    if bytes > limits.max_storage_buffer_binding_size || bytes > limits.max_buffer_size {
        return Err(format!(
            "{label} needs {bytes} bytes, above the effective GPU buffer limit"
        ));
    }
    Ok(())
}

fn preflight_sentinel(
    limits: &wgpu::Limits,
    header_bytes: u64,
    mass_bytes: u64,
) -> Result<(), String> {
    if limits.max_uniform_buffer_binding_size < 16
        || limits.max_bindings_per_bind_group < 4
        || limits.max_storage_buffers_per_shader_stage < 3
        || limits.max_compute_invocations_per_workgroup < 1
        || limits.max_compute_workgroup_size_x < 1
        || limits.max_compute_workgroups_per_dimension < 1
    {
        return Err("GPU adapter limits cannot run the ABI sentinel".into());
    }
    for (label, bytes) in [
        ("abi cell headers", header_bytes),
        ("abi component masses", mass_bytes),
        ("abi result", RESULT_BYTES),
    ] {
        check_storage_size(label, bytes, limits)?;
    }
    if RESULT_BYTES > limits.max_buffer_size {
        return Err("abi staging exceeds the effective GPU buffer limit".into());
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
            let mut state = callback_completion.lock().expect("map completion lock");
            state.result =
                Some(result.map_err(|error| format!("GPU ABI staging map failed: {error}")));
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
        .map_err(|error| format!("GPU ABI submission did not complete: {error}"))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentinel_rejects_insufficient_limits_before_allocating() {
        let mut limits = wgpu::Limits {
            max_storage_buffer_binding_size: 32,
            ..Default::default()
        };
        assert!(preflight_sentinel(&limits, 48, 48).is_err());
        limits.max_storage_buffer_binding_size = 128;
        limits.max_storage_buffers_per_shader_stage = 2;
        assert!(preflight_sentinel(&limits, 48, 48).is_err());
    }
}
