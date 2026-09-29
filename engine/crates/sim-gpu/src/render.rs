//! Direct display of the committed, resident two-phase scene.
//!
//! The caller selects the committed mass buffer and submits the encoder. This
//! module never maps simulation state or advances the scene clock.

#![allow(dead_code)] // E08 scene and browser callers will use this renderer.

use particle_sim::Grid;
use wgpu::util::DeviceExt;

const RENDER_PARAMS_BYTES: u64 = 48;

/// Visible world rectangle in cell coordinates, with the origin at the top left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RenderViewport {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

impl RenderViewport {
    /// Shows the full grid at any target resolution.
    pub fn full(grid: Grid) -> Self {
        Self {
            left: 0.0,
            top: 0.0,
            width: grid.width() as f32,
            height: grid.height() as f32,
        }
    }

    fn validate(self, grid: Grid) -> Result<(), String> {
        if !self.left.is_finite()
            || !self.top.is_finite()
            || !self.width.is_finite()
            || !self.height.is_finite()
            || self.left < 0.0
            || self.top < 0.0
            || self.width <= 0.0
            || self.height <= 0.0
            || self.left + self.width > grid.width() as f32
            || self.top + self.height > grid.height() as f32
        {
            return Err("GPU render viewport must fit inside the scene grid".into());
        }
        Ok(())
    }
}

/// Resident inputs for one frame. The mass buffer has two f32 component planes:
/// liquid first, then carrier. Headers have one eight-byte entry per cell.
pub(crate) struct RenderScene<'a> {
    pub grid: Grid,
    pub phase_density_kg_m3: [f32; 2],
    pub headers: &'a wgpu::Buffer,
    pub committed_mass_kg: &'a wgpu::Buffer,
}

/// A render pipeline for one target texture format.
pub(crate) struct SceneRenderer {
    pipeline: wgpu::RenderPipeline,
}

impl SceneRenderer {
    /// Compiles the M3 liquid, carrier, and fixed-wall display shader.
    /// The target format must support color attachment usage on this device.
    /// The palette is encoded as display values, so an sRGB target would
    /// apply a second transfer function and change those colors.
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Result<Self, String> {
        if target_format.is_srgb() {
            return Err("GPU scene display needs a non-sRGB target format".into());
        }
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene display shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/render_scene.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("scene display pipeline"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Ok(Self { pipeline })
    }

    /// Encodes one full-target draw without submitting it or reading state back.
    ///
    /// The caller must supply a texture view matching `target_size_px` and the
    /// format given to `new`. `encoder` may already contain a simulation step.
    pub fn encode(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        target_size_px: [u32; 2],
        viewport: RenderViewport,
        scene: RenderScene<'_>,
    ) -> Result<(), String> {
        let params = render_params(device, target_size_px, viewport, &scene)?;
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene display inputs"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: scene.headers.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: scene.committed_mass_kg.as_entire_binding(),
                },
            ],
        });
        let color_attachment = wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("scene display"),
            color_attachments: &[Some(color_attachment)],
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
        Ok(())
    }
}

fn render_params(
    device: &wgpu::Device,
    target_size_px: [u32; 2],
    viewport: RenderViewport,
    scene: &RenderScene<'_>,
) -> Result<wgpu::Buffer, String> {
    viewport.validate(scene.grid)?;
    if target_size_px.contains(&0) {
        return Err("GPU render target dimensions must be positive".into());
    }
    let [liquid_density, carrier_density] = scene.phase_density_kg_m3;
    if !liquid_density.is_finite()
        || !carrier_density.is_finite()
        || liquid_density <= 0.0
        || carrier_density <= 0.0
    {
        return Err("GPU render phase densities must be finite and positive".into());
    }
    let cell_volume_m3 = scene.grid.cell_volume_m3() as f32;
    if !cell_volume_m3.is_finite() || cell_volume_m3 <= 0.0 {
        return Err("GPU render cell volume must be representable in f32".into());
    }
    let cells = scene.grid.cells() as u64;
    if scene.headers.size() < cells * 8 || scene.committed_mass_kg.size() < cells * 2 * 4 {
        return Err("GPU render input buffers are shorter than the scene grid".into());
    }
    if !scene.headers.usage().contains(wgpu::BufferUsages::STORAGE)
        || !scene
            .committed_mass_kg
            .usage()
            .contains(wgpu::BufferUsages::STORAGE)
    {
        return Err("GPU render input buffers need storage usage".into());
    }
    let limits = device.limits();
    if limits.max_uniform_buffer_binding_size < RENDER_PARAMS_BYTES
        || limits.max_storage_buffers_per_shader_stage < 2
        || limits.max_bindings_per_bind_group < 3
    {
        return Err("GPU adapter limits cannot render the scene".into());
    }

    // Three WGSL vec4 values. Encode fields explicitly to avoid Rust padding.
    let mut bytes = [0_u8; RENDER_PARAMS_BYTES as usize];
    let dimensions = [
        scene.grid.width(),
        scene.grid.height(),
        target_size_px[0],
        target_size_px[1],
    ];
    for (index, value) in dimensions.into_iter().enumerate() {
        bytes[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    let scalars = [
        viewport.left,
        viewport.top,
        viewport.width,
        viewport.height,
        liquid_density,
        carrier_density,
        cell_volume_m3,
        0.0,
    ];
    for (index, value) in scalars.into_iter().enumerate() {
        let offset = 16 + index * 4;
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    Ok(
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scene display parameters"),
            contents: &bytes,
            usage: wgpu::BufferUsages::UNIFORM,
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use particle_sim::gpu_layout::GpuCellHeader;
    use std::{
        future::Future,
        sync::Arc,
        task::{Context, Poll, Wake, Waker},
        time::{Duration, Instant},
    };

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
                    assert!(Instant::now() < deadline, "GPU render test timed out");
                    std::thread::park_timeout(Duration::from_millis(100));
                }
            }
        }
    }

    fn assert_rgb(actual: &[u8], expected: [u8; 3]) {
        for (channel, (&actual, expected)) in actual[..3].iter().zip(expected).enumerate() {
            assert!(
                actual.abs_diff(expected) <= 2,
                "channel {channel}: expected {expected}, got {actual}"
            );
        }
        assert_eq!(actual[3], 255);
    }

    fn render_and_read(
        gpu: &crate::GpuContext,
        renderer: &SceneRenderer,
        target_size_px: [u32; 2],
        viewport: RenderViewport,
        scene: RenderScene<'_>,
    ) -> Vec<u8> {
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scene display test target"),
            size: wgpu::Extent3d {
                width: target_size_px[0],
                height: target_size_px[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let row_bytes = (target_size_px[0] * 4).div_ceil(256) * 256;
        let staging = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene display test readback"),
            size: u64::from(row_bytes) * u64::from(target_size_px[1]),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("scene display test"),
            });
        renderer
            .encode(
                &gpu.device,
                &mut encoder,
                &view,
                target_size_px,
                viewport,
                scene,
            )
            .unwrap();
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &staging,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row_bytes),
                    rows_per_image: Some(target_size_px[1]),
                },
            },
            wgpu::Extent3d {
                width: target_size_px[0],
                height: target_size_px[1],
                depth_or_array_layers: 1,
            },
        );
        gpu.queue.submit([encoder.finish()]);
        let (sender, receiver) = std::sync::mpsc::channel();
        staging.map_async(wgpu::MapMode::Read, .., move |result| {
            sender.send(result).unwrap();
        });
        gpu.device
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
        let pixels = (0..target_size_px[1] as usize)
            .flat_map(|row| {
                let start = row * row_bytes as usize;
                mapped[start..start + target_size_px[0] as usize * 4].to_vec()
            })
            .collect();
        drop(mapped);
        staging.unmap();
        pixels
    }

    #[test]
    fn rejects_invalid_viewport() {
        let grid = Grid::new(3.0, 1.0).unwrap();
        assert!(RenderViewport::full(grid).validate(grid).is_ok());
        assert!(
            RenderViewport {
                left: 2.0,
                top: 0.0,
                width: 2.0,
                height: 1.0,
            }
            .validate(grid)
            .is_err()
        );
    }

    #[test]
    #[ignore = "requires a native GPU render adapter"]
    fn native_scene_render_samples_committed_mass_and_wall_headers() {
        let gpu = block_on(crate::GpuContext::new()).unwrap();
        let grid = Grid::new(3.0, 1.0).unwrap();
        let volume = grid.cell_volume_m3() as f32;
        assert!(SceneRenderer::new(&gpu.device, wgpu::TextureFormat::Rgba8UnormSrgb).is_err());
        let renderer = SceneRenderer::new(&gpu.device, wgpu::TextureFormat::Rgba8Unorm).unwrap();
        let header_bytes = [0_u8, 0, 1]
            .into_iter()
            .flat_map(|wall| GpuCellHeader::new(0.0, wall).unwrap().to_le_bytes())
            .collect::<Vec<_>>();
        let masses = [1000.0 * volume, 0.0, 0.0, 0.0, 1.0 * volume, 0.0];
        let mass_bytes = masses
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect::<Vec<_>>();
        let headers = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("render fixture headers"),
                contents: &header_bytes,
                usage: wgpu::BufferUsages::STORAGE,
            });
        let committed_mass_kg = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("render fixture committed mass"),
                contents: &mass_bytes,
                usage: wgpu::BufferUsages::STORAGE,
            });
        let scene = || RenderScene {
            grid,
            phase_density_kg_m3: [1000.0, 1.0],
            headers: &headers,
            committed_mass_kg: &committed_mass_kg,
        };

        let first = render_and_read(&gpu, &renderer, [6, 1], RenderViewport::full(grid), scene());
        for pixel in 0..2 {
            assert_rgb(&first[pixel * 4..pixel * 4 + 4], [35, 116, 203]);
        }
        for pixel in 2..4 {
            assert_rgb(&first[pixel * 4..pixel * 4 + 4], [8, 18, 27]);
        }
        for pixel in 4..6 {
            assert_rgb(&first[pixel * 4..pixel * 4 + 4], [89, 94, 99]);
        }

        // A paused scene can redraw the same committed generation unchanged.
        let repeated =
            render_and_read(&gpu, &renderer, [6, 1], RenderViewport::full(grid), scene());
        assert_eq!(repeated, first);

        let zoomed = render_and_read(
            &gpu,
            &renderer,
            [2, 1],
            RenderViewport {
                left: 1.0,
                top: 0.0,
                width: 2.0,
                height: 1.0,
            },
            scene(),
        );
        assert_rgb(&zoomed[..4], [8, 18, 27]);
        assert_rgb(&zoomed[4..8], [89, 94, 99]);
    }
}
