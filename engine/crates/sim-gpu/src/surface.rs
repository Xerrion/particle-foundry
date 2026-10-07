//! Browser canvas presentation on the device that owns the GPU scene.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use web_sys::HtmlCanvasElement;

use crate::GpuContext;

/// A browser canvas surface and its single compatible GPU device and queue.
pub struct GpuSurface {
    context: GpuContext,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    needs_reconfigure: bool,
    device_lost: Arc<AtomicBool>,
}

impl GpuSurface {
    /// Creates a surface before requesting its compatible WebGPU adapter.
    ///
    /// The canvas backing dimensions must fit the device texture limit.
    /// A linear render target is required because the scene renderer already
    /// encodes display colors.
    pub async fn new(canvas: HtmlCanvasElement) -> Result<Self, String> {
        let width = canvas.width();
        let height = canvas.height();
        if width == 0 || height == 0 {
            return Err("GPU canvas dimensions must be positive".into());
        }

        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|error| format!("GPU canvas surface creation failed: {error}"))?;
        let adapter = crate::request_adapter(&instance, Some(&surface))
            .await
            .map_err(|error| format!("Compatible WebGPU canvas adapter unavailable: {error}"))?;
        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(|format| {
                !format.is_srgb()
                    && adapter
                        .get_texture_format_features(*format)
                        .allowed_usages
                        .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
            })
            .ok_or("GPU canvas has no supported non-sRGB color attachment format")?;
        let mut config = surface
            .get_default_config(&adapter, width, height)
            .ok_or("GPU canvas has no compatible surface configuration")?;
        config.format = format;

        let info = adapter.get_info();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("particle-foundry canvas"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            })
            .await
            .map_err(|error| format!("GPU canvas device initialization failed: {error}"))?;
        validate_dimensions(width, height, device.limits().max_texture_dimension_2d)?;

        let device_lost = Arc::new(AtomicBool::new(false));
        let callback_signal = Arc::clone(&device_lost);
        device.set_device_lost_callback(move |_, _| {
            callback_signal.store(true, Ordering::Release);
        });
        surface.configure(&device, &config);

        Ok(Self {
            context: GpuContext {
                device,
                queue,
                adapter: info,
            },
            surface,
            config,
            needs_reconfigure: false,
            device_lost,
        })
    }

    /// Returns the device and queue that must own the scene and its rendering.
    pub fn context(&self) -> &GpuContext {
        &self.context
    }

    /// Returns the supported linear color attachment format for the canvas.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    /// Returns the configured canvas backing dimensions in pixels.
    pub fn size(&self) -> [u32; 2] {
        [self.config.width, self.config.height]
    }

    /// Reconfigures the canvas backing size within the device texture limit.
    ///
    /// Any acquired frame must be presented or dropped before calling this.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
        self.ensure_device_alive()?;
        validate_dimensions(
            width,
            height,
            self.context.device.limits().max_texture_dimension_2d,
        )?;
        if self.size() == [width, height] {
            return Ok(());
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.context.device, &self.config);
        self.needs_reconfigure = false;
        Ok(())
    }

    /// Acquires one frame. Timeout or occlusion skips this frame.
    ///
    /// Submit rendering on `frame.context().queue` before `frame.present()`.
    /// The frame's borrow prevents a resize until it is presented or dropped.
    pub fn acquire_frame(&mut self) -> Result<Option<GpuFrame<'_>>, String> {
        self.ensure_device_alive()?;
        if self.needs_reconfigure {
            self.surface.configure(&self.context.device, &self.config);
            self.needs_reconfigure = false;
        }
        let current = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.context.device, &self.config);
                self.surface.get_current_texture()
            }
            current => current,
        };
        self.ensure_device_alive()?;
        let texture = match current {
            wgpu::CurrentSurfaceTexture::Success(texture) => texture,
            wgpu::CurrentSurfaceTexture::Suboptimal(texture) => {
                self.needs_reconfigure = true;
                texture
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(None);
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                return Err("GPU canvas remained outdated after reconfiguration".into());
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                return Err("GPU canvas surface was lost; recreate GpuSurface".into());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("GPU canvas frame acquisition failed validation".into());
            }
        };
        Ok(Some(GpuFrame {
            texture,
            surface: self,
        }))
    }

    fn ensure_device_alive(&self) -> Result<(), String> {
        if self.device_lost.load(Ordering::Acquire) {
            return Err("GPU canvas device was lost; recreate GpuSurface".into());
        }
        Ok(())
    }
}

fn validate_dimensions(width: u32, height: u32, max_dimension: u32) -> Result<(), String> {
    if width == 0 || height == 0 {
        return Err("GPU canvas dimensions must be positive".into());
    }
    if width > max_dimension || height > max_dimension {
        return Err(format!(
            "GPU canvas dimensions {width}x{height} exceed device limit {max_dimension}"
        ));
    }
    Ok(())
}

/// An acquired canvas frame that blocks resize until it is consumed.
pub struct GpuFrame<'a> {
    texture: wgpu::SurfaceTexture,
    surface: &'a mut GpuSurface,
}

impl GpuFrame<'_> {
    /// Returns the color texture for a render attachment view.
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture.texture
    }

    /// Returns the device and queue that own this frame and the GPU scene.
    pub fn context(&self) -> &GpuContext {
        &self.surface.context
    }

    /// Presents the frame after the caller submits its render commands.
    pub fn present(self) {
        self.texture.present();
    }
}
