use std::{borrow::Borrow, hash::Hash};

use indexmap::IndexMap;
use mavelin_engine::WindowContext;
use mavelin_shared::{AsValue, Color, Cube3D, Face, Frustum, IPoint2D, IPoint3D, Point2D, Point3D, Transform3D};
use mavelin_world::{SUBCHUNK_SIZE, SUBCHUNK_SIZE_F32, SUBCHUNK_SIZE_I32};
use wgpu::util::DeviceExt;

use super::RenderBuffer;
use crate::render::{RenderInfo, RenderShape};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoxelFace {
    pub position: Point3D,
    pub vertices: [Point3D; 4],
    pub uvs: [Point2D; 4],
    pub lights: [u8; 4],
    pub color: Color,
}

#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct VoxelVertex {
    pub position: Point3D,
    pub uv: Point2D,
    pub color: [u8; 4],
    pub light: u32,
}

impl VoxelVertex {
    const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Uint8x4, 3 => Uint32],
    };
}

#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct CloudVertex {
    pub position: Point3D,
    pub half_size: Point3D,
    pub color: [u8; 4],
    // pub _pad: u32
}

impl CloudVertex {
    const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Uint8x4],
    };
}

pub struct TranslucentSubchunk {
    buffer: RenderBuffer,
    faces: Vec<VoxelFace>,
    last_pos: Point3D,
}

impl TranslucentSubchunk {
    pub fn new(device: &wgpu::Device, mut faces: Vec<VoxelFace>, last_pos: Point3D, origin: IPoint2D) -> Self {
        Self::resort_faces(&mut faces, last_pos, origin);

        Self {
            buffer: VoxelMeshBuilder::build_dynamic_from_slice(device, &faces),
            faces,
            last_pos,
        }
    }

    fn update(&mut self, queue: &wgpu::Queue, last_pos: Point3D, origin: IPoint2D) {
        if self.last_pos.distance_squared(last_pos) > 2.0 && !self.faces.is_empty() {
            Self::resort_faces(&mut self.faces, last_pos, origin);

            let mut builder = VoxelMeshBuilder::new();

            builder.extend_from_slice(&self.faces);

            queue.write_buffer(&self.buffer.vertices, 0, bytemuck::cast_slice(&builder.vertices));
            queue.write_buffer(&self.buffer.indices, 0, bytemuck::cast_slice(&builder.indices));

            self.last_pos = last_pos;
        }
    }

    fn resort_faces(faces: &mut [VoxelFace], last_pos: Point3D, origin: IPoint2D) {
        let origin = origin.as_vec2() * SUBCHUNK_SIZE_F32;
        let local_camera_pos = last_pos - Point3D::new(origin.x, 0.0, origin.y);

        faces.sort_unstable_by(|a, b| {
            local_camera_pos
                .distance_squared(b.position)
                .total_cmp(&local_camera_pos.distance_squared(a.position))
        });
    }
}

struct RenderSubchunk {
    solid: RenderBuffer,
    translucent: TranslucentSubchunk,
}

pub struct VoxelMeshBuilder {
    vertices: Vec<VoxelVertex>,
    indices: Vec<u32>,
    offset: u32,
}

impl VoxelMeshBuilder {
    #[inline]
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            vertices: Vec::with_capacity(capacity * 4),
            indices: Vec::with_capacity(capacity * 6),
            offset: 0,
        }
    }

    #[inline]
    pub fn build_from_slice(device: &wgpu::Device, voxels: &[VoxelFace]) -> RenderBuffer {
        let mut this = Self::new();

        this.extend_from_slice(voxels);

        this.build(device)
    }

    #[inline]
    pub fn build_dynamic_from_slice(device: &wgpu::Device, voxels: &[VoxelFace]) -> RenderBuffer {
        let mut this = Self::new();

        this.extend_from_slice(voxels);

        this.build_dynamic(device)
    }

    #[inline]
    pub fn extend_from_slice(&mut self, voxels: &[VoxelFace]) {
        for voxel in voxels {
            self.push(voxel);
        }
    }

    #[inline]
    fn push_indices(&mut self) {
        let offset = self.offset;

        self.offset += 4;
        self.indices.extend([offset, offset + 1, offset + 2, offset + 3, offset + 2, offset + 1]);
    }

    #[inline]
    pub fn push_transformed(&mut self, voxel: &VoxelFace, matrix: &Transform3D, origin: Point3D) {
        let color = voxel.color.as_value();

        self.vertices.extend((0..4).map(|i| VoxelVertex {
            position: voxel.position + matrix.transform_point3(voxel.vertices[i] - origin) + origin,
            light: voxel.lights[i].into(),
            uv: voxel.uvs[i],
            color,
        }));

        self.push_indices();
    }

    #[inline]
    pub fn push(&mut self, voxel: &VoxelFace) {
        let color = voxel.color.as_value();

        self.vertices.extend((0..4).map(|i| VoxelVertex {
            position: voxel.position + voxel.vertices[i],
            light: voxel.lights[i].into(),
            uv: voxel.uvs[i],
            color,
        }));

        self.push_indices();
    }

    #[inline]
    pub fn render_full_bright(
        self,
        context: &WindowContext,
        render_pass: &mut wgpu::RenderPass,
        renderer: &mut ChunkRenderer,
        matrix: Transform3D,
    ) -> RenderInfo {
        let buffer = self.build(context.device);

        renderer.render_buffer(context.queue, render_pass, &buffer, matrix, true)
    }

    #[inline]
    pub fn render(self, context: &WindowContext, render_pass: &mut wgpu::RenderPass, renderer: &mut ChunkRenderer, matrix: Transform3D) -> RenderInfo {
        let buffer = self.build(context.device);

        renderer.render_buffer(context.queue, render_pass, &buffer, matrix, false)
    }

    #[inline]
    pub fn build(self, device: &wgpu::Device) -> RenderBuffer {
        RenderBuffer::new(device, &self.vertices, &self.indices)
    }

    #[inline]
    pub fn build_dynamic(self, device: &wgpu::Device) -> RenderBuffer {
        RenderBuffer::new(device, &self.vertices, &self.indices)
    }
}

pub struct ChunkRenderer {
    solid_render_pipeline: wgpu::RenderPipeline,
    translucent_render_pipeline: wgpu::RenderPipeline,
    cloud_render_pipeline: wgpu::RenderPipeline,
    fog_bind_group: wgpu::BindGroup,

    voxel_bind_group: wgpu::BindGroup,
    voxel: VoxelUniform,
    voxel_buffer: wgpu::Buffer,

    fragment_bind_group: wgpu::BindGroup,
    fog: FogUniform,
    fog_buffer: wgpu::Buffer,

    cloud_buffer: wgpu::Buffer,
    cloud_indices_buffer: wgpu::Buffer,
    cloud_indices_count: usize,

    subchunks: IndexMap<(IPoint2D, usize), RenderSubchunk>,
    last_position: IPoint3D,
    sun_position: f32,
    fog_color: Color,
}

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct FogUniform {
    fog_color: [f32; 4],
    fog_env_start: f32,
    fog_env_end: f32,
    fog_render_dist_start: f32,
    fog_render_dist_end: f32,
    enabled: u32,
    _pad: [u32; 3],
}
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct VoxelUniform {
    sun_position: Point3D,
    _pad: [u32; 2],
}

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct VoxelImmediates {
    matrix: Transform3D,
    chunk_offset: Point3D,
    _pad: u32,
}

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct CloudImmediates {
    matrix: Transform3D,
    radii: f32,
    _pad: [u32; 3],
}

impl ChunkRenderer {
    #[inline]
    #[allow(clippy::too_many_lines)]
    pub fn new(context: &WindowContext, texture: &wgpu::Texture, lightmap: &wgpu::Texture) -> Self {
        let voxel = std::fs::read_to_string("./resources/shaders/voxel.wgsl").unwrap();
        let clouds = std::fs::read_to_string("./resources/shaders/clouds.wgsl").unwrap();

        let shader = context.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Chunk Renderer Shader"),
            source: wgpu::ShaderSource::Wgsl(voxel.into()),
        });

        let cloud_shader = context.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Chunk Renderer Shader"),
            source: wgpu::ShaderSource::Wgsl(clouds.into()),
        });

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let lightmap_view = lightmap.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = context.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });

        let voxel_buffer = context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Voxel Buffer"),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            size: size_of::<VoxelUniform>() as u64,
            mapped_at_creation: false,
        });

        let fog_buffer = context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Fog Buffer"),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            size: size_of::<FogUniform>() as u64,
            mapped_at_creation: false,
        });

        let cloud_buffer = context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Cloud Buffer: Vertices"),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            size: (size_of::<CloudVertex>() * 6 * 128) as u64,
            mapped_at_creation: false,
        });

        let cloud_indices_buffer = context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Cloud Buffer: Indices"),
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            size: (size_of::<u32>() * 6 * 128) as u64,
            mapped_at_creation: false,
        });

        let voxel_bind_group_layout = context.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("Chunk Renderer Bind Group Layout"),
        });

        let voxel_bind_group = context.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &voxel_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: voxel_buffer.as_entire_binding(),
            }],
            label: Some("Chunk Renderer Bind Group"),
        });

        let fog_bind_group_layout = context.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("Chunk Renderer Bind Group Layout"),
        });

        let fog_bind_group = context.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &fog_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: fog_buffer.as_entire_binding(),
            }],
            label: Some("Chunk Renderer Bind Group"),
        });

        let fragment_bind_group_layout = context.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("Chunk Renderer Bind Group Layout"),
        });

        let fragment_bind_group = context.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &fragment_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&lightmap_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
            label: Some("Chunk Renderer Bind Group"),
        });

        let render_pipeline_layout = context.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Chunk Renderer Pipeline Layout"),
            bind_group_layouts: &[Some(&voxel_bind_group_layout), Some(&fragment_bind_group_layout), Some(&fog_bind_group_layout)],
            immediate_size: size_of::<VoxelImmediates>() as u32,
        });

        let cloud_render_pipeline_layout = context.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Cloud Renderer Pipeline Layout"),
            bind_group_layouts: &[],
            immediate_size: size_of::<CloudImmediates>() as u32,
        });

        let solid_render_pipeline = context.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Chunk Renderer Solid Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(VoxelVertex::LAYOUT)],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: *context.surface_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Cw,
                cull_mode: Some(wgpu::Face::Back),
                ..wgpu::PrimitiveState::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: mavelin_engine::Texture::DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let translucent_render_pipeline = context.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Chunk Renderer Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(VoxelVertex::LAYOUT)],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: *context.surface_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Cw,
                cull_mode: Some(wgpu::Face::Back),
                ..wgpu::PrimitiveState::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: mavelin_engine::Texture::DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let cloud_render_pipeline = context.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Cloud Renderer Pipeline"),
            layout: Some(&cloud_render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &cloud_shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(CloudVertex::LAYOUT)],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &cloud_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: *context.surface_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Cw,
                cull_mode: Some(wgpu::Face::Back),
                ..wgpu::PrimitiveState::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: mavelin_engine::Texture::DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            solid_render_pipeline,
            translucent_render_pipeline,
            cloud_render_pipeline,
            voxel_bind_group,
            fragment_bind_group,
            fog_bind_group,
            voxel: VoxelUniform {
                sun_position: Point3D::ZERO,
                _pad: [0; 2],
            },
            voxel_buffer,
            fog: FogUniform {
                enabled: 1,
                fog_color: [0.0; 4],
                fog_env_start: SUBCHUNK_SIZE_F32 * 2.0,
                fog_env_end: SUBCHUNK_SIZE_F32 * 6.0,
                fog_render_dist_start: SUBCHUNK_SIZE_F32 * 3.0,
                fog_render_dist_end: SUBCHUNK_SIZE_F32 * 6.0,
                _pad: [0; 3],
            },
            fog_buffer,
            cloud_buffer,
            cloud_indices_buffer,
            cloud_indices_count: 0,
            subchunks: IndexMap::new(),
            last_position: IPoint3D::ZERO,
            sun_position: 0.0,
            fog_color: Color::BLACK,
        }
    }

    pub fn set_clouds(&mut self, queue: &wgpu::Queue, device: &wgpu::Device, clouds: impl Iterator<Item = (Cube3D, Color)>) {
        let mut vertices = Vec::with_capacity(self.cloud_indices_count / 6);
        let mut indices = Vec::with_capacity(self.cloud_indices_count);
        let mut current_offset = 0;

        for (Cube3D { origin, size }, color) in clouds {
            for face in Face::ALL {
                let face_vertices = face.as_vertices();

                vertices.extend((0..4).map(|i| CloudVertex {
                    position: origin + face_vertices[i] * size,
                    half_size: size / 2.0,
                    color: color.as_value(),
                }));

                let offset = current_offset;

                current_offset += 4;
                indices.extend([offset, offset + 1, offset + 2, offset + 3, offset + 2, offset + 1]);
            }
        }

        if indices.len() > self.cloud_indices_count {
            self.cloud_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Cloud Buffer: Vertices"),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                contents: bytemuck::cast_slice(&vertices),
            });

            self.cloud_indices_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Cloud Buffer: Indices"),
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                contents: bytemuck::cast_slice(&indices),
            });

            self.cloud_indices_count = indices.len();
        } else {
            queue.write_buffer(&self.cloud_buffer, 0, bytemuck::cast_slice(&vertices));
            queue.write_buffer(&self.cloud_indices_buffer, 0, bytemuck::cast_slice(&indices));
        }
    }

    #[inline]
    pub fn set_subchunk(&mut self, origin: (IPoint2D, usize), solid: RenderBuffer, translucent: TranslucentSubchunk) {
        self.subchunks.insert(origin, RenderSubchunk { solid, translucent });
    }

    #[inline]
    pub const fn set_sun_position(&mut self, value: f32) {
        self.voxel.sun_position.y = value;
    }

    #[inline]
    pub const fn set_fog_color(&mut self, value: Color) {
        self.fog_color = value;
        self.fog.fog_color = value.to_linear_rgba();
    }

    #[inline]
    pub fn update_uniforms(&self, queue: &wgpu::Queue) {
        queue.write_buffer(&self.voxel_buffer, 0, bytemuck::bytes_of(&self.voxel));
        queue.write_buffer(&self.fog_buffer, 0, bytemuck::bytes_of(&self.fog));
    }

    #[inline]
    fn is_subchunk_visible<T: Frustum>(frustum: &T, (origin, subchunk): (IPoint2D, usize)) -> bool {
        let origin = origin.as_vec2() * SUBCHUNK_SIZE_F32;
        let y = (subchunk * SUBCHUNK_SIZE) as f32;
        let origin = Point3D::new(origin.x, y, origin.y);
        let chunk_size = SUBCHUNK_SIZE_F32;
        let chunk_height = SUBCHUNK_SIZE_F32;

        frustum.is_box_visible(origin, origin + Point3D::new(chunk_size, chunk_height, chunk_size))
    }

    #[inline]
    pub fn is_subchunk_rendered<Q: ?Sized + Hash + Eq>(&self, k: &Q) -> bool
    where
        (IPoint2D, usize): Borrow<Q>,
    {
        self.subchunks.contains_key(k)
    }

    pub fn render_buffer(
        &mut self,
        queue: &wgpu::Queue,
        render_pass: &mut wgpu::RenderPass,
        buffer: &RenderBuffer,
        matrix: Transform3D,
        full_bright: bool,
    ) -> RenderInfo {
        self.set_sun_position(if full_bright { const { (1.0 - 0.5) / 0.96 } } else { self.sun_position });
        self.update_uniforms(queue);

        // pass.apply_params(DrawParams {
        //     blend: Some(Blend {
        //         color: (BlendingFactor::SourceAlpha,
        // BlendingFactor::OneMinusSourceAlpha),         alpha:
        // (BlendingFactor::One, BlendingFactor::OneMinusSourceAlpha),     }),
        //     depth: Some(Depth {
        //         test: DepthTest::IfLessOrEqual,
        //         write: true,
        //     }),
        //     culling: Some(BackfaceCullingMode::CullCounterClockwise),
        // });
        render_pass.set_pipeline(&self.solid_render_pipeline);
        render_pass.set_bind_group(0, &self.voxel_bind_group, &[]);
        render_pass.set_bind_group(1, &self.fragment_bind_group, &[]);
        render_pass.set_bind_group(2, &self.fog_bind_group, &[]);
        render_pass.set_vertex_buffer(0, buffer.vertices.slice(..));
        render_pass.set_index_buffer(buffer.indices.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.insert_debug_marker("render_buffer");
        render_pass.set_immediates(
            0,
            bytemuck::bytes_of(&VoxelImmediates {
                chunk_offset: Point3D::ZERO,
                matrix,
                _pad: 0,
            }),
        );

        render_pass.draw_indexed(0..buffer.count as u32, 0, 0..1);

        RenderInfo {
            draw_calls: 1,
            vertices: buffer.count,
        }
    }

    pub fn filter_by_shape(&mut self, center: IPoint2D, shape: RenderShape) -> impl Iterator<Item = IPoint2D> {
        self.subchunks
            .extract_if(.., move |&(origin, _), _| !shape.test(center, origin))
            .map(|((k, _), _)| k)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render<T: Frustum>(
        &mut self,
        queue: &wgpu::Queue,
        render_pass: &mut wgpu::RenderPass,
        camera_pos: Point3D,
        frustum: &T,
        matrix: Transform3D,
    ) -> RenderInfo {
        self.set_sun_position(self.sun_position);
        self.update_uniforms(queue);

        let pos = camera_pos.as_ivec3();

        if self.last_position == IPoint3D::ZERO || self.last_position.distance_squared(pos) > 4 {
            self.subchunks.sort_unstable_by(|&a, _, &b, _| {
                #[inline]
                const fn center((pos, idx): (IPoint2D, usize)) -> IPoint3D {
                    IPoint3D::new(
                        pos.x * SUBCHUNK_SIZE_I32 + SUBCHUNK_SIZE_I32 / 2,
                        idx as i32 * SUBCHUNK_SIZE_I32 + SUBCHUNK_SIZE_I32 / 2,
                        pos.y * SUBCHUNK_SIZE_I32 + SUBCHUNK_SIZE_I32 / 2,
                    )
                }

                pos.distance_squared(center(a)).cmp(&pos.distance_squared(center(b)))
            });

            self.last_position = pos;
        }

        let mut render_info = RenderInfo::default();

        render_pass.set_pipeline(&self.solid_render_pipeline);
        render_pass.set_bind_group(0, &self.voxel_bind_group, &[]);
        render_pass.set_bind_group(1, &self.fragment_bind_group, &[]);
        render_pass.set_bind_group(2, &self.fog_bind_group, &[]);
        render_pass.set_immediates(
            0,
            bytemuck::bytes_of(&VoxelImmediates {
                chunk_offset: Point3D::ZERO,
                matrix,
                _pad: 0,
            }),
        );

        for (&key, subchunk) in &self.subchunks {
            if Self::is_subchunk_visible(frustum, key) && subchunk.solid.count > 0 {
                let chunk_origin = Point3D::new(key.0.x as f32 * SUBCHUNK_SIZE_F32, 0.0, key.0.y as f32 * SUBCHUNK_SIZE_F32);
                let chunk_offset = chunk_origin - camera_pos;

                render_pass.set_immediates(
                    64,
                    bytemuck::bytes_of(&chunk_offset.to_array()),
                );

                render_pass.set_vertex_buffer(0, subchunk.solid.vertices.slice(..));
                render_pass.set_index_buffer(subchunk.solid.indices.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..subchunk.solid.count as u32, 0, 0..1);

                render_info.draw_calls += 1;
            }
        }

        render_pass.set_pipeline(&self.translucent_render_pipeline);
        render_pass.set_immediates(
            0,
            bytemuck::bytes_of(&VoxelImmediates {
                chunk_offset: Point3D::ZERO,
                matrix,
                _pad: 0,
            }),
        );

        for (&key, subchunk) in self.subchunks.iter_mut().rev() {
            if Self::is_subchunk_visible(frustum, key) && subchunk.translucent.buffer.count > 0 {
                let chunk_origin = Point3D::new(key.0.x as f32 * SUBCHUNK_SIZE_F32, 0.0, key.0.y as f32 * SUBCHUNK_SIZE_F32);
                let chunk_offset = chunk_origin - camera_pos;

                render_pass.set_immediates(
                    64,
                    bytemuck::bytes_of(&chunk_offset.to_array()),
                );

                subchunk.translucent.update(queue, camera_pos, key.0);

                render_pass.set_vertex_buffer(0, subchunk.translucent.buffer.vertices.slice(..));
                render_pass.set_index_buffer(subchunk.translucent.buffer.indices.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..subchunk.translucent.buffer.count as u32, 0, 0..1);

                render_info.draw_calls += 1;
            }
        }

        if self.cloud_indices_count > 0 {
            render_pass.set_pipeline(&self.cloud_render_pipeline);
            render_pass.set_immediates(
                0,
                bytemuck::bytes_of(&CloudImmediates {
                    matrix,
                    _pad: [0; 3],
                    radii: 12.0,
                }),
            );

            render_pass.set_vertex_buffer(0, self.cloud_buffer.slice(..));
            render_pass.set_index_buffer(self.cloud_indices_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..self.cloud_indices_count as u32, 0, 0..1);
        }

        render_info
    }
}
