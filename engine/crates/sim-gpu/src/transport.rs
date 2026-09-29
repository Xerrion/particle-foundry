//! One detached f32 phase and marker transport candidate on a closed MAC grid.
//!
//! Face passes own signed fluxes. Cell passes gather those fluxes in X then Y
//! order, matching the f64 reference. The caller must inspect the four-byte
//! status before publishing the candidate buffers. A failed candidate does not
//! replace the input buffers.

use particle_sim::{CELL_WIDTH_M, Grid, REPRESENTED_DEPTH_M, gpu_layout::GpuGridParams};
use wgpu::util::DeviceExt;

/// Maximum normal displacement in cell widths for one transport substep.
pub const MAX_FACE_CFL: f32 = 0.5;
/// Status bit for invalid face geometry, velocity, aperture, or CFL.
pub const STATUS_INVALID_FACE: u32 = 1;
/// Status bit for invalid phase, marker, or wall state.
pub const STATUS_INVALID_CELL: u32 = 2;
/// Status bit for a directional donor losing more amount than it owns.
pub const STATUS_DONOR_OVERDRAW: u32 = 4;
/// Status bit for a completed cell with an incompatible phase volume.
pub const STATUS_VOLUME_CLOSURE: u32 = 8;
/// Status bit for a shared-face correction beyond its bounded CFL budget.
pub const STATUS_CORRECTION_LIMIT: u32 = 16;

const WORKGROUP_SIZE: u32 = 64;
const FACE_FLUX_BYTES: u64 = 5 * size_of::<f32>() as u64;
// Each face is visited once per round. The shader limits one adjustment to
// 1e-5 cell widths, so eight rounds cap total correction at 8e-5 per face.
const LOCAL_CLOSURE_ROUNDS: usize = 8;

/// Physical values for one bounded transport substep.
#[derive(Clone, Copy, Debug)]
pub struct GpuTransportStep {
    /// Accepted substep duration in seconds.
    pub dt_s: f32,
    /// Liquid density in kg/m³.
    pub liquid_density_kg_m3: f32,
    /// Carrier density in kg/m³.
    pub carrier_density_kg_m3: f32,
}

/// Resident input fields. Mass and marker arrays are component-major with
/// liquid in slot zero and carrier in slot one. Headers use `GpuCellHeader`.
pub struct GpuTransportInput<'a> {
    /// Two component-major f32 phase-mass slots, in kg.
    pub mass_kg: &'a wgpu::Buffer,
    /// Two component-major f32 associated-marker slots.
    pub marker: &'a wgpu::Buffer,
    /// Cell headers with fixed-wall flags.
    pub headers: &'a wgpu::Buffer,
    /// Projected horizontal MAC face speeds in m/s. Requires `COPY_SRC`.
    pub u_velocity_m_s: &'a wgpu::Buffer,
    /// Projected vertical MAC face speeds in m/s. Requires `COPY_SRC`.
    pub v_velocity_m_s: &'a wgpu::Buffer,
    /// Horizontal face aperture fractions in zero to one.
    pub u_aperture: &'a wgpu::Buffer,
    /// Vertical face aperture fractions in zero to one.
    pub v_aperture: &'a wgpu::Buffer,
}

/// Candidate buffers owned by one in-flight substep. Face ledgers have five
/// f32 values per face: signed volume, liquid mass, carrier mass, and markers.
pub struct GpuTransportCandidate {
    /// Relative volume error before the shared-face correction.
    pub base_residual: wgpu::Buffer,
    /// Corrected horizontal speeds used to form one shared phase-flux ledger.
    pub closed_u_velocity_m_s: wgpu::Buffer,
    /// Corrected vertical speeds used to form one shared phase-flux ledger.
    pub closed_v_velocity_m_s: wgpu::Buffer,
    /// Detached liquid/carrier mass after the X sweep.
    pub after_x_mass_kg: wgpu::Buffer,
    /// Detached markers after the X sweep.
    pub after_x_marker: wgpu::Buffer,
    /// Completed candidate mass. Publish only when `status` is zero.
    pub mass_kg: wgpu::Buffer,
    /// Completed candidate markers. Publish only when `status` is zero.
    pub marker: wgpu::Buffer,
    /// Detached unbudgeted face proposals, never consumed by gather or momentum.
    pub provisional_u_flux: wgpu::Buffer,
    /// Detached unbudgeted face proposals, never consumed by gather or momentum.
    pub provisional_v_flux: wgpu::Buffer,
    /// Signed horizontal face flux ledger for compatible momentum.
    pub u_flux: wgpu::Buffer,
    /// Signed vertical face flux ledger for compatible momentum.
    pub v_flux: wgpu::Buffer,
    /// Atomic status word. Read this small buffer before accepting the candidate.
    pub status: wgpu::Buffer,
}

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

/// Reusable X/Y face production and cell-gather pipelines for one grid.
pub struct GpuTransportStage {
    grid: Grid,
    residual: wgpu::ComputePipeline,
    pair: wgpu::ComputePipeline,
    x_face: wgpu::ComputePipeline,
    budget: wgpu::ComputePipeline,
    x_gather: wgpu::ComputePipeline,
    y_face: wgpu::ComputePipeline,
    y_gather: wgpu::ComputePipeline,
}

impl GpuTransportStage {
    /// Compiles the bounded transport passes after checking effective limits.
    pub fn new(device: &wgpu::Device, grid: Grid) -> Result<Self, String> {
        preflight(grid, &device.limits())?;
        let face_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("transport face flux"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/transport_face.wgsl").into()),
        });
        let gather_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("transport cell gather"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/transport_gather.wgsl").into(),
            ),
        });
        let residual_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("transport volume residual"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/transport_close_residual.wgsl").into(),
            ),
        });
        let pair_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("transport local residual pairs"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/transport_pair.wgsl").into()),
        });
        let budget_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("transport donor budget"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/transport_donor_budget.wgsl").into(),
            ),
        });
        let pipeline = |label, module, entry_point| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(label),
                layout: None,
                module,
                entry_point: Some(entry_point),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        Ok(Self {
            grid,
            residual: pipeline(
                "transport volume residual",
                &residual_shader,
                "cell_residual",
            ),
            pair: pipeline("transport local residual pairs", &pair_shader, "pair_faces"),
            x_face: pipeline("transport x faces", &face_shader, "x_faces"),
            budget: pipeline("transport donor budget", &budget_shader, "budget_face"),
            x_gather: pipeline("transport x cells", &gather_shader, "x_cells"),
            y_face: pipeline("transport y faces", &face_shader, "y_faces"),
            y_gather: pipeline("transport y cells", &gather_shader, "y_cells"),
        })
    }

    /// Encodes an isolated candidate from projected velocities. The caller
    /// submits the encoder, then reads `candidate.status`. Zero permits later
    /// coupled-stage validation; nonzero rejects the complete substep.
    pub fn encode(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        input: &GpuTransportInput<'_>,
        candidate: &GpuTransportCandidate,
        step: GpuTransportStep,
    ) -> Result<(), String> {
        if !step.dt_s.is_finite() || step.dt_s <= 0.0 {
            return Err("transport dt must be finite and positive".into());
        }
        if !step.liquid_density_kg_m3.is_finite()
            || !step.carrier_density_kg_m3.is_finite()
            || step.liquid_density_kg_m3 <= 0.0
            || step.carrier_density_kg_m3 <= 0.0
        {
            return Err("transport densities must be finite and positive".into());
        }
        check_buffer_sizes(self.grid, input, candidate)?;
        let cell_volume = (CELL_WIDTH_M * CELL_WIDTH_M * REPRESENTED_DEPTH_M) as f32;
        let mut params = [0u8; 32];
        for (slot, value) in [
            self.grid.width(),
            self.grid.height(),
            self.grid.cells() as u32,
            0,
        ]
        .into_iter()
        .enumerate()
        {
            params[slot * 4..slot * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        for (slot, value) in [
            step.dt_s,
            step.liquid_density_kg_m3,
            step.carrier_density_kg_m3,
            cell_volume,
        ]
        .into_iter()
        .enumerate()
        {
            params[16 + slot * 4..20 + slot * 4].copy_from_slice(&value.to_le_bytes());
        }
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("transport step parameters"),
            contents: &params,
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let residual = closure_group(
            device,
            &self.residual,
            &uniform,
            &[
                input.mass_kg,
                input.headers,
                input.u_velocity_m_s,
                input.v_velocity_m_s,
                input.u_aperture,
                input.v_aperture,
                &candidate.base_residual,
                &candidate.status,
            ],
        );
        let pair_groups = [0_u32, 2, 1, 3].map(|variant| {
            let mut pair_params = params;
            pair_params[12..16].copy_from_slice(&variant.to_le_bytes());
            let pair_uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("transport local pair parameters"),
                contents: &pair_params,
                usage: wgpu::BufferUsages::UNIFORM,
            });
            closure_group(
                device,
                &self.pair,
                &pair_uniform,
                &[
                    input.headers,
                    input.u_aperture,
                    input.v_aperture,
                    &candidate.base_residual,
                    &candidate.closed_u_velocity_m_s,
                    &candidate.closed_v_velocity_m_s,
                    &candidate.status,
                ],
            )
        });
        let x_faces = face_group(
            device,
            &self.x_face,
            &uniform,
            input.mass_kg,
            input.marker,
            input.headers,
            &candidate.closed_u_velocity_m_s,
            input.u_aperture,
            &candidate.provisional_u_flux,
            &candidate.status,
        );
        let budget_groups = [
            (
                0_u32,
                &candidate.provisional_u_flux,
                &candidate.u_flux,
                input.mass_kg,
                input.marker,
            ),
            (
                1_u32,
                &candidate.provisional_v_flux,
                &candidate.v_flux,
                &candidate.after_x_mass_kg,
                &candidate.after_x_marker,
            ),
        ]
        .map(|(axis, provisional, corrected, mass, marker)| {
            let mut budget_params = params;
            budget_params[12..16].copy_from_slice(&axis.to_le_bytes());
            let budget_uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("transport donor budget params"),
                contents: &budget_params,
                usage: wgpu::BufferUsages::UNIFORM,
            });
            budget_group(
                device,
                &self.budget,
                &budget_uniform,
                mass,
                marker,
                provisional,
                corrected,
                &candidate.status,
            )
        });
        let x_cells = gather_group(
            device,
            &self.x_gather,
            &uniform,
            input.mass_kg,
            input.marker,
            input.headers,
            &candidate.u_flux,
            &candidate.after_x_mass_kg,
            &candidate.after_x_marker,
            &candidate.status,
        );
        let y_faces = face_group(
            device,
            &self.y_face,
            &uniform,
            &candidate.after_x_mass_kg,
            &candidate.after_x_marker,
            input.headers,
            &candidate.closed_v_velocity_m_s,
            input.v_aperture,
            &candidate.provisional_v_flux,
            &candidate.status,
        );
        let y_cells = gather_group(
            device,
            &self.y_gather,
            &uniform,
            &candidate.after_x_mass_kg,
            &candidate.after_x_marker,
            input.headers,
            &candidate.v_flux,
            &candidate.mass_kg,
            &candidate.marker,
            &candidate.status,
        );
        encoder.clear_buffer(&candidate.status, 0, None);
        encoder.copy_buffer_to_buffer(
            input.u_velocity_m_s,
            0,
            &candidate.closed_u_velocity_m_s,
            0,
            candidate.closed_u_velocity_m_s.size(),
        );
        encoder.copy_buffer_to_buffer(
            input.v_velocity_m_s,
            0,
            &candidate.closed_v_velocity_m_s,
            0,
            candidate.closed_v_velocity_m_s.size(),
        );
        run_pass(encoder, &self.residual, &residual, self.grid.cells());
        for _ in 0..LOCAL_CLOSURE_ROUNDS {
            for (group, entries) in pair_groups.iter().zip([
                self.grid.u_faces(),
                self.grid.u_faces(),
                self.grid.v_faces(),
                self.grid.v_faces(),
            ]) {
                run_pass(encoder, &self.pair, group, entries);
            }
        }
        run_pass(encoder, &self.x_face, &x_faces, self.grid.u_faces());
        run_pass(
            encoder,
            &self.budget,
            &budget_groups[0],
            self.grid.u_faces(),
        );
        run_pass(encoder, &self.x_gather, &x_cells, self.grid.cells());
        run_pass(encoder, &self.y_face, &y_faces, self.grid.v_faces());
        run_pass(
            encoder,
            &self.budget,
            &budget_groups[1],
            self.grid.v_faces(),
        );
        run_pass(encoder, &self.y_gather, &y_cells, self.grid.cells());
        Ok(())
    }
}

fn preflight(grid: Grid, limits: &wgpu::Limits) -> Result<(), String> {
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

fn check_buffer_sizes(
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

fn closure_group(
    device: &wgpu::Device,
    pipeline: &wgpu::ComputePipeline,
    uniform: &wgpu::Buffer,
    storage: &[&wgpu::Buffer],
) -> wgpu::BindGroup {
    let mut entries = vec![wgpu::BindGroupEntry {
        binding: 0,
        resource: uniform.as_entire_binding(),
    }];
    entries.extend(
        storage
            .iter()
            .enumerate()
            .map(|(index, buffer)| wgpu::BindGroupEntry {
                binding: index as u32 + 1,
                resource: buffer.as_entire_binding(),
            }),
    );
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("transport shared-face closure bindings"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &entries,
    })
}

#[allow(clippy::too_many_arguments)]
fn face_group(
    device: &wgpu::Device,
    pipeline: &wgpu::ComputePipeline,
    uniform: &wgpu::Buffer,
    mass: &wgpu::Buffer,
    marker: &wgpu::Buffer,
    headers: &wgpu::Buffer,
    velocity: &wgpu::Buffer,
    aperture: &wgpu::Buffer,
    flux: &wgpu::Buffer,
    status: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("transport face bindings"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: mass.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: marker.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: headers.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: velocity.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: aperture.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: flux.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: status.as_entire_binding(),
            },
        ],
    })
}

#[allow(clippy::too_many_arguments)]
fn budget_group(
    device: &wgpu::Device,
    pipeline: &wgpu::ComputePipeline,
    uniform: &wgpu::Buffer,
    mass: &wgpu::Buffer,
    marker: &wgpu::Buffer,
    provisional: &wgpu::Buffer,
    corrected: &wgpu::Buffer,
    status: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("transport donor budget bindings"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: mass.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: marker.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: provisional.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: corrected.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: status.as_entire_binding(),
            },
        ],
    })
}

#[allow(clippy::too_many_arguments)]
fn gather_group(
    device: &wgpu::Device,
    pipeline: &wgpu::ComputePipeline,
    uniform: &wgpu::Buffer,
    mass: &wgpu::Buffer,
    marker: &wgpu::Buffer,
    headers: &wgpu::Buffer,
    flux: &wgpu::Buffer,
    output_mass: &wgpu::Buffer,
    output_marker: &wgpu::Buffer,
    status: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("transport gather bindings"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: mass.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: marker.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: headers.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: flux.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: output_mass.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: output_marker.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: status.as_entire_binding(),
            },
        ],
    })
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

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
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

    #[test]
    #[ignore = "requires a native Metal GPU"]
    fn metal_tiny_phase_marker_flux_survives_full_and_partial_transfer() {
        let gpu = block_on(crate::GpuContext::new()).expect("native GPU adapter");
        assert_eq!(gpu.backend(), "Metal");
        let grid = Grid::new(2.0, 2.0).unwrap();
        let cells = grid.cells();
        let volume = grid.cell_volume_m3() as f32;
        let tiny_mass = 1.9347336e-21_f32;
        let tiny_marker = 4.8368337e-19_f32;
        let mass = [
            tiny_mass,
            0.0,
            1000.0 * volume,
            1000.0 * volume,
            1.2 * volume,
            1.2 * volume,
            0.0,
            0.0,
        ];
        let marker = [tiny_marker, 0.0, 0.25, 0.25, 0.05, 0.05, 0.0, 0.0];
        let headers = (0..cells)
            .flat_map(|_| GpuCellHeader::new(0.0, 0).unwrap().to_le_bytes())
            .collect::<Vec<_>>();
        let mass_buffer = buffer(&gpu.device, "tiny phase mass", &f32_bytes(&mass));
        let marker_buffer = buffer(&gpu.device, "tiny phase marker", &f32_bytes(&marker));
        let header_buffer = buffer(&gpu.device, "tiny phase headers", &headers);
        let mut aperture = vec![0.0_f32; grid.v_faces()];
        let face = grid.v_face_index(0, 1).unwrap();
        aperture[face] = 1.0;
        let aperture_buffer = buffer(&gpu.device, "tiny phase aperture", &f32_bytes(&aperture));
        let stage = GpuTransportStage::new(&gpu.device, grid).unwrap();
        let dt = (1.0 / 60.0) as f32;
        let mut params = [0_u8; 32];
        for (slot, value) in [grid.width(), grid.height(), cells as u32, 0]
            .into_iter()
            .enumerate()
        {
            params[slot * 4..slot * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        for (slot, value) in [dt, 1000.0_f32, 1.2_f32, volume].into_iter().enumerate() {
            params[16 + slot * 4..20 + slot * 4].copy_from_slice(&value.to_le_bytes());
        }
        let uniform = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tiny phase face params"),
                contents: &params,
                usage: wgpu::BufferUsages::UNIFORM,
            });
        for (name, speed, expected_fraction) in [
            ("full", 6.512966e-17_f32, 1.0_f64),
            ("partial", 5e-19_f32, 0.4307_f64),
        ] {
            let mut velocity = vec![0.0_f32; grid.v_faces()];
            velocity[face] = speed;
            let velocity_buffer = buffer(&gpu.device, "tiny phase v", &f32_bytes(&velocity));
            let candidate = GpuTransportCandidate::new(&gpu.device, grid).unwrap();
            let bindings = face_group(
                &gpu.device,
                &stage.y_face,
                &uniform,
                &mass_buffer,
                &marker_buffer,
                &header_buffer,
                &velocity_buffer,
                &aperture_buffer,
                &candidate.v_flux,
                &candidate.status,
            );
            let mut encoder = gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("tiny phase marker flux"),
                });
            encoder.clear_buffer(&candidate.status, 0, None);
            run_pass(&mut encoder, &stage.y_face, &bindings, grid.v_faces());
            gpu.queue.submit([encoder.finish()]);
            let status = read_bytes(&gpu.device, &gpu.queue, &candidate.status);
            assert_eq!(u32::from_le_bytes(status.try_into().unwrap()), 0);
            let flux = read_f32(&gpu.device, &gpu.queue, &candidate.v_flux);
            let moved_mass = flux[face * 5 + 1];
            let moved_marker = flux[face * 5 + 3];
            let moved_fraction = f64::from(moved_mass) / f64::from(tiny_mass);
            assert!(
                (moved_fraction - expected_fraction).abs() < 1e-3,
                "{name} mass transfer fraction {moved_fraction}"
            );
            let expected_marker = f64::from(tiny_marker) * moved_fraction;
            assert!(
                (f64::from(moved_marker) - expected_marker).abs() <= expected_marker * 1e-5,
                "{name} marker transfer: expected {expected_marker:e}, got {moved_marker:e}"
            );
        }
        gpu.dispose();
    }

    #[test]
    #[ignore = "requires a native Metal GPU"]
    fn metal_two_outlet_budget_preserves_phase_and_marker_on_both_axes() {
        let gpu = block_on(crate::GpuContext::new()).expect("native GPU adapter");
        assert_eq!(gpu.backend(), "Metal");
        let mass = [0.001_f32, 0.0009999996, 0.001, 0.0, 2.7105054e-20, 0.0];
        let marker = [0.25_f32, 0.24999982, 0.25, 0.0, 8.881784e-16, 0.0];
        let headers = (0..3)
            .flat_map(|_| GpuCellHeader::new(0.0, 0).unwrap().to_le_bytes())
            .collect::<Vec<_>>();
        let mass_buffer = buffer(&gpu.device, "two-outlet phase mass", &f32_bytes(&mass));
        let marker_buffer = buffer(&gpu.device, "two-outlet marker", &f32_bytes(&marker));
        let header_buffer = buffer(&gpu.device, "two-outlet headers", &headers);
        let mut provisional = vec![0.0_f32; 4 * 5];
        provisional[5..10].copy_from_slice(&[
            -3.0224773e-13,
            -3.0224767e-10,
            -2.7105054e-20,
            -7.556189e-8,
            -8.881784e-16,
        ]);
        provisional[10..15].copy_from_slice(&[
            4.856996e-13,
            4.856995e-10,
            2.7105054e-20,
            1.2142483e-7,
            8.881784e-16,
        ]);
        let provisional_buffer = buffer(
            &gpu.device,
            "two-outlet provisional ledger",
            &f32_bytes(&provisional),
        );
        for (axis, grid) in [
            (0_u32, Grid::new(3.0, 1.0).unwrap()),
            (1_u32, Grid::new(1.0, 3.0).unwrap()),
        ] {
            let stage = GpuTransportStage::new(&gpu.device, grid).unwrap();
            let candidate = GpuTransportCandidate::new(&gpu.device, grid).unwrap();
            let corrected = if axis == 0 {
                &candidate.u_flux
            } else {
                &candidate.v_flux
            };
            let (gather_pipeline, output_mass, output_marker) = if axis == 0 {
                (
                    &stage.x_gather,
                    &candidate.after_x_mass_kg,
                    &candidate.after_x_marker,
                )
            } else {
                (&stage.y_gather, &candidate.mass_kg, &candidate.marker)
            };
            let mut params = [0_u8; 32];
            for (slot, value) in [grid.width(), grid.height(), 3, axis]
                .into_iter()
                .enumerate()
            {
                params[slot * 4..slot * 4 + 4].copy_from_slice(&value.to_le_bytes());
            }
            for (slot, value) in [1.0_f32 / 60.0, 1000.0, 1.2, grid.cell_volume_m3() as f32]
                .into_iter()
                .enumerate()
            {
                params[16 + slot * 4..20 + slot * 4].copy_from_slice(&value.to_le_bytes());
            }
            let uniform = gpu
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("two-outlet budget parameters"),
                    contents: &params,
                    usage: wgpu::BufferUsages::UNIFORM,
                });
            let budget_bindings = budget_group(
                &gpu.device,
                &stage.budget,
                &uniform,
                &mass_buffer,
                &marker_buffer,
                &provisional_buffer,
                corrected,
                &candidate.status,
            );
            let gather_bindings = gather_group(
                &gpu.device,
                gather_pipeline,
                &uniform,
                &mass_buffer,
                &marker_buffer,
                &header_buffer,
                corrected,
                output_mass,
                output_marker,
                &candidate.status,
            );
            let mut encoder = gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("two-outlet donor budget fixture"),
                });
            encoder.clear_buffer(&candidate.status, 0, None);
            run_pass(&mut encoder, &stage.budget, &budget_bindings, 4);
            run_pass(&mut encoder, gather_pipeline, &gather_bindings, 3);
            gpu.queue.submit([encoder.finish()]);
            let status = read_bytes(&gpu.device, &gpu.queue, &candidate.status);
            assert_eq!(
                u32::from_le_bytes(status.try_into().unwrap()),
                0,
                "axis {axis}"
            );
            let flux = read_f32(&gpu.device, &gpu.queue, corrected);
            let negative_carrier = -flux[5 + 2];
            let positive_carrier = flux[10 + 2];
            let negative_marker = -flux[5 + 4];
            let positive_marker = flux[10 + 4];
            assert_eq!(flux[5], provisional[5]);
            assert_eq!(flux[10], provisional[10]);
            assert!(
                negative_carrier > 0.0 && positive_carrier > 0.0,
                "axis {axis}"
            );
            assert!(
                negative_marker > 0.0 && positive_marker > 0.0,
                "axis {axis}"
            );
            assert!(
                ((negative_carrier + positive_carrier) - mass[4]).abs() <= mass[4] * 1e-5,
                "axis {axis}: carrier budget"
            );
            assert!(
                ((negative_marker + positive_marker) - marker[4]).abs() <= marker[4] * 1e-5,
                "axis {axis}: marker budget"
            );
            for face in [1, 2] {
                let offset = face * 5;
                let volume_from_phases =
                    flux[offset + 1].abs() / 1000.0 + flux[offset + 2].abs() / 1.2;
                assert!(
                    (volume_from_phases - flux[offset].abs()).abs() <= flux[offset].abs() * 1e-5,
                    "axis {axis}: face {face} volume changed"
                );
            }
            let result_mass = read_f32(&gpu.device, &gpu.queue, output_mass);
            let result_marker = read_f32(&gpu.device, &gpu.queue, output_marker);
            for (slot, (before, after)) in [
                (&mass[..3], &result_mass[..3]),
                (&mass[3..], &result_mass[3..]),
                (&marker[..3], &result_marker[..3]),
                (&marker[3..], &result_marker[3..]),
            ]
            .into_iter()
            .enumerate()
            {
                let before: f64 = before.iter().map(|value| f64::from(*value)).sum();
                let after: f64 = after.iter().map(|value| f64::from(*value)).sum();
                assert!(
                    (after - before).abs() <= before * 1e-5,
                    "axis {axis}: inventory slot {slot} drifted: {before:e} to {after:e}"
                );
            }
        }
        gpu.dispose();
    }

    #[test]
    #[ignore = "requires a native Metal GPU"]
    fn metal_y_budget_uses_inventory_after_x_transfer() {
        let gpu = block_on(crate::GpuContext::new()).expect("native GPU adapter");
        assert_eq!(gpu.backend(), "Metal");
        let grid = Grid::new(3.0, 3.0).unwrap();
        let cells = grid.cells();
        let volume = grid.cell_volume_m3() as f32;
        let water_unit = 1000.0 * volume;
        let air_unit = 1.2 * volume;
        let center = grid.cell_index(1, 1).unwrap();
        let left = grid.cell_index(0, 1).unwrap();
        let right = grid.cell_index(2, 1).unwrap();
        let top = grid.cell_index(1, 0).unwrap();
        let bottom = grid.cell_index(1, 2).unwrap();
        let mut liquid = vec![0.0_f32; cells];
        let mut carrier = vec![air_unit; cells];
        let mut liquid_marker = vec![0.0_f32; cells];
        let mut carrier_marker = vec![0.25_f32; cells];
        liquid[left] = 1.25 * water_unit;
        carrier[left] = 0.0;
        liquid_marker[left] = 2.5;
        carrier_marker[left] = 0.0;
        liquid[center] = 0.1 * water_unit;
        carrier[center] = 0.9 * air_unit;
        liquid_marker[center] = 0.2;
        carrier_marker[center] = 0.9;
        carrier[right] = 1.25 * air_unit;
        carrier_marker[right] = 0.5;
        carrier[top] = 0.75 * air_unit;
        carrier[bottom] = 0.75 * air_unit;
        let mass = liquid.into_iter().chain(carrier).collect::<Vec<_>>();
        let marker = liquid_marker
            .into_iter()
            .chain(carrier_marker)
            .collect::<Vec<_>>();
        let headers = (0..cells)
            .flat_map(|_| GpuCellHeader::new(0.0, 0).unwrap().to_le_bytes())
            .collect::<Vec<_>>();
        let dt = 0.01_f32;
        let face_speed = 0.25 * CELL_WIDTH_M as f32 / dt;
        let left_face = grid.u_face_index(1, 1).unwrap();
        let right_face = grid.u_face_index(2, 1).unwrap();
        let top_face = grid.v_face_index(1, 1).unwrap();
        let bottom_face = grid.v_face_index(1, 2).unwrap();
        let mut u = vec![0.0_f32; grid.u_faces()];
        let mut v = vec![0.0_f32; grid.v_faces()];
        let mut u_open = vec![0.0_f32; grid.u_faces()];
        let mut v_open = vec![0.0_f32; grid.v_faces()];
        u[left_face] = face_speed;
        u[right_face] = -face_speed;
        v[top_face] = -face_speed;
        v[bottom_face] = face_speed;
        u_open[left_face] = 1.0;
        u_open[right_face] = 1.0;
        v_open[top_face] = 1.0;
        v_open[bottom_face] = 1.0;
        let mass_buffer = buffer(&gpu.device, "x then y phase mass", &f32_bytes(&mass));
        let marker_buffer = buffer(&gpu.device, "x then y marker", &f32_bytes(&marker));
        let header_buffer = buffer(&gpu.device, "x then y headers", &headers);
        let u_buffer = buffer(&gpu.device, "x then y horizontal speed", &f32_bytes(&u));
        let v_buffer = buffer(&gpu.device, "x then y vertical speed", &f32_bytes(&v));
        let u_open_buffer = buffer(
            &gpu.device,
            "x then y horizontal aperture",
            &f32_bytes(&u_open),
        );
        let v_open_buffer = buffer(
            &gpu.device,
            "x then y vertical aperture",
            &f32_bytes(&v_open),
        );
        let input = GpuTransportInput {
            mass_kg: &mass_buffer,
            marker: &marker_buffer,
            headers: &header_buffer,
            u_velocity_m_s: &u_buffer,
            v_velocity_m_s: &v_buffer,
            u_aperture: &u_open_buffer,
            v_aperture: &v_open_buffer,
        };
        let stage = GpuTransportStage::new(&gpu.device, grid).unwrap();
        let candidate = GpuTransportCandidate::new(&gpu.device, grid).unwrap();
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("x then y donor budget fixture"),
            });
        stage
            .encode(
                &gpu.device,
                &mut encoder,
                &input,
                &candidate,
                GpuTransportStep {
                    dt_s: dt,
                    liquid_density_kg_m3: 1000.0,
                    carrier_density_kg_m3: 1.2,
                },
            )
            .unwrap();
        gpu.queue.submit([encoder.finish()]);
        let status = read_bytes(&gpu.device, &gpu.queue, &candidate.status);
        assert_eq!(u32::from_le_bytes(status.try_into().unwrap()), 0);

        let after_x_mass = read_f32(&gpu.device, &gpu.queue, &candidate.after_x_mass_kg);
        let after_x_marker = read_f32(&gpu.device, &gpu.queue, &candidate.after_x_marker);
        assert!(after_x_mass[center] > mass[center] + 0.2 * water_unit);
        assert!(after_x_marker[center] > marker[center] + 0.4);

        let provisional_y = read_f32(&gpu.device, &gpu.queue, &candidate.provisional_v_flux);
        let corrected_y = read_f32(&gpu.device, &gpu.queue, &candidate.v_flux);
        let top_liquid = -corrected_y[top_face * 5 + 1];
        let bottom_liquid = corrected_y[bottom_face * 5 + 1];
        let top_marker = -corrected_y[top_face * 5 + 3];
        let bottom_marker = corrected_y[bottom_face * 5 + 3];
        assert!(top_liquid > 0.0 && bottom_liquid > 0.0);
        assert!(top_marker > 0.0 && bottom_marker > 0.0);
        assert!(
            top_liquid + bottom_liquid > mass[center] * 1.05,
            "the two Y outlets must exceed the pre-X liquid budget"
        );
        assert!(top_liquid + bottom_liquid < after_x_mass[center]);
        for face in [top_face, bottom_face] {
            for slot in 0..5 {
                assert_eq!(
                    corrected_y[face * 5 + slot],
                    provisional_y[face * 5 + slot],
                    "Y face {face} slot {slot} changed despite the after-X budget"
                );
            }
        }

        let result_mass = read_f32(&gpu.device, &gpu.queue, &candidate.mass_kg);
        let result_marker = read_f32(&gpu.device, &gpu.queue, &candidate.marker);
        for cell in 0..cells {
            let fill =
                result_mass[cell] / 1000.0 / volume + result_mass[cells + cell] / 1.2 / volume;
            assert!((fill - 1.0).abs() <= 1e-5, "cell {cell} fill {fill}");
        }
        let total = |values: &[f32]| values.iter().map(|&value| f64::from(value)).sum::<f64>();
        for (before, after) in [
            (&mass[..cells], &result_mass[..cells]),
            (&mass[cells..], &result_mass[cells..]),
            (&marker[..cells], &result_marker[..cells]),
            (&marker[cells..], &result_marker[cells..]),
        ] {
            let expected = total(before);
            let actual = total(after);
            assert!(
                (actual - expected).abs() <= expected * 1e-6,
                "transport total changed from {expected:e} to {actual:e}"
            );
        }
        gpu.dispose();
    }

    #[test]
    #[ignore = "requires a native GPU adapter"]
    fn closed_vortex_matches_cpu_phase_and_marker_candidate() {
        let gpu = block_on(crate::GpuContext::new()).expect("native GPU adapter for E07 fixture");
        let grid = Grid::new(2.0, 2.0).unwrap();
        let cell_volume = grid.cell_volume_m3();
        let fractions = [0.1, 0.45, 0.8, 0.3];
        let liquid = fractions.map(|alpha| alpha * cell_volume * 1000.0);
        let carrier = fractions.map(|alpha| (1.0 - alpha) * cell_volume * 1.2);
        let liquid_marker = [1.0, 0.5, 2.0, 0.25];
        let carrier_marker = [0.25, 1.0, 0.5, 2.0];
        let initial = TransportInventory::new(
            grid,
            1000.0,
            1.2,
            liquid.to_vec(),
            carrier.to_vec(),
            liquid_marker.to_vec(),
            carrier_marker.to_vec(),
            vec![false; grid.cells()],
        )
        .unwrap();
        let mut u = vec![0.0; grid.u_faces()];
        let mut v = vec![0.0; grid.v_faces()];
        u[grid.u_face_index(1, 0).unwrap()] = 1.0;
        u[grid.u_face_index(1, 1).unwrap()] = -1.0;
        v[grid.v_face_index(0, 1).unwrap()] = -1.0;
        v[grid.v_face_index(1, 1).unwrap()] = 1.0;
        let mut u_aperture = vec![0.0; grid.u_faces()];
        let mut v_aperture = vec![0.0; grid.v_faces()];
        for y in 0..grid.height() {
            u_aperture[grid.u_face_index(1, y).unwrap()] = 1.0;
        }
        for x in 0..grid.width() {
            v_aperture[grid.v_face_index(x, 1).unwrap()] = 1.0;
        }
        let fields = PressureFields::new(
            grid,
            [Boundary::Closed; 4],
            vec![1000.0; grid.cells()],
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
        let cpu = initial
            .candidate(
                &fields,
                &FaceValues {
                    u: u.clone(),
                    v: v.clone(),
                },
                0.0025,
            )
            .unwrap();

        let mass = liquid
            .into_iter()
            .chain(carrier)
            .map(|value| value as f32)
            .collect::<Vec<_>>();
        let marker = liquid_marker
            .into_iter()
            .chain(carrier_marker)
            .map(|value| value as f32)
            .collect::<Vec<_>>();
        let headers = (0..grid.cells())
            .flat_map(|_| GpuCellHeader::new(0.0, 0).unwrap().to_le_bytes())
            .collect::<Vec<_>>();
        let mass_buffer = buffer(&gpu.device, "fixture phase mass", &f32_bytes(&mass));
        let marker_buffer = buffer(&gpu.device, "fixture marker", &f32_bytes(&marker));
        let header_buffer = buffer(&gpu.device, "fixture headers", &headers);
        let to_f32 = |values: &[f64]| values.iter().map(|value| *value as f32).collect::<Vec<_>>();
        let u_buffer = buffer(&gpu.device, "fixture u", &f32_bytes(&to_f32(&u)));
        let v_buffer = buffer(&gpu.device, "fixture v", &f32_bytes(&to_f32(&v)));
        let u_open_buffer = buffer(
            &gpu.device,
            "fixture u aperture",
            &f32_bytes(&to_f32(&u_aperture)),
        );
        let v_open_buffer = buffer(
            &gpu.device,
            "fixture v aperture",
            &f32_bytes(&to_f32(&v_aperture)),
        );
        let input = GpuTransportInput {
            mass_kg: &mass_buffer,
            marker: &marker_buffer,
            headers: &header_buffer,
            u_velocity_m_s: &u_buffer,
            v_velocity_m_s: &v_buffer,
            u_aperture: &u_open_buffer,
            v_aperture: &v_open_buffer,
        };
        let stage = GpuTransportStage::new(&gpu.device, grid).unwrap();
        let candidate = GpuTransportCandidate::new(&gpu.device, grid).unwrap();
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("transport fixture"),
            });
        stage
            .encode(
                &gpu.device,
                &mut encoder,
                &input,
                &candidate,
                GpuTransportStep {
                    dt_s: 0.0025,
                    liquid_density_kg_m3: 1000.0,
                    carrier_density_kg_m3: 1.2,
                },
            )
            .unwrap();
        gpu.queue.submit([encoder.finish()]);
        let status = read_bytes(&gpu.device, &gpu.queue, &candidate.status);
        assert_eq!(u32::from_le_bytes(status.try_into().unwrap()), 0);
        let actual_mass = read_f32(&gpu.device, &gpu.queue, &candidate.mass_kg);
        let actual_marker = read_f32(&gpu.device, &gpu.queue, &candidate.marker);
        let mut max_scaled_amount_error = 0.0_f64;
        let mut max_alpha_error = 0.0_f64;
        for cell in 0..grid.cells() {
            for (actual, expected, scale) in [
                (
                    actual_mass[cell],
                    cpu.inventory.liquid_mass_kg()[cell],
                    0.001,
                ),
                (
                    actual_mass[grid.cells() + cell],
                    cpu.inventory.carrier_mass_kg()[cell],
                    1.2e-6,
                ),
                (
                    actual_marker[cell],
                    cpu.inventory.liquid_marker()[cell],
                    1.0,
                ),
                (
                    actual_marker[grid.cells() + cell],
                    cpu.inventory.carrier_marker()[cell],
                    1.0,
                ),
            ] {
                assert!(actual >= 0.0, "cell {cell}: negative transported amount");
                max_scaled_amount_error =
                    max_scaled_amount_error.max(((actual as f64 - expected) / scale).abs());
                assert!(
                    ((actual as f64 - expected) / scale).abs() <= 1e-5,
                    "cell {cell}: GPU {actual}, CPU {expected}"
                );
            }
            let alpha = actual_mass[cell] as f64 / 1000.0 / cell_volume;
            assert!((0.0..=1.0).contains(&alpha));
            max_alpha_error =
                max_alpha_error.max((alpha - cpu.inventory.alpha(cell).unwrap()).abs());
            assert!((alpha - cpu.inventory.alpha(cell).unwrap()).abs() <= 1e-5);
        }
        let total = |values: &[f32]| values.iter().map(|value| *value as f64).sum::<f64>();
        let total_liquid = total(&actual_mass[..grid.cells()]);
        let total_carrier = total(&actual_mass[grid.cells()..]);
        let total_liquid_marker = total(&actual_marker[..grid.cells()]);
        let total_carrier_marker = total(&actual_marker[grid.cells()..]);
        let before = initial.totals();
        let max_relative_total_drift = [
            (total_liquid, before.liquid_mass_kg),
            (total_carrier, before.carrier_mass_kg),
            (total_liquid_marker, before.liquid_marker),
            (total_carrier_marker, before.carrier_marker),
        ]
        .into_iter()
        .map(|(actual, expected)| ((actual - expected) / expected).abs())
        .fold(0.0_f64, f64::max);
        assert!(max_relative_total_drift <= 1e-5);
        println!(
            "GPU transport parity: max alpha error {max_alpha_error:.3e}, max scaled amount error {max_scaled_amount_error:.3e}, max relative total drift {max_relative_total_drift:.3e}"
        );
        let face_flux = read_f32(&gpu.device, &gpu.queue, &candidate.u_flux);
        let face = grid.u_face_index(1, 0).unwrap();
        assert!((face_flux[face * 5 + 1] as f64 - cpu.fluxes.liquid_mass_kg.u[face]).abs() < 1e-8);

        let rejected = GpuTransportCandidate::new(&gpu.device, grid).unwrap();
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("transport excessive CFL fixture"),
            });
        stage
            .encode(
                &gpu.device,
                &mut encoder,
                &input,
                &rejected,
                GpuTransportStep {
                    dt_s: 0.006,
                    liquid_density_kg_m3: 1000.0,
                    carrier_density_kg_m3: 1.2,
                },
            )
            .unwrap();
        gpu.queue.submit([encoder.finish()]);
        let status = read_bytes(&gpu.device, &gpu.queue, &rejected.status);
        assert_ne!(
            u32::from_le_bytes(status.try_into().unwrap()) & STATUS_INVALID_FACE,
            0
        );
        gpu.dispose();
    }

    #[test]
    #[ignore = "requires a native GPU adapter"]
    fn local_shared_face_correction_closes_without_losing_phase_or_marker() {
        let gpu = block_on(crate::GpuContext::new()).expect("native GPU transport adapter");
        let grid = Grid::new(2.0, 2.0).unwrap();
        let volume = grid.cell_volume_m3() as f32;
        let water_mass = 1000.0 * volume;
        let mass = [
            water_mass * (1.0 - 8e-6),
            water_mass * (1.0 + 8e-6),
            water_mass,
            water_mass,
            0.0,
            0.0,
            0.0,
            0.0,
        ];
        let marker = [0.25_f32, 0.5, 0.75, 1.0, 0.0, 0.0, 0.0, 0.0];
        let headers = (0..grid.cells())
            .flat_map(|_| GpuCellHeader::new(0.0, 0).unwrap().to_le_bytes())
            .collect::<Vec<_>>();
        let mut u = vec![0.0_f32; grid.u_faces()];
        u[grid.u_face_index(1, 0).unwrap()] = 5e-6;
        let v = vec![0.0_f32; grid.v_faces()];
        let mut u_aperture = vec![0.0_f32; grid.u_faces()];
        let mut v_aperture = vec![0.0_f32; grid.v_faces()];
        for y in 0..grid.height() {
            u_aperture[grid.u_face_index(1, y).unwrap()] = 1.0;
        }
        for x in 0..grid.width() {
            v_aperture[grid.v_face_index(x, 1).unwrap()] = 1.0;
        }
        let mass_buffer = buffer(&gpu.device, "imbalanced water mass", &f32_bytes(&mass));
        let marker_buffer = buffer(&gpu.device, "imbalanced marker", &f32_bytes(&marker));
        let header_buffer = buffer(&gpu.device, "open cell headers", &headers);
        let u_buffer = buffer(&gpu.device, "small divergent u", &f32_bytes(&u));
        let v_buffer = buffer(&gpu.device, "zero v", &f32_bytes(&v));
        let u_open_buffer = buffer(&gpu.device, "open u faces", &f32_bytes(&u_aperture));
        let v_open_buffer = buffer(&gpu.device, "open v faces", &f32_bytes(&v_aperture));
        let input = GpuTransportInput {
            mass_kg: &mass_buffer,
            marker: &marker_buffer,
            headers: &header_buffer,
            u_velocity_m_s: &u_buffer,
            v_velocity_m_s: &v_buffer,
            u_aperture: &u_open_buffer,
            v_aperture: &v_open_buffer,
        };
        let stage = GpuTransportStage::new(&gpu.device, grid).unwrap();
        let candidate = GpuTransportCandidate::new(&gpu.device, grid).unwrap();
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("conservative local closure fixture"),
            });
        stage
            .encode(
                &gpu.device,
                &mut encoder,
                &input,
                &candidate,
                GpuTransportStep {
                    dt_s: 0.01,
                    liquid_density_kg_m3: 1000.0,
                    carrier_density_kg_m3: 1.2,
                },
            )
            .unwrap();
        gpu.queue.submit([encoder.finish()]);
        let status = read_bytes(&gpu.device, &gpu.queue, &candidate.status);
        assert_eq!(u32::from_le_bytes(status.try_into().unwrap()), 0);
        let result_mass = read_f32(&gpu.device, &gpu.queue, &candidate.mass_kg);
        let result_marker = read_f32(&gpu.device, &gpu.queue, &candidate.marker);
        let face_flux = read_f32(&gpu.device, &gpu.queue, &candidate.u_flux);
        let face = grid.u_face_index(1, 0).unwrap();
        let source_fill = mass[0] / water_mass;
        let uncorrected_fill = source_fill - u[face] * 0.01 / CELL_WIDTH_M as f32;
        assert!(
            (uncorrected_fill - 1.0).abs() > 1e-5,
            "uncorrected candidate must cross the closure gate"
        );
        for cell in 0..grid.cells() {
            let fill = result_mass[cell] / 1000.0 / volume;
            assert!((fill - 1.0).abs() < 3e-6, "cell {cell} fill {fill}");
            assert_eq!(result_mass[grid.cells() + cell], 0.0);
            assert_eq!(result_marker[grid.cells() + cell], 0.0);
        }
        let sum = |values: &[f32]| values.iter().map(|&value| f64::from(value)).sum::<f64>();
        let before_mass = sum(&mass[..grid.cells()]);
        let after_mass = sum(&result_mass[..grid.cells()]);
        let before_marker = sum(&marker[..grid.cells()]);
        let after_marker = sum(&result_marker[..grid.cells()]);
        assert!((after_mass - before_mass).abs() <= before_mass * 1e-7);
        assert!((after_marker - before_marker).abs() <= before_marker * 1e-7);
        assert!(
            face_flux[face * 5] < 0.0,
            "correction reverses the excess outflow"
        );
        assert!((face_flux[face * 5 + 1] / 1000.0 - face_flux[face * 5]).abs() <= 1e-13);
        gpu.dispose();
    }

    #[test]
    #[ignore = "requires a native GPU adapter"]
    fn resolved_near_uniform_gradient_uses_cpu_geometric_branch() {
        let gpu = block_on(crate::GpuContext::new()).expect("native GPU adapter for E07 fixture");
        let grid = Grid::new(2.0, 2.0).unwrap();
        let fractions = [0.5, 0.500_000_5, 0.5, 0.5];
        let cell_volume = grid.cell_volume_m3();
        let liquid = fractions.map(|alpha| alpha * cell_volume * 1000.0);
        let carrier = fractions.map(|alpha| (1.0 - alpha) * cell_volume * 1.2);
        let initial = TransportInventory::new(
            grid,
            1000.0,
            1.2,
            liquid.to_vec(),
            carrier.to_vec(),
            vec![0.0; grid.cells()],
            vec![0.0; grid.cells()],
            vec![false; grid.cells()],
        )
        .unwrap();
        let mut u = vec![0.0; grid.u_faces()];
        let mut v = vec![0.0; grid.v_faces()];
        u[grid.u_face_index(1, 0).unwrap()] = 1.0;
        u[grid.u_face_index(1, 1).unwrap()] = -1.0;
        v[grid.v_face_index(0, 1).unwrap()] = -1.0;
        v[grid.v_face_index(1, 1).unwrap()] = 1.0;
        let mut u_aperture = vec![0.0; grid.u_faces()];
        let mut v_aperture = vec![0.0; grid.v_faces()];
        for y in 0..grid.height() {
            u_aperture[grid.u_face_index(1, y).unwrap()] = 1.0;
        }
        for x in 0..grid.width() {
            v_aperture[grid.v_face_index(x, 1).unwrap()] = 1.0;
        }
        let fields = PressureFields::new(
            grid,
            [Boundary::Closed; 4],
            vec![1000.0; grid.cells()],
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
        let cpu = initial
            .candidate(
                &fields,
                &FaceValues {
                    u: u.clone(),
                    v: v.clone(),
                },
                0.0025,
            )
            .unwrap();

        let mass = liquid
            .into_iter()
            .chain(carrier)
            .map(|value| value as f32)
            .collect::<Vec<_>>();
        let marker = vec![0.0_f32; grid.cells() * 2];
        let headers = (0..grid.cells())
            .flat_map(|_| GpuCellHeader::new(0.0, 0).unwrap().to_le_bytes())
            .collect::<Vec<_>>();
        let to_f32 = |values: &[f64]| values.iter().map(|value| *value as f32).collect::<Vec<_>>();
        let mass_buffer = buffer(&gpu.device, "near-uniform mass", &f32_bytes(&mass));
        let marker_buffer = buffer(&gpu.device, "near-uniform marker", &f32_bytes(&marker));
        let header_buffer = buffer(&gpu.device, "near-uniform headers", &headers);
        let u_buffer = buffer(&gpu.device, "near-uniform u", &f32_bytes(&to_f32(&u)));
        let v_buffer = buffer(&gpu.device, "near-uniform v", &f32_bytes(&to_f32(&v)));
        let u_aperture_buffer = buffer(
            &gpu.device,
            "near-uniform u aperture",
            &f32_bytes(&to_f32(&u_aperture)),
        );
        let v_aperture_buffer = buffer(
            &gpu.device,
            "near-uniform v aperture",
            &f32_bytes(&to_f32(&v_aperture)),
        );
        let input = GpuTransportInput {
            mass_kg: &mass_buffer,
            marker: &marker_buffer,
            headers: &header_buffer,
            u_velocity_m_s: &u_buffer,
            v_velocity_m_s: &v_buffer,
            u_aperture: &u_aperture_buffer,
            v_aperture: &v_aperture_buffer,
        };
        let stage = GpuTransportStage::new(&gpu.device, grid).unwrap();
        let candidate = GpuTransportCandidate::new(&gpu.device, grid).unwrap();
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("near-uniform transport fixture"),
            });
        stage
            .encode(
                &gpu.device,
                &mut encoder,
                &input,
                &candidate,
                GpuTransportStep {
                    dt_s: 0.0025,
                    liquid_density_kg_m3: 1000.0,
                    carrier_density_kg_m3: 1.2,
                },
            )
            .unwrap();
        gpu.queue.submit([encoder.finish()]);
        let status = read_bytes(&gpu.device, &gpu.queue, &candidate.status);
        assert_eq!(u32::from_le_bytes(status.try_into().unwrap()), 0);
        let actual_mass = read_f32(&gpu.device, &gpu.queue, &candidate.mass_kg);
        let max_alpha_error = (0..grid.cells())
            .map(|cell| {
                let gpu_alpha = actual_mass[cell] as f64 / 1000.0 / cell_volume;
                (gpu_alpha - cpu.inventory.alpha(cell).unwrap()).abs()
            })
            .fold(0.0_f64, f64::max);
        println!("near-uniform GPU alpha parity: max error {max_alpha_error:.3e}");
        assert!(max_alpha_error <= 1e-5, "frozen one-tick alpha parity gate");
        gpu.dispose();
    }
}
