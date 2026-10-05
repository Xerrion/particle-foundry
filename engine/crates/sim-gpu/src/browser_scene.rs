//! One browser canvas, GPU scene, and renderer on the same WebGPU device.

use web_sys::HtmlCanvasElement;

use crate::{
    GpuSurface,
    render::{RenderViewport, SceneRenderer},
    scene::{GpuCoupledScene, GpuPaintMaterial, GpuTickOutcome, demo::m3_demo_scene},
};

/// Accepted model position and selected GPU generation for a browser scene.
/// A candidate substep does not change these values before it passes all gates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GpuBrowserProgress {
    /// Browser scene identity. Reset increments this value.
    pub epoch: u64,
    /// Number of completed outer ticks in this epoch.
    pub tick: u64,
    /// Sum of accepted nominal substep durations in seconds.
    pub accepted_time_s: f64,
    /// Unfinished portion of the current outer tick in seconds.
    pub remaining_outer_s: f64,
    /// Index of the selected committed GPU buffer generation.
    pub committed_generation: u32,
    /// Per-epoch committed-state revision. Accepted substeps and paint increment it.
    pub state_revision: u64,
}

/// Result of one bounded browser advance call.
/// Pressure diagnostics are present when the call completes an outer tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GpuBrowserAdvance {
    /// Number of substeps accepted during this call.
    pub accepted_substeps: u32,
    /// True when this call completed one outer tick.
    pub completed_outer_tick: bool,
    /// Committed position after this call.
    pub progress: GpuBrowserProgress,
    /// Final pressure residual scaled by the accepted step duration.
    pub scaled_residual: Option<f32>,
    /// Final pressure divergence scaled by the accepted step duration.
    pub scaled_divergence: Option<f32>,
    /// Number of pressure iterations in the final accepted substep.
    pub pressure_iterations: Option<u32>,
    /// Difference between accepted nominal time and encoded shader time.
    pub encoded_time_error_s: Option<f64>,
    /// Candidate attempts, including rejected refinement attempts.
    pub attempted_candidates: u32,
    /// Rejected candidates followed by a smaller duration attempt.
    pub refinement_retries: u32,
    /// Compact completion bytes copied by rejected attempts.
    pub rejected_readback_bytes: u64,
    /// Compact completion bytes copied by accepted candidates and CFL selection.
    pub readback_bytes: u64,
}

/// One committed cell, sampled through a bounded GPU readback.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GpuBrowserProbe {
    /// Zero-based scene cell coordinates.
    pub cell: [u32; 2],
    /// Liquid and carrier mass in kilograms.
    pub mass_kg: [f32; 2],
    /// Whether this cell is a fixed wall.
    pub fixed_wall: bool,
    /// Selected state at probe submission and completion.
    pub progress: GpuBrowserProgress,
}

/// Experimental M3 browser scene with one GPU owner for simulation and display.
/// This scene does not replace the live legacy sandbox.
pub struct GpuBrowserScene {
    surface: GpuSurface,
    scene: GpuCoupledScene,
    renderer: SceneRenderer,
}

impl GpuBrowserScene {
    /// Creates one canvas-compatible device, a resident two-phase scene, and its renderer.
    pub async fn new(canvas: HtmlCanvasElement) -> Result<Self, String> {
        let surface = GpuSurface::new(canvas).await?;
        let scene = m3_demo_scene(surface.context(), 1)?;
        let renderer = SceneRenderer::new(&surface.context().device, surface.format())?;
        Ok(Self {
            surface,
            scene,
            renderer,
        })
    }

    /// Returns the selected adapter backend for diagnostics.
    pub fn backend(&self) -> String {
        self.surface.context().backend()
    }

    /// Returns the adapter that owns this scene and its canvas.
    pub fn adapter_info(&self) -> &wgpu::AdapterInfo {
        &self.surface.context().adapter
    }

    /// Returns accepted progress without mapping scene fields to JavaScript.
    pub fn progress(&self) -> GpuBrowserProgress {
        let committed = self.scene.progress();
        GpuBrowserProgress {
            epoch: committed.epoch,
            tick: committed.tick,
            accepted_time_s: committed.accepted_time_s,
            remaining_outer_s: committed.remaining_outer_s,
            committed_generation: committed.committed_generation as u32,
            state_revision: committed.state_revision,
        }
    }

    /// Draws the selected committed generation. Returns false when no frame is available.
    pub fn render(&mut self) -> Result<bool, String> {
        let target_size_px = self.surface.size();
        let Some(frame) = self.surface.acquire_frame()? else {
            return Ok(false);
        };
        let view = frame
            .texture()
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.scene.render_committed(
            &self.renderer,
            &view,
            target_size_px,
            RenderViewport::full(self.scene.grid()),
        )?;
        frame.present();
        Ok(true)
    }

    /// Reconfigures the canvas backing dimensions after the current frame ends.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
        self.surface.resize(width, height)
    }

    /// Replaces fluid below a bounded circular brush in the detached GPU generation.
    /// Fixed walls stay unchanged. The scene commits at zero accepted model time.
    pub fn paint(
        &mut self,
        center_x: u32,
        center_y: u32,
        radius: u32,
        material: &str,
    ) -> Result<GpuBrowserProgress, String> {
        let material = match material {
            "water" => GpuPaintMaterial::Water,
            "air" => GpuPaintMaterial::Air,
            _ => return Err("GPU paint material must be water or air".into()),
        };
        self.scene.paint(center_x, center_y, radius, material)?;
        Ok(self.progress())
    }

    /// Samples one cell without downloading either complete phase field.
    pub async fn probe(&self, x: u32, y: u32) -> Result<GpuBrowserProbe, String> {
        let sample = self.scene.probe_cell(x, y).await?;
        Ok(GpuBrowserProbe {
            cell: sample.cell,
            mass_kg: sample.mass_kg,
            fixed_wall: sample.fixed_wall,
            progress: self.progress(),
        })
    }

    /// Copies a detached committed generation on explicit request only.
    /// The PFCP prototype has no recovery/load API and is distinct from PFSN.
    pub async fn checkpoint_prototype(&self) -> Result<Vec<u8>, String> {
        self.scene.checkpoint_prototype().await
    }

    /// Runs at most `max_substeps` complete candidate steps.
    /// A failed candidate returns an error without publishing its candidate buffers.
    pub async fn advance(&mut self, max_substeps: u32) -> Result<GpuBrowserAdvance, String> {
        let outcome = self.scene.advance_outer_tick(max_substeps).await?;
        let progress = self.progress();
        Ok(match outcome {
            GpuTickOutcome::Complete {
                accepted_substeps,
                last_pressure,
                encoded_time_error_s,
                readback_bytes,
                attempted_candidates,
                refinement_retries,
                rejected_readback_bytes,
            } => GpuBrowserAdvance {
                accepted_substeps,
                completed_outer_tick: true,
                progress,
                scaled_residual: Some(last_pressure.scaled_residual),
                scaled_divergence: Some(last_pressure.scaled_divergence),
                pressure_iterations: Some(last_pressure.iterations),
                encoded_time_error_s: Some(encoded_time_error_s),
                readback_bytes,
                attempted_candidates,
                refinement_retries,
                rejected_readback_bytes,
            },
            GpuTickOutcome::Paused {
                accepted_substeps,
                remaining_s,
                readback_bytes,
                attempted_candidates,
                refinement_retries,
                rejected_readback_bytes,
            } => {
                debug_assert_eq!(remaining_s, progress.remaining_outer_s);
                GpuBrowserAdvance {
                    accepted_substeps,
                    completed_outer_tick: false,
                    progress,
                    scaled_residual: None,
                    scaled_divergence: None,
                    pressure_iterations: None,
                    encoded_time_error_s: None,
                    readback_bytes,
                    attempted_candidates,
                    refinement_retries,
                    rejected_readback_bytes,
                }
            }
        })
    }

    /// Starts the fixed M3 scene again on the same canvas device.
    /// The new epoch distinguishes observations from the previous scene.
    pub fn reset(&mut self) -> Result<GpuBrowserProgress, String> {
        let next_epoch = self
            .scene
            .progress()
            .epoch
            .checked_add(1)
            .ok_or("GPU browser scene epoch counter exhausted")?;
        let scene = m3_demo_scene(self.surface.context(), next_epoch)?;
        self.scene = scene;
        Ok(self.progress())
    }
}
