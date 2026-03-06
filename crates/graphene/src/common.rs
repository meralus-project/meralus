#[cfg(feature = "image-rendering")] use std::path::PathBuf;
use std::{borrow::Cow, io::Cursor, rc::Rc};
#[cfg(feature = "text-rendering")]
use std::{collections::hash_map::HashMap, num::TryFromIntError};

#[cfg(feature = "text-rendering")]
use fontdue::{
    Font, FontSettings,
    layout::{CoordinateSystem, GlyphRasterConfig, Layout, TextStyle},
};
#[cfg(feature = "image-rendering")] use glam::Vec3Swizzles;
use glam::{Mat4, Vec2, Vec3};
use glow::HasContext;
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

// use meralus_engine::WindowDisplay;
use crate::{Color, ElementType, GlPrimitive, IndexBuffer, Program, Texture2d, Vertex, VertexBuffer};
// #[cfg(feature = "image-rendering")]
// use meralus_shared::ConvertTo;
// #[cfg(feature = "text-rendering")]
// use meralus_shared::{ConvertFrom, IntConversionError};
#[cfg(feature = "shape-rendering")]
use crate::{Point2D, RRect, Rect, Size2D, Thickness};
use crate::{RenderInfo, Shader, VertexBuffers, impl_vertex};

pub struct ShapeShader;

impl Shader for ShapeShader {
    const FRAGMENT: &str = include_str!("../../../resources/shaders/shape-es.fs");
    const VERTEX: &str = include_str!("../../../resources/shaders/shape-es.vs");
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CommonVertex {
    pub position: Vec3,
    pub uv: Vec2,
    pub color: Color,
}

impl_vertex! {
    CommonVertex {
        position: (glow::FLOAT, 3),
        uv: (glow::FLOAT, 2),
        color: (glow::UNSIGNED_BYTE, 4)
    }
}

#[cfg(feature = "shape-rendering")]
pub struct ShapeGeometryBuilder {
    buffers: VertexBuffers<CommonVertex, u32>,
    first_vertex: u32,
    first_index: u32,
    vertex_offset: u32,
    color: Color,
    pub white_pixel_uv: Vec2,
    uv_rect: Option<(Vec2, Vec2, Point2D, Size2D)>,
}

#[cfg(feature = "shape-rendering")]
impl ShapeGeometryBuilder {
    pub const fn new(buffers: VertexBuffers<CommonVertex, u32>, white_pixel_uv: Vec2, color: Color) -> Self {
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

    pub const fn set_uv_rect(&mut self, uv_rect: (Vec2, Vec2, Point2D, Size2D)) {
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
        let position = Vec2::from_array(vertex.position().to_array());

        self.buffers.vertices.push(CommonVertex {
            position: position.extend(0.0),
            color: self.color,
            uv: if let Some((offset, uv_size, origin, size)) = self.uv_rect {
                offset + uv_size * ((position - Vec2::new(origin.x, origin.y)) / Vec2::new(size.width, size.height))
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
            position: Vec3::from_array(vertex.position().extend(0.0).to_array()),
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
    pub fn new(white_pixel_uv: Vec2) -> Self {
        let builder = ShapeGeometryBuilder::new(VertexBuffers::new(), white_pixel_uv, Color::RED);
        let tessellator = FillTessellator::new();
        let options = FillOptions::default();

        Self { builder, tessellator, options }
    }

    pub const fn set_uv_rect(&mut self, uv_rect: (Vec2, Vec2, Point2D, Size2D)) {
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
    Rect(Rect),
    RoundRect(RRect),
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
pub enum ObjectFit {
    Stretch,
    Cover,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg(feature = "image-rendering")]
pub enum AtlasKey {
    #[cfg(feature = "text-rendering")]
    Text(Option<GlyphRasterConfig>),
    #[cfg(feature = "image-rendering")]
    Image(usize),
    ImagePath(PathBuf),
    WhitePixel,
}

#[allow(dead_code)]
pub struct CommonRenderer<T: HasContext> {
    pub shader: Program<T>,
    #[cfg(feature = "image-rendering")]
    atlas: TextureAtlas<AtlasKey>,
    pub texture: Texture2d<T>,
    #[cfg(feature = "image-rendering")]
    images: Vec<(Vec2, Vec2)>,

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
    transform: Option<Mat4>,

    matrix: Option<Mat4>,
    window_matrix: Mat4,

    pub gl: Rc<T>,
}

#[cfg(feature = "image-rendering")]
const TEXT_BASE_VERTICES: [(Vec3, Vec2); 4] = [
    (Vec3::new(0.0, 1.0, 0.0), Vec2::new(0.0, 1.0)),
    (Vec3::new(0.0, 0.0, 0.0), Vec2::new(0.0, 0.0)),
    (Vec3::new(1.0, 1.0, 0.0), Vec2::new(1.0, 1.0)),
    (Vec3::new(1.0, 0.0, 0.0), Vec2::new(1.0, 0.0)),
];

impl<T: HasContext> CommonRenderer<T> {
    pub fn new(gl: T) -> Result<Self, String> {
        #[cfg(feature = "image-rendering")]
        let mut atlas = TextureAtlas::new(4096).with_spacing(4);
        #[cfg(feature = "image-rendering")]
        let image = ImageBuffer::from_pixel(24, 24, image::Rgba([255, 255, 255, 255]));

        #[cfg(feature = "image-rendering")]
        let (offset, size, _) = atlas.append(AtlasKey::WhitePixel, &image);
        #[cfg(not(feature = "image-rendering"))]
        let offset = Vec2::ZERO;
        #[cfg(not(feature = "image-rendering"))]
        let size = Vec2::splat(24.0) / 4096.0;

        let gl = Rc::new(gl);

        let texture = Texture2d::empty(&gl, 4096, 4096).unwrap();

        texture.writable().write(
            (offset.x * 4096.0) as u32,
            (offset.y * 4096.0) as u32,
            (size.x * 4096.0) as u32,
            (size.y * 4096.0) as u32,
            &[255u8; 24 * 24 * 4],
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
            shader: ShapeShader::program(&gl)?,
            buffers: VertexBuffers::new(),
            transform: None,
            window_matrix: Mat4::IDENTITY,
            matrix: None,
            #[cfg(feature = "image-rendering")]
            images: Vec::new(),
            gl,
        })
    }

    #[cfg(feature = "text-rendering")]
    pub fn fonts(&self) -> &[Font] {
        &self.fonts
    }

    #[cfg(feature = "text-rendering")]
    pub const fn white_pixel_uv(&self) -> Vec2 {
        self.tessellator.builder.white_pixel_uv
    }

    pub const fn set_matrix(&mut self, matrix: Mat4) {
        self.matrix = Some(matrix);
    }

    pub const fn set_default_matrix(&mut self) {
        self.matrix = None;
    }

    pub const fn set_window_matrix(&mut self, matrix: Mat4) {
        self.window_matrix = matrix;
    }

    pub const fn set_transform(&mut self, transform: Option<Mat4>) {
        self.transform = transform;
    }

    #[cfg(feature = "image-rendering")]
    pub fn add_image(&mut self, data: Vec<u8>) -> Result<AtlasKey, image::ImageError> {
        let image = image::ImageReader::new(Cursor::new(data)).with_guessed_format()?.decode()?;
        let image = image.to_rgba8();
        let (width, height) = image.dimensions();

        let (offset, size, _) = self.atlas.append(AtlasKey::Image(self.images.len()), &image);

        let key = AtlasKey::Image(self.images.len());

        let texture = self.texture.writable();

        texture.write(
            (offset.x * 4096.0) as u32,
            (offset.y * 4096.0) as u32,
            (size.x * 4096.0) as u32,
            (size.y * 4096.0) as u32,
            image.as_raw(),
        );

        self.images.push((offset, size));

        Ok(key)
    }

    /// # Errors
    ///
    /// Returns [`TextureCreationError`] if texture creation on GPU failed.
    #[cfg(feature = "text-rendering")]
    pub fn add_font<F: Into<String>>(&mut self, name: F, data: &[u8]) {
        if let Ok(font) = Font::from_bytes(data, FontSettings::default()) {
            self.font_name_map.insert(name.into(), self.fonts.len());

            self.fonts.push(font);
        }
    }

    #[cfg(feature = "text-rendering")]
    pub fn measure<F: AsRef<str>, V: AsRef<str>>(&self, font: F, text: V, size: f32, max_width: Option<f32>) -> Option<Size2D> {
        self.font_name_map.get(font.as_ref()).copied().and_then(|font_index| {
            let text = text.as_ref();

            self.layout
                .measure(&self.fonts, &TextStyle::new(text, size, font_index), max_width)
                .into_iter()
                .try_fold(Size2D::ZERO, |mut metrics, glyph| {
                    metrics.width = metrics.width.max(glyph.x + glyph.width as f32);
                    metrics.height = metrics.height.max(glyph.y + glyph.height as f32);

                    Some(metrics)
                })
        })
    }

    #[cfg(all(feature = "shape-rendering", feature = "image-rendering"))]
    pub fn draw_round_image(&mut self, origin: Point2D, size: Size2D, corner_radius: Thickness, key: &AtlasKey) {
        if let Some((offset, uv_size, _)) = self.atlas.get_texture_uv(&key) {
            self.tessellator.set_vertex_offsset(self.buffers.vertices.len() as u32);
            self.tessellator.set_uv_rect((offset, uv_size, origin, size));
            self.tessellator
                .tessellate_with_color(Color::WHITE, |builder| {
                    builder.add_rounded_rectangle(
                        &bytemuck::cast(Rect::new(origin, size).to_box2()),
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
        }
    }

    #[cfg(all(feature = "shape-rendering", feature = "image-rendering"))]
    pub fn draw_round_image_path<P: AsRef<std::path::Path>>(
        &mut self,
        origin: Point2D,
        size: Size2D,
        corner_radius: Thickness,
        path: P,
    ) -> Result<(), image::ImageError> {
        let path = path.as_ref();
        let key = AtlasKey::ImagePath(path.to_path_buf());

        let (offset, uv_size) = if let Some((offset, size, _)) = self.atlas.get_texture_uv(&key) {
            (offset, size)
        } else {
            let image = image::ImageReader::open(path)?.with_guessed_format()?.decode()?;
            let image = image.to_rgba8();
            let (width, height) = image.dimensions();

            let (offset, size, _) = self.atlas.append(key, &image);

            let texture = self.texture.writable();

            texture.write(
                (offset.x * 4096.0) as u32,
                (offset.y * 4096.0) as u32,
                (size.x * 4096.0) as u32,
                (size.y * 4096.0) as u32,
                image.as_raw(),
            );

            (offset, size)
        };

        self.tessellator.set_vertex_offsset(self.buffers.vertices.len() as u32);
        self.tessellator.set_uv_rect((offset, uv_size, origin, size));
        self.tessellator
            .tessellate_with_color(Color::WHITE, |builder| {
                builder.add_rounded_rectangle(
                    &bytemuck::cast(Rect::new(origin, size).to_box2()),
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
    pub fn draw_image(&mut self, origin: Point2D, size: Size2D, key: &AtlasKey, object_fit: ObjectFit) {
        if let Some((offset, uv_size, _)) = self.atlas.get_texture_uv(key) {
            let resulting_size = size.to_raw() / 4096.0;

            let (offset, uv_size) = match object_fit {
                ObjectFit::Stretch => (offset, uv_size),
                ObjectFit::Cover => {
                    let r = resulting_size / uv_size;
                    let ratio = r.max_element();
                    let scaled_size = uv_size * ratio;
                    let mut diff = Vec2::ZERO;

                    if scaled_size.x > resulting_size.x {
                        diff.x = scaled_size.x - resulting_size.x;
                    }

                    if scaled_size.y > resulting_size.y {
                        diff.y = scaled_size.y - resulting_size.y;
                    }

                    (offset + diff / 2.0 / ratio, resulting_size / ratio)
                }
            };

            self.buffers.vertices.extend(TEXT_BASE_VERTICES.map(|(position, uv)| {
                let mut position = (Vec2::new(origin.x, origin.y) + position.xy() * Vec2::new(size.width, size.height)).extend(position.z);

                if let Some(transform) = &self.transform {
                    position = transform.transform_point3(position);
                }

                CommonVertex {
                    position,
                    color: Color::WHITE,
                    uv: offset + (uv * uv_size),
                }
            }));

            self.buffers
                .indices
                .extend([0, 1, 2, 3, 2, 1].map(|index| (self.buffers.vertices.len() - TEXT_BASE_VERTICES.len()) as u32 + index));
        }
    }

    #[cfg(feature = "image-rendering")]
    pub fn draw_image_path<P: AsRef<std::path::Path>>(
        &mut self,
        origin: Point2D,
        size: Size2D,
        path: P,
        object_fit: ObjectFit,
    ) -> Result<(), image::ImageError> {
        let path = path.as_ref();
        let key = AtlasKey::ImagePath(path.to_path_buf());
        let resulting_size = size.to_raw() / 4096.0;

        let (offset, uv_size) = if let Some((offset, size, _)) = self.atlas.get_texture_uv(&key) {
            match object_fit {
                ObjectFit::Stretch => (offset, size),
                ObjectFit::Cover => {
                    let r = resulting_size / size;
                    let ratio = r.max_element();
                    let scaled_size = size * ratio;
                    let mut diff = Vec2::ZERO;

                    if scaled_size.x > resulting_size.x {
                        diff.x = scaled_size.x - resulting_size.x;
                    }

                    if scaled_size.y > resulting_size.y {
                        diff.y = scaled_size.y - resulting_size.y;
                    }

                    (offset + diff / 2.0 / ratio, resulting_size / ratio)
                }
            }
        } else {
            let image = image::ImageReader::open(path)?.with_guessed_format()?.decode()?;
            let image = image.to_rgba8();
            let (width, height) = image.dimensions();

            let (offset, size, _) = self.atlas.append(key, &image);

            let texture = self.texture.writable();

            texture.write(
                (offset.x * 4096.0) as u32,
                (offset.y * 4096.0) as u32,
                (size.x * 4096.0) as u32,
                (size.y * 4096.0) as u32,
                image.as_raw(),
            );

            match object_fit {
                ObjectFit::Stretch => (offset, size),
                ObjectFit::Cover => {
                    let r = resulting_size / size;
                    let ratio = r.max_element();
                    let scaled_size = size * ratio;
                    let mut diff = Vec2::ZERO;

                    if scaled_size.x > resulting_size.x {
                        diff.x = scaled_size.x - resulting_size.x;
                    }

                    if scaled_size.y > resulting_size.y {
                        diff.y = scaled_size.y - resulting_size.y;
                    }

                    (offset + diff / 2.0 / ratio, resulting_size / ratio)
                }
            }
        };

        self.buffers.vertices.extend(TEXT_BASE_VERTICES.map(|(position, uv)| {
            let mut position = (Vec2::new(origin.x, origin.y) + position.xy() * Vec2::new(size.width, size.height)).extend(position.z);

            if let Some(transform) = &self.transform {
                position = transform.transform_point3(position);
            }

            CommonVertex {
                position,
                color: Color::WHITE,
                uv: offset + (uv * uv_size),
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
            |builder| builder.add_rectangle(&bytemuck::cast(Rect::new(origin, size).to_box2()), Winding::Positive),
            color,
        )
    }

    #[cfg(feature = "shape-rendering")]
    pub fn draw_round_rect(&mut self, origin: Point2D, size: Size2D, corner_radius: Thickness, color: Color) -> Result<(), TessellationError> {
        self.draw_shape(
            |builder| {
                builder.add_rounded_rectangle(
                    &bytemuck::cast(Rect::new(origin, size).to_box2()),
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
    pub fn draw_image_in_path(&mut self, path: Path, key: &AtlasKey) {
        if let Some((offset, uv_size, _)) = self.atlas.get_texture_uv(&key) {
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
        }
    }

    #[cfg(all(feature = "shape-rendering", feature = "image-rendering"))]
    pub fn draw_image_path_in_path<P: AsRef<std::path::Path>>(&mut self, path: Path, image_path: P) -> Result<(), image::ImageError> {
        let image_path = image_path.as_ref();
        let key = AtlasKey::ImagePath(image_path.to_path_buf());

        let (offset, uv_size) = if let Some((offset, size, _)) = self.atlas.get_texture_uv(&key) {
            (offset, size)
        } else {
            let image = image::ImageReader::open(image_path)?.with_guessed_format()?.decode()?;
            let image = image.to_rgba8();
            let (width, height) = image.dimensions();

            let (offset, size, _) = self.atlas.append(key, &image);

            let texture = self.texture.writable();

            texture.write(
                (offset.x * 4096.0) as u32,
                (offset.y * 4096.0) as u32,
                (size.x * 4096.0) as u32,
                (size.y * 4096.0) as u32,
                image.as_raw(),
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
    pub fn draw_text<F: AsRef<str>, V: AsRef<str>>(
        &mut self,
        origin: Point2D,
        font: F,
        text: V,
        color: Color,
        font_size: f32,
        max_width: Option<f32>,
    ) -> Result<(), TryFromIntError> {
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

                    let mut image = ImageBuffer::from_pixel(metrics.width as u32, metrics.height as u32, image::Rgba([0, 0, 0, 255]));
                    let (width, height) = image.dimensions();

                    for (pixel, alpha) in image.pixels_mut().zip(bitmap) {
                        *pixel = image::Rgba([255, 255, 255, alpha]);
                    }

                    let (offset, size, _) = self.atlas.append(key, &image);

                    let texture = self.texture.writable();

                    texture.write(
                        (offset.x * 4096.0) as u32,
                        (offset.y * 4096.0) as u32,
                        (size.x * 4096.0) as u32,
                        (size.y * 4096.0) as u32,
                        image.as_raw(),
                    );

                    (offset, size)
                };

                self.buffers.vertices.extend(TEXT_BASE_VERTICES.map(|(position, uv)| {
                    let mut position = (Vec2::new(origin.x, origin.y) + Vec2::new(glyph.x, glyph.y) + position.xy() * (size * 4096.0)).extend(position.z);

                    if let Some(transform) = &self.transform {
                        position = transform.transform_point3(position);
                    }

                    CommonVertex {
                        position,
                        color,
                        uv: offset + (uv * size),
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
    pub fn render_lines(&mut self, vertices: &[CommonVertex], matrix: Option<Mat4>) -> Result<RenderInfo, ()> {
        let matrix = matrix.or(self.matrix).unwrap_or(self.window_matrix);

        unsafe {
            let u_atlas = self.gl.get_uniform_location(self.shader.ptr, "atlas").unwrap();
            let u_matrix = self.gl.get_uniform_location(self.shader.ptr, "matrix").unwrap();

            self.gl.use_program(Some(self.shader.ptr));
            self.gl.uniform_1_i32(Some(&u_atlas), 0);
            self.gl.uniform_matrix_4_f32_slice(Some(&u_matrix), false, &matrix.to_cols_array());
            self.gl.active_texture(glow::TEXTURE0);
        }

        let vertex_buffer = unsafe {
            let buffer = self.gl.create_buffer().unwrap();
            let array = self.gl.create_vertex_array().unwrap();

            self.gl.bind_vertex_array(Some(array));
            self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(buffer));

            let stride = std::mem::size_of::<CommonVertex>() as i32;

            for (name, offset, (ty, size), normalized) in CommonVertex::BINDINGS {
                let loc = self.shader.attributes.get(name.as_ref()).copied().unwrap();

                self.gl.vertex_attrib_pointer_f32(loc, *size, *ty, *normalized, stride, *offset as i32);
            }

            self.gl
                .buffer_data_u8_slice(glow::ARRAY_BUFFER, bytemuck::cast_slice(vertices), glow::STATIC_DRAW);

            array
        };

        let vertices = vertices.len();

        unsafe {
            self.gl.bind_vertex_array(Some(vertex_buffer));
            self.texture.bind();
            self.gl.draw_arrays(glow::LINES, 0, vertices as i32);
            self.texture.unbind();
        }

        Ok(RenderInfo { draw_calls: 1, vertices })
    }

    fn draw_elements<V: Vertex + bytemuck::NoUninit, I: GlPrimitive + bytemuck::NoUninit>(
        &self,
        vertex_buffer: &VertexBuffer<T, V>,
        index_buffer: &IndexBuffer<T, I>,
        count: usize,
        offset: usize,
    ) {
        vertex_buffer.bind();
        index_buffer.bind();

        unsafe {
            self.gl
                .draw_elements(index_buffer.element_type.as_gl(), count as i32, I::gl_code(), offset as i32);
        }

        index_buffer.unbind();
        vertex_buffer.unbind();
    }

    #[must_use = "RenderInfo itself needs to be extended into other"]
    pub fn render(&mut self, matrix: Option<Mat4>) -> Result<RenderInfo, ()> {
        let matrix = matrix.or(self.matrix).unwrap_or(self.window_matrix);
        let vertex_buffer = VertexBuffer::new(&self.gl, &self.shader, &self.buffers.vertices).unwrap();
        let index_buffer = IndexBuffer::new(&self.gl, ElementType::Triangles, &self.buffers.indices).unwrap();

        let vertices = self.buffers.vertices.len();
        let count = self.buffers.indices.len();

        self.buffers.clear();

        unsafe {
            self.gl.enable(glow::BLEND);
            self.gl
                .blend_func_separate(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA, glow::ONE, glow::ONE_MINUS_SRC_ALPHA);

            self.shader.bind().with_uniform("atlas", 0).with_uniform("matrix", matrix);

            self.gl.active_texture(glow::TEXTURE0);
        }

        self.texture.bind();
        self.draw_elements(&vertex_buffer, &index_buffer, count, 0);
        self.texture.unbind();

        Ok(RenderInfo { draw_calls: 1, vertices })
    }
}

pub enum FilterMode {
    Blur(u16, [i32; 2]),
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
