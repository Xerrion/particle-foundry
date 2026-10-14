//! Candidate-to-commit GPU graph for the limited sealed two-phase scene.
//!
//! Every stage writes detached buffers. The five small stage status words are
//! checked together before pressure projection. Pressure independently checks
//! its true residual and corrected divergence. Only then does this owner copy
//! the candidate inventory into the alternate generation and accept time.

mod candidate;
mod clock;
mod lifecycle;
mod readback;
mod stepping;
mod validation;

use candidate::CandidateBuffers;
use clock::{refined_selection, select_clock_durations};
use readback::{read_fixture_committed_state, read_stage_statuses};
pub(crate) use validation::validate_scene_fixture;
#[cfg(test)]
use validation::{e07_mixed_vortex_seed, e07_uniform_air_seed};

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
    candidates: CandidateBuffers,
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
    SubmittedStages,
    BeforeCommit,
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

#[cfg(test)]
mod tests;
