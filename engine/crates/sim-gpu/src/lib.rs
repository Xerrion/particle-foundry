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
        let adapter = request_adapter(&instance, None)
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

/// Prefer the device intended for sustained compute and canvas rendering.
pub(crate) async fn request_adapter(
    instance: &wgpu::Instance,
    surface: Option<&wgpu::Surface<'_>>,
) -> Result<wgpu::Adapter, wgpu::RequestAdapterError> {
    let options = wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: surface,
        ..Default::default()
    };
    let result = instance.request_adapter(&options).await;
    // Headless browser GPU startup can return null on the first request.
    // Retry once before reporting unavailable; never create a fallback scene.
    #[cfg(target_arch = "wasm32")]
    if result.is_err() {
        return instance.request_adapter(&options).await;
    }
    result
}
