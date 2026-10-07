//! Bounded f32 closed-boundary MAC projection on resident GPU buffers.
//!
//! The caller owns both predictor fields and distinct candidate output fields.
//! A failed solve leaves those candidate fields unpublished. This module has no
//! CPU solver path and reads back only a 32-byte completion record per batch.

// E07 stage-graph integration will call the crate-private projector.
#![allow(dead_code)]

use particle_sim::{CELL_WIDTH_M, Grid};

const WORKGROUP_SIZE: u32 = 64;
const STATUS_BYTES: u64 = 32;
const CONVERGED_WORD_OFFSET: u64 = 6 * size_of::<u32>() as u64;
const PARAM_BYTES: u64 = 48;
const MAX_ITERATIONS: u32 = 1024;
// Browser mapping boundaries are costly. The measured 64-iteration batch
// balances fewer completion waits against work padded past convergence.
// Retain the native batch until the native hardware comparison is repeated.
#[cfg(target_arch = "wasm32")]
const ITERATIONS_PER_BATCH: u32 = 64;
#[cfg(not(target_arch = "wasm32"))]
const ITERATIONS_PER_BATCH: u32 = 32;

/// GPU fields supplied by the stage graph. Every field is a distinct buffer.
///
/// Arrays are row-major in the portable MAC layout. Density is positive and
/// finite. Interior apertures are finite in [0, 1]. Closed outer apertures and
/// blocked predictor faces are zero. The stage graph validates these facts at
/// the scene boundary before this GPU-only operation.
#[derive(Clone, Copy)]
pub(crate) struct PressureFields<'a> {
    pub density: &'a wgpu::Buffer,
    /// Static minimum row-major cell index for each cell's closed component.
    pub root_cell: &'a wgpu::Buffer,
    pub aperture_u: &'a wgpu::Buffer,
    pub aperture_v: &'a wgpu::Buffer,
    pub predictor_u: &'a wgpu::Buffer,
    pub predictor_v: &'a wgpu::Buffer,
    pub corrected_u: &'a wgpu::Buffer,
    pub corrected_v: &'a wgpu::Buffer,
}

/// Fixed GPU iteration budget and frozen f32 acceptance gates.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PressureConfig {
    pub dt_s: f32,
    /// Uniform acceleration already applied to the vertical predictor.
    pub gravity_m_s2: f32,
    pub max_iterations: u32,
    /// `dt² max|b - A pressure|`.
    pub scaled_residual_tolerance: f32,
    /// `dt max|D corrected|` for a zero-source sealed fixture.
    pub scaled_divergence_tolerance: f32,
}

impl Default for PressureConfig {
    fn default() -> Self {
        Self {
            dt_s: 1.0 / 60.0,
            gravity_m_s2: 0.0,
            max_iterations: 512,
            scaled_residual_tolerance: 1e-5,
            scaled_divergence_tolerance: 1e-5,
        }
    }
}

/// Two independently reduced gates from one candidate pressure correction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PressureDiagnostics {
    pub iterations: u32,
    pub scaled_residual: f32,
    pub scaled_divergence: f32,
    /// Actual compact completion records copied from GPU to host for this solve.
    pub readback_bytes: u64,
}

/// Numerical nonconvergence can be retried with a smaller candidate duration.
#[derive(Debug)]
pub(crate) enum PressureError {
    Nonconvergence(String),
    Invalid(String),
}

impl PressureError {
    pub(crate) fn is_retryable(&self) -> bool {
        matches!(self, Self::Nonconvergence(_))
    }
}

impl std::fmt::Display for PressureError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Nonconvergence(message) | Self::Invalid(message) => formatter.write_str(message),
        }
    }
}

impl From<String> for PressureError {
    fn from(message: String) -> Self {
        Self::Invalid(message)
    }
}

impl From<PressureError> for String {
    fn from(error: PressureError) -> Self {
        error.to_string()
    }
}

struct Pipelines {
    rhs: wgpu::ComputePipeline,
    hydrostatic: wgpu::ComputePipeline,
    hydrostatic_reference: wgpu::ComputePipeline,
    init_preconditioner: wgpu::ComputePipeline,
    init_preconditioner_from_guess: wgpu::ComputePipeline,
    apply_search: wgpu::ComputePipeline,
    update_solution: wgpu::ComputePipeline,
    update_preconditioner: wgpu::ComputePipeline,
    update_search: wgpu::ComputePipeline,
    reduce_initial: wgpu::ComputePipeline,
    reduce_denominator: wgpu::ComputePipeline,
    reduce_updated: wgpu::ComputePipeline,
    reduce_cached: wgpu::ComputePipeline,
    finish_initial: wgpu::ComputePipeline,
    finish_alpha: wgpu::ComputePipeline,
    finish_beta: wgpu::ComputePipeline,
    finish_final_residual: wgpu::ComputePipeline,
    correct_u: wgpu::ComputePipeline,
    correct_v: wgpu::ComputePipeline,
    divergence: wgpu::ComputePipeline,
    finish_completion: wgpu::ComputePipeline,
    gauge_reset: wgpu::ComputePipeline,
    gauge_link: wgpu::ComputePipeline,
    gauge_mean: wgpu::ComputePipeline,
    gauge_center: wgpu::ComputePipeline,
}

#[derive(Debug)]
struct SolveStatus {
    scaled_residual: f32,
    scaled_divergence: f32,
    iterations: u32,
    converged: u32,
    invalid: u32,
}

/// Scratch and pipelines for one base-spacing closed grid.
pub(crate) struct GpuPressureProjector {
    grid: Grid,
    rhs: wgpu::Buffer,
    hydrostatic_base: wgpu::Buffer,
    pressure: wgpu::Buffer,
    accepted_guess: wgpu::Buffer,
    has_accepted_guess: bool,
    residual: wgpu::Buffer,
    preconditioned: wgpu::Buffer,
    search: wgpu::Buffer,
    applied_search: wgpu::Buffer,
    component_heads: wgpu::Buffer,
    component_next: wgpu::Buffer,
    component_mean: wgpu::Buffer,
    partials: wgpu::Buffer,
    status: wgpu::Buffer,
    partial_count: u32,
    pipelines: Pipelines,
    last_readback_bytes: u64,
}

mod bindings;
mod readback;
mod setup;
mod solve;

use readback::{read_status, stage_status_readback};

impl GpuPressureProjector {
    /// Compact records copied for the latest solve, including a rejected solve.
    pub(crate) fn last_readback_bytes(&self) -> u64 {
        self.last_readback_bytes
    }

    /// Promotes the numerical guess with the caller's accepted scene commit.
    /// Rejected candidates cannot change this cache or poison a later retry.
    pub(crate) fn encode_accepted_guess(&mut self, encoder: &mut wgpu::CommandEncoder) {
        encoder.copy_buffer_to_buffer(
            &self.pressure,
            0,
            &self.accepted_guess,
            0,
            self.pressure.size(),
        );
        self.has_accepted_guess = true;
    }

    fn check_fields(
        &self,
        device: &wgpu::Device,
        fields: &PressureFields<'_>,
    ) -> Result<(), String> {
        let limits = device.limits();
        for (label, buffer, bytes) in [
            ("density", fields.density, self.grid.cells() as u64 * 4),
            (
                "component roots",
                fields.root_cell,
                self.grid.cells() as u64 * 4,
            ),
            (
                "horizontal aperture",
                fields.aperture_u,
                self.grid.u_faces() as u64 * 4,
            ),
            (
                "vertical aperture",
                fields.aperture_v,
                self.grid.v_faces() as u64 * 4,
            ),
            (
                "horizontal predictor",
                fields.predictor_u,
                self.grid.u_faces() as u64 * 4,
            ),
            (
                "vertical predictor",
                fields.predictor_v,
                self.grid.v_faces() as u64 * 4,
            ),
            (
                "horizontal candidate",
                fields.corrected_u,
                self.grid.u_faces() as u64 * 4,
            ),
            (
                "vertical candidate",
                fields.corrected_v,
                self.grid.v_faces() as u64 * 4,
            ),
        ] {
            if buffer.size() != bytes
                || bytes > limits.max_storage_buffer_binding_size
                || !buffer.usage().contains(wgpu::BufferUsages::STORAGE)
            {
                return Err(format!("{label} is not a valid GPU pressure field"));
            }
        }
        Ok(())
    }
}
fn validate_config(config: PressureConfig) -> Result<(), String> {
    if !config.dt_s.is_finite() || config.dt_s <= 0.0 {
        return Err("GPU pressure time step must be positive and finite".into());
    }
    if !config.gravity_m_s2.is_finite() {
        return Err("GPU pressure gravity must be finite".into());
    }
    if config.max_iterations > MAX_ITERATIONS {
        return Err(format!(
            "GPU pressure iteration budget exceeds {MAX_ITERATIONS}"
        ));
    }
    for (name, value) in [
        ("scaled residual", config.scaled_residual_tolerance),
        ("scaled divergence", config.scaled_divergence_tolerance),
    ] {
        if !value.is_finite() || value <= 0.0 {
            return Err(format!(
                "GPU pressure {name} tolerance must be positive and finite"
            ));
        }
    }
    Ok(())
}

fn shader(device: &wgpu::Device, label: &str, source: &'static str) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    })
}

fn pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    entry: &str,
) -> wgpu::ComputePipeline {
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(entry),
        layout: None,
        module: shader,
        entry_point: Some(entry),
        compilation_options: Default::default(),
        cache: None,
    })
}

fn bind(
    device: &wgpu::Device,
    pipeline: &wgpu::ComputePipeline,
    entries: &[(u32, &wgpu::Buffer)],
) -> wgpu::BindGroup {
    let entries = entries
        .iter()
        .map(|(binding, buffer)| wgpu::BindGroupEntry {
            binding: *binding,
            resource: buffer.as_entire_binding(),
        })
        .collect::<Vec<_>>();
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("pressure stage"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &entries,
    })
}

fn dispatch(
    pass: &mut wgpu::ComputePass<'_>,
    pipeline: &wgpu::ComputePipeline,
    bindings: &wgpu::BindGroup,
    groups: u32,
) {
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bindings, &[]);
    pass.dispatch_workgroups(groups, 1, 1);
}

#[cfg(test)]
mod tests;
