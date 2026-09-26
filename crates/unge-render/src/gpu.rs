use crate::{Quad, Scene};
use unge_core::Viewport;
use wgpu::util::DeviceExt;

pub struct GpuRenderer {
    pipeline: wgpu::RenderPipeline,
    camera: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    instances: wgpu::Buffer,
    capacity: usize,
    count: u32,
}
impl GpuRenderer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let camera = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("UNGE camera"),
            contents: bytemuck::cast_slice(&[0.0f32; 8]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("UNGE graph"),
            source: wgpu::ShaderSource::Wgsl(include_str!("graph.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("UNGE instanced graph"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Quad>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![0=>Float32x4,1=>Float32x4,2=>Float32x4],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let capacity = 256;
        let instances = Self::buffer(device, capacity);
        Self {
            pipeline,
            camera,
            bind_group,
            instances,
            capacity,
            count: 0,
        }
    }
    fn buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("UNGE instance buffer"),
            size: (capacity * std::mem::size_of::<Quad>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &Scene,
        viewport: Viewport,
    ) -> unge_core::Result<()> {
        viewport.validate()?;
        let bytes = scene
            .quads
            .len()
            .checked_mul(std::mem::size_of::<Quad>())
            .ok_or_else(|| unge_core::Error::Invalid("scene too large".into()))?;
        if bytes as u64 > device.limits().max_buffer_size || scene.quads.len() > u32::MAX as usize {
            return Err(unge_core::Error::Invalid(
                "scene exceeds GPU buffer limit".into(),
            ));
        }
        if scene.quads.len() > self.capacity {
            self.capacity = scene
                .quads
                .len()
                .next_power_of_two()
                .min((device.limits().max_buffer_size as usize) / std::mem::size_of::<Quad>());
            self.instances = Self::buffer(device, self.capacity);
        }
        self.count = scene.quads.len() as u32;
        if self.count > 0 {
            queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&scene.quads));
        }
        queue.write_buffer(
            &self.camera,
            0,
            bytemuck::cast_slice(&[
                viewport.origin[0],
                viewport.origin[1],
                viewport.zoom,
                0.0,
                viewport.size[0],
                viewport.size[1],
                0.0,
                0.0,
            ]),
        );
        Ok(())
    }
    /// One draw call for all visible geometry. Text is an optional host overlay pass.
    pub fn render(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("UNGE graph pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.04,
                        g: 0.052,
                        b: 0.075,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.instances.slice(..));
        pass.draw(0..6, 0..self.count);
    }
}
