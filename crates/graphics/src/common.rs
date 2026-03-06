use std::borrow::Cow;
#[cfg(feature = "image-rendering")] use std::path::PathBuf;

#[cfg(feature = "text-rendering")]
use ahash::{HashMap, HashMapExt};
#[cfg(feature = "text-rendering")]
use fontdue::{
    Font, FontSettings,
    layout::{CoordinateSystem, GlyphRasterConfig, Layout, TextStyle},
};
use glium::{
    DrawError, DrawParameters, IndexBuffer, Program, Rect, Surface, Texture2d, VertexBuffer,
    index::{NoIndices, PrimitiveType},
    texture::{ClientFormat, RawImage2d, TextureCreationError},
    uniform,
    uniforms::{MagnifySamplerFilter, MinifySamplerFilter},
};
#[cfg(feature = "image-rendering")] use image::ImageBuffer;
#[cfg(feature = "shape-rendering")]
use lyon_tessellation::{
    FillBuilder, FillGeometryBuilder, FillOptions, FillTessellator, FillVertex, GeometryBuilder, GeometryBuilderError, StrokeGeometryBuilder, StrokeVertex,
    TessellationError, VertexId,
    math::Transform,
    path::{
        Winding,
        builder::{BorderRadii, NoAttributes, Transformed},
    },
};
#[cfg(feature = "image-rendering")] use meck::TextureAtlas;
use meralus_engine::WindowDisplay;
#[cfg(feature = "image-rendering")]
use meralus_shared::ConvertTo;
use meralus_shared::{Color, Point2D, Point3D, Transform3D};
#[cfg(feature = "text-rendering")]
use meralus_shared::{ConvertFrom, IntConversionError};
#[cfg(feature = "shape-rendering")]
use meralus_shared::{RRect2D, Rect2D, Size2D, Thickness, Vector2D};

use super::Shader;
use crate::{BLENDING, RenderInfo, VertexBuffers, impl_vertex};

pub struct ShapeShader;

impl Shader for ShapeShader {
    const FRAGMENT: &str = "./resources/shaders/shape.fs";
    const VERTEX: &str = "./resources/shaders/shape.vs";
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CommonVertex {
    pub position: Point3D,
    pub color: Color,
    pub uv: Point2D,
}

impl_vertex! {
    CommonVertex {
        position: [f32; 3],
        color: [u8; 4],
        uv: [f32; 2]
    }
}

#[cfg(feature = "shape-rendering")]
pub struct ShapeGeometryBuilder {
    buffers: VertexBuffers<CommonVertex, u32>,
    first_vertex: u32,
    first_index: u32,
    vertex_offset: u32,
    color: Color,
    pub white_pixel_uv: Point2D,
    uv_rect: Option<(Point2D, Vector2D, Point2D, Size2D)>,
}

#[cfg(feature = "shape-rendering")]
impl ShapeGeometryBuilder {
    pub const fn new(buffers: VertexBuffers<CommonVertex, u32>, white_pixel_uv: Point2D, color: Color) -> Self {
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

    fn build(&mut self) -> VertexBuffers<CommonVertex, u32> {
        let (num_vertices, num_indices) = (self.buffers.vertices.capacity(), self.buffers.indices.capacity());

        self.vertex_offset = 0;

        std::mem::replace(&mut self.buffers, VertexBuffers::with_capacity(num_vertices, num_indices))
    }
}

#[cfg(feature = "shape-rendering")]
impl GeometryBuilder for ShapeGeometryBuilder {
    fn begin_geometry(&mut self) {
        self.first_vertex = self.buffers.vertices.len() as u32;
        self.first_index = self.buffers.indices.len() as u32;
    }

    fn add_triangle(&mut self, a: VertexId, b: VertexId, c: VertexId) {
        debug_assert!(a != b);
        debug_assert!(a != c);
        debug_assert!(b != c);
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

#[cfg(feature = "shape-rendering")]
impl FillGeometryBuilder for ShapeGeometryBuilder {
    fn add_fill_vertex(&mut self, vertex: FillVertex) -> Result<VertexId, GeometryBuilderError> {
        let position = Point2D::from_array(vertex.position().to_array());

        self.buffers.vertices.push(CommonVertex {
            position: position.extend(0.0),
            color: self.color,
            uv: if let Some((offset, uv_size, origin, size)) = self.uv_rect {
                Point2D::new(
                    offset.x + uv_size.x * ((position.x - origin.x) / size.width),
                    offset.y + uv_size.y * ((position.y - origin.y) / size.height),
                )
            } else {
                self.white_pixel_uv
            },
        });

        let len = self.buffers.vertices.len();

        if len > u32::MAX as usize {
            return Err(GeometryBuilderError::TooManyVertices);
        }

        Ok(VertexId((len - 1) as u32))
    }
}

#[cfg(feature = "shape-rendering")]
impl StrokeGeometryBuilder for ShapeGeometryBuilder {
    fn add_stroke_vertex(&mut self, vertex: StrokeVertex) -> Result<VertexId, GeometryBuilderError> {
        self.buffers.vertices.push(CommonVertex {
            position: Point3D::from_array(vertex.position().extend(0.0).to_array()),
            color: self.color,
            uv: self.white_pixel_uv,
        });

        let len = self.buffers.vertices.len();

        if len > u32::MAX as usize {
            return Err(GeometryBuilderError::TooManyVertices);
        }

        Ok(VertexId((len - 1) as u32))
    }
}

#[cfg(feature = "shape-rendering")]
pub struct CommonTessellator {
    pub builder: ShapeGeometryBuilder,
    tessellator: FillTessellator,
    options: FillOptions,
}

#[cfg(feature = "shape-rendering")]
impl CommonTessellator {
    pub fn new(white_pixel_uv: Point2D) -> Self {
        let builder = ShapeGeometryBuilder::new(VertexBuffers::new(), white_pixel_uv, Color::RED);
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

    pub fn build(&mut self) -> VertexBuffers<CommonVertex, u32> {
        self.builder.build()
    }
}

#[derive(Debug)]
#[cfg(feature = "shape-rendering")]
enum Op {
    Circle { origin: Point2D, radius: f32 },
    Rect(Rect2D),
    RoundRect(RRect2D),
    Begin(Point2D),
    LineTo(Point2D),
    CubicBezierTo(Point2D, Point2D, Point2D),
    Close,
}

#[derive(Debug, Default)]
#[cfg(feature = "shape-rendering")]
pub struct Path {
    ops: Vec<Op>,
}

#[cfg(feature = "shape-rendering")]
impl Path {
    pub fn add_circle(&mut self, origin: Point2D, radius: f32) -> &mut Self {
        self.ops.push(Op::Circle { origin, radius });

        self
    }

    pub fn add_rect(&mut self, origin: Point2D, size: Size2D) -> &mut Self {
        self.ops.push(Op::Rect(Rect2D::new(origin, size)));

        self
    }

    pub fn add_round_rect(&mut self, origin: Point2D, size: Size2D, corner_radius: Thickness) -> &mut Self {
        self.ops.push(Op::RoundRect(RRect2D::new(origin, size, corner_radius)));

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
pub enum ObjectFit {
    Stretch,
    Cover,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg(feature = "image-rendering")]
pub enum AtlasKey {
    #[cfg(feature = "text-rendering")]
    Text(Option<GlyphRasterConfig>),
    Image(PathBuf),
    WhitePixel,
}

#[allow(dead_code)]
pub struct CommonRenderer {
    pub(crate) shader: Program,
    #[cfg(feature = "image-rendering")]
    atlas: TextureAtlas<AtlasKey>,
    pub texture: Texture2d,

    // SHAPE RENDERING
    #[cfg(feature = "shape-rendering")]
    pub tessellator: CommonTessellator,

    // TEXT RENDERING
    #[cfg(feature = "text-rendering")]
    layout: Layout,
    #[cfg(feature = "text-rendering")]
    font_name_map: HashMap<String, usize>,
    #[cfg(feature = "text-rendering")]
    fonts: Vec<Font>,

    // COMMON RENDERING
    pub(crate) buffers: VertexBuffers<CommonVertex, u32>,

    // VERTICES TRANSFORMATION
    transform: Option<Transform3D>,

    matrix: Option<Transform3D>,
    window_matrix: Transform3D,
}

#[cfg(feature = "image-rendering")]
const TEXT_BASE_VERTICES: [(Point3D, Point2D); 4] = [
    (Point3D::new(0.0, 1.0, 0.0), Point2D::new(0.0, 1.0)),
    (Point3D::new(0.0, 0.0, 0.0), Point2D::new(0.0, 0.0)),
    (Point3D::new(1.0, 1.0, 0.0), Point2D::new(1.0, 1.0)),
    (Point3D::new(1.0, 0.0, 0.0), Point2D::new(1.0, 0.0)),
];

impl CommonRenderer {
    pub fn new(display: &WindowDisplay) -> Result<Self, TextureCreationError> {
        #[cfg(feature = "image-rendering")]
        let mut atlas = TextureAtlas::new(4096).with_spacing(4);
        #[cfg(feature = "image-rendering")]
        let image = ImageBuffer::from_pixel(24, 24, image::Rgba([255, 255, 255, 255]));

        #[cfg(feature = "image-rendering")]
        let (offset, size, _) = atlas.append(AtlasKey::WhitePixel, &image);
        #[cfg(not(feature = "image-rendering"))]
        let offset = Point2D::ZERO;
        #[cfg(not(feature = "image-rendering"))]
        let size = Point2D::splat(24.0) / 4096.0;

        let texture = Texture2d::empty(display, 4096, 4096)?;

        texture.write(
            Rect {
                left: (offset.x * 4096.0) as u32,
                bottom: (offset.y * 4096.0) as u32,
                width: (size.x * 4096.0) as u32,
                height: (size.y * 4096.0) as u32,
            },
            RawImage2d {
                data: Cow::Owned(vec![255u8; 24 * 24 * 4]),
                width: 24,
                height: 24,
                format: ClientFormat::U8U8U8U8,
            },
        );

        Ok(Self {
            #[cfg(feature = "shape-rendering")]
            tessellator: CommonTessellator::new(offset + (size / 2.0)),
            #[cfg(feature = "image-rendering")]
            atlas,
            texture,
            #[cfg(feature = "text-rendering")]
            layout: Layout::new(CoordinateSystem::PositiveYDown),
            #[cfg(feature = "text-rendering")]
            font_name_map: HashMap::new(),
            #[cfg(feature = "text-rendering")]
            fonts: Vec::new(),
            shader: ShapeShader::program(display),
            buffers: VertexBuffers::new(),
            transform: None,
            window_matrix: Transform3D::IDENTITY,
            matrix: None,
        })
    }

    #[cfg(feature = "text-rendering")]
    pub fn fonts(&self) -> &[Font] {
        &self.fonts
    }

    pub fn window_matrix(&self) -> Transform3D {
        self.window_matrix
    }

    #[cfg(feature = "text-rendering")]
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
    #[cfg(feature = "text-rendering")]
    pub fn add_font<T: Into<String>>(&mut self, name: T, data: &[u8]) {
        if let Ok(font) = Font::from_bytes(data, FontSettings::default()) {
            self.font_name_map.insert(name.into(), self.fonts.len());

            self.fonts.push(font);
        }
    }

    #[cfg(feature = "text-rendering")]
    pub fn measure<F: AsRef<str>, T: AsRef<str>>(&self, font: F, text: T, size: f32, max_width: Option<f32>) -> Option<Size2D> {
        self.font_name_map.get(font.as_ref()).copied().and_then(|font_index| {
            let text = text.as_ref();

            self.layout
                .measure(&self.fonts, &TextStyle::new(text, size, font_index), max_width)
                .into_iter()
                .try_fold(Size2D::ZERO, |mut metrics, glyph| {
                    metrics.width = metrics.width.max(glyph.x + f32::convert_from(glyph.width).ok()?);
                    metrics.height = metrics.height.max(glyph.y + f32::convert_from(glyph.height).ok()?);

                    Some(metrics)
                })
        })
    }

    #[cfg(all(feature = "shape-rendering", feature = "image-rendering"))]
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
            let (width, height) = image.dimensions();

            let (offset, size, _) = self.atlas.append(key, &image);

            self.texture.write(
                Rect {
                    left: (offset.x * 4096.0).convert().unwrap(),
                    bottom: (offset.y * 4096.0).convert().unwrap(),
                    width: (size.x * 4096.0).convert().unwrap(),
                    height: (size.y * 4096.0).convert().unwrap(),
                },
                RawImage2d {
                    data: Cow::Owned(image.into_raw()),
                    width,
                    height,
                    format: ClientFormat::U8U8U8U8,
                },
            );

            (offset, size)
        };

        self.tessellator.set_vertex_offsset(self.buffers.vertices.len() as u32);
        self.tessellator.set_uv_rect((offset, uv_size, origin, size));
        self.tessellator
            .tessellate_with_color(Color::WHITE, |builder| {
                builder.add_rounded_rectangle(
                    &bytemuck::cast(Rect2D::new(origin, size).to_box2()),
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

        self.buffers.vertices.extend(buffers.vertices.into_iter().map(|mut vertice| {
            vertice.position = self
                .transform
                .as_ref()
                .map_or(vertice.position, |transform| transform.transform_point3(vertice.position));

            vertice
        }));

        self.buffers.indices.extend(buffers.indices);

        Ok(())
    }

    #[cfg(feature = "image-rendering")]
    pub fn draw_image<P: AsRef<std::path::Path>>(&mut self, origin: Point2D, size: Size2D, path: P, object_fit: ObjectFit) -> Result<(), image::ImageError> {
        let path = path.as_ref();
        let key = AtlasKey::Image(path.to_path_buf());
        let resulting_scale = size.to_vector() / 4096.0;

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
            let (width, height) = image.dimensions();

            let (offset, size, _) = self.atlas.append(key, &image);

            self.texture.write(
                Rect {
                    left: (offset.x * 4096.0).convert().unwrap(),
                    bottom: (offset.y * 4096.0).convert().unwrap(),
                    width: (size.x * 4096.0).convert().unwrap(),
                    height: (size.y * 4096.0).convert().unwrap(),
                },
                RawImage2d {
                    data: Cow::Owned(image.into_raw()),
                    width,
                    height,
                    format: ClientFormat::U8U8U8U8,
                },
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
            let mut position = (origin + Point2D::new(position.x * size.width, position.y * size.height)).extend(position.z);

            if let Some(transform) = &self.transform {
                position = transform.transform_point3(position);
            }

            CommonVertex {
                position,
                color: Color::WHITE,
                uv: offset + Point2D::new(uv.x * uv_size.x, uv.y * uv_size.y),
            }
        }));

        self.buffers
            .indices
            .extend([0, 1, 2, 3, 2, 1].map(|index| (self.buffers.vertices.len() - TEXT_BASE_VERTICES.len()) as u32 + index));

        Ok(())
    }

    #[cfg(feature = "shape-rendering")]
    pub fn draw_circle(&mut self, origin: Point2D, radius: f32, color: Color) -> Result<(), TessellationError> {
        self.draw_shape(|builder| builder.add_circle(bytemuck::cast(origin), radius, Winding::Positive), color)
    }

    #[cfg(feature = "shape-rendering")]
    pub fn draw_rect(&mut self, origin: Point2D, size: Size2D, color: Color) -> Result<(), TessellationError> {
        self.draw_shape(
            |builder| builder.add_rectangle(&bytemuck::cast(Rect2D::new(origin, size).to_box2()), Winding::Positive),
            color,
        )
    }

    #[cfg(feature = "shape-rendering")]
    pub fn draw_round_rect(&mut self, origin: Point2D, size: Size2D, corner_radius: Thickness, color: Color) -> Result<(), TessellationError> {
        self.draw_shape(
            |builder| {
                builder.add_rounded_rectangle(
                    &bytemuck::cast(Rect2D::new(origin, size).to_box2()),
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

    #[cfg(all(feature = "shape-rendering", feature = "image-rendering"))]
    pub fn draw_image_path<P: AsRef<std::path::Path>>(&mut self, path: Path, image_path: P) -> Result<(), image::ImageError> {
        let image_path = image_path.as_ref();
        let key = AtlasKey::Image(image_path.to_path_buf());

        let (offset, uv_size) = if let Some((offset, size, _)) = self.atlas.get_texture_uv(&key) {
            (offset, size)
        } else {
            let image = image::ImageReader::open(image_path)?.with_guessed_format()?.decode()?;
            let image = image.to_rgba8();
            let (width, height) = image.dimensions();

            let (offset, size, _) = self.atlas.append(key, &image);

            self.texture.write(
                Rect {
                    left: (offset.x * 4096.0).convert().unwrap(),
                    bottom: (offset.y * 4096.0).convert().unwrap(),
                    width: (size.x * 4096.0).convert().unwrap(),
                    height: (size.y * 4096.0).convert().unwrap(),
                },
                RawImage2d {
                    data: Cow::Owned(image.into_raw()),
                    width,
                    height,
                    format: ClientFormat::U8U8U8U8,
                },
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
                    max = max.max(rect.origin + rect.size.to_vector());
                }
                Op::RoundRect(rrect) => {
                    min = min.min(rrect.origin);
                    max = max.max(rrect.origin + rrect.size.to_vector());
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
        let size = (max - min).to_size();

        self.tessellator.set_vertex_offsset(self.buffers.vertices.len() as u32);
        self.tessellator.set_uv_rect((offset, uv_size, origin, size));
        self.tessellator
            .tessellate_with_color(Color::WHITE, |builder| {
                for op in path.ops {
                    match op {
                        Op::Circle { origin, radius } => builder.add_circle(bytemuck::cast(origin), radius, Winding::Positive),
                        Op::Rect(rect) => builder.add_rectangle(&bytemuck::cast(rect.to_box2()), Winding::Positive),
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

        self.buffers.vertices.extend(buffers.vertices.into_iter().map(|mut vertice| {
            vertice.position = self
                .transform
                .as_ref()
                .map_or(vertice.position, |transform| transform.transform_point3(vertice.position));

            vertice
        }));

        self.buffers.indices.extend(buffers.indices);
        Ok(())
    }

    #[cfg(feature = "shape-rendering")]
    pub fn draw_path(&mut self, path: Path, color: Color) -> Result<(), TessellationError> {
        self.draw_shape(
            |builder| {
                for op in path.ops {
                    match op {
                        Op::Circle { origin, radius } => builder.add_circle(bytemuck::cast(origin), radius, Winding::Positive),
                        Op::Rect(rect) => builder.add_rectangle(&bytemuck::cast(rect.to_box2()), Winding::Positive),
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

    #[cfg(feature = "shape-rendering")]
    pub fn draw_shape<F: FnOnce(&mut NoAttributes<FillBuilder>)>(&mut self, tessellate: F, color: Color) -> Result<(), TessellationError> {
        self.tessellator.set_vertex_offsset(self.buffers.vertices.len() as u32);
        self.tessellator.tessellate_with_color(color, tessellate)?;

        let buffers = self.tessellator.build();

        self.buffers.vertices.extend(buffers.vertices.into_iter().map(|mut vertice| {
            vertice.position = self
                .transform
                .as_ref()
                .map_or(vertice.position, |transform| transform.transform_point3(vertice.position));

            vertice
        }));

        self.buffers.indices.extend(buffers.indices);

        Ok(())
    }

    #[cfg(feature = "text-rendering")]
    pub fn draw_text<F: AsRef<str>, T: AsRef<str>>(
        &mut self,
        origin: Point2D,
        font: F,
        text: T,
        color: Color,
        font_size: f32,
        max_width: Option<f32>,
    ) -> Result<(), IntConversionError> {
        if let Some(font_index) = self.font_name_map.get(font.as_ref()).copied() {
            let text = text.as_ref();

            self.layout.clear();
            self.layout.set_max_width(max_width);
            self.layout.append(&self.fonts, &TextStyle::new(text, font_size, font_index));

            let font = &self.fonts[font_index];

            for glyph in self.layout.glyphs() {
                if glyph.width == 0 || glyph.height == 0 {
                    continue;
                }

                let key = AtlasKey::Text(Some(glyph.key));

                let (offset, size) = if let Some((offset, size, _)) = self.atlas.get_texture_uv(&key) {
                    (offset, size)
                } else {
                    let (metrics, bitmap) = font.rasterize_config(glyph.key);

                    let mut image = ImageBuffer::from_pixel(metrics.width.convert()?, metrics.height.convert()?, image::Rgba([0, 0, 0, 255]));
                    let (width, height) = image.dimensions();

                    for (pixel, alpha) in image.pixels_mut().zip(bitmap) {
                        *pixel = image::Rgba([255, 255, 255, alpha]);
                    }

                    let (offset, size, _) = self.atlas.append(key, &image);

                    self.texture.write(
                        Rect {
                            left: (offset.x * 4096.0).convert()?,
                            bottom: (offset.y * 4096.0).convert()?,
                            width: (size.x * 4096.0).convert()?,
                            height: (size.y * 4096.0).convert()?,
                        },
                        RawImage2d {
                            data: Cow::Owned(image.into_raw()),
                            width,
                            height,
                            format: ClientFormat::U8U8U8U8,
                        },
                    );

                    (offset, size)
                };

                self.buffers.vertices.extend(TEXT_BASE_VERTICES.map(|(position, uv)| {
                    let mut position =
                        (origin + Point2D::new(glyph.x, glyph.y) + Point2D::new(position.x * size.x * 4096.0, position.y * size.y * 4096.0)).extend(position.z);

                    if let Some(transform) = &self.transform {
                        position = transform.transform_point3(position);
                    }

                    CommonVertex {
                        position,
                        color,
                        uv: offset + Point2D::new(uv.x * size.x, uv.y * size.y),
                    }
                }));

                self.buffers
                    .indices
                    .extend([0, 1, 2, 3, 2, 1].map(|index| (self.buffers.vertices.len() - TEXT_BASE_VERTICES.len()) as u32 + index));
            }
        }

        Ok(())
    }

    #[must_use = "RenderInfo itself needs to be extended into other"]
    pub fn render_lines<S: Surface>(
        &mut self,
        surface: &mut S,
        display: &WindowDisplay,
        vertices: &[CommonVertex],
        matrix: Option<Transform3D>,
    ) -> Result<RenderInfo, DrawError> {
        let matrix = matrix.or(self.matrix).unwrap_or(self.window_matrix);
        let vertex_buffer = VertexBuffer::new(display, vertices).unwrap();
        let uniforms = uniform! {
            atlas: self.texture
                .sampled()
                .minify_filter(MinifySamplerFilter::Nearest)
                .magnify_filter(MagnifySamplerFilter::Nearest),
            matrix: matrix.to_cols_array_2d(),
        };

        let vertices = vertex_buffer.len();

        surface.draw(&vertex_buffer, NoIndices(PrimitiveType::LinesList), &self.shader, &uniforms, &DrawParameters {
            blend: BLENDING,
            ..DrawParameters::default()
        })?;

        Ok(RenderInfo { draw_calls: 1, vertices })
    }

    #[must_use = "RenderInfo itself needs to be extended into other"]
    pub fn render<S: Surface>(&mut self, surface: &mut S, display: &WindowDisplay, matrix: Option<Transform3D>) -> Result<RenderInfo, DrawError> {
        let matrix = matrix.or(self.matrix).unwrap_or(self.window_matrix);
        let vertex_buffer = VertexBuffer::new(display, &self.buffers.vertices).unwrap();
        let index_buffer = IndexBuffer::new(display, PrimitiveType::TrianglesList, &self.buffers.indices).unwrap();

        self.buffers.clear();

        let uniforms = uniform! {
            atlas: self.texture
                .sampled()
                .minify_filter(MinifySamplerFilter::Nearest)
                .magnify_filter(MagnifySamplerFilter::Nearest),
            matrix: matrix.to_cols_array_2d(),
        };

        let vertices = vertex_buffer.len();

        surface.draw(&vertex_buffer, &index_buffer, &self.shader, &uniforms, &DrawParameters {
            blend: BLENDING,
            ..DrawParameters::default()
        })?;

        Ok(RenderInfo { draw_calls: 1, vertices })
    }
}

#[cfg(all(feature = "shape-rendering", feature = "polymorpher"))]
impl polymorpher::path::PathBuilder for Path {
    type Path = Self;

    fn move_to(&mut self, point: polymorpher::geometry::Point) {
        self.begin(bytemuck::cast(point));
    }

    fn line_to(&mut self, point: polymorpher::geometry::Point) {
        self.line_to(bytemuck::cast(point));
    }

    fn cubic_to(&mut self, ctrl1: polymorpher::geometry::Point, ctrl2: polymorpher::geometry::Point, to: polymorpher::geometry::Point) {
        self.cubic_bezier_to(bytemuck::cast(ctrl1), bytemuck::cast(ctrl2), bytemuck::cast(to));
    }

    fn close(&mut self) {
        self.close();
    }

    fn build(self) -> Self::Path {
        self
    }
}
