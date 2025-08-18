use std::borrow::Borrow;

use ahash::{HashMap, HashMapExt};
use fontdue::{
    Font, FontSettings,
    layout::{CoordinateSystem, GlyphRasterConfig, Layout, TextStyle},
};
use glam::{Mat4, Vec2, Vec3, vec2, vec3};
use glium::{
    DrawParameters, Frame, Program, Rect, Surface, Texture2d, VertexBuffer,
    index::{NoIndices, PrimitiveType},
    texture::{RawImage2d, TextureCreationError},
    uniform,
    uniforms::MagnifySamplerFilter,
    vertex::BufferCreationError,
};
use image::ImageBuffer;
use meck::TextureAtlas;
use meralus_engine::WindowDisplay;
use meralus_shared::{Color, FromValue, Point2D, Size2D};

use super::Shader;
use crate::{BLENDING, context::RenderInfo, impl_vertex};

pub const FONT: &[u8] = include_bytes!("../../app/resources/PixeloidSans.ttf");
pub const FONT_BOLD: &[u8] = include_bytes!("../../app/resources/PixeloidSans-Bold.ttf");

struct TextShader;

impl Shader for TextShader {
    const FRAGMENT: &str = include_str!("../../app/resources/shaders/text.fs");
    const VERTEX: &str = include_str!("../../app/resources/shaders/text.vs");
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextVertex {
    pub position: Vec3,
    pub character: Vec2,
}

impl_vertex! {
    TextVertex {
        position: [f32; 3],
        character: [f32; 2]
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextDataVertex {
    pub screen_position: Point2D,
    pub offset: Vec2,
    pub size: Vec2,
}

impl TextDataVertex {
    pub const fn from_vec(screen_position: Point2D, offset: Vec2, size: Vec2) -> Self {
        Self { screen_position, offset, size }
    }
}

impl_vertex! {
    TextDataVertex {
        screen_position: [f32; 2],
        offset: [f32; 2],
        size: [f32; 2]
    }
}

pub struct FontInfo {
    pub font: Font,
    pub atlas: TextureAtlas<GlyphRasterConfig>,
    pub texture: Texture2d,
}

impl Borrow<Font> for FontInfo {
    fn borrow(&self) -> &Font {
        &self.font
    }
}

pub struct TextRenderer {
    character: VertexBuffer<TextVertex>,
    character_offset: VertexBuffer<TextDataVertex>,
    font_name_map: HashMap<String, usize>,
    fonts: Vec<FontInfo>,
    layout: Layout,
    shader: Program,
}

impl TextRenderer {
    pub fn new(display: &WindowDisplay, character_limit: usize) -> Result<Self, BufferCreationError> {
        let character = VertexBuffer::new(display, &[
            TextVertex {
                position: vec3(0.0, 1.0, 0.0),
                character: vec2(0.0, 0.0),
            },
            TextVertex {
                position: vec3(0.0, 0.0, 0.0),
                character: vec2(0.0, 1.0),
            },
            TextVertex {
                position: vec3(1.0, 1.0, 0.0),
                character: vec2(1.0, 0.0),
            },
            TextVertex {
                position: vec3(1.0, 0.0, 0.0),
                character: vec2(1.0, 1.0),
            },
        ])?;

        let character_offset = VertexBuffer::dynamic(
            display,
            &(0..character_limit)
                .map(|_| TextDataVertex::from_vec(Point2D::ZERO, Vec2::ZERO, Vec2::ZERO))
                .collect::<Vec<_>>(),
        )?;

        Ok(Self {
            layout: Layout::new(CoordinateSystem::PositiveYDown),
            character,
            character_offset,
            font_name_map: HashMap::new(),
            fonts: Vec::new(),
            shader: TextShader::program(display),
        })
    }

    pub fn fonts(&self) -> &[FontInfo] {
        &self.fonts
    }

    /// # Errors
    ///
    /// Returns [`TextureCreationError`] if texture creation on GPU failed.
    pub fn add_font<T: Into<String>>(&mut self, display: &WindowDisplay, name: T, data: &[u8]) -> Result<(), TextureCreationError> {
        if let Ok(font) = Font::from_bytes(data, FontSettings::default()) {
            self.font_name_map.insert(name.into(), self.fonts.len());

            self.fonts.push(FontInfo {
                font,
                atlas: TextureAtlas::new(4096).with_spacing(4),
                texture: Texture2d::empty(display, 4096, 4096)?,
            });
        }

        Ok(())
    }

    pub fn measure<F: AsRef<str>, T: AsRef<str>>(&self, font: F, text: T, size: f32, max_width: Option<f32>) -> Option<Size2D> {
        self.font_name_map.get(font.as_ref()).copied().map(|font_index| {
            let text = text.as_ref();

            self.layout
                .measure(&self.fonts, &TextStyle::new(text, size, font_index), max_width)
                .into_iter()
                .fold(Size2D::ZERO, |mut metrics, glyph| {
                    metrics.width = metrics.width.max(glyph.x + glyph.width as f32);
                    metrics.height = metrics.height.max(glyph.y + glyph.height as f32);

                    metrics
                })
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render<S: Surface, F: AsRef<str>, T: AsRef<str>>(
        &mut self,
        frame: &mut S,
        matrix: &Mat4,
        position: Point2D,
        font: F,
        text: T,
        size: f32,
        max_width: Option<f32>,
        color: Color,
        clip_area: Option<Rect>,
    ) -> RenderInfo {
        let mut render_info = RenderInfo::default();

        if let Some(font_index) = self.font_name_map.get(font.as_ref()).copied() {
            let text = text.as_ref();

            self.layout.clear();
            self.layout.set_max_width(max_width);
            self.layout.append(&self.fonts, &TextStyle::new(text, size, font_index));

            let glyphs = self.layout.glyphs();

            let font_info = &mut self.fonts[font_index];

            for (i, vertex) in self.character_offset.map().iter_mut().enumerate() {
                if let Some(glyph) = glyphs.get(i) {
                    if glyph.width == 0 || glyph.height == 0 {
                        vertex.screen_position = Point2D::ZERO;
                        vertex.offset = Vec2::ZERO;
                        vertex.size = Vec2::ZERO;

                        continue;
                    }

                    let (offset, size, _) = if let Some(rect) = font_info.atlas.get_texture_uv(&glyph.key) {
                        rect
                    } else {
                        let (metrics, bitmap) = font_info.font.rasterize(glyph.parent, size);

                        let mut image = ImageBuffer::new(metrics.width as u32, metrics.height as u32);

                        for (i, pixel) in image.pixels_mut().enumerate() {
                            let alpha = bitmap[i];

                            *pixel = image::Rgba([255, 255, 255, alpha]);
                        }

                        let result = font_info.atlas.append(glyph.key, &image).unwrap();

                        font_info.texture.write(
                            Rect {
                                left: (result.0.x * 4096.0) as u32,
                                bottom: (result.0.y * 4096.0) as u32,
                                width: (result.1.x * 4096.0) as u32,
                                height: (result.1.y * 4096.0) as u32,
                            },
                            RawImage2d::from_raw_rgba_reversed(image.as_raw(), image.dimensions()),
                        );

                        result
                    };

                    vertex.screen_position = position + Point2D::new(glyph.x, glyph.y);

                    vertex.offset = offset;
                    vertex.size = size;
                } else {
                    vertex.screen_position = Point2D::ZERO;
                    vertex.offset = Vec2::ZERO;
                    vertex.size = Vec2::ZERO;
                }
            }

            let uniforms = uniform! {
                matrix: matrix.to_cols_array_2d(),
                font: font_info
                    .texture
                    .sampled()
                    .magnify_filter(MagnifySamplerFilter::Nearest),
                text_color: <[f32; 4]>::from_value(&color),
            };

            frame
                .draw(
                    (&self.character, self.character_offset.per_instance().unwrap()),
                    NoIndices(PrimitiveType::TriangleStrip),
                    &self.shader,
                    &uniforms,
                    &DrawParameters {
                        blend: BLENDING,
                        scissor: clip_area,
                        ..Default::default()
                    },
                )
                .expect("failed to draw!");

            render_info.vertices += self.character_offset.len();
            render_info.draw_calls += 1;
        }

        render_info
    }
}
