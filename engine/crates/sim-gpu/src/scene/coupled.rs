//! Candidate-to-commit GPU graph for the limited sealed two-phase scene.
//!
//! Every stage writes detached buffers. The five small stage status words are
//! checked together before pressure projection. Pressure independently checks
//! its true residual and corrected divergence. Only then does this owner copy
//! the candidate inventory into the alternate generation and accept time.

use std::{
    future::poll_fn,
    sync::{Arc, Mutex},
    task::{Poll, Waker},
};

use particle_sim::{Grid, OUTER_DT_S};
use wgpu::util::DeviceExt;

use super::{GpuPaintMaterial, GpuScene, GpuSceneSeed, paint::GpuPaintStage};
use crate::{
    GpuContext,
    cfl::{GpuCflSelection, GpuCflStage},
    checkpoint::{self, CheckpointSource, CheckpointStamp},
    density::{self, GpuDensityDeriver, GpuDerivedDensityCandidate},
    gravity::{GpuGravityCandidate, GpuGravityInput, GpuGravityStage, GpuGravityStep},
    momentum::{self, GpuMomentumCandidate, GpuMomentumInput, GpuMomentumStage},
    pressure::{
        GpuPressureProjector, PressureConfig, PressureDiagnostics, PressureError, PressureFields,
    },
    probe::{GpuCellProbe, ProbeSample, ProbeSource, ProbeStamp},
    render::{RenderScene, RenderViewport, SceneRenderer},
    transport::{
        self, GpuTransportCandidate, GpuTransportInput, GpuTransportStage, GpuTransportStep,
    },
    viscosity::{GpuViscosityCandidate, GpuViscosityInput, GpuViscosityStage, GpuViscosityStep},
};

const STAGE_COUNT: usize = 5;
const STAGE_STATUS_BYTES: u64 = (STAGE_COUNT * size_of::<u32>()) as u64;
// A candidate can make one initial attempt and at most four smaller attempts.
// Rejected attempts never consume the accepted-substep budget or model time.
const MAX_CANDIDATE_REFINEMENTS: u32 = 4;
// density_derive.wgsl accepts this relative deficit in cell volume closure.
// A valid nearly pure carrier cell can therefore have density below the
// nominal carrier phase density after f32 transport.
const ACCEPTED_VOLUME_DEFICIT_REL: f32 = 1e-5;

fn conservative_minimum_density(phase_density_kg_m3: [f32; 2]) -> f32 {
    phase_density_kg_m3[0].min(phase_density_kg_m3[1]) * (1.0 - ACCEPTED_VOLUME_DEFICIT_REL)
}

/// A complete outer tick or an explicit work-budget pause after whole accepted
/// substeps. Neither result publishes a partially accepted substep.
#[derive(Clone, Copy, Debug)]
pub(crate) enum GpuTickOutcome {
    Complete {
        accepted_substeps: u32,
        attempted_candidates: u32,
        refinement_retries: u32,
        last_pressure: PressureDiagnostics,
        /// Compact GPU-to-host completion bytes for accepted substeps.
        readback_bytes: u64,
        /// Additional compact completion bytes from rejected candidates.
        /// The CFL observation is shared with the accepted candidate.
        rejected_readback_bytes: u64,
        /// Difference between nominal accepted time and summed f32 shader dt.
        encoded_time_error_s: f64,
    },
    Paused {
        accepted_substeps: u32,
        attempted_candidates: u32,
        refinement_retries: u32,
        remaining_s: f64,
        readback_bytes: u64,
        rejected_readback_bytes: u64,
    },
}

/// Accepted model position and selected generation, never candidate progress.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GpuSceneProgress {
    pub(crate) epoch: u64,
    pub(crate) tick: u64,
    pub(crate) accepted_time_s: f64,
    pub(crate) remaining_outer_s: f64,
    pub(crate) committed_generation: usize,
    pub(crate) state_revision: u64,
}

/// Compact E07 browser qualification result. It does not expose live fields.
#[derive(Clone, Debug)]
pub struct GpuSceneValidationReport {
    /// Backend selected for the browser or native fixture.
    pub backend: String,
    /// Scene generation used by the accepted fixture.
    pub epoch: u64,
    /// Tick after one accepted outer step.
    pub tick: u64,
    /// Nominal model time after the accepted step, in seconds.
    pub accepted_time_s: f64,
    /// Number of accepted bounded substeps in that outer tick.
    pub accepted_substeps: u32,
    /// True scaled pressure residual after projection.
    pub scaled_residual: f32,
    /// True scaled divergence after projection.
    pub scaled_divergence: f32,
    /// Difference between nominal time and summed shader step times.
    pub encoded_time_error_s: f64,
    /// Whether a failed projection left committed state unchanged.
    pub rejection_kept_committed_state: bool,
}

fn e07_uniform_air_seed() -> GpuSceneSeed {
    let grid = Grid::new(2.0, 2.0).expect("fixed E07 fixture grid");
    let volume = grid.cell_volume_m3();
    GpuSceneSeed {
        grid,
        phase_density_kg_m3: [1000.0, 1.0],
        mass_kg: [vec![0.0; grid.cells()], vec![volume; grid.cells()]].concat(),
        marker: vec![0.0; grid.cells() * 2],
        energy_j: vec![0.0; grid.cells()],
        fixed_wall: vec![0; grid.cells()],
        u_velocity_m_s: vec![0.0; grid.u_faces()],
        v_velocity_m_s: vec![0.0; grid.v_faces()],
        u_aperture: vec![0.0, 1.0, 0.0, 0.0, 1.0, 0.0],
        v_aperture: vec![0.0, 0.0, 1.0, 1.0, 0.0, 0.0],
        epoch: 2,
        tick: 7,
        accepted_time_s: 7.0 / 60.0,
    }
}

fn e07_mixed_vortex_seed() -> GpuSceneSeed {
    let mut seed = e07_uniform_air_seed();
    seed.phase_density_kg_m3[1] = 1.2;
    let volume = seed.grid.cell_volume_m3();
    let fractions = [1.0, 0.0, 0.25, 0.75];
    seed.mass_kg = fractions
        .iter()
        .map(|&alpha| alpha * 1000.0 * volume)
        .chain(fractions.iter().map(|&alpha| (1.0 - alpha) * 1.2 * volume))
        .collect();
    seed.marker = fractions
        .iter()
        .map(|&alpha| 0.4 * alpha)
        .chain(fractions.iter().map(|&alpha| 0.8 * (1.0 - alpha)))
        .collect();
    seed.u_velocity_m_s[1] = 0.01;
    seed.u_velocity_m_s[4] = -0.01;
    seed.v_velocity_m_s[2] = -0.01;
    seed.v_velocity_m_s[3] = 0.01;
    seed
}

/// Runs one actual coupled GPU step and one rejected candidate on the same
/// browser or native device. Full committed-state downloads are limited to
/// this qualification fixture; normal steps read compact reductions only.
pub(crate) async fn validate_scene_fixture(
    context: &GpuContext,
) -> Result<GpuSceneValidationReport, String> {
    let mut working = GpuCoupledScene::new(
        context,
        e07_mixed_vortex_seed(),
        GpuScenePhysics {
            cell_dynamic_viscosity_pa_s: Some(vec![0.001; 4]),
            gravity_m_s2: 0.0,
            pressure: PressureConfig::default(),
        },
    )?;
    let GpuTickOutcome::Complete {
        accepted_substeps,
        last_pressure,
        encoded_time_error_s,
        ..
    } = working.advance_outer_tick(8).await?
    else {
        return Err("E07 browser fixture paused before completing one GPU tick".into());
    };
    // The accepted step switched generations. A rejected candidate now uses
    // the other generation, so this checks the actual committed GPU buffers.
    let before = (
        working.scene.epoch,
        working.scene.committed_index,
        working.scene.tick,
        working.scene.accepted_time_s,
        working.encoded_time_s,
        working.remaining_outer_s,
    );
    let committed_before = read_fixture_committed_state(&working.scene).await?;
    working.gravity_m_s2 = 9.80665;
    working.pressure_config.max_iterations = 0;
    let rejected = working
        .advance_outer_tick(8)
        .await
        .is_err_and(|error| error.contains("exhausted its bounded iterations"));
    let committed_after = read_fixture_committed_state(&working.scene).await?;
    let rejection_kept_committed_state = rejected
        && before
            == (
                working.scene.epoch,
                working.scene.committed_index,
                working.scene.tick,
                working.scene.accepted_time_s,
                working.encoded_time_s,
                working.remaining_outer_s,
            )
        && committed_before == committed_after;
    if !rejection_kept_committed_state {
        return Err("E07 rejected GPU candidate changed committed scene state".into());
    }
    Ok(GpuSceneValidationReport {
        backend: context.backend(),
        epoch: working.scene.epoch,
        tick: working.scene.tick,
        accepted_time_s: working.scene.accepted_time_s,
        accepted_substeps,
        scaled_residual: last_pressure.scaled_residual,
        scaled_divergence: last_pressure.scaled_divergence,
        encoded_time_error_s,
        rejection_kept_committed_state,
    })
}

/// Fixed coefficients for the limited nonreactive scene. A None viscosity
/// omits shear, while a provided array has one positive value per cell.
pub(crate) struct GpuScenePhysics {
    pub(crate) cell_dynamic_viscosity_pa_s: Option<Vec<f32>>,
    pub(crate) gravity_m_s2: f32,
    pub(crate) pressure: PressureConfig,
}

/// Owns exactly one committed scene plus reusable stage pipelines and scratch.
/// This is private to the experimental GPU path until browser E08 exports it.
pub(crate) struct GpuCoupledScene {
    scene: GpuScene,
    paint_stage: GpuPaintStage,
    cfl: GpuCflStage,
    transport: GpuTransportStage,
    momentum: GpuMomentumStage,
    density: GpuDensityDeriver,
    viscosity: Option<GpuViscosityStage>,
    viscosity_field: Option<wgpu::Buffer>,
    max_viscosity_pa_s: f32,
    gravity_m_s2: f32,
    gravity: GpuGravityStage,
    pressure_config: PressureConfig,
    pressure: GpuPressureProjector,
    probe: GpuCellProbe,
    remaining_outer_s: f64,
    encoded_time_s: f64,
    #[cfg(test)]
    test_fault: Option<TestFaultPoint>,
    #[cfg(test)]
    test_numerical_rejections: u32,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TestFaultPoint {
    Stage(usize),
    Pressure,
}

/// Numerical gates can request a smaller detached attempt. Invalid inputs,
/// arithmetic failures, mapping failures and injected faults stop immediately.
#[derive(Debug)]
enum CandidateRejection {
    Numerical {
        message: String,
        readback_bytes: u64,
    },
    Terminal(String),
}

impl From<String> for CandidateRejection {
    fn from(message: String) -> Self {
        Self::Terminal(message)
    }
}

impl From<&str> for CandidateRejection {
    fn from(message: &str) -> Self {
        Self::Terminal(message.into())
    }
}

fn stage_rejection(slot: usize, name: &str, status: u32) -> CandidateRejection {
    let retryable_bits = match slot {
        0 => {
            transport::STATUS_DONOR_OVERDRAW
                | transport::STATUS_VOLUME_CLOSURE
                | transport::STATUS_CORRECTION_LIMIT
        }
        1 => momentum::STATUS_DONOR_OVERDRAW,
        2 => density::STATUS_VOLUME_CLOSURE,
        _ => 0,
    };
    let message = format!("GPU {name} candidate rejected with status {status}");
    if status != 0 && status & !retryable_bits == 0 {
        CandidateRejection::Numerical {
            message,
            readback_bytes: STAGE_STATUS_BYTES,
        }
    } else {
        CandidateRejection::Terminal(message)
    }
}

impl GpuCoupledScene {
    /// Returns this scene's grid for a viewport bound to its committed state.
    pub(crate) fn grid(&self) -> Grid {
        self.scene.grid
    }

    /// Reports only committed scene progress. An in-flight candidate does not
    /// alter these values until its pressure and stage gates succeed.
    pub(crate) fn progress(&self) -> GpuSceneProgress {
        GpuSceneProgress {
            epoch: self.scene.epoch,
            tick: self.scene.tick,
            accepted_time_s: self.scene.accepted_time_s,
            remaining_outer_s: self.remaining_outer_s,
            committed_generation: self.scene.committed_index,
            state_revision: self.scene.state_revision,
        }
    }

    fn probe_stamp(&self) -> ProbeStamp {
        ProbeStamp {
            epoch: self.scene.epoch,
            tick: self.scene.tick,
            accepted_time_s: self.scene.accepted_time_s,
            committed_generation: self.scene.committed_index,
            state_revision: self.scene.state_revision,
        }
    }

    fn checkpoint_stamp(&self) -> CheckpointStamp {
        CheckpointStamp {
            epoch: self.scene.epoch,
            tick: self.scene.tick,
            accepted_time_s: self.scene.accepted_time_s,
            remaining_outer_s: self.remaining_outer_s,
            generation: self.scene.committed_index as u32,
            state_revision: self.scene.state_revision,
        }
    }

    /// Reads one committed cell and rejects a result made stale by a later edit.
    pub(crate) async fn probe_cell(&self, x: u32, y: u32) -> Result<ProbeSample, String> {
        let context = &self.scene.context;
        let pending = self.probe.submit(
            &context.device,
            &context.queue,
            ProbeSource {
                grid: self.scene.grid,
                headers: &self.scene.headers,
                committed_mass_kg: &self.scene.committed().mass_kg,
                stamp: self.probe_stamp(),
            },
            [x, y],
        )?;
        let sample = pending.resolve(&context.device).await?;
        if !sample.is_current(self.probe_stamp()) {
            return Err("GPU probe result is stale".into());
        }
        Ok(sample)
    }

    /// Captures one detached generation only when explicitly requested.
    /// Versioned recovery and load remain separate E13 work.
    pub(crate) async fn checkpoint_prototype(&self) -> Result<Vec<u8>, String> {
        let context = &self.scene.context;
        let committed = self.scene.committed();
        let capture = checkpoint::capture(
            &context.device,
            &context.queue,
            CheckpointSource {
                grid: self.scene.grid,
                stamp: self.checkpoint_stamp(),
                fields: [
                    &committed.mass_kg,
                    &committed.marker,
                    &committed.density_kg_m3,
                    &committed.u_velocity_m_s,
                    &committed.v_velocity_m_s,
                ],
            },
        )
        .await?;
        if !capture.is_current(self.checkpoint_stamp()) {
            return Err("GPU checkpoint capture is stale".into());
        }
        Ok(capture.into_prototype_bytes())
    }

    /// Applies a bounded control edit to detached resident state at zero model time.
    pub(crate) fn paint(
        &mut self,
        center_x: u32,
        center_y: u32,
        radius: u32,
        material: GpuPaintMaterial,
    ) -> Result<(), String> {
        self.paint_stage
            .apply(&mut self.scene, center_x, center_y, radius, material)
    }

    /// Draws the selected committed generation on the scene's own device.
    /// A failed candidate never reaches this renderer. No simulation field is
    /// mapped or copied to the host for a normal frame.
    pub(crate) fn render_committed(
        &self,
        renderer: &SceneRenderer,
        target: &wgpu::TextureView,
        target_size_px: [u32; 2],
        viewport: RenderViewport,
    ) -> Result<(), String> {
        let context = &self.scene.context;
        let mut encoder = context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("committed scene display"),
            });
        renderer.encode(
            &context.device,
            &mut encoder,
            target,
            target_size_px,
            viewport,
            RenderScene {
                grid: self.scene.grid,
                phase_density_kg_m3: self.scene.phase_density_kg_m3,
                headers: &self.scene.headers,
                committed_mass_kg: &self.scene.committed().mass_kg,
            },
        )?;
        context.queue.submit([encoder.finish()]);
        Ok(())
    }

    /// Preflights and constructs one private GPU scene with static wall geometry.
    pub(crate) fn new(
        context: &GpuContext,
        seed: GpuSceneSeed,
        physics: GpuScenePhysics,
    ) -> Result<Self, String> {
        let GpuScenePhysics {
            cell_dynamic_viscosity_pa_s,
            gravity_m_s2,
            pressure,
        } = physics;
        if !gravity_m_s2.is_finite() {
            return Err("GPU scene gravity must be finite".into());
        }
        if !pressure.scaled_residual_tolerance.is_finite()
            || pressure.scaled_residual_tolerance <= 0.0
            || !pressure.scaled_divergence_tolerance.is_finite()
            || pressure.scaled_divergence_tolerance <= 0.0
        {
            return Err("GPU scene pressure tolerances must be positive and finite".into());
        }
        if let Some(viscosity) = &cell_dynamic_viscosity_pa_s
            && (viscosity.len() != seed.grid.cells()
                || viscosity
                    .iter()
                    .any(|value| !value.is_finite() || *value <= 0.0))
        {
            return Err("GPU scene viscosity array is invalid".into());
        }
        let grid = seed.grid;
        let scene = GpuScene::new(context, seed)?;
        let paint_stage = GpuPaintStage::new(&context.device, grid)?;
        let cfl = GpuCflStage::new(&context.device, grid)?;
        let transport = GpuTransportStage::new(&context.device, grid)?;
        let momentum = GpuMomentumStage::new(&context.device, grid)?;
        let density = GpuDensityDeriver::new(&context.device, grid)?;
        let (viscosity, viscosity_field, max_viscosity_pa_s) =
            if let Some(values) = cell_dynamic_viscosity_pa_s {
                let maximum = values.iter().copied().fold(0.0_f32, f32::max);
                let field = context
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("scene fixed dynamic viscosity"),
                        contents: &super::f32_bytes(&values),
                        usage: wgpu::BufferUsages::STORAGE,
                    });
                (
                    Some(GpuViscosityStage::new(&context.device, grid)?),
                    Some(field),
                    maximum,
                )
            } else {
                (None, None, 0.0)
            };
        let gravity = GpuGravityStage::new(&context.device, grid)?;
        let pressure_stage = GpuPressureProjector::new(&context.device, grid)?;
        let probe = GpuCellProbe::new(&context.device)?;
        if STAGE_STATUS_BYTES > context.device.limits().max_buffer_size {
            return Err("GPU scene stage status staging exceeds adapter limit".into());
        }
        let encoded_time_s = scene.accepted_time_s;
        Ok(Self {
            scene,
            paint_stage,
            cfl,
            transport,
            momentum,
            density,
            viscosity,
            viscosity_field,
            max_viscosity_pa_s,
            gravity_m_s2,
            gravity,
            pressure_config: pressure,
            pressure: pressure_stage,
            probe,
            remaining_outer_s: 0.0,
            encoded_time_s,
            #[cfg(test)]
            test_fault: None,
            #[cfg(test)]
            test_numerical_rejections: 0,
        })
    }

    /// Accepts up to `max_substeps` detached candidates. Each candidate permits
    /// at most four smaller retries for numerical rejection. A budget pause
    /// retains accepted substeps and the remaining portion of the tick.
    pub(crate) async fn advance_outer_tick(
        &mut self,
        max_substeps: u32,
    ) -> Result<GpuTickOutcome, String> {
        if max_substeps == 0 {
            return Err("GPU outer tick needs a positive substep budget".into());
        }
        if self.scene.tick == u64::MAX {
            return Err("GPU scene tick counter exhausted".into());
        }
        let mut remaining_outer_s = if self.remaining_outer_s == 0.0 {
            OUTER_DT_S
        } else {
            self.remaining_outer_s
        };
        let mut readback_bytes = 0_u64;
        let mut rejected_readback_bytes = 0_u64;
        let mut attempted_candidates = 0_u32;
        let mut refinement_retries = 0_u32;
        for accepted_substeps in 0..max_substeps {
            let measurement = self
                .cfl
                .measure(
                    &self.scene.context.device,
                    &self.scene.context.queue,
                    &self.scene.committed().u_velocity_m_s,
                    &self.scene.committed().v_velocity_m_s,
                )
                .await?;
            let minimum_density = conservative_minimum_density(self.scene.phase_density_kg_m3);
            let max_nu = if self.max_viscosity_pa_s == 0.0 {
                0.0
            } else {
                f64::from(self.max_viscosity_pa_s) / f64::from(minimum_density)
            };
            let mut selected = measurement.select_duration(remaining_outer_s, max_nu)?;
            let (mut nominal_dt_s, mut encoded_dt_s) = select_clock_durations(
                remaining_outer_s,
                selected,
                self.scene.accepted_time_s - self.encoded_time_s,
            )?;
            let mut candidate_refinements = 0_u32;
            let (pressure, next_remaining_s, next_encoded_time) = loop {
                let next_remaining_s = remaining_outer_s - nominal_dt_s;
                if next_remaining_s < 0.0 {
                    return Err("GPU scene selected more than remaining outer time".into());
                }
                let next_encoded_time = self.encoded_time_s + f64::from(encoded_dt_s);
                if !next_encoded_time.is_finite() || next_encoded_time <= self.encoded_time_s {
                    return Err("GPU encoded time cannot advance".into());
                }
                attempted_candidates = attempted_candidates
                    .checked_add(1)
                    .ok_or("GPU candidate attempt counter exhausted")?;
                let mut pressure_config = self.pressure_config;
                pressure_config.dt_s = encoded_dt_s;
                pressure_config.gravity_m_s2 = self.gravity_m_s2;
                match self
                    .advance_candidate(
                        encoded_dt_s,
                        nominal_dt_s,
                        self.gravity_m_s2,
                        pressure_config,
                    )
                    .await
                {
                    Ok(pressure) => break (pressure, next_remaining_s, next_encoded_time),
                    Err(CandidateRejection::Terminal(message)) => return Err(message),
                    Err(CandidateRejection::Numerical {
                        message,
                        readback_bytes: rejected_bytes,
                    }) => {
                        rejected_readback_bytes += rejected_bytes;
                        if candidate_refinements == MAX_CANDIDATE_REFINEMENTS {
                            return Err(format!(
                                "GPU candidate refinement exhausted after {} attempts at dt {encoded_dt_s}: {message}",
                                candidate_refinements + 1,
                            ));
                        }
                        selected = refined_selection(selected, nominal_dt_s)?;
                        (nominal_dt_s, encoded_dt_s) = select_clock_durations(
                            remaining_outer_s,
                            selected,
                            self.scene.accepted_time_s - self.encoded_time_s,
                        )?;
                        candidate_refinements += 1;
                        refinement_retries = refinement_retries
                            .checked_add(1)
                            .ok_or("GPU refinement counter exhausted")?;
                    }
                }
            };
            // Each accepted candidate maps an 8-byte CFL result, a 20-byte
            // stage status and the pressure solver's counted 32-byte records.
            readback_bytes += 8 + STAGE_STATUS_BYTES + pressure.readback_bytes;
            self.encoded_time_s = next_encoded_time;
            remaining_outer_s = next_remaining_s;
            self.remaining_outer_s = remaining_outer_s;
            if remaining_outer_s == 0.0 {
                self.scene.tick += 1;
                return Ok(GpuTickOutcome::Complete {
                    accepted_substeps: accepted_substeps + 1,
                    attempted_candidates,
                    refinement_retries,
                    last_pressure: pressure,
                    readback_bytes,
                    rejected_readback_bytes,
                    encoded_time_error_s: self.scene.accepted_time_s - self.encoded_time_s,
                });
            }
        }
        Ok(GpuTickOutcome::Paused {
            accepted_substeps: max_substeps,
            attempted_candidates,
            refinement_retries,
            remaining_s: remaining_outer_s,
            readback_bytes,
            rejected_readback_bytes,
        })
    }

    /// Advances one f32 substep. Any failure preserves the committed generation
    /// and accepted time for this substep, including pressure nonconvergence.
    async fn advance_candidate(
        &mut self,
        dt_s: f32,
        nominal_dt_s: f64,
        gravity_m_s2: f32,
        pressure_config: PressureConfig,
    ) -> Result<PressureDiagnostics, CandidateRejection> {
        let next_time = self.scene.accepted_time_s + nominal_dt_s;
        if !next_time.is_finite() || next_time <= self.scene.accepted_time_s {
            return Err("GPU accepted time cannot advance".into());
        }
        let next_revision = self
            .scene
            .state_revision
            .checked_add(1)
            .ok_or("GPU scene state revision exhausted")?;
        let device = &self.scene.context.device;
        let queue = &self.scene.context.queue;
        let grid = self.scene.grid;
        let committed = self.scene.committed();
        let candidate = self.scene.candidate();
        let transport = GpuTransportCandidate::new(device, grid)?;
        let momentum = GpuMomentumCandidate::new(device, grid)?;
        let density = GpuDerivedDensityCandidate::new(device, grid)?;
        let viscosity = if self.viscosity.is_some() {
            Some(GpuViscosityCandidate::new(device, grid)?)
        } else {
            None
        };
        let gravity = GpuGravityCandidate::new(device, grid)?;
        // This staging buffer is unique to the candidate. Dropping an async
        // step during map_async cannot poison a later candidate's readback.
        let stage_status_staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene stage status staging"),
            size: STAGE_STATUS_BYTES,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("sealed GPU candidate stages"),
        });
        self.transport.encode(
            device,
            &mut encoder,
            &GpuTransportInput {
                mass_kg: &committed.mass_kg,
                marker: &committed.marker,
                headers: &self.scene.headers,
                u_velocity_m_s: &committed.u_velocity_m_s,
                v_velocity_m_s: &committed.v_velocity_m_s,
                u_aperture: &self.scene.u_aperture,
                v_aperture: &self.scene.v_aperture,
            },
            &transport,
            GpuTransportStep {
                dt_s,
                liquid_density_kg_m3: self.scene.phase_density_kg_m3[0],
                carrier_density_kg_m3: self.scene.phase_density_kg_m3[1],
            },
        )?;
        self.momentum.encode(
            device,
            &mut encoder,
            &GpuMomentumInput {
                source_mass_kg: &committed.mass_kg,
                after_x_mass_kg: &transport.after_x_mass_kg,
                final_mass_kg: &transport.mass_kg,
                source_u_velocity_m_s: &committed.u_velocity_m_s,
                source_v_velocity_m_s: &committed.v_velocity_m_s,
                u_aperture: &self.scene.u_aperture,
                v_aperture: &self.scene.v_aperture,
                u_flux: &transport.u_flux,
                v_flux: &transport.v_flux,
            },
            &momentum,
        )?;
        self.density.encode(
            device,
            &mut encoder,
            &transport.mass_kg,
            &self.scene.headers,
            &density,
            self.scene.phase_density_kg_m3,
        )?;
        let (after_shear_u, after_shear_v) = if let (Some(stage), Some(field), Some(result)) =
            (&self.viscosity, &self.viscosity_field, &viscosity)
        {
            stage.encode(
                device,
                &mut encoder,
                &GpuViscosityInput {
                    density_kg_m3: &density.density_kg_m3,
                    cell_dynamic_viscosity_pa_s: field,
                    u_velocity_m_s: &momentum.u_velocity_m_s,
                    v_velocity_m_s: &momentum.v_velocity_m_s,
                    u_aperture: &self.scene.u_aperture,
                    v_aperture: &self.scene.v_aperture,
                },
                result,
                GpuViscosityStep {
                    dt_s,
                    minimum_density_kg_m3: conservative_minimum_density(
                        self.scene.phase_density_kg_m3,
                    ),
                    maximum_dynamic_viscosity_pa_s: self.max_viscosity_pa_s,
                },
            )?;
            (&result.u_velocity_m_s, &result.v_velocity_m_s)
        } else {
            (&momentum.u_velocity_m_s, &momentum.v_velocity_m_s)
        };
        self.gravity.encode(
            device,
            &mut encoder,
            &GpuGravityInput {
                u_velocity_m_s: after_shear_u,
                v_velocity_m_s: after_shear_v,
                u_aperture: &self.scene.u_aperture,
                v_aperture: &self.scene.v_aperture,
            },
            &gravity,
            GpuGravityStep { dt_s, gravity_m_s2 },
        )?;
        for (slot, status) in [
            Some(&transport.status),
            Some(&momentum.status),
            Some(&density.status),
            viscosity.as_ref().map(|value| &value.status),
            Some(&gravity.status),
        ]
        .into_iter()
        .enumerate()
        {
            let offset = (slot * size_of::<u32>()) as u64;
            if let Some(status) = status {
                encoder.copy_buffer_to_buffer(status, 0, &stage_status_staging, offset, 4);
            } else {
                encoder.clear_buffer(&stage_status_staging, offset, Some(4));
            }
        }
        queue.submit([encoder.finish()]);
        let statuses = read_stage_statuses(device, &stage_status_staging).await?;
        for (slot, (name, status)) in ["transport", "momentum", "density", "viscosity", "gravity"]
            .into_iter()
            .zip(statuses)
            .enumerate()
        {
            if status != 0 {
                return Err(stage_rejection(slot, name, status));
            }
            #[cfg(test)]
            if self.test_fault == Some(TestFaultPoint::Stage(slot)) {
                return Err(format!("injected GPU candidate failure after {name}").into());
            }
            #[cfg(not(test))]
            let _ = slot;
        }
        let pressure = self
            .pressure
            .project_closed(
                device,
                queue,
                PressureFields {
                    density: &density.density_kg_m3,
                    root_cell: &self.scene.component_root,
                    aperture_u: &self.scene.u_aperture,
                    aperture_v: &self.scene.v_aperture,
                    predictor_u: &gravity.u_velocity_m_s,
                    predictor_v: &gravity.v_velocity_m_s,
                    corrected_u: &candidate.u_velocity_m_s,
                    corrected_v: &candidate.v_velocity_m_s,
                },
                pressure_config,
            )
            .await
            .map_err(|error: PressureError| {
                // A zero solve budget is an explicit failure fixture. Do not
                // turn it into success by reducing the pressure forcing.
                if error.is_retryable() && pressure_config.max_iterations != 0 {
                    CandidateRejection::Numerical {
                        message: error.to_string(),
                        readback_bytes: STAGE_STATUS_BYTES + self.pressure.last_readback_bytes(),
                    }
                } else {
                    CandidateRejection::Terminal(error.to_string())
                }
            })?;
        #[cfg(test)]
        if self.test_fault == Some(TestFaultPoint::Pressure) {
            return Err("injected GPU candidate failure after pressure".into());
        }
        #[cfg(test)]
        if self.test_numerical_rejections > 0 {
            self.test_numerical_rejections -= 1;
            return Err(CandidateRejection::Numerical {
                message: "test numerical gate rejected candidate".into(),
                readback_bytes: STAGE_STATUS_BYTES + pressure.readback_bytes,
            });
        }
        let mut commit = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("accepted GPU inventory copy"),
        });
        for (source, target) in [
            (&transport.mass_kg, &candidate.mass_kg),
            (&transport.marker, &candidate.marker),
            (&density.density_kg_m3, &candidate.density_kg_m3),
        ] {
            commit.copy_buffer_to_buffer(source, 0, target, 0, target.size());
        }
        self.pressure.encode_accepted_guess(&mut commit);
        queue.submit([commit.finish()]);
        self.scene.committed_index = 1 - self.scene.committed_index;
        self.scene.accepted_time_s = next_time;
        self.scene.state_revision = next_revision;
        Ok(pressure)
    }
}

/// Halve the rejected nominal duration without exceeding either stability
/// limit. Clock selection still owns the nominal and encoded time balance.
fn refined_selection(
    selected: GpuCflSelection,
    rejected_nominal_dt_s: f64,
) -> Result<GpuCflSelection, String> {
    let cap_s = rejected_nominal_dt_s * 0.5;
    let mut dt_s = cap_s as f32;
    if f64::from(dt_s) > cap_s {
        dt_s = f32::from_bits(dt_s.to_bits() - 1);
    }
    if !dt_s.is_finite() || dt_s <= 0.0 || f64::from(dt_s) > cap_s {
        return Err("GPU refined candidate duration cannot be represented".into());
    }
    Ok(GpuCflSelection {
        dt_s,
        max_cfl_dt_s: Some(selected.max_cfl_dt_s.unwrap_or(f64::INFINITY).min(cap_s)),
        max_diffusion_dt_s: selected.max_diffusion_dt_s,
    })
}

/// The host clock keeps the nominal f64 outer duration. WGSL receives f32 dt.
/// On an uncapped final substep, choose the nearest stable f32 encoding while
/// carrying the prior encoding error, so that shader time does not drift in
/// one direction over many ticks. A cap near the outer duration uses two
/// balanced steps instead of leaving a tiny final step. The accepted clock
/// always sums the nominal durations; `encoded_time_s` records the actual
/// values sent to WGSL.
fn select_clock_durations(
    remaining_s: f64,
    selected: GpuCflSelection,
    encoding_lag_s: f64,
) -> Result<(f64, f32), String> {
    if !encoding_lag_s.is_finite() {
        return Err("GPU encoded-time difference is invalid".into());
    }
    let stability_limit_s = selected
        .max_cfl_dt_s
        .unwrap_or(f64::INFINITY)
        .min(selected.max_diffusion_dt_s.unwrap_or(f64::INFINITY));
    let completes_outer = remaining_s <= stability_limit_s;
    let mut nominal_dt_s = if completes_outer {
        remaining_s
    } else {
        f64::from(selected.dt_s)
    };
    let mut encoded_dt_s = selected.dt_s;
    if completes_outer {
        let desired = nominal_dt_s + encoding_lag_s;
        let nearest = desired as f32;
        let per_step_error_limit = nominal_dt_s * 8.0 * f32::EPSILON as f64;
        if nearest.is_finite()
            && nearest > 0.0
            && f64::from(nearest) <= stability_limit_s
            && (f64::from(nearest) - nominal_dt_s).abs() <= per_step_error_limit
        {
            encoded_dt_s = nearest;
        } else if nearest.is_finite()
            && f64::from(nearest) > stability_limit_s
            && f64::from(selected.dt_s) < remaining_s
        {
            // A cap exactly at the nominal boundary cannot use the nearest
            // f32 encoding. Accept the safe lower step and carry the small
            // remainder into another bounded substep rather than lose time.
            nominal_dt_s = f64::from(selected.dt_s);
        }
    }
    if nominal_dt_s < remaining_s && f64::from(selected.dt_s) >= remaining_s * 0.5 {
        let half = (remaining_s * 0.5) as f32;
        if half.is_finite()
            && half > 0.0
            && f64::from(half) < remaining_s
            && f64::from(half) <= stability_limit_s
        {
            nominal_dt_s = f64::from(half);
            encoded_dt_s = half;
        }
    }
    if !nominal_dt_s.is_finite()
        || nominal_dt_s <= 0.0
        || nominal_dt_s > remaining_s
        || !encoded_dt_s.is_finite()
        || encoded_dt_s <= 0.0
        || f64::from(encoded_dt_s) > stability_limit_s
    {
        return Err("GPU selected time step violates clock or stability bound".into());
    }
    Ok((nominal_dt_s, encoded_dt_s))
}

#[derive(Default)]
struct MapCompletion {
    result: Option<Result<(), String>>,
    waker: Option<Waker>,
}

async fn read_stage_statuses(
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
async fn read_fixture_committed_state(scene: &GpuScene) -> Result<[Vec<u8>; 5], String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use particle_sim::contracts::Boundary;
    use particle_sim_cpu::{
        ReferenceSession,
        coupled::{CoupledStepConfig, CoupledStepOutcome},
        fluid::{FaceValues, PressureFields as CpuPressureFields},
        solver::SolveConfig,
        substep::SubstepClock,
        transport::TransportInventory,
    };
    use std::{
        future::Future,
        task::{Context, Wake},
        time::{Duration, Instant},
    };

    fn uniform_air_seed() -> GpuSceneSeed {
        e07_uniform_air_seed()
    }

    fn mixed_vortex_seed() -> GpuSceneSeed {
        e07_mixed_vortex_seed()
    }

    fn cpu_mixed_vortex_step(seed: &GpuSceneSeed) -> ReferenceSession {
        let cells = seed.grid.cells();
        let inventory = TransportInventory::new(
            seed.grid,
            seed.phase_density_kg_m3[0],
            seed.phase_density_kg_m3[1],
            seed.mass_kg[..cells].to_vec(),
            seed.mass_kg[cells..].to_vec(),
            seed.marker[..cells].to_vec(),
            seed.marker[cells..].to_vec(),
            vec![false; cells],
        )
        .unwrap();
        let density = (0..cells)
            .map(|cell| {
                (seed.mass_kg[cell] + seed.mass_kg[cells + cell]) / seed.grid.cell_volume_m3()
            })
            .collect();
        let fields = CpuPressureFields::new(
            seed.grid,
            [Boundary::Closed; 4],
            density,
            vec![0.0; cells],
            FaceValues {
                u: seed.u_velocity_m_s.clone(),
                v: seed.v_velocity_m_s.clone(),
            },
            FaceValues {
                u: seed.u_aperture.clone(),
                v: seed.v_aperture.clone(),
            },
        )
        .unwrap();
        let mut session = ReferenceSession::new(seed.grid);
        session.set_pressure_fields(fields).unwrap();
        session.set_transport_inventory(inventory).unwrap();
        let viscosity = vec![0.001; cells];
        let mut clock = SubstepClock::new(OUTER_DT_S, 8).unwrap();
        let outcome = session
            .advance_coupled_substep(
                &mut clock,
                CoupledStepConfig {
                    gravity_m_s2: 0.0,
                    cell_dynamic_viscosity_pa_s: Some(&viscosity),
                    pressure: SolveConfig::default(),
                },
            )
            .unwrap();
        assert!(matches!(outcome, CoupledStepOutcome::Advanced { .. }));
        assert!((clock.accepted_time_s() - OUTER_DT_S).abs() < 1e-14);
        session
    }

    fn read_f32(context: &GpuContext, source: &wgpu::Buffer) -> Vec<f32> {
        let staging = context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("coupled parity test readback"),
            size: source.size(),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("coupled parity test copy"),
            });
        encoder.copy_buffer_to_buffer(source, 0, &staging, 0, source.size());
        context.queue.submit([encoder.finish()]);
        let (sender, receiver) = std::sync::mpsc::channel();
        staging.map_async(wgpu::MapMode::Read, .., move |result| {
            sender.send(result).unwrap();
        });
        context
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(Duration::from_secs(10)),
            })
            .unwrap();
        receiver
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .unwrap();
        let mapped = staging.get_mapped_range(..);
        let values = mapped
            .as_chunks::<4>()
            .0
            .iter()
            .map(|bytes| f32::from_le_bytes(*bytes))
            .collect();
        drop(mapped);
        staging.unmap();
        values
    }

    fn committed_metadata(scene: &GpuCoupledScene) -> (u64, usize, u64, f64, f64, f64) {
        (
            scene.scene.epoch,
            scene.scene.committed_index,
            scene.scene.tick,
            scene.scene.accepted_time_s,
            scene.encoded_time_s,
            scene.remaining_outer_s,
        )
    }

    #[test]
    fn f32_shader_steps_keep_nominal_f64_outer_time() {
        let selection = GpuCflSelection {
            dt_s: f32::from_bits((OUTER_DT_S as f32).to_bits() - 1),
            max_cfl_dt_s: None,
            max_diffusion_dt_s: None,
        };
        let mut accepted_time = 0.0_f64;
        let mut encoded_time = 0.0_f64;
        for tick in 1..=1_000 {
            let (nominal, encoded) =
                select_clock_durations(OUTER_DT_S, selection, accepted_time - encoded_time)
                    .unwrap();
            accepted_time += nominal;
            encoded_time += f64::from(encoded);
            assert!((accepted_time - f64::from(tick) * OUTER_DT_S).abs() <= 1e-10);
            assert!((accepted_time - encoded_time).abs() < 1e-9);
        }
        let capped = GpuCflSelection {
            dt_s: 0.001,
            max_cfl_dt_s: Some(0.0011),
            max_diffusion_dt_s: None,
        };
        let (nominal, encoded) = select_clock_durations(OUTER_DT_S, capped, 0.0).unwrap();
        assert_eq!(nominal, f64::from(encoded));
        assert!(f64::from(encoded) <= 0.0011);

        let exact_cap = GpuCflSelection {
            dt_s: f32::from_bits((OUTER_DT_S as f32).to_bits() - 1),
            max_cfl_dt_s: Some(OUTER_DT_S),
            max_diffusion_dt_s: None,
        };
        let (nominal, encoded) = select_clock_durations(OUTER_DT_S, exact_cap, 0.0).unwrap();
        assert_eq!(nominal, f64::from(encoded));
        assert!(nominal > OUTER_DT_S * 0.49);
        assert!(nominal < OUTER_DT_S * 0.51);
        let remaining = OUTER_DT_S - nominal;
        let rounded_final = remaining as f32;
        let final_dt = if f64::from(rounded_final) > remaining {
            f32::from_bits(rounded_final.to_bits() - 1)
        } else {
            rounded_final
        };
        let final_selection = GpuCflSelection {
            dt_s: final_dt,
            ..exact_cap
        };
        let (final_nominal, final_encoded) =
            select_clock_durations(remaining, final_selection, 0.0).unwrap();
        assert_eq!(final_nominal, remaining);
        assert!(f64::from(final_encoded) <= OUTER_DT_S);
        assert_eq!(nominal + final_nominal, OUTER_DT_S);
    }

    #[test]
    fn near_outer_cfl_cap_balances_two_shader_steps() {
        let cap = 0.016_563_048_586_25_f64;
        let rounded = cap as f32;
        let selected_dt = if f64::from(rounded) > cap {
            f32::from_bits(rounded.to_bits() - 1)
        } else {
            rounded
        };
        let first_selection = GpuCflSelection {
            dt_s: selected_dt,
            max_cfl_dt_s: Some(cap),
            max_diffusion_dt_s: None,
        };
        let (first_nominal, first_encoded) =
            select_clock_durations(OUTER_DT_S, first_selection, 0.0).unwrap();
        assert_eq!(first_nominal, f64::from(first_encoded));
        assert!(first_nominal > OUTER_DT_S * 0.49);
        assert!(first_nominal < OUTER_DT_S * 0.51);
        assert!(first_nominal <= cap);

        let remaining = OUTER_DT_S - first_nominal;
        let rounded_final = remaining as f32;
        let final_dt = if f64::from(rounded_final) > remaining {
            f32::from_bits(rounded_final.to_bits() - 1)
        } else {
            rounded_final
        };
        let final_selection = GpuCflSelection {
            dt_s: final_dt,
            max_cfl_dt_s: Some(cap),
            max_diffusion_dt_s: None,
        };
        let (final_nominal, final_encoded) =
            select_clock_durations(remaining, final_selection, 0.0).unwrap();
        assert_eq!(final_nominal, remaining);
        assert!(f64::from(final_encoded) <= cap);
        assert_eq!(first_nominal + final_nominal, OUTER_DT_S);
        assert!((OUTER_DT_S - f64::from(first_encoded) - f64::from(final_encoded)).abs() < 1e-9);
    }

    #[test]
    fn numerical_refinement_halves_duration_and_retains_clock_balance() {
        let initial = GpuCflSelection {
            dt_s: OUTER_DT_S as f32,
            max_cfl_dt_s: None,
            max_diffusion_dt_s: Some(0.1),
        };
        let mut accepted_time = 0.0_f64;
        let mut encoded_time = 0.0_f64;
        for tick in 1..=1_000 {
            let mut selection = initial;
            let mut nominal = OUTER_DT_S;
            let mut encoded = initial.dt_s;
            for _ in 0..MAX_CANDIDATE_REFINEMENTS {
                selection = refined_selection(selection, nominal).unwrap();
                let (next_nominal, next_encoded) =
                    select_clock_durations(OUTER_DT_S, selection, accepted_time - encoded_time)
                        .unwrap();
                assert!(next_nominal <= nominal * 0.5);
                assert!(next_encoded <= encoded * 0.5);
                assert_eq!(selection.max_diffusion_dt_s, Some(0.1));
                nominal = next_nominal;
                encoded = next_encoded;
            }
            accepted_time += nominal;
            encoded_time += f64::from(encoded);
            let remaining = OUTER_DT_S - nominal;
            let final_selection = GpuCflSelection {
                dt_s: remaining as f32,
                ..initial
            };
            let (final_nominal, final_encoded) =
                select_clock_durations(remaining, final_selection, accepted_time - encoded_time)
                    .unwrap();
            accepted_time += final_nominal;
            encoded_time += f64::from(final_encoded);
            assert!((accepted_time - f64::from(tick) * OUTER_DT_S).abs() <= 1e-10);
            assert!((accepted_time - encoded_time).abs() < 1e-9);
        }
        for invalid_duration in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::MIN_POSITIVE] {
            assert!(refined_selection(initial, invalid_duration).is_err());
        }
    }

    #[test]
    fn stage_refinement_excludes_invalid_fields_and_arithmetic() {
        for (slot, status) in [
            (0, transport::STATUS_DONOR_OVERDRAW),
            (0, transport::STATUS_VOLUME_CLOSURE),
            (0, transport::STATUS_CORRECTION_LIMIT),
            (1, momentum::STATUS_DONOR_OVERDRAW),
            (2, density::STATUS_VOLUME_CLOSURE),
        ] {
            assert!(matches!(
                stage_rejection(slot, "fixture", status),
                CandidateRejection::Numerical { .. }
            ));
        }
        for (slot, status) in [
            (0, transport::STATUS_INVALID_FACE),
            (0, transport::STATUS_INVALID_CELL),
            (
                0,
                transport::STATUS_VOLUME_CLOSURE | transport::STATUS_INVALID_CELL,
            ),
            (0, 1 << 31),
            (1, momentum::STATUS_INVALID_LEDGER),
            (1, momentum::STATUS_EMPTY_OPEN_FACE),
            (2, density::STATUS_INVALID_CELL),
            (3, crate::viscosity::STATUS_NONFINITE_RESULT),
            (4, crate::gravity::STATUS_NONFINITE_RESULT),
        ] {
            assert!(matches!(
                stage_rejection(slot, "fixture", status),
                CandidateRejection::Terminal(_)
            ));
        }
    }

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
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(value) => return value,
                Poll::Pending => {
                    assert!(Instant::now() < deadline, "GPU coupled test timed out");
                    std::thread::park_timeout(Duration::from_millis(100));
                }
            }
        }
    }

    #[test]
    #[ignore = "requires a native GPU compute adapter"]
    fn closed_scene_accepts_one_complete_zero_force_tick() {
        block_on(async {
            let context = GpuContext::new().await.unwrap();
            let mut scene = GpuCoupledScene::new(
                &context,
                uniform_air_seed(),
                GpuScenePhysics {
                    cell_dynamic_viscosity_pa_s: Some(vec![0.001; 4]),
                    gravity_m_s2: 0.0,
                    pressure: PressureConfig::default(),
                },
            )
            .unwrap();
            let before_time = scene.scene.accepted_time_s;
            let before_index = scene.scene.committed_index;
            let outcome = scene.advance_outer_tick(4).await.unwrap();
            match outcome {
                GpuTickOutcome::Complete {
                    accepted_substeps,
                    last_pressure,
                    encoded_time_error_s,
                    ..
                } => {
                    assert_eq!(accepted_substeps, 1);
                    assert!(last_pressure.scaled_residual <= 1e-5);
                    assert!(last_pressure.scaled_divergence <= 1e-5);
                    assert!(encoded_time_error_s.abs() < 1e-9);
                }
                GpuTickOutcome::Paused { .. } => panic!("zero flow should complete one tick"),
            }
            assert_eq!(scene.scene.committed_index, 1 - before_index);
            assert_eq!(scene.scene.tick, 8);
            assert!((scene.scene.accepted_time_s - before_time - OUTER_DT_S).abs() < 1e-15);
        });
    }

    #[test]
    #[ignore = "requires a native GPU compute adapter"]
    fn numerical_retry_counts_attempts_and_respects_accepted_substep_budget() {
        block_on(async {
            let context = GpuContext::new().await.unwrap();
            let mut scene = GpuCoupledScene::new(
                &context,
                uniform_air_seed(),
                GpuScenePhysics {
                    cell_dynamic_viscosity_pa_s: Some(vec![0.001; 4]),
                    gravity_m_s2: 0.0,
                    pressure: PressureConfig::default(),
                },
            )
            .unwrap();
            let before = scene.progress();
            let committed_before = read_fixture_committed_state(&scene.scene).await.unwrap();
            scene.test_numerical_rejections = 1;
            let GpuTickOutcome::Paused {
                accepted_substeps,
                attempted_candidates,
                refinement_retries,
                remaining_s,
                readback_bytes,
                rejected_readback_bytes,
            } = scene.advance_outer_tick(1).await.unwrap()
            else {
                panic!("one accepted refined step must pause with remaining time");
            };
            assert_eq!(accepted_substeps, 1);
            assert_eq!(attempted_candidates, 2);
            assert_eq!(refinement_retries, 1);
            assert!(rejected_readback_bytes >= STAGE_STATUS_BYTES + 32);
            assert_eq!(readback_bytes, rejected_readback_bytes + 8);
            assert_eq!(scene.scene.tick, before.tick);
            assert_eq!(scene.scene.state_revision, before.state_revision + 1);
            assert_eq!(scene.scene.committed_index, 1 - before.committed_generation);
            assert!(remaining_s >= OUTER_DT_S * 0.5);
            assert!(
                (remaining_s + scene.scene.accepted_time_s - before.accepted_time_s - OUTER_DT_S)
                    .abs()
                    < 1e-15
            );
            assert_eq!(
                read_fixture_committed_state(&scene.scene).await.unwrap(),
                committed_before
            );
            let GpuTickOutcome::Complete {
                accepted_substeps,
                attempted_candidates,
                refinement_retries,
                rejected_readback_bytes,
                encoded_time_error_s,
                ..
            } = scene.advance_outer_tick(1).await.unwrap()
            else {
                panic!("the remaining zero-force step must complete the outer tick");
            };
            assert_eq!(accepted_substeps, 1);
            assert_eq!(attempted_candidates, 1);
            assert_eq!(refinement_retries, 0);
            assert_eq!(rejected_readback_bytes, 0);
            assert_eq!(scene.scene.tick, before.tick + 1);
            assert_eq!(scene.remaining_outer_s, 0.0);
            assert!(
                (scene.scene.accepted_time_s - before.accepted_time_s - OUTER_DT_S).abs() < 1e-15
            );
            assert!(encoded_time_error_s.abs() < 1e-9);
        });
    }

    #[test]
    #[ignore = "requires a native GPU compute adapter"]
    fn numerical_refinement_exhaustion_preserves_committed_fields_and_time() {
        block_on(async {
            let context = GpuContext::new().await.unwrap();
            let mut scene = GpuCoupledScene::new(
                &context,
                uniform_air_seed(),
                GpuScenePhysics {
                    cell_dynamic_viscosity_pa_s: None,
                    gravity_m_s2: 0.0,
                    pressure: PressureConfig::default(),
                },
            )
            .unwrap();
            let before = scene.progress();
            let encoded_before = scene.encoded_time_s;
            let committed_before = read_fixture_committed_state(&scene.scene).await.unwrap();
            scene.test_numerical_rejections = MAX_CANDIDATE_REFINEMENTS + 1;
            let error = scene.advance_outer_tick(1).await.unwrap_err();
            assert!(
                error.contains("refinement exhausted after 5 attempts"),
                "{error}"
            );
            assert_eq!(scene.test_numerical_rejections, 0);
            assert_eq!(scene.progress(), before);
            assert_eq!(scene.encoded_time_s, encoded_before);
            assert_eq!(
                read_fixture_committed_state(&scene.scene).await.unwrap(),
                committed_before
            );
            assert!(matches!(
                scene.advance_outer_tick(1).await.unwrap(),
                GpuTickOutcome::Complete {
                    accepted_substeps: 1,
                    attempted_candidates: 1,
                    refinement_retries: 0,
                    ..
                }
            ));
        });
    }

    #[test]
    #[ignore = "requires a native GPU compute adapter"]
    fn pressure_rejection_preserves_committed_bytes_and_retry_matches_clean_run() {
        block_on(async {
            let context = GpuContext::new().await.unwrap();
            let make_scene = || {
                GpuCoupledScene::new(
                    &context,
                    mixed_vortex_seed(),
                    GpuScenePhysics {
                        cell_dynamic_viscosity_pa_s: Some(vec![0.001; 4]),
                        gravity_m_s2: 0.0,
                        pressure: PressureConfig::default(),
                    },
                )
                .unwrap()
            };
            let mut retried = make_scene();
            let mut clean = make_scene();
            assert!(matches!(
                retried.advance_outer_tick(8).await.unwrap(),
                GpuTickOutcome::Complete { .. }
            ));
            assert!(matches!(
                clean.advance_outer_tick(8).await.unwrap(),
                GpuTickOutcome::Complete { .. }
            ));
            assert_eq!(retried.scene.committed_index, 1);
            assert_eq!(
                read_fixture_committed_state(&retried.scene).await.unwrap(),
                read_fixture_committed_state(&clean.scene).await.unwrap()
            );
            let before = committed_metadata(&retried);
            let committed_before = read_fixture_committed_state(&retried.scene).await.unwrap();
            retried.gravity_m_s2 = 9.80665;
            retried.pressure_config.max_iterations = 0;
            let error = retried.advance_outer_tick(8).await.unwrap_err();
            assert!(
                error.contains("exhausted its bounded iterations"),
                "unexpected pressure error: {error}"
            );
            assert_eq!(committed_metadata(&retried), before);
            assert_eq!(
                read_fixture_committed_state(&retried.scene).await.unwrap(),
                committed_before
            );
            retried.gravity_m_s2 = 0.0;
            retried.pressure_config.max_iterations = PressureConfig::default().max_iterations;
            assert!(matches!(
                retried.advance_outer_tick(8).await.unwrap(),
                GpuTickOutcome::Complete { .. }
            ));
            assert!(matches!(
                clean.advance_outer_tick(8).await.unwrap(),
                GpuTickOutcome::Complete { .. }
            ));
            assert_eq!(committed_metadata(&retried), committed_metadata(&clean));
            assert_eq!(
                read_fixture_committed_state(&retried.scene).await.unwrap(),
                read_fixture_committed_state(&clean.scene).await.unwrap()
            );
        });
    }

    #[test]
    #[ignore = "requires a native GPU compute adapter"]
    fn injected_faults_after_each_stage_and_pressure_preserve_committed_state() {
        block_on(async {
            let context = GpuContext::new().await.unwrap();
            let mut scene = GpuCoupledScene::new(
                &context,
                uniform_air_seed(),
                GpuScenePhysics {
                    cell_dynamic_viscosity_pa_s: Some(vec![0.001; 4]),
                    gravity_m_s2: 0.0,
                    pressure: PressureConfig::default(),
                },
            )
            .unwrap();
            assert!(matches!(
                scene.advance_outer_tick(8).await.unwrap(),
                GpuTickOutcome::Complete { .. }
            ));
            assert_eq!(scene.scene.committed_index, 1);
            let before = committed_metadata(&scene);
            let committed_before = read_fixture_committed_state(&scene.scene).await.unwrap();
            for fault in [
                TestFaultPoint::Stage(0),
                TestFaultPoint::Stage(1),
                TestFaultPoint::Stage(2),
                TestFaultPoint::Stage(3),
                TestFaultPoint::Stage(4),
                TestFaultPoint::Pressure,
            ] {
                scene.test_fault = Some(fault);
                let error = scene.advance_outer_tick(8).await.unwrap_err();
                assert!(
                    error.contains("injected GPU candidate failure"),
                    "unexpected {fault:?} error: {error}"
                );
                assert_eq!(committed_metadata(&scene), before, "fault: {fault:?}");
                assert_eq!(
                    read_fixture_committed_state(&scene.scene).await.unwrap(),
                    committed_before,
                    "fault: {fault:?}"
                );
            }
            scene.test_fault = None;
            assert!(matches!(
                scene.advance_outer_tick(8).await.unwrap(),
                GpuTickOutcome::Complete { .. }
            ));
        });
    }

    #[test]
    #[ignore = "requires a native GPU compute adapter"]
    fn mixed_vortex_matches_cpu_at_one_accepted_outer_time() {
        block_on(async {
            let reference_seed = mixed_vortex_seed();
            let reference = cpu_mixed_vortex_step(&reference_seed);
            let context = GpuContext::new().await.unwrap();
            let mut gpu = GpuCoupledScene::new(
                &context,
                mixed_vortex_seed(),
                GpuScenePhysics {
                    cell_dynamic_viscosity_pa_s: Some(vec![0.001; 4]),
                    gravity_m_s2: 0.0,
                    pressure: PressureConfig::default(),
                },
            )
            .unwrap();
            let outcome = gpu.advance_outer_tick(8).await.unwrap();
            let GpuTickOutcome::Complete {
                last_pressure,
                accepted_substeps,
                ..
            } = outcome
            else {
                panic!("mixed vortex did not complete the outer tick");
            };
            assert_eq!(accepted_substeps, 1);
            assert!(last_pressure.scaled_divergence <= 1e-5);
            assert!((gpu.scene.accepted_time_s - (7.0 / 60.0 + OUTER_DT_S)).abs() < 1e-14);

            // Full-state downloads are test-only oracle comparisons. Normal
            // scene stepping reads only compact status and CFL reductions.
            let committed = gpu.scene.committed();
            let mass = read_f32(&context, &committed.mass_kg);
            let marker = read_f32(&context, &committed.marker);
            let u = read_f32(&context, &committed.u_velocity_m_s);
            let v = read_f32(&context, &committed.v_velocity_m_s);
            let cpu_inventory = reference.transport_inventory().unwrap();
            let cpu_fields = reference.pressure_fields().unwrap();
            let volume = reference_seed.grid.cell_volume_m3();
            let mut alpha_absolute_error = 0.0;
            for (cell, &gpu_mass) in mass.iter().take(reference_seed.grid.cells()).enumerate() {
                let gpu_alpha = f64::from(gpu_mass) / (1000.0 * volume);
                alpha_absolute_error += (gpu_alpha - cpu_inventory.alpha(cell).unwrap()).abs();
            }
            alpha_absolute_error /= reference_seed.grid.cells() as f64;
            assert!(
                alpha_absolute_error <= 1e-5,
                "alpha MAE {alpha_absolute_error}"
            );
            let mut squared_velocity_error = 0.0;
            let mut count = 0;
            for (gpu_faces, cpu_faces) in [
                (&u, &cpu_fields.velocity_m_s().u),
                (&v, &cpu_fields.velocity_m_s().v),
            ] {
                for (&gpu_speed, &cpu_speed) in gpu_faces.iter().zip(cpu_faces) {
                    squared_velocity_error += (f64::from(gpu_speed) - cpu_speed).powi(2);
                    count += 1;
                }
            }
            let scaled_velocity_rms = (squared_velocity_error / f64::from(count)).sqrt() / 0.01;
            assert!(
                scaled_velocity_rms <= 1e-4,
                "scaled velocity RMS {scaled_velocity_rms}"
            );
            for (gpu_values, cpu_values, scale) in [
                (&mass[..4], cpu_inventory.liquid_mass_kg(), 0.001),
                (&mass[4..], cpu_inventory.carrier_mass_kg(), 1e-6),
                (&marker[..4], cpu_inventory.liquid_marker(), 1.0),
                (&marker[4..], cpu_inventory.carrier_marker(), 1.0),
            ] {
                let gpu_total: f64 = gpu_values.iter().map(|&value| f64::from(value)).sum();
                let cpu_total: f64 = cpu_values.iter().sum();
                assert!(
                    (gpu_total - cpu_total).abs() <= 1e-5 * scale,
                    "GPU total {gpu_total} differs from CPU {cpu_total}"
                );
            }
        });
    }

    #[test]
    #[ignore = "requires a native GPU compute adapter"]
    fn public_fixture_validation_reports_acceptance_and_rejection() {
        block_on(async {
            let context = GpuContext::new().await.unwrap();
            let report = context.validate_scene_fixture().await.unwrap();
            assert_eq!(report.epoch, 2);
            assert_eq!(report.tick, 8);
            assert_eq!(report.accepted_substeps, 1);
            assert!((report.accepted_time_s - 8.0 / 60.0).abs() < 1e-14);
            assert!(report.scaled_residual <= 1e-5);
            assert!(report.scaled_divergence <= 1e-5);
            assert!(report.encoded_time_error_s.abs() < 1e-9);
            assert!(report.rejection_kept_committed_state);
        });
    }
}
