//! Narrow browser lifecycle boundary; no JS-visible simulation arrays.

use particle_sim::Grid;
use particle_sim_cpu::ReferenceSession;
use particle_sim_gpu::GpuContext;
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
mod browser_scene {
    use std::{cell::RefCell, rc::Rc};

    use particle_sim_gpu::{
        GpuBrowserAdvance, GpuBrowserProbe, GpuBrowserProgress, GpuBrowserScene,
    };
    use wasm_bindgen::prelude::*;

    struct State {
        owner: Option<GpuBrowserScene>,
        latest: GpuBrowserProgress,
        backend: String,
        disposed: bool,
        reset_pending: bool,
    }

    /// Owns one experimental browser GPU scene and its canvas surface.
    /// Async operations take the owner out before returning a Promise, so a
    /// synchronous render, reset or dispose never holds a wasm-bindgen borrow.
    #[wasm_bindgen]
    pub struct BrowserGpuScene {
        state: Rc<RefCell<State>>,
    }

    impl Drop for BrowserGpuScene {
        fn drop(&mut self) {
            self.dispose();
        }
    }

    #[wasm_bindgen]
    impl BrowserGpuScene {
        /// Returns the adapter selected for this canvas and scene.
        pub fn backend(&self) -> Result<String, JsValue> {
            let state = self.state.borrow();
            if state.disposed {
                return Err(JsValue::from_str("GPU browser scene disposed"));
            }
            Ok(state.backend.clone())
        }

        /// Renders only the latest committed generation. A busy owner leaves
        /// the previous canvas frame in place and reports false.
        pub fn render(&self) -> Result<bool, JsValue> {
            let mut state = self.state.borrow_mut();
            if state.disposed {
                return Err(JsValue::from_str("GPU browser scene disposed"));
            }
            match state.owner.as_mut() {
                Some(owner) => owner.render().map_err(|error| JsValue::from_str(&error)),
                None => Ok(false),
            }
        }

        /// Resizes the canvas backing target. The caller can retry after an
        /// in-flight advance settles; no surface is reconfigured mid-frame.
        pub fn resize(&self, width: JsValue, height: JsValue) -> Result<(), JsValue> {
            let width = checked_u32(width, "GPU canvas width", u32::MAX)?;
            let height = checked_u32(height, "GPU canvas height", u32::MAX)?;
            let mut state = self.state.borrow_mut();
            if state.disposed {
                return Err(JsValue::from_str("GPU browser scene disposed"));
            }
            state
                .owner
                .as_mut()
                .ok_or_else(|| JsValue::from_str("GPU browser scene busy"))?
                .resize(width, height)
                .map_err(|error| JsValue::from_str(&error))
        }

        /// Applies one bounded water or air brush while the scene owner is idle.
        /// The commit changes resident GPU state without advancing model time.
        pub fn paint(
            &self,
            center_x: JsValue,
            center_y: JsValue,
            radius: JsValue,
            material: JsValue,
        ) -> Result<String, JsValue> {
            let center_x = checked_nonnegative_u32(center_x, "GPU paint x", u32::MAX)?;
            let center_y = checked_nonnegative_u32(center_y, "GPU paint y", u32::MAX)?;
            let radius = checked_nonnegative_u32(radius, "GPU paint radius", 16)?;
            let material = material
                .as_string()
                .ok_or_else(|| JsValue::from_str("GPU paint material must be water or air"))?;
            let mut state = self.state.borrow_mut();
            if state.disposed {
                return Err(JsValue::from_str("GPU browser scene disposed"));
            }
            let progress = state
                .owner
                .as_mut()
                .ok_or_else(|| JsValue::from_str("GPU browser scene busy"))?
                .paint(center_x, center_y, radius, &material)
                .map_err(|error| JsValue::from_str(&error))?;
            state.latest = progress;
            Ok(format_progress(progress, false, false))
        }

        /// Returns a committed-only stamp. During an advance, this is the
        /// last settled stamp and busy is true.
        pub fn status_json(&self) -> Result<String, JsValue> {
            let state = self.state.borrow();
            if state.disposed {
                return Err(JsValue::from_str("GPU browser scene disposed"));
            }
            Ok(format_progress(
                state.latest,
                state.owner.is_none(),
                state.reset_pending,
            ))
        }

        /// Runs a bounded batch and returns a Promise containing completed
        /// work, pressure diagnostics and the resulting committed stamp.
        pub fn advance(&self, max_substeps: JsValue) -> Result<JsValue, JsValue> {
            let max_substeps = checked_u32(max_substeps, "GPU substep budget", 8)?;
            let state = Rc::clone(&self.state);
            let mut owner = {
                let mut current = state.borrow_mut();
                if current.disposed {
                    return Err(JsValue::from_str("GPU browser scene disposed"));
                }
                current
                    .owner
                    .take()
                    .ok_or_else(|| JsValue::from_str("GPU browser scene busy"))?
            };
            Ok(wasm_bindgen_futures::future_to_promise(async move {
                let result = owner.advance(max_substeps).await;
                let mut current = state.borrow_mut();
                if current.disposed {
                    return Err(JsValue::from_str("GPU browser scene disposed"));
                }
                if current.reset_pending {
                    current.reset_pending = false;
                    let reset = owner.reset();
                    current.latest = owner.progress();
                    current.owner = Some(owner);
                    return Err(JsValue::from_str(&match reset {
                        Ok(_) => "GPU browser scene reset during advance".to_owned(),
                        Err(error) => format!("GPU browser scene reset failed: {error}"),
                    }));
                }
                current.latest = owner.progress();
                current.owner = Some(owner);
                result
                    .map(|advance| JsValue::from_str(&format_advance(advance)))
                    .map_err(|error| JsValue::from_str(&error))
            })
            .into())
        }

        /// Asynchronously samples one committed cell. A reset during the map
        /// rejects the old result after starting the requested new epoch.
        pub fn probe(&self, x: JsValue, y: JsValue) -> Result<JsValue, JsValue> {
            let x = checked_nonnegative_u32(x, "GPU probe x", u32::MAX)?;
            let y = checked_nonnegative_u32(y, "GPU probe y", u32::MAX)?;
            let state = Rc::clone(&self.state);
            let owner = {
                let mut current = state.borrow_mut();
                if current.disposed {
                    return Err(JsValue::from_str("GPU browser scene disposed"));
                }
                current
                    .owner
                    .take()
                    .ok_or_else(|| JsValue::from_str("GPU browser scene busy"))?
            };
            Ok(wasm_bindgen_futures::future_to_promise(async move {
                let result = owner.probe(x, y).await;
                let mut current = state.borrow_mut();
                if current.disposed {
                    return Err(JsValue::from_str("GPU browser scene disposed"));
                }
                if current.reset_pending {
                    current.reset_pending = false;
                    let mut owner = owner;
                    let reset = owner.reset();
                    current.latest = owner.progress();
                    current.owner = Some(owner);
                    return Err(JsValue::from_str(&match reset {
                        Ok(_) => "GPU browser scene reset during probe".to_owned(),
                        Err(error) => format!("GPU browser scene reset failed: {error}"),
                    }));
                }
                current.latest = owner.progress();
                current.owner = Some(owner);
                result
                    .map(|sample| JsValue::from_str(&format_probe(sample)))
                    .map_err(|error| JsValue::from_str(&error))
            })
            .into())
        }

        /// Explicitly captures a detached committed generation. PFCP version
        /// zero is a diagnostic prototype, not a durable recovery snapshot.
        pub fn checkpoint_prototype(&self) -> Result<JsValue, JsValue> {
            let state = Rc::clone(&self.state);
            let owner = {
                let mut current = state.borrow_mut();
                if current.disposed {
                    return Err(JsValue::from_str("GPU browser scene disposed"));
                }
                current
                    .owner
                    .take()
                    .ok_or_else(|| JsValue::from_str("GPU browser scene busy"))?
            };
            Ok(wasm_bindgen_futures::future_to_promise(async move {
                let result = owner.checkpoint_prototype().await;
                let mut current = state.borrow_mut();
                if current.disposed {
                    return Err(JsValue::from_str("GPU browser scene disposed"));
                }
                if current.reset_pending {
                    current.reset_pending = false;
                    let mut owner = owner;
                    let reset = owner.reset();
                    current.latest = owner.progress();
                    current.owner = Some(owner);
                    return Err(JsValue::from_str(&match reset {
                        Ok(_) => "GPU browser scene reset during checkpoint".to_owned(),
                        Err(error) => format!("GPU browser scene reset failed: {error}"),
                    }));
                }
                current.latest = owner.progress();
                current.owner = Some(owner);
                result
                    .map(|bytes| JsValue::from(js_sys::Uint8Array::from(bytes.as_slice())))
                    .map_err(|error| JsValue::from_str(&error))
            })
            .into())
        }

        /// Starts a fresh epoch on the same canvas device. If a batch is in
        /// flight, its reply is invalidated and the reset runs when it settles.
        pub fn reset(&self) -> Result<String, JsValue> {
            let mut state = self.state.borrow_mut();
            if state.disposed {
                return Err(JsValue::from_str("GPU browser scene disposed"));
            }
            if let Some(owner) = state.owner.as_mut() {
                let progress = owner.reset().map_err(|error| JsValue::from_str(&error))?;
                state.latest = progress;
            } else {
                state.reset_pending = true;
            }
            Ok(format_progress(
                state.latest,
                state.owner.is_none(),
                state.reset_pending,
            ))
        }

        /// Idempotent teardown. An in-flight batch releases its owner when it
        /// settles, without destroying another cloned device handle.
        pub fn dispose(&self) {
            let mut state = self.state.borrow_mut();
            state.disposed = true;
            state.owner = None;
        }
    }

    /// Creates the experimental 480 x 270 GPU scene on a canvas-compatible device.
    #[wasm_bindgen]
    pub async fn create_browser_gpu_scene(
        canvas: web_sys::HtmlCanvasElement,
    ) -> Result<BrowserGpuScene, JsValue> {
        let owner = GpuBrowserScene::new(canvas)
            .await
            .map_err(|error| JsValue::from_str(&error))?;
        let latest = owner.progress();
        let backend = owner.backend();
        Ok(BrowserGpuScene {
            state: Rc::new(RefCell::new(State {
                owner: Some(owner),
                latest,
                backend,
                disposed: false,
                reset_pending: false,
            })),
        })
    }

    fn format_progress(progress: GpuBrowserProgress, busy: bool, reset_pending: bool) -> String {
        format!(
            "{{\"epoch\":{},\"tick\":{},\"acceptedTimeS\":{},\"remainingOuterS\":{},\"committedGeneration\":{},\"stateRevision\":{},\"busy\":{},\"resetPending\":{}}}",
            progress.epoch,
            progress.tick,
            progress.accepted_time_s,
            progress.remaining_outer_s,
            progress.committed_generation,
            progress.state_revision,
            busy,
            reset_pending,
        )
    }

    fn checked_u32(input: JsValue, label: &str, maximum: u32) -> Result<u32, JsValue> {
        let Some(value) = input.as_f64() else {
            return Err(JsValue::from_str(&format!("{label} must be an integer")));
        };
        if !value.is_finite() || value.fract() != 0.0 || value < 1.0 || value > f64::from(maximum) {
            return Err(JsValue::from_str(&format!(
                "{label} must be an integer between 1 and {maximum}"
            )));
        }
        Ok(value as u32)
    }

    fn checked_nonnegative_u32(input: JsValue, label: &str, maximum: u32) -> Result<u32, JsValue> {
        let Some(value) = input.as_f64() else {
            return Err(JsValue::from_str(&format!("{label} must be an integer")));
        };
        if !value.is_finite() || value.fract() != 0.0 || value < 0.0 || value > f64::from(maximum) {
            return Err(JsValue::from_str(&format!(
                "{label} must be an integer between 0 and {maximum}"
            )));
        }
        Ok(value as u32)
    }

    fn format_advance(advance: GpuBrowserAdvance) -> String {
        fn optional<T: std::fmt::Display>(value: Option<T>) -> String {
            value.map_or_else(|| "null".to_owned(), |value| value.to_string())
        }
        format!(
            "{{\"acceptedSubsteps\":{},\"completedOuterTick\":{},\"epoch\":{},\"tick\":{},\"acceptedTimeS\":{},\"remainingOuterS\":{},\"committedGeneration\":{},\"stateRevision\":{},\"scaledResidual\":{},\"scaledDivergence\":{},\"pressureIterations\":{},\"encodedTimeErrorS\":{},\"readbackBytes\":{}}}",
            advance.accepted_substeps,
            advance.completed_outer_tick,
            advance.progress.epoch,
            advance.progress.tick,
            advance.progress.accepted_time_s,
            advance.progress.remaining_outer_s,
            advance.progress.committed_generation,
            advance.progress.state_revision,
            optional(advance.scaled_residual),
            optional(advance.scaled_divergence),
            optional(advance.pressure_iterations),
            optional(advance.encoded_time_error_s),
            advance.readback_bytes,
        )
    }

    fn format_probe(sample: GpuBrowserProbe) -> String {
        format!(
            "{{\"x\":{},\"y\":{},\"liquidMassKg\":{},\"carrierMassKg\":{},\"fixedWall\":{},\"epoch\":{},\"tick\":{},\"acceptedTimeS\":{},\"committedGeneration\":{},\"stateRevision\":{}}}",
            sample.cell[0],
            sample.cell[1],
            sample.mass_kg[0],
            sample.mass_kg[1],
            sample.fixed_wall,
            sample.progress.epoch,
            sample.progress.tick,
            sample.progress.accepted_time_s,
            sample.progress.committed_generation,
            sample.progress.state_revision,
        )
    }
}

#[cfg(target_arch = "wasm32")]
pub use browser_scene::*;

/// Exactly one selected lifecycle owner. GPU construction never creates a CPU world.
enum Owner {
    Cpu(Box<ReferenceSession>),
    Gpu(GpuContext),
}

/// Browser engine bootstrap. Numerical stepping remains unavailable until its gates pass.
#[wasm_bindgen]
pub struct Engine {
    owner: Option<Owner>,
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.dispose();
    }
}

#[wasm_bindgen]
impl Engine {
    /// Returns the actual execution path. This does not assert fluid support.
    pub fn backend(&self) -> Result<String, JsValue> {
        match self.owner.as_ref() {
            Some(Owner::Cpu(reference)) => {
                let _geometry = reference.grid();
                Ok("cpu-reference-bootstrap".into())
            }
            Some(Owner::Gpu(context)) => Ok(context.backend()),
            None => Err(JsValue::from_str("engine disposed")),
        }
    }

    /// Runs the Rust/WGSL layout sentinel on this browser GPU device.
    /// A successful report means the shader readback matched the Rust-packed bytes.
    pub fn validate_gpu_abi(&self) -> Result<JsValue, JsValue> {
        // Release the wasm-bindgen Engine borrow before JS can dispose it during this Promise.
        let context = match self.owner.as_ref() {
            Some(Owner::Gpu(context)) => context.clone(),
            Some(Owner::Cpu(_)) => {
                return Err(JsValue::from_str(
                    "GPU ABI validation requires a GPU engine",
                ));
            }
            None => return Err(JsValue::from_str("engine disposed")),
        };
        Ok(wasm_bindgen_futures::future_to_promise(async move {
            let report = context
                .validate_abi()
                .await
                .map_err(|error| JsValue::from_str(&error))?;
            Ok(JsValue::from_str(&format!(
                "{{\"backend\":\"{}\",\"maxStorageBufferBindingSize\":{},\"maxBufferSize\":{},\"maxComputeWorkgroupsPerDimension\":{},\"maxComputeInvocationsPerWorkgroup\":{},\"sentinelBytes\":{},\"coreFieldBytes\":{}}}",
                report.backend,
                report.max_storage_buffer_binding_size,
                report.max_buffer_size,
                report.max_compute_workgroups_per_dimension,
                report.max_compute_invocations_per_workgroup,
                report.sentinel_bytes,
                report.core_field_bytes,
            )))
        })
        .into())
    }

    /// Runs one small GPU scene tick and checks that a rejected candidate kept the committed state.
    /// This fixture does not advance a live browser scene.
    pub fn validate_gpu_scene_fixture(&self) -> Result<JsValue, JsValue> {
        // The Promise owns its GPU context so JS can dispose the Engine while it runs.
        let context = match self.owner.as_ref() {
            Some(Owner::Gpu(context)) => context.clone(),
            Some(Owner::Cpu(_)) => {
                return Err(JsValue::from_str(
                    "GPU scene validation requires a GPU engine",
                ));
            }
            None => return Err(JsValue::from_str("engine disposed")),
        };
        Ok(wasm_bindgen_futures::future_to_promise(async move {
            let report = context
                .validate_scene_fixture()
                .await
                .map_err(|error| JsValue::from_str(&error))?;
            Ok(JsValue::from_str(&format!(
                "{{\"backend\":\"{}\",\"epoch\":{},\"tick\":{},\"acceptedTimeS\":{},\"acceptedSubsteps\":{},\"scaledResidual\":{},\"scaledDivergence\":{},\"encodedTimeErrorS\":{},\"rejectionKeptCommittedState\":{}}}",
                report.backend,
                report.epoch,
                report.tick,
                report.accepted_time_s,
                report.accepted_substeps,
                report.scaled_residual,
                report.scaled_divergence,
                report.encoded_time_error_s,
                report.rejection_kept_committed_state,
            )))
        })
        .into())
    }

    /// Stops ownership once. Repeated disposal is safe.
    pub fn dispose(&mut self) {
        if let Some(Owner::Gpu(context)) = self.owner.take() {
            context.dispose();
        }
    }
}

/// Validates dimensions before selecting one owner; GPU failure is an explicit rejection.
#[wasm_bindgen]
pub async fn initialize(width: f64, height: f64, execution: String) -> Result<Engine, JsValue> {
    let grid = Grid::new(width, height).map_err(JsValue::from_str)?;
    let owner = match execution.as_str() {
        "cpu-reference" => Owner::Cpu(Box::new(ReferenceSession::new(grid))),
        "wgpu" => Owner::Gpu(
            GpuContext::new()
                .await
                .map_err(|error| JsValue::from_str(&error))?,
        ),
        _ => return Err(JsValue::from_str("unsupported execution backend")),
    };
    Ok(Engine { owner: Some(owner) })
}
