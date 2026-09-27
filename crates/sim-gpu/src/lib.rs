//! One wgpu device/queue for a future resident world and its renderer.
//!
//! Device availability is not numerical or supported-scene acceptance.

/// Owns GPU resources without a ticking CPU world mirror.
pub struct GpuContext {
    device: wgpu::Device,
    _queue: wgpu::Queue,
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
            _queue: queue,
            adapter: info,
        })
    }

    /// Actual execution backend reported by the selected adapter.
    pub fn backend(&self) -> String {
        format!("{:?}", self.adapter.backend)
    }

    /// Explicitly destroys the device during browser teardown.
    pub fn dispose(self) {
        self.device.destroy();
    }
}
