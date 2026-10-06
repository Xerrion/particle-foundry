use particle_sim::{Grid, gpu_layout::GpuGridParams};

use super::{FACE_FLUX_BYTES, GpuTransportCandidate, GpuTransportInput, WORKGROUP_SIZE};

impl GpuTransportCandidate {
    /// Allocates one isolated candidate generation and its face ledgers.
    pub fn new(device: &wgpu::Device, grid: Grid) -> Result<Self, String> {
        preflight(grid, &device.limits())?;
        let cell_bytes = 2 * grid.cells() as u64 * size_of::<f32>() as u64;
        let buffer = |label, size, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        let state_usage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST;
        let flux_usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC;
        Ok(Self {
            base_residual: buffer(
                "transport base volume residual",
                grid.cells() as u64 * 4,
                state_usage,
            ),
            closed_u_velocity_m_s: buffer(
                "transport closed u velocity",
                grid.u_faces() as u64 * 4,
                state_usage,
            ),
            closed_v_velocity_m_s: buffer(
                "transport closed v velocity",
                grid.v_faces() as u64 * 4,
                state_usage,
            ),
            after_x_mass_kg: buffer("transport after-x mass", cell_bytes, state_usage),
            after_x_marker: buffer("transport after-x marker", cell_bytes, state_usage),
            mass_kg: buffer("transport candidate mass", cell_bytes, state_usage),
            marker: buffer("transport candidate marker", cell_bytes, state_usage),
            provisional_u_flux: buffer(
                "transport provisional horizontal face flux",
                grid.u_faces() as u64 * FACE_FLUX_BYTES,
                flux_usage,
            ),
            provisional_v_flux: buffer(
                "transport provisional vertical face flux",
                grid.v_faces() as u64 * FACE_FLUX_BYTES,
                flux_usage,
            ),
            u_flux: buffer(
                "transport horizontal face flux",
                grid.u_faces() as u64 * FACE_FLUX_BYTES,
                flux_usage,
            ),
            v_flux: buffer(
                "transport vertical face flux",
                grid.v_faces() as u64 * FACE_FLUX_BYTES,
                flux_usage,
            ),
            status: buffer(
                "transport candidate status",
                size_of::<u32>() as u64,
                wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_SRC
                    | wgpu::BufferUsages::COPY_DST,
            ),
        })
    }
}

pub(super) fn preflight(grid: Grid, limits: &wgpu::Limits) -> Result<(), String> {
    GpuGridParams::new(grid, 2).map_err(|error| error.to_string())?;
    if limits.max_compute_invocations_per_workgroup < WORKGROUP_SIZE
        || limits.max_compute_workgroup_size_x < WORKGROUP_SIZE
        || limits.max_bindings_per_bind_group < 9
        || limits.max_storage_buffers_per_shader_stage < 8
        || limits.max_uniform_buffer_binding_size < 32
    {
        return Err("GPU adapter limits cannot run transport passes".into());
    }
    for entries in [grid.cells(), grid.u_faces(), grid.v_faces()] {
        if (entries as u32).div_ceil(WORKGROUP_SIZE) > limits.max_compute_workgroups_per_dimension {
            return Err("GPU transport dispatch exceeds effective workgroup limit".into());
        }
    }
    for bytes in [
        2 * grid.cells() as u64 * size_of::<f32>() as u64,
        grid.cells() as u64 * 8,
        grid.u_faces() as u64 * FACE_FLUX_BYTES,
        grid.v_faces() as u64 * FACE_FLUX_BYTES,
        grid.u_faces() as u64 * size_of::<f32>() as u64,
        grid.v_faces() as u64 * size_of::<f32>() as u64,
    ] {
        if bytes > limits.max_storage_buffer_binding_size || bytes > limits.max_buffer_size {
            return Err("GPU transport field exceeds effective buffer limit".into());
        }
    }
    Ok(())
}

pub(super) fn check_buffer_sizes(
    grid: Grid,
    input: &GpuTransportInput<'_>,
    candidate: &GpuTransportCandidate,
) -> Result<(), String> {
    for (label, velocity) in [
        ("u velocity", input.u_velocity_m_s),
        ("v velocity", input.v_velocity_m_s),
    ] {
        if !velocity.usage().contains(wgpu::BufferUsages::COPY_SRC) {
            return Err(format!("{label} needs COPY_SRC for detached correction"));
        }
    }
    let mass_bytes = 2 * grid.cells() as u64 * size_of::<f32>() as u64;
    let header_bytes = grid.cells() as u64 * 8;
    let u_bytes = grid.u_faces() as u64 * size_of::<f32>() as u64;
    let v_bytes = grid.v_faces() as u64 * size_of::<f32>() as u64;
    for (label, buffer, bytes) in [
        ("input mass", input.mass_kg, mass_bytes),
        (
            "base volume residual",
            &candidate.base_residual,
            grid.cells() as u64 * 4,
        ),
        (
            "closed u velocity",
            &candidate.closed_u_velocity_m_s,
            u_bytes,
        ),
        (
            "closed v velocity",
            &candidate.closed_v_velocity_m_s,
            v_bytes,
        ),
        ("input marker", input.marker, mass_bytes),
        ("cell headers", input.headers, header_bytes),
        ("u velocity", input.u_velocity_m_s, u_bytes),
        ("v velocity", input.v_velocity_m_s, v_bytes),
        ("u aperture", input.u_aperture, u_bytes),
        ("v aperture", input.v_aperture, v_bytes),
        ("after-x mass", &candidate.after_x_mass_kg, mass_bytes),
        ("after-x marker", &candidate.after_x_marker, mass_bytes),
        ("candidate mass", &candidate.mass_kg, mass_bytes),
        ("candidate marker", &candidate.marker, mass_bytes),
        (
            "provisional u face flux",
            &candidate.provisional_u_flux,
            grid.u_faces() as u64 * FACE_FLUX_BYTES,
        ),
        (
            "provisional v face flux",
            &candidate.provisional_v_flux,
            grid.v_faces() as u64 * FACE_FLUX_BYTES,
        ),
        (
            "u face flux",
            &candidate.u_flux,
            grid.u_faces() as u64 * FACE_FLUX_BYTES,
        ),
        (
            "v face flux",
            &candidate.v_flux,
            grid.v_faces() as u64 * FACE_FLUX_BYTES,
        ),
        (
            "transport status",
            &candidate.status,
            size_of::<u32>() as u64,
        ),
    ] {
        if buffer.size() != bytes {
            return Err(format!("{label} must have exactly {bytes} bytes"));
        }
    }
    Ok(())
}
