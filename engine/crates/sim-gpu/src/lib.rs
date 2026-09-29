//! One wgpu device/queue for a future resident world and its renderer.
//!
//! Device availability is not numerical or supported-scene acceptance.

mod abi;
#[cfg(target_arch = "wasm32")]
mod browser_scene;
mod cfl;
mod checkpoint;
mod density;
mod gravity;
mod momentum;
mod pressure;
mod probe;
mod render;
mod scene;
#[cfg(target_arch = "wasm32")]
mod surface;
pub mod transport;
pub mod viscosity;

pub use abi::{GpuAbiReport, GpuCoreFieldPlan};
#[cfg(target_arch = "wasm32")]
pub use browser_scene::{GpuBrowserAdvance, GpuBrowserProbe, GpuBrowserProgress, GpuBrowserScene};
pub use scene::GpuSceneValidationReport;
#[cfg(target_arch = "wasm32")]
pub use surface::{GpuFrame, GpuSurface};

/// Owns GPU resources without a ticking CPU world mirror.
#[derive(Clone)]
pub struct GpuContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter: wgpu::AdapterInfo,
}

impl GpuContext {
    /// Requests baseline compute support. Never substitutes WebGL or a different model.
    pub async fn new() -> Result<Self, String> {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .map_err(|error| format!("WebGPU/native compute adapter unavailable: {error}"))?;
        let info = adapter.get_info();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("particle-foundry"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            })
            .await
            .map_err(|error| format!("Compute device initialization failed: {error}"))?;
        Ok(Self {
            device,
            queue,
            adapter: info,
        })
    }

    /// Actual execution backend reported by the selected adapter.
    pub fn backend(&self) -> String {
        format!("{:?}", self.adapter.backend)
    }

    /// Qualifies one small closed GPU fluid scene on this actual device.
    /// This experimental E07 fixture does not promote the live sandbox.
    pub async fn validate_scene_fixture(&self) -> Result<GpuSceneValidationReport, String> {
        scene::validate_scene_fixture(self).await
    }

    /// Releases this owner. Other context clones and scene owners stay usable.
    pub fn dispose(self) {
        drop(self);
    }
}
