use std::path::PathBuf;

use ahash::{HashMap, HashMapExt};
use horns::{
    Blend, BlendingFactor, DrawParams, ElementType, Error, IndexBuffer, Program, RenderBackend, RenderPass, Shader, Texture2d, VertexBuffer, impl_vertex,
};
use image::ImageBuffer;
use lyon_tessellation::{
    FillBuilder, FillGeometryBuilder, FillOptions, FillTessellator, FillVertex, GeometryBuilder, GeometryBuilderError, StrokeGeometryBuilder, StrokeVertex,
    TessellationError, VertexId,
    math::Transform,
    path::{
        Winding,
        builder::{BorderRadii, NoAttributes, Transformed},
    },
};
use meck::TextureViewAtlas;
use meralus_shared::{AsValue, Color, ConvertTo, Point2D, Point3D, RRect, Rect, Size2D, Thickness, Transform3D, USize2D, Vector2D, Vector4D};
use swash::{CacheKey, FontRef, shape::ShapeContext};

use crate::render::{RawRenderBuffer, context::RenderInfo};

pub struct ShapeShader;

impl Shader for ShapeShader {
    fn fragment(&self) -> String {
        std::fs::read_to_string("./resources/shaders/shape.fs").unwrap()
    }

    fn vertex(&self) -> String {
        std::fs::read_to_string("./resources/shaders/shape.vs").unwrap()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct CommonVertex {
    pub clip: Vector4D,
    pub position: Point3D,
    pub uv: Point2D,
    pub color: [u8; 4],
    pub _pad: [u8; 8],
}

impl_vertex! {
    CommonVertex {
        position: [f32; 3],
        color: [u8; 4],
        uv: [f32; 2],
        clip: [f32; 4],
        _pad: [u8; 8]
    }
}

pub struct ShapeGeometryBuilder {
    buffers: RawRenderBuffer<CommonVertex, u32>,
    first_vertex: u32,
    first_index: u32,
    vertex_offset: u32,
    color: Color,
    pub white_pixel_uv: Point2D,
    uv_rect: Option<(Point2D, Vector2D, Point2D, Size2D)>,
}

impl ShapeGeometryBuilder {
    pub const fn new(buffers: RawRenderBuffer<CommonVertex, u32>, white_pixel_uv: Point2D, color: Color) -> Self {
        let first_vertex = buffers.vertices.len() as u32;
        let first_index = buffers.indices.len() as u32;

        Self {
            buffers,
            first_vertex,
            first_index,
            vertex_offset: 0,
            color,
            white_pixel_uv,
            uv_rect: None,
        }
    }

    pub const fn set_uv_rect(&mut self, uv_rect: (Point2D, Vector2D, Point2D, Size2D)) {
        self.uv_rect.replace(uv_rect);
    }

    pub const fn take_uv_rect(&mut self) {
        self.uv_rect.take();
    }

    pub const fn set_color(&mut self, color: Color) {
        self.color = color;
    }

    pub const fn set_vertex_offsset(&mut self, offset: u32) {
        self.vertex_offset = offset;
    }

    fn build(&mut self) -> RawRenderBuffer<CommonVertex, u32> {
        let (num_vertices, num_indices) = (self.buffers.vertices.len(), self.buffers.indices.len());

        self.vertex_offset = 0;

        std::mem::replace(&mut self.buffers, RawRenderBuffer::with_capacity(num_vertices, num_indices))
    }
}

impl GeometryBuilder for ShapeGeometryBuilder {
    fn begin_geometry(&mut self) {
        self.first_vertex = self.buffers.vertices.len() as u32;
        self.first_index = self.buffers.indices.len() as u32;
    }

    fn add_triangle(&mut self, a: VertexId, b: VertexId, c: VertexId) {
        debug_assert_ne!(a, b);
        debug_assert_ne!(a, c);
        debug_assert_ne!(b, c);
        debug_assert!(a != VertexId::INVALID);
        debug_assert!(b != VertexId::INVALID);
        debug_assert!(c != VertexId::INVALID);

        self.buffers.indices.push((a + self.vertex_offset).into());
        self.buffers.indices.push((b + self.vertex_offset).into());
        self.buffers.indices.push((c + self.vertex_offset).into());
    }

    fn abort_geometry(&mut self) {
        self.buffers.vertices.truncate(self.first_vertex as usize);
        self.buffers.indices.truncate(self.first_index as usize);
    }
}

impl FillGeometryBuilder for ShapeGeometryBuilder {
    fn add_fill_vertex(&mut self, vertex: FillVertex) -> Result<VertexId, GeometryBuilderError> {
        let position = Point2D::from_array(vertex.position().to_array());

        self.buffers.vertices.push(CommonVertex {
            position: position.extend(0.0),
            color: self.color.as_value(),
            uv: if let Some((offset, uv_size, origin, size)) = self.uv_rect {
                Point2D::new(
                    uv_size.x.mul_add((position.x - origin.x) / size.x, offset.x),
                    uv_size.y.mul_add((position.y - origin.y) / size.y, offset.y),
                )
            } else {
                self.white_pixel_uv
            },
            clip: Vector4D::new(0.0, 0.0, 1.0, 1.0),
            _pad: [0; 8],
        });

        let len = self.buffers.vertices.len();

        if len > u32::MAX as usize {
            return Err(GeometryBuilderError::TooManyVertices);
        }

        Ok(VertexId((len - 1) as u32))
    }
}

impl StrokeGeometryBuilder for ShapeGeometryBuilder {
    fn add_stroke_vertex(&mut self, vertex: StrokeVertex) -> Result<VertexId, GeometryBuilderError> {
        self.buffers.vertices.push(CommonVertex {
            position: Point3D::from_array(vertex.position().extend(0.0).to_array()),
            color: self.color.as_value(),
            uv: self.white_pixel_uv,
            clip: Vector4D::new(0.0, 0.0, 1.0, 1.0),
            _pad: [0; 8],
        });

        let len = self.buffers.vertices.len();

        if len > u32::MAX as usize {
            return Err(GeometryBuilderError::TooManyVertices);
        }

        Ok(VertexId((len - 1) as u32))
    }
}

pub struct CommonTessellator {
    pub builder: ShapeGeometryBuilder,
    tessellator: FillTessellator,
    options: FillOptions,
}

impl CommonTessellator {
    pub fn new(white_pixel_uv: Point2D) -> Self {
        let builder = ShapeGeometryBuilder::new(RawRenderBuffer::new(), white_pixel_uv, Color::RED);
        let tessellator = FillTessellator::new();
        let options = FillOptions::default();

        Self { builder, tessellator, options }
    }

    pub const fn set_uv_rect(&mut self, uv_rect: (Point2D, Vector2D, Point2D, Size2D)) {
        self.builder.set_uv_rect(uv_rect);
    }

    pub const fn take_uv_rect(&mut self) {
        self.builder.take_uv_rect();
    }

    const fn set_vertex_offsset(&mut self, offset: u32) {
        self.builder.set_vertex_offsset(offset);
    }

    #[allow(dead_code)]
    pub fn transformed_tessellate_with_color<F: FnOnce(&mut NoAttributes<Transformed<FillBuilder, Transform>>)>(
        &mut self,
        color: Color,
        transform: Transform,
        tessellate: F,
    ) -> Result<(), TessellationError> {
        self.builder.set_color(color);

        let mut builder = self.tessellator.builder(&self.options, &mut self.builder).transformed(transform);

        tessellate(&mut builder);

        builder.build()
    }

    pub fn tessellate_with_color<F: FnOnce(&mut NoAttributes<FillBuilder>)>(&mut self, color: Color, tessellate: F) -> Result<(), TessellationError> {
        self.builder.set_color(color);

        let mut builder = self.tessellator.builder(&self.options, &mut self.builder);

        tessellate(&mut builder);

        builder.build()
    }

    pub fn build(&mut self) -> RawRenderBuffer<CommonVertex, u32> {
        self.builder.build()
    }
}

#[derive(Debug)]
#[allow(dead_code)]
enum Op {
    Circle { origin: Point2D, radius: f32 },
    Rect(Rect),
    RoundRect(RRect),
    Begin(Point2D),
    LineTo(Point2D),
    CubicBezierTo(Point2D, Point2D, Point2D),
    Close,
}

#[derive(Debug, Default)]

pub struct Path {
    ops: Vec<Op>,
}

#[allow(dead_code)]
impl Path {
    pub fn add_circle(&mut self, origin: Point2D, radius: f32) -> &mut Self {
        self.ops.push(Op::Circle { origin, radius });

        self
    }

    pub fn add_rect(&mut self, origin: Point2D, size: Size2D) -> &mut Self {
        self.ops.push(Op::Rect(Rect::new(origin, size)));

        self
    }

    pub fn add_round_rect(&mut self, origin: Point2D, size: Size2D, corner_radius: Thickness) -> &mut Self {
        self.ops.push(Op::RoundRect(RRect::new(origin, size, corner_radius)));

        self
    }

    pub fn line_to(&mut self, point: Point2D) -> &mut Self {
        self.ops.push(Op::LineTo(point));

        self
    }

    pub fn cubic_bezier_to(&mut self, ctrl1: Point2D, ctrl2: Point2D, to: Point2D) -> &mut Self {
        self.ops.push(Op::CubicBezierTo(ctrl1, ctrl2, to));

        self
    }

    pub fn begin(&mut self, at: Point2D) -> &mut Self {
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

pub enum AtlasKey {
    // Text(Option<GlyphRasterConfig>),
    Image(PathBuf),
    WhitePixel,
}

#[allow(dead_code)]
pub struct CommonRenderer {
    shader: Program,
    vbo: VertexBuffer<CommonVertex, ShapeShader>,
    ibo: IndexBuffer<u32>,
    texture: Texture2d,
    atlas: TextureViewAtlas<AtlasKey>,

    // SHAPE RENDERING
    tessellator: CommonTessellator,

    // TEXT RENDERING
    font_name_map: HashMap<String, usize>,

    fonts: Vec<OwnedFont>,

    // COMMON RENDERING
    pub(crate) buffers: RawRenderBuffer<CommonVertex, u32>,

    // VERTICES TRANSFORMATION
    transform: Option<Transform3D>,

    matrix: Option<Transform3D>,
    window_matrix: Transform3D,

    pub clip: Option<(Point2D, Point2D)>,
}

pub struct OwnedFont {
    pub data: Vec<u8>,
    pub offset: u32,
    pub key: CacheKey,
}

const TEXT_BASE_VERTICES: [(Point3D, Point2D); 4] = [
    (Point3D::new(0.0, 1.0, 0.0), Point2D::new(0.0, 1.0)),
    (Point3D::new(0.0, 0.0, 0.0), Point2D::new(0.0, 0.0)),
    (Point3D::new(1.0, 1.0, 0.0), Point2D::new(1.0, 1.0)),
    (Point3D::new(1.0, 0.0, 0.0), Point2D::new(1.0, 0.0)),
];

impl CommonRenderer {
    const PREALLOCATE_INDICES: usize = Self::PREALLOCATE_VERTICES * 2;
    const PREALLOCATE_VERTICES: usize = 16 * 16 * 16 * 72;

    pub fn new(backend: &RenderBackend) -> Result<Self, Error> {
        let mut atlas = TextureViewAtlas::new(4096).with_spacing(4);

        let image = ImageBuffer::from_pixel(24, 24, image::Rgba([255, 255, 255, 255]));

        let (offset, size, _) = atlas.append(AtlasKey::WhitePixel, &image);

        let texture = backend.create_empty_texture2d(4096, 4096)?;

        texture.writable().write(
            (offset.x * 4096.0) as u32,
            (offset.y * 4096.0) as u32,
            (size.x * 4096.0) as u32,
            (size.y * 4096.0) as u32,
            &[255u8; 24 * 24 * 4],
        );

        let shader = backend.create_program(&ShapeShader)?;
        let vbo = backend.create_empty_vertex_buffer(Self::PREALLOCATE_VERTICES, &shader, true)?;
        let ibo = backend.create_empty_index_buffer(ElementType::Triangles, Self::PREALLOCATE_INDICES, true)?;

        println!("vbo + ibo created");

        Ok(Self {
            shader,
            vbo,
            ibo,

            tessellator: CommonTessellator::new(offset + (size / 2.0)),

            atlas,
            texture,

            font_name_map: HashMap::new(),
            fonts: Vec::new(),

            buffers: RawRenderBuffer::new(),

            transform: None,
            window_matrix: Transform3D::IDENTITY,
            matrix: None,
            clip: None,
        })
    }

    #[allow(dead_code)]
    pub fn fonts(&self) -> &[OwnedFont] {
        &self.fonts
    }

    pub const fn window_matrix(&self) -> Transform3D {
        self.window_matrix
    }

    pub const fn white_pixel_uv(&self) -> Point2D {
        self.tessellator.builder.white_pixel_uv
    }

    pub const fn set_matrix(&mut self, matrix: Transform3D) {
        self.matrix = Some(matrix);
    }

    pub const fn set_default_matrix(&mut self) {
        self.matrix = None;
    }

    pub const fn set_window_matrix(&mut self, matrix: Transform3D) {
        self.window_matrix = matrix;
    }

    pub const fn set_transform(&mut self, transform: Option<Transform3D>) {
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

    pub fn measure<F: AsRef<str>, T: AsRef<str>>(&self, font: F, text: T, size: f32, _max_width: Option<f32>) -> Option<Size2D> {
        self.font_name_map.get(font.as_ref()).copied().map(|font_index| {
            let text = text.as_ref();

            let font_ref = FontRef::from_index(&self.fonts[font_index].data, 0).unwrap();

            let mut shape_context = ShapeContext::new();
            let mut shaper = shape_context.builder(font_ref).size(size).build();
            let _metrics = font_ref.glyph_metrics(&[]).scale(size);

            shaper.add_str(text);

            let mut metrics = Size2D::ZERO;
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

    pub fn draw_round_image<P: AsRef<std::path::Path>>(
        &mut self,
        origin: Point2D,
        size: Size2D,
        corner_radius: Thickness,
        path: P,
    ) -> Result<(), image::ImageError> {
        let path = path.as_ref();
        let key = AtlasKey::Image(path.to_path_buf());

        let (offset, uv_size) = if let Some((offset, size, _)) = self.atlas.get_texture_uv(&key) {
            (offset, size)
        } else {
            let image = image::ImageReader::open(path)?.with_guessed_format()?.decode()?;
            let image = image.to_rgba8();
            let (_width, _height) = image.dimensions();

            let (offset, size, _) = self.atlas.append(key, &image);

            self.texture.writable().write(
                (offset.x * 4096.0).convert().unwrap(),
                (offset.y * 4096.0).convert().unwrap(),
                (size.x * 4096.0).convert().unwrap(),
                (size.y * 4096.0).convert().unwrap(),
                image.as_raw(), /* RawImage2d {
                                 *     data: Cow::Owned(image.into_raw()),
                                 *     width,
                                 *     height,
                                 *     format: ClientFormat::U8U8U8U8,
                                 * }, */
            );

            (offset, size)
        };

        self.tessellator.set_vertex_offsset(self.buffers.vertices.len() as u32);
        self.tessellator.set_uv_rect((offset, uv_size, origin, size));
        self.tessellator
            .tessellate_with_color(Color::WHITE, |builder| {
                builder.add_rounded_rectangle(
                    &bytemuck::cast(Rect::new(origin, size).to_box2d()),
                    &BorderRadii {
                        top_left: corner_radius.top_left(),
                        top_right: corner_radius.top_right(),
                        bottom_left: corner_radius.bottom_left(),
                        bottom_right: corner_radius.bottom_right(),
                    },
                    Winding::Positive,
                );
            })
            .unwrap();

        self.tessellator.take_uv_rect();

        let buffers = self.tessellator.build();

        self.buffers.vertices.extend(buffers.vertices.into_iter().map(|mut vertex| {
            vertex.position = self
                .transform
                .as_ref()
                .map_or(vertex.position, |transform| transform.transform_point3(vertex.position));

            vertex
        }));

        self.buffers.indices.extend(buffers.indices);

        Ok(())
    }

    pub fn draw_image<P: AsRef<std::path::Path>>(&mut self, origin: Point2D, size: Size2D, path: P, object_fit: ObjectFit) -> Result<(), image::ImageError> {
        let path = path.as_ref();
        let key = AtlasKey::Image(path.to_path_buf());
        let resulting_scale = size / 4096.0;

        let (offset, uv_size) = if let Some((offset, scale, _)) = self.atlas.get_texture_uv(&key) {
            match object_fit {
                ObjectFit::Stretch => (offset, scale),
                ObjectFit::Cover => {
                    let r = Size2D::new(resulting_scale.x / scale.x, resulting_scale.y / scale.y);
                    let ratio = r.max_element();
                    let scale = scale * ratio;
                    let mut diff = Point2D::ZERO;

                    if scale.x > resulting_scale.x {
                        diff.x = scale.x - resulting_scale.x;
                    }

                    if scale.y > resulting_scale.y {
                        diff.y = scale.y - resulting_scale.y;
                    }

                    (offset + diff / 2.0 / ratio, resulting_scale / ratio)
                }
            }
        } else {
            let image = image::ImageReader::open(path)?.with_guessed_format()?.decode()?;
            let image = image.to_rgba8();

            let (offset, size, _) = self.atlas.append(key, &image);

            self.texture.writable().write(
                (offset.x * 4096.0).convert().unwrap(),
                (offset.y * 4096.0).convert().unwrap(),
                (size.x * 4096.0).convert().unwrap(),
                (size.y * 4096.0).convert().unwrap(),
                image.as_raw(),
            );

            match object_fit {
                ObjectFit::Stretch => (offset, size),
                ObjectFit::Cover => {
                    let r = resulting_scale / size;
                    let ratio = r.max_element();
                    let scaled_size = size * ratio;
                    let mut diff = Point2D::ZERO;

                    if scaled_size.x > resulting_scale.x {
                        diff.x = scaled_size.x - resulting_scale.x;
                    }

                    if scaled_size.y > resulting_scale.y {
                        diff.y = scaled_size.y - resulting_scale.y;
                    }

                    (offset + diff / 2.0 / ratio, resulting_scale / ratio)
                }
            }
        };

        self.buffers.vertices.extend(TEXT_BASE_VERTICES.map(|(position, uv)| {
            let mut position = (origin + Point2D::new(position.x * size.x, position.y * size.y)).extend(position.z);

            if let Some(transform) = &self.transform {
                position = transform.transform_point3(position);
            }

            CommonVertex {
                position,
                color: Color::WHITE.as_value(),
                uv: offset + Point2D::new(uv.x * uv_size.x, uv.y * uv_size.y),
                clip: Vector4D::new(0.0, 0.0, 1.0, 1.0),
                _pad: [0; 8],
            }
        }));

        self.buffers
            .indices
            .extend([0, 1, 2, 3, 2, 1].map(|index| (self.buffers.vertices.len() - TEXT_BASE_VERTICES.len()) as u32 + index));

        Ok(())
    }

    #[allow(dead_code)]
    pub fn draw_circle(&mut self, origin: Point2D, radius: f32, color: Color) -> Result<(), TessellationError> {
        self.draw_shape(|builder| builder.add_circle(bytemuck::cast(origin), radius, Winding::Positive), color)
    }

    pub fn draw_rect(&mut self, origin: Point2D, size: Size2D, color: Color) -> Result<(), TessellationError> {
        self.draw_shape(
            |builder| builder.add_rectangle(&bytemuck::cast(Rect::new(origin, size).to_box2d()), Winding::Positive),
            color,
        )
    }

    pub fn draw_round_rect(&mut self, origin: Point2D, size: Size2D, corner_radius: Thickness, color: Color) -> Result<(), TessellationError> {
        self.draw_shape(
            |builder| {
                builder.add_rounded_rectangle(
                    &bytemuck::cast(Rect::new(origin, size).to_box2d()),
                    &BorderRadii {
                        top_left: corner_radius.top_left(),
                        top_right: corner_radius.top_right(),
                        bottom_left: corner_radius.bottom_left(),
                        bottom_right: corner_radius.bottom_right(),
                    },
                    Winding::Positive,
                );
            },
            color,
        )
    }

    pub fn draw_image_path<P: AsRef<std::path::Path>>(&mut self, path: Path, image_path: P) -> Result<(), image::ImageError> {
        let image_path = image_path.as_ref();
        let key = AtlasKey::Image(image_path.to_path_buf());

        let (offset, uv_size) = if let Some((offset, size, _)) = self.atlas.get_texture_uv(&key) {
            (offset, size)
        } else {
            let image = image::ImageReader::open(image_path)?.with_guessed_format()?.decode()?;
            let image = image.to_rgba8();
            let (_width, _height) = image.dimensions();

            let (offset, size, _) = self.atlas.append(key, &image);

            self.texture.writable().write(
                (offset.x * 4096.0).convert().unwrap(),
                (offset.y * 4096.0).convert().unwrap(),
                (size.x * 4096.0).convert().unwrap(),
                (size.y * 4096.0).convert().unwrap(),
                image.as_raw(),
                // RawImage2d {
                //     data: Cow::Owned(image.into_raw()),
                //     width,
                //     height,
                //     format: ClientFormat::U8U8U8U8,
                // },
            );

            (offset, size)
        };

        let [mut min, mut max] = [Point2D::ZERO; 2];

        for op in &path.ops {
            match op {
                &Op::Circle { origin, radius } => {
                    min = min.min(origin - radius / 2.0);
                    max = max.min(origin + radius / 2.0);
                }
                Op::Rect(rect) => {
                    min = min.min(rect.origin);
                    max = max.max(rect.origin + rect.size);
                }
                Op::RoundRect(rrect) => {
                    min = min.min(rrect.origin);
                    max = max.max(rrect.origin + rrect.size);
                }
                &Op::Begin(at) => {
                    min = min.min(at);
                    max = max.max(at);
                }
                &Op::LineTo(to) => {
                    min = min.min(to);
                    max = max.max(to);
                }
                &Op::CubicBezierTo(ctrl1, ctrl2, to) => {
                    min = min.min(ctrl1).min(ctrl2).min(to);
                    max = max.max(ctrl1).max(ctrl2).max(to);
                }
                Op::Close => {}
            }
        }

        let origin = min;
        let size = max - min;

        self.tessellator.set_vertex_offsset(self.buffers.vertices.len() as u32);
        self.tessellator.set_uv_rect((offset, uv_size, origin, size));
        self.tessellator
            .tessellate_with_color(Color::WHITE, |builder| {
                for op in path.ops {
                    match op {
                        Op::Circle { origin, radius } => builder.add_circle(bytemuck::cast(origin), radius, Winding::Positive),
                        Op::Rect(rect) => builder.add_rectangle(&bytemuck::cast(rect.to_box2d()), Winding::Positive),
                        Op::RoundRect(rrect) => builder.add_rounded_rectangle(
                            &bytemuck::cast(rrect.as_box()),
                            &BorderRadii {
                                top_left: rrect.corner_radius.top_left(),
                                top_right: rrect.corner_radius.top_right(),
                                bottom_left: rrect.corner_radius.bottom_left(),
                                bottom_right: rrect.corner_radius.bottom_right(),
                            },
                            Winding::Positive,
                        ),
                        Op::Begin(at) => {
                            builder.begin(bytemuck::cast(at));
                        }
                        Op::LineTo(to) => {
                            builder.line_to(bytemuck::cast(to));
                        }
                        Op::CubicBezierTo(ctrl1, ctrl2, to) => {
                            builder.cubic_bezier_to(bytemuck::cast(ctrl1), bytemuck::cast(ctrl2), bytemuck::cast(to));
                        }
                        Op::Close => builder.close(),
                    }
                }
            })
            .unwrap();

        self.tessellator.take_uv_rect();

        let buffers = self.tessellator.build();

        self.buffers.vertices.extend(buffers.vertices.into_iter().map(|mut vertex| {
            vertex.position = self
                .transform
                .as_ref()
                .map_or(vertex.position, |transform| transform.transform_point3(vertex.position));

            vertex
        }));

        self.buffers.indices.extend(buffers.indices);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn draw_path(&mut self, path: Path, color: Color) -> Result<(), TessellationError> {
        self.draw_shape(
            |builder| {
                for op in path.ops {
                    match op {
                        Op::Circle { origin, radius } => builder.add_circle(bytemuck::cast(origin), radius, Winding::Positive),
                        Op::Rect(rect) => builder.add_rectangle(&bytemuck::cast(rect.to_box2d()), Winding::Positive),
                        Op::RoundRect(rrect) => builder.add_rounded_rectangle(
                            &bytemuck::cast(rrect.as_box()),
                            &BorderRadii {
                                top_left: rrect.corner_radius.top_left(),
                                top_right: rrect.corner_radius.top_right(),
                                bottom_left: rrect.corner_radius.bottom_left(),
                                bottom_right: rrect.corner_radius.bottom_right(),
                            },
                            Winding::Positive,
                        ),
                        Op::Begin(at) => {
                            builder.begin(bytemuck::cast(at));
                        }
                        Op::LineTo(to) => {
                            builder.line_to(bytemuck::cast(to));
                        }
                        Op::CubicBezierTo(ctrl1, ctrl2, to) => {
                            builder.cubic_bezier_to(bytemuck::cast(ctrl1), bytemuck::cast(ctrl2), bytemuck::cast(to));
                        }
                        Op::Close => builder.close(),
                    }
                }
            },
            color,
        )
    }

    pub fn draw_shape<F: FnOnce(&mut NoAttributes<FillBuilder>)>(&mut self, tessellate: F, color: Color) -> Result<(), TessellationError> {
        self.tessellator.set_vertex_offsset(self.buffers.vertices.len() as u32);
        self.tessellator.tessellate_with_color(color, tessellate)?;

        let buffers = self.tessellator.build();

        self.buffers.vertices.extend(buffers.vertices.into_iter().map(|mut vertex| {
            vertex.position = self
                .transform
                .as_ref()
                .map_or(vertex.position, |transform| transform.transform_point3(vertex.position));

            if let Some(clip) = self.clip {
                vertex.clip = Vector4D::new(clip.0.x, clip.0.y, clip.1.x, clip.1.y);
            }

            vertex
        }));

        self.buffers.indices.extend(buffers.indices);

        Ok(())
    }

    pub fn draw_text<F: AsRef<str>, T: AsRef<str>>(&mut self, origin: Point2D, font: F, text: T, color: Color, font_size: f32, _max_width: Option<f32>) {
        use swash::{FontRef, scale::ScaleContext};

        self.tessellator.set_vertex_offsset(self.buffers.vertices.len() as u32);

        if let Some(font_index) = self.font_name_map.get(font.as_ref()).copied() {
            use swash::shape::ShapeContext;

            let text = text.as_ref();

            let OwnedFont { data, offset, key, .. } = &self.fonts[font_index];
            let font_ref = FontRef {
                data,
                offset: *offset,
                key: *key,
            };

            let mut shape_context = ShapeContext::new();
            let mut scale_context = ScaleContext::new();
            let mut scaler = scale_context.builder(font_ref).hint(true).size(font_size).build();
            let mut shaper = shape_context.builder(font_ref).size(font_size).build();
            let _metrics = font_ref.glyph_metrics(&[]).scale(font_size);

            shaper.add_str(text);

            self.tessellator
                .tessellate_with_color(color, |b| {
                    let mut x = origin.x;
                    let mut y = origin.y + font_size;

                    shaper.shape_with(|cluster| {
                        use swash::text::cluster::Whitespace;

                        if matches!(cluster.info.whitespace(), Whitespace::Newline) {
                            x = origin.x;
                            y += font_size;
                        }

                        for glyph in cluster.glyphs {
                            if let Some(mut outline) = scaler.scale_outline(glyph.id) {
                                use swash::zeno::{PathData, Transform};

                                outline.transform(&Transform::scale(1.0, -1.0).then_translate(x, y));

                                for command in outline.path().commands() {
                                    match command {
                                        swash::zeno::Command::MoveTo(vector) => {
                                            b.begin(euclid::Point2D::new(vector.x, vector.y));
                                        }
                                        swash::zeno::Command::LineTo(vector) => {
                                            b.line_to(euclid::Point2D::new(vector.x, vector.y));
                                        }
                                        swash::zeno::Command::CurveTo(vector, vector1, vector2) => {
                                            b.cubic_bezier_to(
                                                euclid::Point2D::new(vector.x, vector.y),
                                                euclid::Point2D::new(vector1.x, vector1.y),
                                                euclid::Point2D::new(vector2.x, vector2.y),
                                            );
                                        }
                                        swash::zeno::Command::QuadTo(vector, vector1) => {
                                            b.quadratic_bezier_to(euclid::Point2D::new(vector.x, vector.y), euclid::Point2D::new(vector1.x, vector1.y));
                                        }
                                        swash::zeno::Command::Close => {
                                            b.close();
                                        }
                                    }
                                }
                            }

                            x += cluster.advance();
                        }
                    });
                })
                .unwrap();

            let buffers = self.tessellator.build();

            self.buffers.vertices.extend(buffers.vertices.into_iter().map(|mut vertex| {
                vertex.position = self
                    .transform
                    .as_ref()
                    .map_or(vertex.position, |transform| transform.transform_point3(vertex.position));

                vertex
            }));

            self.buffers.indices.extend(buffers.indices);
        }
    }

    // #[must_use = "RenderInfo itself needs to be extended into other"]
    // pub fn render_lines<S: Surface>(
    //     &mut self,
    //     surface: &mut S,
    //     display: &WindowDisplay,
    //     vertices: &[CommonVertex],
    //     matrix: Option<Transform3D>,
    // ) -> Result<RenderInfo, DrawError> {
    //     let matrix = matrix.or(self.matrix).unwrap_or(self.window_matrix);
    //     let vertex_buffer = VertexBuffer::new(display, vertices).unwrap();
    //     let uniforms = uniform! {
    //         atlas: self.texture
    //             .sampled()
    //             .minify_filter(MinifySamplerFilter::Nearest)
    //             .magnify_filter(MagnifySamplerFilter::Nearest),
    //         resolution:
    // UPoint2D::from_tuple(surface.get_dimensions()).as_::<f32>().to_array(),
    //         matrix: matrix.to_cols_array_2d(),
    //     };

    //     let vertices = vertex_buffer.len();

    //     surface.draw(&vertex_buffer, NoIndices(PrimitiveType::LinesList),
    // &self.shader, &uniforms, &DrawParameters {         blend: BLENDING,
    //         ..DrawParameters::default()
    //     })?;

    //     Ok(RenderInfo { draw_calls: 1, vertices })
    // }

    #[must_use = "RenderInfo itself needs to be extended into other"]
    pub fn render(&mut self, pass: &mut RenderPass, _backend: &RenderBackend, matrix: Option<Transform3D>, size: USize2D) -> RenderInfo {
        let matrix = matrix.or(self.matrix).unwrap_or(self.window_matrix);

        let vertices = self.buffers.vertices.len();
        let count = self.buffers.indices.len();

        self.vbo.dynamic_write(&self.buffers.vertices);
        self.ibo.dynamic_write(&self.buffers.indices);

        self.buffers.clear();

        self.shader
            .bind()
            .with_uniform("atlas", &self.texture)
            .with_uniform("resolution", size.as_vec2().to_array())
            .with_uniform("matrix", matrix);

        pass.apply_params(DrawParams {
            blend: Some(Blend {
                color: (BlendingFactor::SourceAlpha, BlendingFactor::OneMinusSourceAlpha),
                alpha: (BlendingFactor::One, BlendingFactor::OneMinusSourceAlpha),
            }),
            depth: None,
            culling: None,
        });

        pass.draw_elements_slice(&self.vbo, &self.ibo, count, 0);
        pass.reset_params();

        RenderInfo { draw_calls: 1, vertices }
    }
}
