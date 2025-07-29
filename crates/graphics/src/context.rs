use std::mem::replace;

use glam::{Mat2, Mat4};
use glium::{Frame, IndexBuffer, Rect, VertexBuffer};
use lyon_tessellation::{
    FillBuilder, TessellationError,
    math::{Transform, Vector},
    path::{
        Winding,
        builder::{BorderRadii, NoAttributes},
    },
};
use meralus_engine::WindowDisplay;
use meralus_shared::{Color, Point2D, RRect2D, Rect2D, Size2D};

use crate::{ShapeRenderer, ShapeTessellator, ShapeVertex, TextRenderer};

pub struct RenderInfo {
    pub draw_calls: usize,
    pub vertices: usize,
}

impl RenderInfo {
    pub const fn default() -> Self {
        Self { draw_calls: 0, vertices: 0 }
    }

    pub const fn extend(&mut self, other: &Self) {
        self.draw_calls += other.draw_calls;
        self.vertices += other.vertices;
    }

    #[must_use]
    pub const fn take(&mut self) -> Self {
        Self {
            draw_calls: replace(&mut self.draw_calls, 0),
            vertices: replace(&mut self.vertices, 0),
        }
    }
}

struct Text {
    position: Point2D,
    font: String,
    data: String,
    size: f32,
    color: Color,
    clip: Option<Rect2D>,
    matrix: Option<Mat4>,
    max_width: Option<f32>,
}

pub struct RenderContext {
    window_size: Size2D,
    bounds: Rect2D,
    texts: Vec<Text>,
    clip: Option<Rect2D>,
    matrix: Option<Mat4>,
    tessellator: ShapeTessellator,
}

impl RenderContext {
    pub fn new(display: &WindowDisplay) -> Self {
        let (width, height) = display.get_framebuffer_dimensions();

        Self {
            window_size: Size2D::new(width as f32, height as f32),
            bounds: Rect2D::new(Point2D::ZERO, Size2D::new(width as f32, height as f32)),
            texts: Vec::new(),
            clip: None,
            matrix: None,
            tessellator: ShapeTessellator::new(),
        }
    }

    pub fn tessellate_with_color<F: FnOnce(NoAttributes<FillBuilder>) -> Result<(), TessellationError>>(&mut self, color: Color, tessellate: F) {
        self.tessellator.tessellate_with_color(color, tessellate).unwrap();
    }

    // pub fn draw_shape(&mut self, vertex_buffer: &VertexBuffer<ShapeVertex>, index_buffer: &IndexBuffer<u32>) {
    //     self.shape_renderer.draw(self.frame, self.display, vertex_buffer, index_buffer);
    // }

    // pub const fn text_renderer(&self) -> &TextRenderer {
    //     self.text_renderer
    // }

    pub const fn get_bounds(&self) -> Rect2D {
        self.bounds
    }

    // pub fn measure_text<F: AsRef<str>, T: AsRef<str>>(&self, font: F, text: T, size: f32) -> Option<Size2D> {
    //     self.text_renderer.measure(font, text, size, None)
    // }

    pub fn draw_text<F: Into<String>, T: Into<String>>(&mut self, position: Point2D, font: F, text: T, size: f32, color: Color, max_width: Option<f32>) {
        self.texts.push(Text {
            position,
            font: font.into(),
            data: text.into(),
            size,
            color,
            clip: self.clip,
            matrix: self.matrix,
            max_width,
        });
    }

    pub const fn add_transform(&mut self, transform: Mat4) {
        self.matrix.replace(transform);
    }

    pub const fn remove_transform(&mut self) {
        self.matrix.take();
    }

    pub fn draw_rect(&mut self, rectangle: Rect2D, color: Color) {
        if let Some(transform) = self.matrix {
            let (scale, _, translation) = transform.to_scale_rotation_translation();

            self.tessellator
                .transformed_tessellate_with_color(
                    color,
                    Transform::scale(scale.x, scale.y).then_translate(Vector::new(translation.x, translation.y)),
                    |builder| {
                        builder.add_rectangle(&bytemuck::cast(rectangle.to_box2()), Winding::Positive);
                    },
                )
                .unwrap();
        } else {
            self.tessellator
                .tessellate_with_color(color, |mut builder| {
                    builder.add_rectangle(&bytemuck::cast(rectangle.to_box2()), Winding::Positive);

                    builder.build()
                })
                .unwrap();
        }
    }

    pub fn draw_rounded_rect(&mut self, rectangle: RRect2D, color: Color) {
        if let Some(transform) = self.matrix {
            let (scale, _, translation) = transform.to_scale_rotation_translation();

            self.tessellator
                .transformed_tessellate_with_color(
                    color,
                    Transform::scale(scale.x, scale.y).then_translate(Vector::new(translation.x, translation.y)),
                    |builder| {
                        builder.add_rounded_rectangle(
                            &bytemuck::cast(rectangle.as_box()),
                            &BorderRadii {
                                top_left: rectangle.corner_radius.left(),
                                top_right: rectangle.corner_radius.top(),
                                bottom_left: rectangle.corner_radius.right(),
                                bottom_right: rectangle.corner_radius.bottom(),
                            },
                            Winding::Positive,
                        );
                    },
                )
                .unwrap();
        } else {
            self.tessellator
                .tessellate_with_color(color, |mut builder| {
                    builder.add_rounded_rectangle(
                        &bytemuck::cast(rectangle.as_box()),
                        &BorderRadii {
                            top_left: rectangle.corner_radius.left(),
                            top_right: rectangle.corner_radius.top(),
                            bottom_left: rectangle.corner_radius.right(),
                            bottom_right: rectangle.corner_radius.bottom(),
                        },
                        Winding::Positive,
                    );

                    builder.build()
                })
                .unwrap();
        }
    }

    pub fn finish(
        self,
        shape_renderer: &mut ShapeRenderer,
        text_renderer: &mut TextRenderer,
        display: &WindowDisplay,
        frame: &mut Frame,
        window_matrix: Mat4,
    ) -> RenderInfo {
        let mut render_info = RenderInfo::default();

        let (v, i) = self.tessellator.build(display);

        if v.len() > 0 {
            shape_renderer.draw(frame, display, &v, &i);

            render_info.draw_calls += 1;
            render_info.vertices += v.len();
        }

        for text in self.texts {
            render_info.extend(&text_renderer.render(
                frame,
                &(window_matrix * text.matrix.unwrap_or_default()),
                text.position,
                text.font,
                text.data,
                text.size,
                text.max_width,
                text.color,
                text.clip.map(|area| Rect {
                    left: area.origin.x.floor() as u32,
                    bottom: (self.window_size.height - area.origin.y - area.size.height).floor() as u32,
                    width: area.size.width.floor() as u32,
                    height: area.size.height.floor() as u32,
                }),
            ));
        }

        render_info
    }

    pub fn ui<F: FnOnce(&mut RenderContext, Rect2D)>(&mut self, func: F) {
        func(self, self.bounds);
    }

    pub fn transformed<F: FnOnce(&mut RenderContext, Rect2D)>(&mut self, transform: Mat4, func: F) {
        self.add_transform(transform);

        func(self, self.bounds);

        self.remove_transform();
    }

    pub fn fill(&mut self, color: Color) {
        self.draw_rect(self.bounds, color);
    }

    pub fn clipped<F: FnOnce(&mut RenderContext, Rect2D)>(&mut self, bounds: Rect2D, func: F) {
        self.clip.replace(bounds);

        func(self, self.bounds);

        self.clip.take();
    }

    pub fn clipped_bounds<F: FnOnce(&mut RenderContext, Rect2D)>(&mut self, bounds: Rect2D, func: F) {
        let tmp = self.bounds;

        self.clip.replace(bounds);
        self.bounds = bounds;

        func(self, self.bounds);

        self.bounds = tmp;
        self.clip.take();
    }

    pub fn bounds<F: FnOnce(&mut RenderContext, Rect2D)>(&mut self, bounds: Rect2D, func: F) {
        let tmp = self.bounds;

        self.bounds = bounds;

        func(self, self.bounds);

        self.bounds = tmp;
    }

    pub fn padding<F: FnOnce(&mut RenderContext, Rect2D)>(&mut self, value: f32, func: F) {
        self.bounds.origin += Point2D::ONE * value;
        self.bounds.size -= Size2D::ONE * value * 2.0;
        self.bounds.size = self.bounds.size.max(Size2D::ZERO);

        func(self, self.bounds);

        self.bounds.origin -= Point2D::ONE.to_vector() * value;
        self.bounds.size += Size2D::ONE * value * 2.0;
    }
}
