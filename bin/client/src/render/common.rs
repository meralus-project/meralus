use std::collections::hash_map::Entry;

use ahash::{HashMap, HashMapExt};
use etagere::{AllocId, AtlasAllocator};
use lyon_tessellation::{FillOptions, FillTessellator, VertexBuffers, geometry_builder::simple_builder};
use mavelin_engine::WindowContext;
use mavelin_shared::{AsValue, Color, RRect, Rect, Thickness};
use swash::{
    CacheKey, FontRef,
    scale::{Render, ScaleContext, Source, StrikeWith},
    shape::ShapeContext,
    text::cluster::Whitespace,
    zeno::{Format, Vector},
};

use crate::render::RawRenderBuffer;

#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct CommonVertex {
    pub position: glam::Vec2,
    pub local_uv: glam::Vec2,
    pub radii: Thickness,
    pub half_size: [f32; 2],
    pub color: [u8; 4],
    pub mode: u32,
}

impl CommonVertex {
    const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4, 3 => Float32x2, 4 => Uint8x4, 5 => Uint32],
    };
}

#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct ShapeData {
    pub color: [f32; 4],
    pub half_size: [f32; 2],
    pub radii: Thickness,
    pub mode: u32,
    _pad: u32,
}

#[derive(Debug)]
#[allow(dead_code)]
enum Op {
    Circle { origin: glam::Vec2, radius: f32 },
    Rect(Rect),
    RoundRect(RRect),
    Begin(glam::Vec2),
    LineTo(glam::Vec2),
    CubicBezierTo(glam::Vec2, glam::Vec2, glam::Vec2),
    Close,
}

#[derive(Debug, Default)]

pub struct Path {
    ops: Vec<Op>,
}

#[allow(dead_code)]
impl Path {
    pub fn add_circle(&mut self, origin: glam::Vec2, radius: f32) -> &mut Self {
        self.ops.push(Op::Circle { origin, radius });

        self
    }

    pub fn add_rect(&mut self, origin: glam::Vec2, size: glam::Vec2) -> &mut Self {
        self.ops.push(Op::Rect(Rect::new(origin, size)));

        self
    }

    pub fn add_round_rect(&mut self, origin: glam::Vec2, size: glam::Vec2, corner_radius: Thickness) -> &mut Self {
        self.ops.push(Op::RoundRect(RRect::new(origin, size, corner_radius)));

        self
    }

    pub fn line_to(&mut self, point: glam::Vec2) -> &mut Self {
        self.ops.push(Op::LineTo(point));

        self
    }

    pub fn cubic_bezier_to(&mut self, ctrl1: glam::Vec2, ctrl2: glam::Vec2, to: glam::Vec2) -> &mut Self {
        self.ops.push(Op::CubicBezierTo(ctrl1, ctrl2, to));

        self
    }

    pub fn begin(&mut self, at: glam::Vec2) -> &mut Self {
        self.ops.push(Op::Begin(at));

        self
    }

    pub fn close(&mut self) -> &mut Self {
        self.ops.push(Op::Close);

        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
#[allow(dead_code)]
pub enum ObjectFit {
    Stretch,
    Cover,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GlyphKey(CacheKey, u32, u16);

impl GlyphKey {
    const fn new(font: CacheKey, font_size: f32, glyph: u16) -> Self {
        Self(font, font_size.to_bits(), glyph)
    }
}

#[allow(dead_code)]
pub struct CommonRenderer {
    render_pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    vbo: wgpu::Buffer,
    ibo: wgpu::Buffer,
    matrix_buffer: wgpu::Buffer,
    texture: wgpu::Texture,
    atlas: AtlasAllocator,

    // TEXT RENDERING
    font_name_map: HashMap<String, usize>,
    glyph_map: HashMap<GlyphKey, (AllocId, glam::IVec2)>,
    fonts: Vec<OwnedFont>,

    // COMMON RENDERING
    pub(crate) buffers: RawRenderBuffer<CommonVertex>,

    // VERTICES TRANSFORMATION
    transform: Option<glam::Mat4>,

    matrix: Option<glam::Mat4>,
    window_matrix: glam::Mat4,

    pub clip: Option<(glam::Vec2, glam::Vec2)>,
}

pub struct OwnedFont {
    pub data: Vec<u8>,
    pub offset: u32,
    pub key: CacheKey,
}

impl CommonRenderer {
    const PREALLOCATE_INDICES: usize = Self::PREALLOCATE_VERTICES * 2;
    const PREALLOCATE_VERTICES: usize = 16 * 16 * 16 * 72;

    #[allow(clippy::too_many_lines)]
    pub fn new(context: &WindowContext) -> Self {
        let atlas = AtlasAllocator::new(euclid::Size2D::splat(4096));
        let texture = context.device.create_texture(&wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width: 4096,
                height: 4096,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label: None,
            view_formats: &[],
        });

        let vbo = context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Common Render VBO"),
            size: (Self::PREALLOCATE_VERTICES * size_of::<CommonVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let ibo = context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Common Render IBO"),
            size: (Self::PREALLOCATE_INDICES * size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let matrix_buffer = context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Common Render Matrix Buffer"),
            size: size_of::<[f32; 16]>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let shader = context.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Common Renderer Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../../../resources/shaders/shape.wgsl").into()),
        });

        let atlas_texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = context.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let texture_bind_group_layout = context.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
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
            label: Some("Common Renderer Bind Group Layout"),
        });

        let bind_group = context.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: matrix_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&atlas_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
            label: Some("Common Renderer Bind Group"),
        });

        let render_pipeline_layout = context.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Common Renderer Pipeline Layout"),
            bind_group_layouts: &[Some(&texture_bind_group_layout)],
            immediate_size: 0,
        });
        //         color: (BlendingFactor::SourceAlpha,
        // BlendingFactor::OneMinusSourceAlpha),         alpha:
        // (BlendingFactor::One, BlendingFactor::OneMinusSourceAlpha),     }),

        let render_pipeline = context.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Common Renderer Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(CommonVertex::LAYOUT)],
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
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..wgpu::PrimitiveState::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: mavelin_engine::Texture::DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: None,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            render_pipeline,
            bind_group,
            matrix_buffer,
            vbo,
            ibo,

            atlas,
            texture,

            font_name_map: HashMap::new(),
            glyph_map: HashMap::new(),
            fonts: Vec::new(),

            buffers: RawRenderBuffer::new(),

            transform: None,
            window_matrix: glam::Mat4::IDENTITY,
            matrix: None,
            clip: None,
        }
    }

    #[allow(dead_code)]
    pub fn fonts(&self) -> &[OwnedFont] {
        &self.fonts
    }

    pub const fn window_matrix(&self) -> glam::Mat4 {
        self.window_matrix
    }

    #[allow(dead_code)]
    pub const fn set_matrix(&mut self, matrix: glam::Mat4) {
        self.matrix = Some(matrix);
    }

    #[allow(dead_code)]
    pub const fn set_default_matrix(&mut self) {
        self.matrix = None;
    }

    pub fn set_window_matrix(&mut self, queue: &wgpu::Queue, matrix: glam::Mat4) {
        self.window_matrix = matrix;

        queue.write_buffer(&self.matrix_buffer, 0, bytemuck::cast_slice(&matrix.to_cols_array()));
    }

    #[allow(dead_code)]
    pub const fn set_transform(&mut self, transform: Option<glam::Mat4>) {
        self.transform = transform;
    }

    /// # Errors
    ///
    /// Returns [`TextureCreationError`] if texture creation on GPU failed.
    pub fn add_font<T: Into<String>>(&mut self, name: T, data: &[u8]) {
        // if let Ok(font) = Font::from_bytes(data, FontSettings::default()) {
        use swash::FontRef;

        let font_info = FontRef::from_index(data, 0).unwrap();

        self.font_name_map.insert(name.into(), self.fonts.len());

        self.fonts.push(OwnedFont {
            data: font_info.data.to_vec(),
            // font,
            offset: font_info.offset,
            key: font_info.key,
        });
        // }
    }

    pub fn measure<F: AsRef<str>, T: AsRef<str>>(&self, font: F, text: T, size: f32, _max_width: Option<f32>) -> Option<glam::Vec2> {
        self.font_name_map.get(font.as_ref()).copied().map(|font_index| {
            let text = text.as_ref();

            let font_ref = FontRef::from_index(&self.fonts[font_index].data, 0).unwrap();

            let mut shape_context = ShapeContext::new();
            let mut shaper = shape_context.builder(font_ref).size(size).build();
            let _metrics = font_ref.glyph_metrics(&[]).scale(size);

            shaper.add_str(text);

            let mut metrics = glam::Vec2::ZERO;
            let mut x = 0.0;
            let mut y = size;

            shaper.shape_with(|cluster| {
                use swash::text::cluster::Whitespace;

                if matches!(cluster.info.whitespace(), Whitespace::Newline) {
                    metrics.x = metrics.x.max(x);

                    x = 0.0;
                    y += size;
                }

                for _glyph in cluster.glyphs {
                    x += cluster.advance();
                }
            });

            metrics.x = metrics.x.max(x);
            metrics.with_y(y)
        })
    }

    fn push_quad(&mut self, positions: [glam::Vec2; 4], local_uvs: [glam::Vec2; 4], half_size: glam::Vec2, radii: Thickness, color: Color) {
        let base = self.buffers.vertices.len() as u32;

        self.buffers.vertices.extend((0..4).map(|i| CommonVertex {
            position: positions[i],
            local_uv: local_uvs[i],
            color: [color.get_red(), color.get_green(), color.get_blue(), color.get_alpha()],
            half_size: half_size.to_array(),
            radii,
            mode: 0,
        }));

        self.buffers.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    #[allow(dead_code)]
    pub fn draw_circle(&mut self, origin: glam::Vec2, radius: f32, color: Color) {
        self.draw_round_rect(origin, glam::Vec2::splat(radius * 2.0), Thickness::all(radius), color);
    }

    pub fn draw_rect(&mut self, origin: glam::Vec2, size: glam::Vec2, color: Color) {
        self.draw_round_rect(origin, size, Thickness::default(), color);
    }

    pub fn draw_round_rect(&mut self, origin: glam::Vec2, size: glam::Vec2, radii: Thickness, color: Color) {
        let h = size * 0.5;
        let c = origin + h;

        self.push_quad(
            [c - h, c + h.with_y(-h.y), c + h, c - h.with_y(-h.y)],
            [-h, h.with_y(-h.y), h, h.with_x(-h.x)],
            h,
            radii,
            color,
        );
    }

    #[allow(dead_code)]
    pub fn push_lyon_path(&mut self, path: &lyon_tessellation::path::Path, color: Color) {
        let mut geom: VertexBuffers<euclid::default::Point2D<f32>, u16> = VertexBuffers::new();

        FillTessellator::new()
            .tessellate_path(path, &FillOptions::default(), &mut simple_builder(&mut geom))
            .expect("tessellation failed");

        let base = self.buffers.vertices.len() as u32;

        self.buffers
            .vertices
            .extend(bytemuck::cast_vec(geom.vertices).into_iter().map(|position| CommonVertex {
                position,
                local_uv: glam::Vec2::ZERO,
                half_size: [0.0; 2],
                radii: Thickness::default(),
                color: [color.get_red(), color.get_green(), color.get_blue(), color.get_alpha()],
                mode: 2,
            }));

        for i in geom.indices {
            self.buffers.indices.push(base + u32::from(i));
        }
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub fn draw_text<F: AsRef<str>, T: AsRef<str>>(
        &mut self,
        queue: &wgpu::Queue,
        origin: glam::Vec2,
        font: F,
        text: T,
        color: Color,
        font_size: f32,
        _max_width: Option<f32>,
    ) {
        if let Some(font_index) = self.font_name_map.get(font.as_ref()).copied() {
            let text = text.as_ref();

            let OwnedFont { data, offset, key, .. } = &self.fonts[font_index];
            let key = *key;
            let font_ref = FontRef { data, offset: *offset, key };

            let mut shape_context = ShapeContext::new();
            let mut scale_context = ScaleContext::new();
            let mut scaler = scale_context.builder(font_ref).hint(true).size(font_size).build();
            let mut shaper = shape_context.builder(font_ref).size(font_size).build();

            shaper.add_str(text);

            let mut x = origin.x;
            let mut y = origin.y + font_size;

            shaper.shape_with(|cluster| {
                if matches!(cluster.info.whitespace(), Whitespace::Newline) {
                    x = origin.x;
                    y += font_size;
                }

                for glyph in cluster.glyphs {
                    if !cluster.info.is_whitespace() {
                        let key = GlyphKey::new(key, font_size, glyph.id);
                        let (rect, offset) = match self.glyph_map.entry(key) {
                            Entry::Occupied(entry) => {
                                let (alloc, offset) = *entry.get();

                                (self.atlas.get(alloc), offset)
                            }
                            Entry::Vacant(entry) => {
                                let image = Render::new(&[Source::ColorOutline(0), Source::ColorBitmap(StrikeWith::BestFit), Source::Outline])
                                    .format(Format::Alpha)
                                    .offset(Vector::new(glyph.x, glyph.y))
                                    .render(&mut scaler, glyph.id)
                                    .unwrap();

                                let alloc = self
                                    .atlas
                                    .allocate(etagere::size2(image.placement.width.cast_signed(), image.placement.height.cast_signed()))
                                    .unwrap();

                                let buffer = image::GrayImage::from_raw(image.placement.width, image.placement.height, image.data).unwrap();
                                let offset = glam::IVec2::new(image.placement.left, image.placement.top);

                                entry.insert((alloc.id, offset));

                                let buffer = image::DynamicImage::ImageLuma8(buffer);
                                let buffer = buffer.to_rgba8().into_raw();

                                queue.write_texture(
                                    wgpu::TexelCopyTextureInfoBase {
                                        texture: &self.texture,
                                        mip_level: 0,
                                        origin: wgpu::Origin3d {
                                            x: alloc.rectangle.min.x.cast_unsigned(),
                                            y: alloc.rectangle.min.y.cast_unsigned(),
                                            z: 0,
                                        },
                                        aspect: wgpu::TextureAspect::All,
                                    },
                                    &buffer,
                                    wgpu::TexelCopyBufferLayout {
                                        offset: 0,
                                        bytes_per_row: Some(4 * image.placement.width),
                                        rows_per_image: Some(image.placement.height),
                                    },
                                    wgpu::Extent3d {
                                        width: image.placement.width,
                                        height: image.placement.height,
                                        depth_or_array_layers: 1,
                                    },
                                );

                                (alloc.rectangle, offset)
                            }
                        };

                        let atlas_size = self.atlas.size();
                        let u0 = rect.min.x as f32 / atlas_size.width as f32;
                        let v0 = rect.min.y as f32 / atlas_size.height as f32;
                        let u1 = rect.max.x as f32 / atlas_size.width as f32;
                        let v1 = rect.max.y as f32 / atlas_size.height as f32;

                        let base = self.buffers.vertices.len() as u32;
                        let base_point = glam::Vec2::new(x + offset.x as f32, y - offset.y as f32);

                        self.buffers.vertices.extend(
                            [
                                base_point,
                                base_point + glam::Vec2::new(rect.width() as f32, 0.0),
                                base_point + glam::Vec2::new(rect.width() as f32, rect.height() as f32),
                                base_point + glam::Vec2::new(0.0, rect.height() as f32),
                            ]
                            .into_iter()
                            .zip([
                                glam::Vec2::new(u0, v0),
                                glam::Vec2::new(u1, v0),
                                glam::Vec2::new(u1, v1),
                                glam::Vec2::new(u0, v1),
                            ])
                            .map(|(position, local_uv)| CommonVertex {
                                position,
                                local_uv,
                                color: color.as_value(),
                                half_size: [0.0; 2],
                                radii: Thickness::default(),
                                mode: 1,
                            }),
                        );

                        self.buffers.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
                    }

                    x += cluster.advance();
                }
            });
        }
    }

    // #[must_use = "RenderInfo itself needs to be extended into other"]
    // pub fn render_lines<S: Surface>(
    //     &mut self,
    //     surface: &mut S,
    //     display: &WindowDisplay,
    //     vertices: &[CommonVertex],
    //     matrix: Option<Mat4>,
    // ) -> Result<RenderInfo, DrawError> {
    //     let matrix = matrix.or(self.matrix).unwrap_or(self.window_matrix);
    //     let vertex_buffer = VertexBuffer::new(display, vertices).unwrap();
    //     let uniforms = uniform! {
    //         atlas: self.texture
    //             .sampled()
    //             .minify_filter(MinifySamplerFilter::Nearest)
    //             .magnify_filter(MagnifySamplerFilter::Nearest),
    //         resolution:
    // glam::UVec2::from_tuple(surface.get_dimensions()).as_::<f32>().to_array(),
    //         matrix: matrix.to_cols_array_2d(),
    //     };

    //     let vertices = vertex_buffer.len();

    //     surface.draw(&vertex_buffer, NoIndices(PrimitiveType::LinesList),
    // &self.shader, &uniforms, &DrawParameters {         blend: BLENDING,
    //         ..DrawParameters::default()
    //     })?;

    //     Ok(RenderInfo { draw_calls: 1, vertices })
    // }

    pub fn render(&mut self, render_pass: &mut wgpu::RenderPass, context: &WindowContext) -> super::RenderInfo {
        let vertices = self.buffers.vertices.len();
        let indices = self.buffers.indices.len();

        context.queue.write_buffer(&self.vbo, 0, bytemuck::cast_slice(&self.buffers.vertices));
        context.queue.write_buffer(&self.ibo, 0, bytemuck::cast_slice(&self.buffers.indices));

        self.buffers.clear();

        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vbo.slice(..));
        render_pass.set_index_buffer(self.ibo.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..indices as u32, 0, 0..1);

        super::RenderInfo { draw_calls: 1, vertices }
    }
}
