//! Shaded triangle meshes, drawn instanced.
//!
//! Each mesh is uploaded once and stays on the GPU; a frame only sends one small record per drawn
//! instance (its placement and colour). Instances of the same mesh are drawn with a single call.

use std::collections::BTreeMap;

use bytemuck::{Pod, Zeroable};
use gc_mesh::MeshData;

use crate::{COLOR_FORMAT, DEPTH_FORMAT, MSAA_SAMPLES, ViewTarget};

/// One placement of an uploaded mesh.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Instance {
    /// Key the mesh was uploaded under.
    pub mesh: u64,
    /// Column-major matrix from the mesh's frame to eye-relative world coordinates.
    pub model: [[f32; 4]; 4],
    /// Linear RGBA; alpha below 1 draws translucent.
    pub color: [f32; 4],
}

/// A straight line of constant width on screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    /// Its ends in eye-relative world coordinates.
    pub a: [f32; 3],
    /// See `a`.
    pub b: [f32; 3],
    /// Linear RGBA.
    pub color: [f32; 4],
    /// Width in pixels.
    pub width: f32,
}

/// One end of a hairline edge: drawn one pixel wide by the hardware.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct EdgeVertex {
    /// Eye-relative position.
    pub position: [f32; 3],
    /// Linear RGBA.
    pub color: [f32; 4],
}

/// The camera and light of one frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    /// Column-major matrix from eye-relative world coordinates to clip space (reverse-Z).
    pub clip_from_relative: [[f32; 4]; 4],
    /// Direction the light travels.
    pub light: [f32; 3],
    /// Direction the camera looks along.
    pub forward: [f32; 3],
    /// A slicing plane `(normal, offset)` in eye-relative coordinates: what lies on the side the normal
    /// points to (`dot(normal, p) + offset > 0`) is not drawn. `None`: nothing is cut away.
    pub slice: Option<[f32; 4]>,
}

/// What a frame cost.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DrawStats {
    /// Draw calls issued.
    pub draw_calls: u32,
    /// Triangles submitted.
    pub triangles: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ViewUniform {
    clip_from_relative: [[f32; 4]; 4],
    light: [f32; 4],
    forward: [f32; 4],
    viewport: [f32; 4],
    slice: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct LineRaw {
    a: [f32; 3],
    b: [f32; 3],
    color: [f32; 4],
    width: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct InstanceRaw {
    model: [[f32; 4]; 4],
    color: [f32; 4],
}

struct GpuMesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
}

/// Draws meshes into a [`ViewTarget`].
pub struct MeshRenderer {
    opaque: wgpu::RenderPipeline,
    translucent: wgpu::RenderPipeline,
    view_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    meshes: BTreeMap<u64, GpuMesh>,
    instances: Option<(wgpu::Buffer, u64)>,
    lines: wgpu::RenderPipeline,
    line_buffer: Option<(wgpu::Buffer, u64)>,
    edges: wgpu::RenderPipeline,
    edge_buffer: Option<(wgpu::Buffer, u64)>,
}

impl MeshRenderer {
    /// Creates the pipelines.
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gc_render.mesh"),
            source: wgpu::ShaderSource::Wgsl(include_str!("mesh.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gc_render.mesh.view"),
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
        let view_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gc_render.mesh.view"),
            size: size_of::<ViewUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gc_render.mesh.view"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: view_buffer.as_entire_binding() }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gc_render.mesh"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |label, blend, depth_write| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[
                        Some(wgpu::VertexBufferLayout {
                            array_stride: 12,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![0 => Float32x3],
                        }),
                        Some(wgpu::VertexBufferLayout {
                            array_stride: size_of::<InstanceRaw>() as u64,
                            step_mode: wgpu::VertexStepMode::Instance,
                            attributes: &wgpu::vertex_attr_array![
                                1 => Float32x4, 2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4
                            ],
                        }),
                    ],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(depth_write),
                    depth_compare: Some(wgpu::CompareFunction::Greater),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState { count: MSAA_SAMPLES, ..Default::default() },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: COLOR_FORMAT,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let lines = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gc_render.lines"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_line"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<LineRaw>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4, 3 => Float32],
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState { count: MSAA_SAMPLES, ..Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_line"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: COLOR_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let edges = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gc_render.edges"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_edge"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<EdgeVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4],
                })],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState { count: MSAA_SAMPLES, ..Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_edge"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: COLOR_FORMAT,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Self {
            lines,
            line_buffer: None,
            edges,
            edge_buffer: None,
            opaque: pipeline("gc_render.mesh.opaque", None, true),
            translucent: pipeline(
                "gc_render.mesh.translucent",
                Some(wgpu::BlendState::ALPHA_BLENDING),
                false,
            ),
            view_buffer,
            bind_group,
            meshes: BTreeMap::new(),
            instances: None,
        }
    }

    /// True when a mesh was uploaded under `key`.
    pub fn has_mesh(&self, key: u64) -> bool {
        self.meshes.contains_key(&key)
    }

    /// Uploads a mesh under `key`, replacing what was there.
    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, key: u64, mesh: &MeshData) {
        let buffer = |label, bytes: &[u8], usage| {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: (bytes.len() as u64).next_multiple_of(4).max(4),
                usage: usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&buffer, 0, bytes);
            buffer
        };
        let gpu = GpuMesh {
            vertices: buffer(
                "gc_render.mesh.vertices",
                bytemuck::cast_slice(mesh.positions()),
                wgpu::BufferUsages::VERTEX,
            ),
            indices: buffer(
                "gc_render.mesh.indices",
                bytemuck::cast_slice(mesh.indices()),
                wgpu::BufferUsages::INDEX,
            ),
            index_count: u32::try_from(mesh.indices().len()).unwrap_or(u32::MAX),
        };
        self.meshes.insert(key, gpu);
    }

    /// Forgets every uploaded mesh whose key `keep` rejects.
    pub fn retain(&mut self, keep: impl Fn(u64) -> bool) {
        self.meshes.retain(|key, _| keep(*key));
    }

    /// Clears the target and draws the instances. Instances whose mesh was never uploaded are skipped.
    pub fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &ViewTarget,
        clear: [f64; 4],
        view: &View,
        instances: &[Instance],
        lines: &[Line],
        edges: &[EdgeVertex],
    ) -> DrawStats {
        let uniform = ViewUniform {
            clip_from_relative: view.clip_from_relative,
            light: [view.light[0], view.light[1], view.light[2], 0.0],
            forward: [view.forward[0], view.forward[1], view.forward[2], 0.0],
            viewport: [
                target.size().0 as f32,
                target.size().1 as f32,
                if view.slice.is_some() { 1.0 } else { 0.0 },
                0.0,
            ],
            slice: view.slice.unwrap_or([0.0; 4]),
        };
        // Lines: one small record each, drawn after the opaque solids and before the translucent ones.
        let line_records: Vec<LineRaw> =
            lines.iter().map(|l| LineRaw { a: l.a, b: l.b, color: l.color, width: l.width }).collect();
        let line_bytes: &[u8] = bytemuck::cast_slice(&line_records);
        if !line_bytes.is_empty()
            && self.line_buffer.as_ref().is_none_or(|(_, capacity)| *capacity < line_bytes.len() as u64)
        {
            let capacity = (line_bytes.len() as u64).next_power_of_two().max(4096);
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("gc_render.lines"),
                size: capacity,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.line_buffer = Some((buffer, capacity));
        }
        queue.write_buffer(&self.view_buffer, 0, bytemuck::bytes_of(&uniform));

        // Opaque instances first, then translucent ones; within each, instances of a mesh are adjacent.
        let mut order: Vec<&Instance> =
            instances.iter().filter(|i| self.meshes.contains_key(&i.mesh)).collect();
        order.sort_by_key(|i| (i.color[3] < 0.999, i.mesh));
        let raw: Vec<InstanceRaw> =
            order.iter().map(|i| InstanceRaw { model: i.model, color: i.color }).collect();
        let bytes: &[u8] = bytemuck::cast_slice(&raw);
        if self.instances.as_ref().is_none_or(|(_, capacity)| *capacity < bytes.len() as u64) {
            let capacity = (bytes.len() as u64).next_power_of_two().max(4096);
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("gc_render.mesh.instances"),
                size: capacity,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.instances = Some((buffer, capacity));
        }
        let mut stats = DrawStats::default();
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("gc_render.mesh") });
        {
            let mut pass = target
                .begin_pass(&mut encoder, wgpu::Color { r: clear[0], g: clear[1], b: clear[2], a: clear[3] });
            if let Some((buffer, _)) = self.instances.as_ref()
                && !order.is_empty()
            {
                queue.write_buffer(buffer, 0, bytes);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(1, buffer.slice(..));
                let mut first = 0_usize;
                while first < order.len() {
                    let (mesh, translucent) = (order[first].mesh, order[first].color[3] < 0.999);
                    let count = order[first..]
                        .iter()
                        .take_while(|i| i.mesh == mesh && (i.color[3] < 0.999) == translucent)
                        .count();
                    if let Some(gpu) = self.meshes.get(&mesh) {
                        pass.set_pipeline(if translucent { &self.translucent } else { &self.opaque });
                        pass.set_vertex_buffer(0, gpu.vertices.slice(..));
                        pass.set_index_buffer(gpu.indices.slice(..), wgpu::IndexFormat::Uint32);
                        let range = u32::try_from(first).unwrap_or(u32::MAX)
                            ..u32::try_from(first + count).unwrap_or(u32::MAX);
                        pass.draw_indexed(0..gpu.index_count, 0, range);
                        stats.draw_calls += 1;
                        stats.triangles += u64::from(gpu.index_count / 3) * count as u64;
                    }
                    first += count;
                }
            }
            // Edges, pairs of vertices, in one draw.
            let edge_bytes: &[u8] = bytemuck::cast_slice(edges);
            if !edge_bytes.is_empty()
                && self.edge_buffer.as_ref().is_none_or(|(_, capacity)| *capacity < edge_bytes.len() as u64)
            {
                let capacity = (edge_bytes.len() as u64).next_power_of_two().max(4096);
                let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("gc_render.edges"),
                    size: capacity,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                self.edge_buffer = Some((buffer, capacity));
            }
            if let Some((buffer, _)) = self.edge_buffer.as_ref()
                && edges.len() >= 2
            {
                queue.write_buffer(buffer, 0, edge_bytes);
                pass.set_pipeline(&self.edges);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..u32::try_from(edges.len() / 2 * 2).unwrap_or(u32::MAX), 0..1);
                stats.draw_calls += 1;
            }
            if let Some((buffer, _)) = self.line_buffer.as_ref()
                && !line_records.is_empty()
            {
                queue.write_buffer(buffer, 0, line_bytes);
                pass.set_pipeline(&self.lines);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..6, 0..u32::try_from(line_records.len()).unwrap_or(u32::MAX));
                stats.draw_calls += 1;
            }
        }
        queue.submit([encoder.finish()]);
        stats
    }
}
