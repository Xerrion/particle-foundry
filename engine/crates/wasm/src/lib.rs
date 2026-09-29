//! Narrow browser lifecycle boundary; no JS-visible simulation arrays.

use particle_sim::Grid;
use particle_sim_cpu::ReferenceSession;
use particle_sim_gpu::GpuContext;
use wasm_bindgen::prelude::*;

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
