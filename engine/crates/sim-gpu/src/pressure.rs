//! Bounded f32 closed-boundary MAC projection on resident GPU buffers.
//!
//! The caller owns both predictor fields and distinct candidate output fields.
//! A failed solve leaves those candidate fields unpublished. This module has no
//! CPU solver path and reads back only a 32-byte completion record per batch.

// E07 stage-graph integration will call the crate-private projector.
#![allow(dead_code)]

use std::{
    future::poll_fn,
    sync::{Arc, Mutex},
    task::{Poll, Waker},
};

use particle_sim::{CELL_WIDTH_M, Grid};
use wgpu::util::DeviceExt;

const WORKGROUP_SIZE: u32 = 64;
const STATUS_BYTES: u64 = 32;
const CONVERGED_WORD_OFFSET: u64 = 6 * size_of::<u32>() as u64;
const PARAM_BYTES: u64 = 48;
const MAX_ITERATIONS: u32 = 1024;
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

impl GpuPressureProjector {
    /// Allocates only solver scratch after checking effective adapter limits.
    /// Face-density coefficients are recomputed from current density and
    /// aperture buffers on each dispatch, so stale coefficients cannot survive
    /// a fixture step.
    pub(crate) fn new(device: &wgpu::Device, grid: Grid) -> Result<Self, String> {
        if grid.cell_width_m() != CELL_WIDTH_M {
            return Err("GPU pressure projection requires the base 0.01 m cell width".into());
        }
        let cells = grid.cells() as u32;
        let partial_count = cells.div_ceil(WORKGROUP_SIZE);
        let u_groups = (grid.u_faces() as u32).div_ceil(WORKGROUP_SIZE);
        let v_groups = (grid.v_faces() as u32).div_ceil(WORKGROUP_SIZE);
        let limits = device.limits();
        if WORKGROUP_SIZE > limits.max_compute_invocations_per_workgroup
            || WORKGROUP_SIZE > limits.max_compute_workgroup_size_x
            || partial_count > limits.max_compute_workgroups_per_dimension
            || u_groups > limits.max_compute_workgroups_per_dimension
            || v_groups > limits.max_compute_workgroups_per_dimension
            || limits.max_storage_buffers_per_shader_stage < 8
            || limits.max_bindings_per_bind_group < 13
            || limits.max_uniform_buffer_binding_size < PARAM_BYTES
        {
            return Err("GPU pressure projection exceeds effective adapter limits".into());
        }
        let cell_bytes = u64::from(cells) * size_of::<f32>() as u64;
        let hydrostatic_bytes = cell_bytes + u64::from(grid.height()) * size_of::<f32>() as u64;
        let partial_bytes = u64::from(partial_count) * 16;
        for (name, bytes) in [
            ("pressure cell scratch", cell_bytes),
            ("pressure hydrostatic reference", hydrostatic_bytes),
            ("pressure residual reduction", partial_bytes),
            ("pressure completion", STATUS_BYTES),
        ] {
            if bytes > limits.max_storage_buffer_binding_size || bytes > limits.max_buffer_size {
                return Err(format!("{name} exceeds effective GPU storage limits"));
            }
        }

        let scratch = |label| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: cell_bytes,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_SRC
                    | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let rhs = scratch("pressure rhs");
        let hydrostatic_base = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pressure hydrostatic base and face reference"),
            size: hydrostatic_bytes,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let pressure = scratch("pressure dynamic correction");
        let accepted_guess = scratch("accepted pressure starting guess");
        let residual = scratch("pressure recursive residual");
        let preconditioned = scratch("pressure preconditioned residual");
        let search = scratch("pressure search direction");
        let applied_search = scratch("pressure applied search");
        let component_heads = scratch("pressure component list heads");
        let component_next = scratch("pressure component list links");
        let component_mean = scratch("pressure component means");
        let partials = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pressure partial reductions"),
            size: partial_bytes,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let status = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pressure completion status"),
            size: STATUS_BYTES,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let rhs_shader = shader(
            device,
            "pressure rhs",
            include_str!("../shaders/pressure_rhs.wgsl"),
        );
        let hydrostatic_shader = shader(
            device,
            "pressure hydrostatic initial guess",
            include_str!("../shaders/pressure_hydrostatic.wgsl"),
        );
        let pcg_shader = shader(
            device,
            "pressure PCG",
            include_str!("../shaders/pressure_pcg.wgsl"),
        );
        let finish_shader = shader(
            device,
            "pressure finish",
            include_str!("../shaders/pressure_finish.wgsl"),
        );
        let correct_shader = shader(
            device,
            "pressure correction",
            include_str!("../shaders/pressure_correct.wgsl"),
        );
        let divergence_shader = shader(
            device,
            "pressure divergence",
            include_str!("../shaders/pressure_divergence.wgsl"),
        );
        let gauge_shader = shader(
            device,
            "pressure component mean gauge",
            include_str!("../shaders/pressure_gauge.wgsl"),
        );
        let pipelines = Pipelines {
            rhs: pipeline(device, &rhs_shader, "main"),
            hydrostatic: pipeline(device, &hydrostatic_shader, "main"),
            hydrostatic_reference: pipeline(device, &hydrostatic_shader, "reference"),
            init_preconditioner: pipeline(device, &pcg_shader, "init_preconditioner"),
            init_preconditioner_from_guess: pipeline(
                device,
                &pcg_shader,
                "init_preconditioner_from_guess",
            ),
            apply_search: pipeline(device, &pcg_shader, "apply_search"),
            update_solution: pipeline(device, &pcg_shader, "update_solution"),
            update_preconditioner: pipeline(device, &pcg_shader, "update_preconditioner"),
            update_search: pipeline(device, &pcg_shader, "update_search"),
            reduce_initial: pipeline(device, &pcg_shader, "reduce_initial"),
            reduce_denominator: pipeline(device, &pcg_shader, "reduce_denominator"),
            reduce_updated: pipeline(device, &pcg_shader, "reduce_updated"),
            finish_initial: pipeline(device, &finish_shader, "initial"),
            finish_alpha: pipeline(device, &finish_shader, "alpha"),
            finish_beta: pipeline(device, &finish_shader, "beta"),
            finish_final_residual: pipeline(device, &finish_shader, "final_residual"),
            correct_u: pipeline(device, &correct_shader, "u"),
            correct_v: pipeline(device, &correct_shader, "v"),
            divergence: pipeline(device, &divergence_shader, "main"),
            finish_completion: pipeline(device, &finish_shader, "completion"),
            gauge_reset: pipeline(device, &gauge_shader, "reset"),
            gauge_link: pipeline(device, &gauge_shader, "link"),
            gauge_mean: pipeline(device, &gauge_shader, "mean"),
            gauge_center: pipeline(device, &gauge_shader, "center"),
        };
        Ok(Self {
            grid,
            rhs,
            hydrostatic_base,
            pressure,
            accepted_guess,
            has_accepted_guess: false,
            residual,
            preconditioned,
            search,
            applied_search,
            component_heads,
            component_next,
            component_mean,
            partials,
            status,
            partial_count,
            pipelines,
            last_readback_bytes: 0,
        })
    }

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

    /// Projects a sealed, zero-volume-source predictor into distinct candidate
    /// face buffers. The caller must publish them only after this returns `Ok`.
    /// Every iteration and reduction stays on the GPU. A 32-byte completion
    /// record is mapped per bounded batch. No full cell or face array is read back.
    pub(crate) async fn project_closed(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        fields: PressureFields<'_>,
        config: PressureConfig,
    ) -> Result<PressureDiagnostics, PressureError> {
        self.last_readback_bytes = 0;
        validate_config(config)?;
        self.check_fields(device, &fields)?;
        let mut params = Vec::with_capacity(PARAM_BYTES as usize);
        for word in [
            self.grid.width(),
            self.grid.height(),
            self.grid.cells() as u32,
            self.partial_count,
        ] {
            params.extend_from_slice(&word.to_le_bytes());
        }
        for word in [
            self.grid.cell_width_m() as f32,
            config.dt_s,
            config.scaled_residual_tolerance,
            config.scaled_divergence_tolerance,
            config.gravity_m_s2,
            if self.has_accepted_guess && config.max_iterations > 0 {
                1.0
            } else {
                0.0
            },
            0.0,
            0.0,
        ] {
            params.extend_from_slice(&word.to_le_bytes());
        }
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("pressure parameters"),
            contents: &params,
            usage: wgpu::BufferUsages::UNIFORM,
        });
        queue.write_buffer(&self.status, 0, &[0; STATUS_BYTES as usize]);

        let rhs_bind = bind(
            device,
            &self.pipelines.rhs,
            &[
                (0, &uniform),
                (1, fields.aperture_u),
                (2, fields.aperture_v),
                (3, fields.predictor_u),
                (4, fields.predictor_v),
                (5, &self.rhs),
                (6, &self.pressure),
                (7, &self.hydrostatic_base),
            ],
        );
        let hydrostatic_bind = (config.gravity_m_s2 != 0.0).then(|| {
            bind(
                device,
                &self.pipelines.hydrostatic,
                &[
                    (0, &uniform),
                    (2, fields.aperture_v),
                    (3, &self.hydrostatic_base),
                ],
            )
        });
        let hydrostatic_reference_bind = (config.gravity_m_s2 != 0.0).then(|| {
            bind(
                device,
                &self.pipelines.hydrostatic_reference,
                &[
                    (0, &uniform),
                    (1, fields.density),
                    (2, fields.aperture_v),
                    (3, &self.hydrostatic_base),
                ],
            )
        });
        let init_pipeline = if hydrostatic_bind.is_some() {
            &self.pipelines.init_preconditioner_from_guess
        } else {
            &self.pipelines.init_preconditioner
        };
        let init_preconditioner = if hydrostatic_bind.is_some() {
            bind(
                device,
                init_pipeline,
                &[
                    (0, &uniform),
                    (1, fields.density),
                    (2, fields.aperture_u),
                    (3, fields.aperture_v),
                    (4, &self.rhs),
                    (12, &self.hydrostatic_base),
                    (5, &self.pressure),
                    (6, &self.residual),
                    (7, &self.preconditioned),
                ],
            )
        } else {
            bind(
                device,
                init_pipeline,
                &[
                    (0, &uniform),
                    (1, fields.density),
                    (2, fields.aperture_u),
                    (3, fields.aperture_v),
                    (4, &self.rhs),
                    (5, &self.pressure),
                    (6, &self.residual),
                    (7, &self.preconditioned),
                ],
            )
        };
        let apply_search = bind(
            device,
            &self.pipelines.apply_search,
            &[
                (0, &uniform),
                (1, fields.density),
                (2, fields.aperture_u),
                (3, fields.aperture_v),
                (8, &self.search),
                (9, &self.applied_search),
            ],
        );
        let update_solution = bind(
            device,
            &self.pipelines.update_solution,
            &[
                (0, &uniform),
                (5, &self.pressure),
                (6, &self.residual),
                (8, &self.search),
                (9, &self.applied_search),
                (11, &self.status),
            ],
        );
        let update_preconditioner = bind(
            device,
            &self.pipelines.update_preconditioner,
            &[
                (0, &uniform),
                (1, fields.density),
                (2, fields.aperture_u),
                (3, fields.aperture_v),
                (4, &self.rhs),
                (5, &self.pressure),
                (12, &self.hydrostatic_base),
                (6, &self.residual),
                (7, &self.preconditioned),
            ],
        );
        let update_search = bind(
            device,
            &self.pipelines.update_search,
            &[
                (0, &uniform),
                (7, &self.preconditioned),
                (8, &self.search),
                (11, &self.status),
            ],
        );
        let reduce_initial = bind(
            device,
            &self.pipelines.reduce_initial,
            &[
                (0, &uniform),
                (6, &self.residual),
                (7, &self.preconditioned),
                (10, &self.partials),
            ],
        );
        let reduce_denominator = bind(
            device,
            &self.pipelines.reduce_denominator,
            &[
                (0, &uniform),
                (8, &self.search),
                (9, &self.applied_search),
                (10, &self.partials),
            ],
        );
        let reduce_updated = bind(
            device,
            &self.pipelines.reduce_updated,
            &[
                (0, &uniform),
                (1, fields.density),
                (2, fields.aperture_u),
                (3, fields.aperture_v),
                (4, &self.rhs),
                (5, &self.pressure),
                (12, &self.hydrostatic_base),
                (6, &self.residual),
                (10, &self.partials),
            ],
        );
        let finish_initial = bind(
            device,
            &self.pipelines.finish_initial,
            &[(0, &uniform), (1, &self.partials), (2, &self.status)],
        );
        let finish_alpha = bind(
            device,
            &self.pipelines.finish_alpha,
            &[(0, &uniform), (1, &self.partials), (2, &self.status)],
        );
        let finish_beta = bind(
            device,
            &self.pipelines.finish_beta,
            &[(0, &uniform), (1, &self.partials), (2, &self.status)],
        );
        let finish_completion = bind(
            device,
            &self.pipelines.finish_completion,
            &[(0, &uniform), (1, &self.partials), (2, &self.status)],
        );
        let correct_u = bind(
            device,
            &self.pipelines.correct_u,
            &[
                (0, &uniform),
                (1, fields.density),
                (2, fields.aperture_u),
                (4, fields.predictor_u),
                (6, &self.pressure),
                (9, &self.hydrostatic_base),
                (7, fields.corrected_u),
            ],
        );
        let correct_v = bind(
            device,
            &self.pipelines.correct_v,
            &[
                (0, &uniform),
                (1, fields.density),
                (3, fields.aperture_v),
                (5, fields.predictor_v),
                (6, &self.pressure),
                (8, fields.corrected_v),
                (9, &self.hydrostatic_base),
            ],
        );
        let divergence = bind(
            device,
            &self.pipelines.divergence,
            &[
                (0, &uniform),
                (1, fields.aperture_u),
                (2, fields.aperture_v),
                (3, fields.corrected_u),
                (4, fields.corrected_v),
                (5, &self.partials),
            ],
        );

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("pressure PCG initialization"),
        });
        if self.has_accepted_guess && config.max_iterations > 0 {
            encoder.copy_buffer_to_buffer(
                &self.accepted_guess,
                0,
                &self.pressure,
                0,
                self.pressure.size(),
            );
        }
        dispatch(
            &mut encoder,
            &self.pipelines.rhs,
            &rhs_bind,
            self.partial_count,
        );
        if let Some(hydrostatic_bind) = &hydrostatic_bind {
            dispatch(
                &mut encoder,
                &self.pipelines.hydrostatic_reference,
                hydrostatic_reference_bind
                    .as_ref()
                    .expect("gravity reference binding"),
                self.grid.height().div_ceil(WORKGROUP_SIZE),
            );
            dispatch(
                &mut encoder,
                &self.pipelines.hydrostatic,
                hydrostatic_bind,
                self.grid.width().div_ceil(WORKGROUP_SIZE),
            );
        }
        dispatch(
            &mut encoder,
            init_pipeline,
            &init_preconditioner,
            self.partial_count,
        );
        encoder.copy_buffer_to_buffer(&self.preconditioned, 0, &self.search, 0, self.search.size());
        dispatch(
            &mut encoder,
            &self.pipelines.reduce_initial,
            &reduce_initial,
            self.partial_count,
        );
        dispatch(
            &mut encoder,
            &self.pipelines.finish_initial,
            &finish_initial,
            1,
        );
        let staging = stage_status_readback(device, &mut encoder, &self.status);
        queue.submit([encoder.finish()]);
        self.last_readback_bytes += STATUS_BYTES;
        let mut status = read_status(device, staging).await?;
        let mut status_readbacks = 1_u64;
        if status.invalid != 0 {
            return Err(PressureError::Invalid(format!(
                "GPU pressure arithmetic rejected the initial fields: {status:?}"
            )));
        }

        while status.converged == 0 && status.iterations < config.max_iterations {
            let previous_iterations = status.iterations;
            let batch = (config.max_iterations - status.iterations).min(ITERATIONS_PER_BATCH);
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("bounded pressure PCG batch"),
            });
            for _ in 0..batch {
                dispatch(
                    &mut encoder,
                    &self.pipelines.apply_search,
                    &apply_search,
                    self.partial_count,
                );
                dispatch(
                    &mut encoder,
                    &self.pipelines.reduce_denominator,
                    &reduce_denominator,
                    self.partial_count,
                );
                dispatch(&mut encoder, &self.pipelines.finish_alpha, &finish_alpha, 1);
                dispatch(
                    &mut encoder,
                    &self.pipelines.update_solution,
                    &update_solution,
                    self.partial_count,
                );
                dispatch(
                    &mut encoder,
                    &self.pipelines.update_preconditioner,
                    &update_preconditioner,
                    self.partial_count,
                );
                dispatch(
                    &mut encoder,
                    &self.pipelines.reduce_updated,
                    &reduce_updated,
                    self.partial_count,
                );
                dispatch(&mut encoder, &self.pipelines.finish_beta, &finish_beta, 1);
                dispatch(
                    &mut encoder,
                    &self.pipelines.update_search,
                    &update_search,
                    self.partial_count,
                );
            }
            let staging = stage_status_readback(device, &mut encoder, &self.status);
            queue.submit([encoder.finish()]);
            self.last_readback_bytes += STATUS_BYTES;
            status = read_status(device, staging).await?;
            status_readbacks += 1;
            if status.invalid != 0 {
                return Err(PressureError::Invalid(format!(
                    "GPU pressure arithmetic failed: {status:?}"
                )));
            }
            if status.converged == 0 && status.iterations <= previous_iterations {
                return Err(PressureError::Invalid(format!(
                    "GPU pressure PCG made no bounded progress: {status:?}"
                )));
            }
        }
        if status.converged == 0 {
            // The half-tolerance stop reserves rounding headroom. At the fixed
            // iteration cap, a residual already inside the final gate can
            // still be corrected and measured. Neither gate is bypassed.
            let within_final_residual = status.scaled_residual <= config.scaled_residual_tolerance;
            if config.max_iterations == 0 || !within_final_residual {
                return Err(PressureError::Nonconvergence(format!(
                    "GPU pressure projection exhausted its bounded iterations: {status:?}"
                )));
            }
            // Status.converged is word six in pressure_finish.wgsl. This only
            // enables final validation; publication still requires its result.
            queue.write_buffer(&self.status, CONVERGED_WORD_OFFSET, &1_u32.to_le_bytes());
        }

        let gauge_reset = bind(
            device,
            &self.pipelines.gauge_reset,
            &[(0, &uniform), (2, &self.component_heads)],
        );
        let gauge_link = bind(
            device,
            &self.pipelines.gauge_link,
            &[
                (0, &uniform),
                (1, fields.root_cell),
                (2, &self.component_heads),
                (3, &self.component_next),
            ],
        );
        let gauge_mean = bind(
            device,
            &self.pipelines.gauge_mean,
            &[
                (0, &uniform),
                (1, fields.root_cell),
                (2, &self.component_heads),
                (3, &self.component_next),
                (4, &self.pressure),
                (5, &self.component_mean),
            ],
        );
        let gauge_center = bind(
            device,
            &self.pipelines.gauge_center,
            &[
                (0, &uniform),
                (1, fields.root_cell),
                (4, &self.pressure),
                (5, &self.component_mean),
            ],
        );
        let finish_final_residual = bind(
            device,
            &self.pipelines.finish_final_residual,
            &[(0, &uniform), (1, &self.partials), (2, &self.status)],
        );
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("pressure correction and divergence gate"),
        });
        let u_groups = (self.grid.u_faces() as u32).div_ceil(WORKGROUP_SIZE);
        let v_groups = (self.grid.v_faces() as u32).div_ceil(WORKGROUP_SIZE);
        // The hydrostatic seed uses the top of each open column as its pressure
        // reference. Mean-centering it magnifies f32 cancellation in thin air.
        if hydrostatic_bind.is_none() {
            dispatch(
                &mut encoder,
                &self.pipelines.gauge_reset,
                &gauge_reset,
                self.partial_count,
            );
            dispatch(
                &mut encoder,
                &self.pipelines.gauge_link,
                &gauge_link,
                self.partial_count,
            );
            dispatch(
                &mut encoder,
                &self.pipelines.gauge_mean,
                &gauge_mean,
                self.partial_count,
            );
            dispatch(
                &mut encoder,
                &self.pipelines.gauge_center,
                &gauge_center,
                self.partial_count,
            );
        }
        dispatch(
            &mut encoder,
            &self.pipelines.reduce_updated,
            &reduce_updated,
            self.partial_count,
        );
        dispatch(
            &mut encoder,
            &self.pipelines.finish_final_residual,
            &finish_final_residual,
            1,
        );
        dispatch(
            &mut encoder,
            &self.pipelines.correct_u,
            &correct_u,
            u_groups,
        );
        dispatch(
            &mut encoder,
            &self.pipelines.correct_v,
            &correct_v,
            v_groups,
        );
        dispatch(
            &mut encoder,
            &self.pipelines.divergence,
            &divergence,
            self.partial_count,
        );
        dispatch(
            &mut encoder,
            &self.pipelines.finish_completion,
            &finish_completion,
            1,
        );
        let staging = stage_status_readback(device, &mut encoder, &self.status);
        queue.submit([encoder.finish()]);
        self.last_readback_bytes += STATUS_BYTES;
        status = read_status(device, staging).await?;
        status_readbacks += 1;
        let diagnostics = PressureDiagnostics {
            iterations: status.iterations,
            scaled_residual: status.scaled_residual,
            scaled_divergence: status.scaled_divergence,
            readback_bytes: status_readbacks * STATUS_BYTES,
        };
        if status.invalid != 0 || status.converged == 0 {
            let message = format!("GPU pressure projection failed its final gate: {status:?}");
            return Err(if status.invalid != 0 {
                PressureError::Invalid(message)
            } else {
                PressureError::Nonconvergence(message)
            });
        }
        Ok(diagnostics)
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

fn stage_status_readback(
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

async fn read_status(device: &wgpu::Device, staging: wgpu::Buffer) -> Result<SolveStatus, String> {
    wait_for_map(device, &staging).await?;
    let mapped = staging.get_mapped_range(..);
    let words = mapped.as_chunks::<4>().0;
    let status = SolveStatus {
        scaled_residual: f32::from_bits(u32::from_le_bytes(words[3])),
        scaled_divergence: f32::from_bits(u32::from_le_bytes(words[4])),
        iterations: u32::from_le_bytes(words[5]),
        converged: u32::from_le_bytes(words[6]),
        invalid: u32::from_le_bytes(words[7]),
    };
    drop(mapped);
    staging.unmap();
    Ok(status)
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

#[cfg(test)]
mod tests {
    mod warm_start;
    use std::{
        future::Future,
        sync::Arc,
        task::{Context, Poll, Wake, Waker},
        time::{Duration, Instant},
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

    async fn read_f32(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        buffer: &wgpu::Buffer,
    ) -> Vec<f32> {
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

    #[test]
    #[ignore = "requires a native compute adapter"]
    fn pcg_stop_reserves_headroom_without_changing_final_gates() {
        block_on(async {
            let context = crate::GpuContext::new().await.unwrap();
            let mut params = Vec::with_capacity(PARAM_BYTES as usize);
            for value in [1_u32, 1, 1, 1] {
                params.extend_from_slice(&value.to_le_bytes());
            }
            for value in [0.01_f32, 1.0, 1e-5, 1e-5, 0.0, 0.0, 0.0, 0.0] {
                params.extend_from_slice(&value.to_le_bytes());
            }
            let uniform = context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("pressure headroom test parameters"),
                    contents: &params,
                    usage: wgpu::BufferUsages::UNIFORM,
                });
            let partials = context.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("pressure headroom test partial"),
                size: 16,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let status_buffer = context.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("pressure headroom test status"),
                size: STATUS_BYTES,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            context
                .queue
                .write_buffer(&status_buffer, 0, &[0; STATUS_BYTES as usize]);
            let finish_shader = shader(
                &context.device,
                "pressure headroom test finish",
                include_str!("../shaders/pressure_finish.wgsl"),
            );
            let initial = pipeline(&context.device, &finish_shader, "initial");
            let final_residual = pipeline(&context.device, &finish_shader, "final_residual");
            let completion = pipeline(&context.device, &finish_shader, "completion");
            let stage_bind = |pipeline: &wgpu::ComputePipeline| {
                bind(
                    &context.device,
                    pipeline,
                    &[(0, &uniform), (1, &partials), (2, &status_buffer)],
                )
            };
            let initial_bind = stage_bind(&initial);
            let final_bind = stage_bind(&final_residual);
            let completion_bind = stage_bind(&completion);
            let write_partial = |maximum: f32| {
                let mut bytes = [0_u8; 16];
                bytes[..4].copy_from_slice(&1.0_f32.to_le_bytes());
                bytes[4..8].copy_from_slice(&maximum.to_le_bytes());
                context.queue.write_buffer(&partials, 0, &bytes);
            };

            write_partial(6.5e-6);
            let mut encoder =
                context
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("pressure headroom internal stop"),
                    });
            dispatch(&mut encoder, &initial, &initial_bind, 1);
            let staging = stage_status_readback(&context.device, &mut encoder, &status_buffer);
            context.queue.submit([encoder.finish()]);
            let status = read_status(&context.device, staging).await.unwrap();
            assert_eq!(status.converged, 0, "{status:?}");
            assert!(status.scaled_residual < 1e-5, "{status:?}");

            write_partial(4.5e-6);
            let mut encoder =
                context
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("pressure headroom converged stop"),
                    });
            dispatch(&mut encoder, &initial, &initial_bind, 1);
            context.queue.submit([encoder.finish()]);

            write_partial(9.5e-6);
            let mut encoder =
                context
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("pressure headroom final gates"),
                    });
            dispatch(&mut encoder, &final_residual, &final_bind, 1);
            dispatch(&mut encoder, &completion, &completion_bind, 1);
            let staging = stage_status_readback(&context.device, &mut encoder, &status_buffer);
            context.queue.submit([encoder.finish()]);
            let status = read_status(&context.device, staging).await.unwrap();
            assert_eq!(status.converged, 1, "{status:?}");
            assert!(status.scaled_residual > 9e-6, "{status:?}");
            assert!(status.scaled_divergence > 9e-6, "{status:?}");

            write_partial(1.0007e-5);
            let mut encoder =
                context
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("pressure headroom rejected divergence"),
                    });
            dispatch(&mut encoder, &completion, &completion_bind, 1);
            let staging = stage_status_readback(&context.device, &mut encoder, &status_buffer);
            context.queue.submit([encoder.finish()]);
            let status = read_status(&context.device, staging).await.unwrap();
            assert_eq!(status.converged, 0, "{status:?}");
            assert!(status.scaled_divergence > 1e-5, "{status:?}");
            context.dispose();
        });
    }

    #[test]
    #[ignore = "requires a native compute adapter"]
    fn mixed_density_hydrostatic_guess_closes_vertical_gravity_faces() {
        block_on(async {
            let context = crate::GpuContext::new().await.unwrap();
            let grid = Grid::new(2.0, 2.0).unwrap();
            let dt_s = 1.0 / 60.0;
            let gravity_m_s2 = 9.80665;
            let density = initial_buffer(&context.device, "density", &[1.2, 1.2, 1000.0, 1000.0]);
            let root_cell = root_buffer(&context.device, &[0, 0, 0, 0]);
            let aperture_u = initial_buffer(
                &context.device,
                "u aperture",
                &[0.0, 1.0, 0.0, 0.0, 1.0, 0.0],
            );
            let aperture_v = initial_buffer(
                &context.device,
                "v aperture",
                &[0.0, 0.0, 1.0, 1.0, 0.0, 0.0],
            );
            let predictor_u = initial_buffer(&context.device, "u predictor", &[0.0; 6]);
            let predictor_v = initial_buffer(
                &context.device,
                "v predictor",
                &[0.0, 0.0, dt_s * gravity_m_s2, dt_s * gravity_m_s2, 0.0, 0.0],
            );
            let corrected_u = candidate_buffer(&context.device, "u candidate", 6);
            let corrected_v = candidate_buffer(&context.device, "v candidate", 6);
            let mut projector = GpuPressureProjector::new(&context.device, grid).unwrap();
            let diagnostics = projector
                .project_closed(
                    &context.device,
                    &context.queue,
                    PressureFields {
                        density: &density,
                        root_cell: &root_cell,
                        aperture_u: &aperture_u,
                        aperture_v: &aperture_v,
                        predictor_u: &predictor_u,
                        predictor_v: &predictor_v,
                        corrected_u: &corrected_u,
                        corrected_v: &corrected_v,
                    },
                    PressureConfig {
                        dt_s,
                        gravity_m_s2,
                        max_iterations: 0,
                        ..PressureConfig::default()
                    },
                )
                .await
                .unwrap();
            assert_eq!(diagnostics.iterations, 0);
            assert!(diagnostics.scaled_residual <= 1e-5, "{diagnostics:?}");
            assert!(diagnostics.scaled_divergence <= 1e-5, "{diagnostics:?}");
            let corrected_v = read_f32(&context.device, &context.queue, &corrected_v).await;
            assert!(corrected_v.iter().all(|velocity| velocity.abs() <= 1e-5));
            context.dispose();
        });
    }

    #[test]
    #[ignore = "requires a native compute adapter"]
    fn horizontal_hydrostatic_variation_converges_with_gravity() {
        block_on(async {
            let context = crate::GpuContext::new().await.unwrap();
            let grid = Grid::new(4.0, 3.0).unwrap();
            let width = grid.width() as usize;
            let height = grid.height() as usize;
            let dt_s = 1.0 / 60.0;
            let gravity_m_s2 = 9.80665;
            let density = [
                950.0, 1000.0, 1050.0, 1100.0, 925.0, 1025.0, 1075.0, 1125.0, 975.0, 995.0, 1055.0,
                1085.0,
            ];
            let mut aperture_u = vec![0.0_f32; grid.u_faces()];
            let mut aperture_v = vec![0.0_f32; grid.v_faces()];
            let predictor_u = vec![0.0_f32; grid.u_faces()];
            let mut predictor_v = vec![0.0_f32; grid.v_faces()];
            for y in 0..height {
                for x in 1..width {
                    aperture_u[y * (width + 1) + x] = 1.0;
                }
            }
            for y in 1..height {
                for x in 0..width {
                    let face = y * width + x;
                    aperture_v[face] = 1.0;
                    predictor_v[face] = dt_s * gravity_m_s2;
                }
            }
            let density = initial_buffer(&context.device, "density", &density);
            let root_cell = root_buffer(&context.device, &vec![0; grid.cells()]);
            let aperture_u = initial_buffer(&context.device, "u aperture", &aperture_u);
            let aperture_v = initial_buffer(&context.device, "v aperture", &aperture_v);
            let predictor_u = initial_buffer(&context.device, "u predictor", &predictor_u);
            let predictor_v = initial_buffer(&context.device, "v predictor", &predictor_v);
            let corrected_u = candidate_buffer(&context.device, "u candidate", grid.u_faces());
            let corrected_v = candidate_buffer(&context.device, "v candidate", grid.v_faces());
            let fields = PressureFields {
                density: &density,
                root_cell: &root_cell,
                aperture_u: &aperture_u,
                aperture_v: &aperture_v,
                predictor_u: &predictor_u,
                predictor_v: &predictor_v,
                corrected_u: &corrected_u,
                corrected_v: &corrected_v,
            };
            let mut projector = GpuPressureProjector::new(&context.device, grid).unwrap();
            let config = PressureConfig {
                dt_s,
                gravity_m_s2,
                ..PressureConfig::default()
            };
            let exhausted = projector
                .project_closed(
                    &context.device,
                    &context.queue,
                    fields,
                    PressureConfig {
                        max_iterations: 0,
                        ..config
                    },
                )
                .await
                .unwrap_err();
            assert!(
                exhausted
                    .to_string()
                    .contains("exhausted its bounded iterations"),
                "horizontal hydrostatic gradients need a dynamic correction: {exhausted}"
            );
            let diagnostics = projector
                .project_closed(&context.device, &context.queue, fields, config)
                .await
                .unwrap();
            eprintln!("horizontal hydrostatic GPU pressure: {diagnostics:?}");
            assert!(diagnostics.iterations > 0, "{diagnostics:?}");
            assert!(diagnostics.scaled_residual <= 1e-5, "{diagnostics:?}");
            assert!(diagnostics.scaled_divergence <= 1e-5, "{diagnostics:?}");
            let mut encoder =
                context
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("pressure test status readback"),
                    });
            let staging = stage_status_readback(&context.device, &mut encoder, &projector.status);
            context.queue.submit([encoder.finish()]);
            let status = read_status(&context.device, staging).await.unwrap();
            assert_eq!(status.invalid, 0, "{status:?}");
            assert_eq!(status.converged, 1, "{status:?}");
            let corrected_u = read_f32(&context.device, &context.queue, &corrected_u).await;
            let corrected_v = read_f32(&context.device, &context.queue, &corrected_v).await;
            assert!(corrected_u.iter().all(|face| face.is_finite()));
            assert!(corrected_v.iter().all(|face| face.is_finite()));
            context.dispose();
        });
    }

    #[test]
    #[ignore = "requires a native compute adapter"]
    fn sub_ulp_dynamic_pressure_survives_residual_and_face_correction() {
        block_on(async {
            let context = crate::GpuContext::new().await.unwrap();
            let grid = Grid::new(2.0, 1.0).unwrap();
            let dx_m = CELL_WIDTH_M as f32;
            let dt_s = 1.0 / 60.0_f32;
            let density_value = 1.2_f32;
            let base_value = 28.243_122_f32;
            let dynamic_step = 0.5e-6_f32;
            assert_eq!(base_value + dynamic_step, base_value);

            let mut params = Vec::with_capacity(PARAM_BYTES as usize);
            for value in [grid.width(), grid.height(), grid.cells() as u32, 1] {
                params.extend_from_slice(&value.to_le_bytes());
            }
            for value in [dx_m, dt_s, 1e-8, 1e-8, 0.0, 0.0, 0.0, 0.0] {
                params.extend_from_slice(&value.to_le_bytes());
            }
            let uniform = context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("split pressure test parameters"),
                    contents: &params,
                    usage: wgpu::BufferUsages::UNIFORM,
                });
            let density = initial_buffer(&context.device, "density", &[density_value; 2]);
            let aperture_u = initial_buffer(&context.device, "u aperture", &[0.0, 1.0, 0.0]);
            let aperture_v = initial_buffer(&context.device, "v aperture", &[0.0; 4]);
            let base = initial_buffer(&context.device, "hydrostatic base", &[base_value; 2]);
            let dynamic = initial_buffer(&context.device, "dynamic pressure", &[0.0, dynamic_step]);
            let edge_weight = 1.0 / density_value / (dx_m * dx_m);
            let rhs = initial_buffer(
                &context.device,
                "rhs",
                &[-edge_weight * dynamic_step, edge_weight * dynamic_step],
            );
            let residual = initial_buffer(&context.device, "recursive residual", &[0.0; 2]);
            let partials = candidate_buffer(&context.device, "true residual partial", 4);
            let predictor = initial_buffer(
                &context.device,
                "u predictor",
                &[0.0, dt_s * dynamic_step / (density_value * dx_m), 0.0],
            );
            let corrected = candidate_buffer(&context.device, "u corrected", grid.u_faces());
            let pcg_shader = shader(
                &context.device,
                "split pressure PCG regression",
                include_str!("../shaders/pressure_pcg.wgsl"),
            );
            let reduction = pipeline(&context.device, &pcg_shader, "reduce_updated");
            let reduction_bind = bind(
                &context.device,
                &reduction,
                &[
                    (0, &uniform),
                    (1, &density),
                    (2, &aperture_u),
                    (3, &aperture_v),
                    (4, &rhs),
                    (5, &dynamic),
                    (6, &residual),
                    (10, &partials),
                    (12, &base),
                ],
            );
            let correction_shader = shader(
                &context.device,
                "split pressure face regression",
                include_str!("../shaders/pressure_correct.wgsl"),
            );
            let correction = pipeline(&context.device, &correction_shader, "u");
            let correction_bind = bind(
                &context.device,
                &correction,
                &[
                    (0, &uniform),
                    (1, &density),
                    (2, &aperture_u),
                    (4, &predictor),
                    (6, &dynamic),
                    (7, &corrected),
                    (9, &base),
                ],
            );
            let mut encoder =
                context
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("split pressure arithmetic regression"),
                    });
            dispatch(&mut encoder, &reduction, &reduction_bind, 1);
            dispatch(&mut encoder, &correction, &correction_bind, 1);
            context.queue.submit([encoder.finish()]);

            let reduced = read_f32(&context.device, &context.queue, &partials).await;
            let corrected = read_f32(&context.device, &context.queue, &corrected).await;
            assert!(reduced[1] * dt_s * dt_s <= 1e-8, "{reduced:?}");
            assert!(corrected[1].abs() <= 1e-8, "{corrected:?}");
            context.dispose();
        });
    }

    #[test]
    #[ignore = "requires a native compute adapter"]
    fn mapped_prior_status_staging_cannot_poison_reused_projector() {
        block_on(async {
            let context = crate::GpuContext::new().await.unwrap();
            let grid = Grid::new(1.0, 1.0).unwrap();
            let density = initial_buffer(&context.device, "density", &[1.2]);
            let root_cell = root_buffer(&context.device, &[0]);
            let aperture_u = initial_buffer(&context.device, "u aperture", &[0.0; 2]);
            let aperture_v = initial_buffer(&context.device, "v aperture", &[0.0; 2]);
            let predictor_u = initial_buffer(&context.device, "u predictor", &[0.0; 2]);
            let predictor_v = initial_buffer(&context.device, "v predictor", &[0.0; 2]);
            let corrected_u = candidate_buffer(&context.device, "u candidate", grid.u_faces());
            let corrected_v = candidate_buffer(&context.device, "v candidate", grid.v_faces());
            let fields = PressureFields {
                density: &density,
                root_cell: &root_cell,
                aperture_u: &aperture_u,
                aperture_v: &aperture_v,
                predictor_u: &predictor_u,
                predictor_v: &predictor_v,
                corrected_u: &corrected_u,
                corrected_v: &corrected_v,
            };
            let mut projector = GpuPressureProjector::new(&context.device, grid).unwrap();

            // Native map polling completes synchronously. Keep an older status
            // staging buffer mapped to model the state left by a canceled map.
            let mut encoder =
                context
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("abandoned pressure status readback"),
                    });
            let abandoned = stage_status_readback(&context.device, &mut encoder, &projector.status);
            context.queue.submit([encoder.finish()]);
            wait_for_map(&context.device, &abandoned).await.unwrap();
            let stale_map = abandoned.get_mapped_range(..);

            for _ in 0..2 {
                let result = projector
                    .project_closed(
                        &context.device,
                        &context.queue,
                        fields,
                        PressureConfig {
                            max_iterations: 0,
                            ..PressureConfig::default()
                        },
                    )
                    .await
                    .unwrap();
                assert_eq!(result.iterations, 0);
                assert_eq!(result.scaled_residual, 0.0);
                assert_eq!(result.scaled_divergence, 0.0);
            }
            drop(stale_map);
            abandoned.unmap();
            context.dispose();
        });
    }

    #[test]
    #[ignore = "requires a native compute adapter"]
    fn manufactured_closed_gradient_converges_on_gpu_and_exhaustion_rejects() {
        block_on(async {
            let context = crate::GpuContext::new().await.unwrap();
            let grid = Grid::new(4.0, 3.0).unwrap();
            let width = grid.width() as usize;
            let height = grid.height() as usize;
            let density = vec![1.2_f32; grid.cells()];
            let mut aperture_u = vec![0.0_f32; grid.u_faces()];
            let mut aperture_v = vec![0.0_f32; grid.v_faces()];
            let mut predictor_u = vec![0.0_f32; grid.u_faces()];
            let mut predictor_v = vec![0.0_f32; grid.v_faces()];
            let known_pressure = [
                0.03, 0.11, -0.06, 0.07, -0.08, 0.02, 0.17, -0.02, 0.09, -0.13, 0.04, -0.05,
            ];
            let dt_s = 1.0 / 60.0;
            for y in 0..height {
                for x in 1..width {
                    let face = y * (width + 1) + x;
                    let left = y * width + x - 1;
                    aperture_u[face] = 1.0;
                    predictor_u[face] =
                        dt_s * (1.0 / 1.2) * (known_pressure[left + 1] - known_pressure[left])
                            / CELL_WIDTH_M as f32;
                }
            }
            for y in 1..height {
                for x in 0..width {
                    let face = y * width + x;
                    let top = (y - 1) * width + x;
                    aperture_v[face] = 1.0;
                    predictor_v[face] =
                        dt_s * (1.0 / 1.2) * (known_pressure[top + width] - known_pressure[top])
                            / CELL_WIDTH_M as f32;
                }
            }
            let density = initial_buffer(&context.device, "density", &density);
            let root_cell = root_buffer(&context.device, &vec![0; grid.cells()]);
            let aperture_u = initial_buffer(&context.device, "u aperture", &aperture_u);
            let aperture_v = initial_buffer(&context.device, "v aperture", &aperture_v);
            let predictor_u = initial_buffer(&context.device, "u predictor", &predictor_u);
            let predictor_v = initial_buffer(&context.device, "v predictor", &predictor_v);
            let corrected_u = candidate_buffer(&context.device, "u candidate", grid.u_faces());
            let corrected_v = candidate_buffer(&context.device, "v candidate", grid.v_faces());
            let fields = PressureFields {
                density: &density,
                root_cell: &root_cell,
                aperture_u: &aperture_u,
                aperture_v: &aperture_v,
                predictor_u: &predictor_u,
                predictor_v: &predictor_v,
                corrected_u: &corrected_u,
                corrected_v: &corrected_v,
            };
            let mut projector = GpuPressureProjector::new(&context.device, grid).unwrap();
            let exhausted = projector
                .project_closed(
                    &context.device,
                    &context.queue,
                    PressureFields { ..fields },
                    PressureConfig {
                        max_iterations: 0,
                        ..PressureConfig::default()
                    },
                )
                .await;
            assert!(
                exhausted.is_err(),
                "zero iterations must reject this predictor"
            );
            let diagnostics = projector
                .project_closed(
                    &context.device,
                    &context.queue,
                    fields,
                    PressureConfig::default(),
                )
                .await
                .unwrap();
            eprintln!("manufactured GPU pressure: {diagnostics:?}");
            assert!(diagnostics.iterations > 0);
            assert!(diagnostics.scaled_residual <= 1e-5, "{diagnostics:?}");
            assert!(diagnostics.scaled_divergence <= 1e-5, "{diagnostics:?}");
            let corrected_u = read_f32(&context.device, &context.queue, &corrected_u).await;
            let corrected_v = read_f32(&context.device, &context.queue, &corrected_v).await;
            let max_speed = corrected_u
                .iter()
                .chain(&corrected_v)
                .fold(0.0_f32, |maximum, speed| maximum.max(speed.abs()));
            assert!(max_speed <= 2e-5, "max corrected speed {max_speed}");
            context.dispose();
        });
    }

    #[test]
    #[ignore = "requires a native compute adapter"]
    fn disconnected_closed_components_project_without_cross_wall_flux() {
        block_on(async {
            let context = crate::GpuContext::new().await.unwrap();
            let grid = Grid::new(5.0, 1.0).unwrap();
            let density = initial_buffer(
                &context.device,
                "density",
                &[1.2, 1.2, 1000.0, 1000.0, 1000.0],
            );
            let root_cell = root_buffer(&context.device, &[0, 0, 2, 3, 3]);
            let aperture_u = initial_buffer(
                &context.device,
                "u aperture",
                &[0.0, 0.7, 0.0, 0.0, 1.0, 0.0],
            );
            let aperture_v = initial_buffer(&context.device, "v aperture", &[0.0; 10]);
            let predictor_u = initial_buffer(
                &context.device,
                "u predictor",
                &[0.0, 0.1, 0.0, 0.0, -0.2, 0.0],
            );
            let predictor_v = initial_buffer(&context.device, "v predictor", &[0.0; 10]);
            let corrected_u = candidate_buffer(&context.device, "u candidate", grid.u_faces());
            let corrected_v = candidate_buffer(&context.device, "v candidate", grid.v_faces());
            let mut projector = GpuPressureProjector::new(&context.device, grid).unwrap();
            let diagnostics = projector
                .project_closed(
                    &context.device,
                    &context.queue,
                    PressureFields {
                        density: &density,
                        root_cell: &root_cell,
                        aperture_u: &aperture_u,
                        aperture_v: &aperture_v,
                        predictor_u: &predictor_u,
                        predictor_v: &predictor_v,
                        corrected_u: &corrected_u,
                        corrected_v: &corrected_v,
                    },
                    PressureConfig::default(),
                )
                .await
                .unwrap();
            eprintln!("disconnected GPU pressure: {diagnostics:?}");
            assert!(diagnostics.scaled_residual <= 1e-5, "{diagnostics:?}");
            assert!(diagnostics.scaled_divergence <= 1e-5, "{diagnostics:?}");
            let u_values = read_f32(&context.device, &context.queue, &corrected_u).await;
            let v_values = read_f32(&context.device, &context.queue, &corrected_v).await;
            assert!(u_values[1].abs() <= 1e-5);
            assert!(u_values[4].abs() <= 1e-5);
            assert_eq!(u_values[2], 0.0);
            assert_eq!(u_values[3], 0.0);
            assert!(v_values.iter().all(|value| *value == 0.0));

            let next_u = candidate_buffer(&context.device, "next u candidate", grid.u_faces());
            let next_v = candidate_buffer(&context.device, "next v candidate", grid.v_faces());
            for step in 0..8 {
                let (source_u, source_v, target_u, target_v) = if step % 2 == 0 {
                    (&corrected_u, &corrected_v, &next_u, &next_v)
                } else {
                    (&next_u, &next_v, &corrected_u, &corrected_v)
                };
                let repeated = projector
                    .project_closed(
                        &context.device,
                        &context.queue,
                        PressureFields {
                            density: &density,
                            root_cell: &root_cell,
                            aperture_u: &aperture_u,
                            aperture_v: &aperture_v,
                            predictor_u: source_u,
                            predictor_v: source_v,
                            corrected_u: target_u,
                            corrected_v: target_v,
                        },
                        PressureConfig::default(),
                    )
                    .await
                    .unwrap();
                assert!(repeated.scaled_residual <= 1e-5, "{repeated:?}");
                assert!(repeated.scaled_divergence <= 1e-5, "{repeated:?}");
            }
            let repeated_u = read_f32(&context.device, &context.queue, &corrected_u).await;
            assert!(repeated_u.iter().all(|face| face.abs() <= 1e-5));

            let still_u = initial_buffer(&context.device, "zero u predictor", &[0.0; 6]);
            let still_v = initial_buffer(&context.device, "zero v predictor", &[0.0; 10]);
            let zero = projector
                .project_closed(
                    &context.device,
                    &context.queue,
                    PressureFields {
                        density: &density,
                        root_cell: &root_cell,
                        aperture_u: &aperture_u,
                        aperture_v: &aperture_v,
                        predictor_u: &still_u,
                        predictor_v: &still_v,
                        corrected_u: &corrected_u,
                        corrected_v: &corrected_v,
                    },
                    PressureConfig {
                        max_iterations: 0,
                        ..PressureConfig::default()
                    },
                )
                .await
                .unwrap();
            assert_eq!(zero.iterations, 0);
            assert_eq!(zero.scaled_residual, 0.0);
            assert_eq!(zero.scaled_divergence, 0.0);
            context.dispose();
        });
    }

    #[test]
    #[ignore = "requires a native compute adapter"]
    fn nonfinite_interior_predictor_is_rejected_by_gpu_reduction() {
        block_on(async {
            let context = crate::GpuContext::new().await.unwrap();
            let grid = Grid::new(2.0, 1.0).unwrap();
            let density = initial_buffer(&context.device, "density", &[1.2; 2]);
            let root_cell = root_buffer(&context.device, &[0, 0]);
            let aperture_u = initial_buffer(&context.device, "u aperture", &[0.0, 1.0, 0.0]);
            let aperture_v = initial_buffer(&context.device, "v aperture", &[0.0; 4]);
            let predictor_u = initial_buffer(&context.device, "u predictor", &[0.0, f32::NAN, 0.0]);
            let predictor_v = initial_buffer(&context.device, "v predictor", &[0.0; 4]);
            let corrected_u = candidate_buffer(&context.device, "u candidate", grid.u_faces());
            let corrected_v = candidate_buffer(&context.device, "v candidate", grid.v_faces());
            let mut projector = GpuPressureProjector::new(&context.device, grid).unwrap();
            let result = projector
                .project_closed(
                    &context.device,
                    &context.queue,
                    PressureFields {
                        density: &density,
                        root_cell: &root_cell,
                        aperture_u: &aperture_u,
                        aperture_v: &aperture_v,
                        predictor_u: &predictor_u,
                        predictor_v: &predictor_v,
                        corrected_u: &corrected_u,
                        corrected_v: &corrected_v,
                    },
                    PressureConfig::default(),
                )
                .await;
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("arithmetic rejected the initial fields")
            );
            context.dispose();
        });
    }

    #[test]
    #[ignore = "requires a native compute adapter"]
    fn smooth_480x270_closed_predictor_meets_both_gates() {
        block_on(async {
            let context = crate::GpuContext::new().await.unwrap();
            let grid = Grid::new(480.0, 270.0).unwrap();
            let width = grid.width() as usize;
            let height = grid.height() as usize;
            let density = vec![1.2_f32; grid.cells()];
            let mut aperture_u = vec![0.0_f32; grid.u_faces()];
            let mut aperture_v = vec![0.0_f32; grid.v_faces()];
            let mut predictor_u = vec![0.0_f32; grid.u_faces()];
            for y in 0..height {
                for x in 1..width {
                    let face = y * (width + 1) + x;
                    aperture_u[face] = 1.0;
                    predictor_u[face] =
                        0.1 * (std::f32::consts::PI * x as f32 / width as f32).sin();
                }
            }
            for y in 1..height {
                for x in 0..width {
                    aperture_v[y * width + x] = 1.0;
                }
            }
            let density = initial_buffer(&context.device, "density", &density);
            let root_cell = root_buffer(&context.device, &vec![0; grid.cells()]);
            let aperture_u = initial_buffer(&context.device, "u aperture", &aperture_u);
            let aperture_v = initial_buffer(&context.device, "v aperture", &aperture_v);
            let predictor_u = initial_buffer(&context.device, "u predictor", &predictor_u);
            let predictor_v =
                initial_buffer(&context.device, "v predictor", &vec![0.0; grid.v_faces()]);
            let corrected_u = candidate_buffer(&context.device, "u candidate", grid.u_faces());
            let corrected_v = candidate_buffer(&context.device, "v candidate", grid.v_faces());
            let mut projector = GpuPressureProjector::new(&context.device, grid).unwrap();
            let start = Instant::now();
            let result = projector
                .project_closed(
                    &context.device,
                    &context.queue,
                    PressureFields {
                        density: &density,
                        root_cell: &root_cell,
                        aperture_u: &aperture_u,
                        aperture_v: &aperture_v,
                        predictor_u: &predictor_u,
                        predictor_v: &predictor_v,
                        corrected_u: &corrected_u,
                        corrected_v: &corrected_v,
                    },
                    PressureConfig {
                        max_iterations: 1024,
                        ..PressureConfig::default()
                    },
                )
                .await;
            eprintln!(
                "480x270 sealed GPU pressure: {result:?}, elapsed {:?}",
                start.elapsed()
            );
            let diagnostics = result.unwrap();
            assert!(diagnostics.iterations <= 1024, "{diagnostics:?}");
            assert!(diagnostics.scaled_residual <= 1e-5, "{diagnostics:?}");
            assert!(diagnostics.scaled_divergence <= 1e-5, "{diagnostics:?}");
            context.dispose();
        });
    }
}
