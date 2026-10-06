//! Narrow browser lifecycle boundary; no JS-visible simulation arrays.

use particle_sim::Grid;
use particle_sim_cpu::ReferenceSession;
use particle_sim_gpu::GpuContext;
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
mod browser_scene;
#[cfg(target_arch = "wasm32")]
mod reporting;

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
