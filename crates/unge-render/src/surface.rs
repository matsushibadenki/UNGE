use crate::{GpuRenderer, Scene};
use unge_core::Viewport;

/// Native surface owned by the Rust application, independent of any WebView.
pub struct SurfaceRenderer {
    surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: GpuRenderer,
    suspended: bool,
}
impl SurfaceRenderer {
    pub async fn new(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        size: [u32; 2],
    ) -> Result<Self, String> {
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(target).map_err(|e| e.to_string())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| e.to_string())?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .map_err(|e| e.to_string())?;
        if size
            .iter()
            .any(|v| *v > device.limits().max_texture_dimension_2d)
        {
            return Err("surface size exceeds GPU limits".into());
        }
        let config = surface
            .get_default_config(&adapter, size[0].max(1), size[1].max(1))
            .ok_or("surface has no supported configuration")?;
        surface.configure(&device, &config);
        let renderer = GpuRenderer::new(&device, config.format);
        Ok(Self {
            surface,
            device,
            queue,
            config,
            renderer,
            suspended: size.contains(&0),
        })
    }
    pub fn resize(&mut self, size: [u32; 2]) -> Result<(), String> {
        self.suspended = size.contains(&0);
        if self.suspended {
            return Ok(());
        }
        if size
            .iter()
            .any(|v| *v > self.device.limits().max_texture_dimension_2d)
        {
            return Err("surface size exceeds GPU limits".into());
        }
        if self.config.width != size[0] || self.config.height != size[1] {
            self.config.width = size[0];
            self.config.height = size[1];
            self.surface.configure(&self.device, &self.config);
        }
        Ok(())
    }
    /// Physical pixels are supplied by the host. Skip minimized and timeout frames.
    pub fn draw(&mut self, scene: &Scene, viewport: Viewport) -> Result<bool, String> {
        if self.suspended {
            return Ok(false);
        }
        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                return Ok(false);
            }
            Err(wgpu::SurfaceError::Timeout) => return Ok(false),
            Err(error) => return Err(error.to_string()),
        };
        self.renderer
            .prepare(&self.device, &self.queue, scene, viewport)
            .map_err(|e| e.to_string())?;
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.renderer.render(&mut encoder, &view);
        self.queue.submit([encoder.finish()]);
        frame.present();
        Ok(true)
    }
}
