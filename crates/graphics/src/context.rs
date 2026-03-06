use std::mem::replace;

use ahash::{HashMap, HashMapExt};
use glium::{
    DrawParameters, IndexBuffer, Surface, Texture2d, VertexBuffer,
    framebuffer::SimpleFrameBuffer,
    index::PrimitiveType,
    uniform,
    uniforms::{MagnifySamplerFilter, MinifySamplerFilter},
};
#[cfg(feature = "shape-rendering")]
use lyon_tessellation::{FillBuilder, TessellationError, path::builder::NoAttributes};
use meralus_engine::WindowDisplay;
use meralus_shared::{Color, Point2D, Point3D, Rect2D, Size2D, Transform3D};
#[cfg(feature = "shape-rendering")]
use meralus_shared::{RRect2D, Thickness};

#[cfg(feature = "image-rendering")] use crate::ObjectFit;
#[cfg(feature = "image-rendering")] use crate::common::Path;
use crate::{BLENDING, CommonRenderer, CommonVertex, VertexBuffers};

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

pub struct RenderContext<'a> {
    window_size: Size2D,
    pub bounds: Rect2D,
    clip: Option<Rect2D>,
    common_renderer: &'a mut CommonRenderer,
    layers: HashMap<usize, (Texture2d, VertexBuffers<CommonVertex, u32>)>,
    current_layer: Option<usize>,
}

#[derive(Clone, Copy)]
#[repr(C)]
#[cfg(feature = "shape-rendering")]
pub struct NativeColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

#[derive(Clone, Copy)]
#[repr(C)]
#[cfg(feature = "shape-rendering")]
pub struct NativeCornerRadius {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_left: f32,
    pub bottom_right: f32,
}

impl<'a> RenderContext<'a> {
    pub fn new(display: &WindowDisplay, common_renderer: &'a mut CommonRenderer) -> Self {
        let (width, height) = display.get_framebuffer_dimensions();

        Self {
            window_size: Size2D::new(width as f32, height as f32),
            bounds: Rect2D::new(Point2D::ZERO, Size2D::new(width as f32, height as f32)),
            clip: None,
            common_renderer,
            layers: HashMap::new(),
            current_layer: None,
        }
    }

    pub const fn get_bounds(&self) -> Rect2D {
        self.bounds
    }

    #[cfg(feature = "text-rendering")]
    pub fn measure_text<F: AsRef<str>, T: AsRef<str>>(&self, font: F, text: T, size: f32, max_width: Option<f32>) -> Option<Size2D> {
        self.common_renderer.measure(font, text, size, max_width)
    }

    #[cfg(feature = "shape-rendering")]
    pub fn tessellate_with_color<F: FnOnce(&mut NoAttributes<FillBuilder>)>(&mut self, color: Color, tessellate: F) -> Result<(), TessellationError> {
        self.common_renderer.draw_shape(tessellate, color)
    }

    #[cfg(feature = "text-rendering")]
    pub fn draw_text<F: Into<String>, T: Into<String>>(&mut self, position: Point2D, font: F, text: T, font_size: f32, color: Color, max_width: Option<f32>) {
        self.common_renderer
            .draw_text(position, font.into(), text.into(), color, font_size, max_width)
            .unwrap();
    }

    pub const fn add_transform(&mut self, transform: Transform3D) {
        self.common_renderer.set_transform(Some(transform));
    }

    pub const fn remove_transform(&mut self) {
        self.common_renderer.set_transform(None);
    }

    #[cfg(feature = "text-rendering")]
    pub fn draw_text_native(&mut self, x: f32, y: f32, font: &&str, text: &&str, font_size: f32, color: &NativeColor) {
        self.common_renderer
            .draw_text(Point2D::new(x, y), font, text, Color::rgb(color.red, color.green, color.blue), font_size, None)
            .unwrap_or_else(|e| panic!("(native) failed to draw text with next params {x}x{y}, {font}-{font_size}, {text}: {e}"));
    }

    #[cfg(feature = "image-rendering")]
    pub fn draw_image_native(&mut self, x: f32, y: f32, w: f32, h: f32, path: &&str, object_fit: &ObjectFit) {
        self.common_renderer
            .draw_image(Point2D::new(x, y), Size2D::new(w, h), path, *object_fit)
            .unwrap_or_else(|e| panic!("(native) failed to draw image with next params {x}x{y}, {w}x{h}, {path}: {e}"));
    }

    #[cfg(all(feature = "shape-rendering", feature = "image-rendering"))]
    pub fn draw_round_image_native(&mut self, x: f32, y: f32, w: f32, h: f32, corner_radius: &NativeCornerRadius, path: &&str) {
        self.common_renderer
            .draw_round_image(
                Point2D::new(x, y),
                Size2D::new(w, h),
                Thickness::new(
                    corner_radius.top_left,
                    corner_radius.top_right,
                    corner_radius.bottom_left,
                    corner_radius.bottom_right,
                ),
                path,
            )
            .unwrap_or_else(|e| panic!("(native) failed to draw rounded image with next params {x}x{y}, {w}x{h}, {path}: {e}"));
    }

    #[cfg(feature = "image-rendering")]
    pub fn draw_image<P: AsRef<std::path::Path>>(&mut self, rectangle: Rect2D, path: P) {
        let path = path.as_ref();

        self.common_renderer
            .draw_image(rectangle.origin, rectangle.size, path, ObjectFit::Stretch)
            .unwrap_or_else(|e| {
                panic!(
                    "(native) failed to draw image with next params {}x{}, {}x{}, {}: {e}",
                    rectangle.origin.x,
                    rectangle.origin.y,
                    rectangle.size.width,
                    rectangle.size.height,
                    path.display()
                )
            });
    }

    #[cfg(all(feature = "shape-rendering", feature = "image-rendering"))]
    pub fn draw_round_image<P: AsRef<std::path::Path>>(&mut self, rectangle: RRect2D, path: P) {
        let path = path.as_ref();

        self.common_renderer
            .draw_round_image(rectangle.origin, rectangle.size, rectangle.corner_radius, path)
            .unwrap_or_else(|e| {
                panic!(
                    "(native) failed to draw rounded image with next params {}x{}, {}x{}, {}: {e}",
                    rectangle.origin.x,
                    rectangle.origin.y,
                    rectangle.size.width,
                    rectangle.size.height,
                    path.display()
                )
            });
    }

    #[cfg(all(feature = "shape-rendering", feature = "image-rendering"))]
    pub fn draw_image_path<P: AsRef<std::path::Path>>(&mut self, path: Path, image_path: P) {
        let image_path = image_path.as_ref();

        self.common_renderer
            .draw_image_path(path, image_path)
            .unwrap_or_else(|e| panic!("(native) failed to draw image path with next params {}: {e}", image_path.display()));
    }

    #[cfg(feature = "shape-rendering")]
    pub fn draw_rrect_native(&mut self, x: f32, y: f32, w: f32, h: f32, corner_radius: &NativeCornerRadius, color: &NativeColor) {
        self.common_renderer
            .draw_round_rect(
                Point2D::new(x, y),
                Size2D::new(w, h),
                Thickness::new(
                    corner_radius.top_left,
                    corner_radius.top_right,
                    corner_radius.bottom_left,
                    corner_radius.bottom_right,
                ),
                Color::rgb(color.red, color.green, color.blue),
            )
            .unwrap();
    }

    #[cfg(feature = "shape-rendering")]
    pub fn draw_rect_native(&mut self, x: f32, y: f32, w: f32, h: f32, color: &NativeColor) {
        self.common_renderer
            .draw_rect(Point2D::new(x, y), Size2D::new(w, h), Color::rgb(color.red, color.green, color.blue))
            .unwrap();
    }

    #[cfg(feature = "shape-rendering")]
    pub fn draw_rect(&mut self, rectangle: Rect2D, color: Color) {
        // if let Some(_transform) = self.matrix {
        //     // let (scale, _, translation) =
        //     // transform.to_scale_rotation_translation();

        //     // self.common_renderer
        //     //     .transformed_tessellate_with_color(
        //     //         color,
        //     //         Transform::scale(scale.x,
        //     // scale.y).then_translate(Vector::new(translation.x,
        //     // translation.y)),         |builder| {
        //     //
        //     // builder.add_rectangle(&bytemuck::cast(rectangle.to_box2()),
        //     // Winding::Positive);         },
        //     //     )
        //     //     .unwrap();
        // } else {
        self.common_renderer.draw_rect(rectangle.origin, rectangle.size, color).unwrap();
        // }
    }

    #[cfg(feature = "shape-rendering")]
    pub fn draw_rounded_rect(&mut self, rectangle: RRect2D, color: Color) {
        // if let Some(_transform) = self.matrix {
        //     // let (scale, _, translation) =
        //     // transform.to_scale_rotation_translation();

        //     // self.tessellator
        //     //     .transformed_tessellate_with_color(
        //     //         color,
        //     //         Transform::scale(scale.x,
        //     // scale.y).then_translate(Vector::new(translation.x,
        //     // translation.y)),         |builder| {
        //     //             builder.add_rounded_rectangle(
        //     //                 &bytemuck::cast(rectangle.as_box()),
        //     //                 &BorderRadii {
        //     //                     top_left: rectangle.corner_radius.left(),
        //     //                     top_right: rectangle.corner_radius.top(),
        //     //                     bottom_left: rectangle.corner_radius.right(),
        //     //                     bottom_right:
        //     // rectangle.corner_radius.bottom(),                 },
        //     //                 Winding::Positive,
        //     //             );
        //     //         },
        //     //     )
        //     //     .unwrap();
        // } else {
        self.common_renderer
            .draw_round_rect(rectangle.origin, rectangle.size, rectangle.corner_radius, color)
            .unwrap();
        // }
    }

    pub fn new_layer(&mut self, display: &WindowDisplay) {
        let layer_idx = self.layers.len();

        self.layers.insert(
            layer_idx,
            (
                Texture2d::empty(display, self.window_size.width as u32, self.window_size.height as u32).unwrap(),
                std::mem::take(&mut self.common_renderer.buffers),
            ),
        );

        self.current_layer = Some(layer_idx);
    }

    pub fn end_render_layer<S: Surface>(&mut self, display: &WindowDisplay, surface: &mut S, color: Color, matrix: Option<Transform3D>) -> Option<RenderInfo> {
        if let Some(layer_idx) = self.current_layer.take() {
            let layer = self.layers.remove(&layer_idx);

            if layer_idx > 0 {
                self.current_layer = Some(layer_idx - 1);
            }

            if let Some((layer, buffers)) = layer {
                let mut buffer = SimpleFrameBuffer::new(display, &layer).unwrap();
                let window_matrix = Transform3D::orthographic_rh_gl(0.0, self.window_size.width, self.window_size.height, 0.0, -1.0, 1.0);
                let _info = self.common_renderer.render(&mut buffer, display, Some(window_matrix)).unwrap();

                let vertices = TEXT_BASE_VERTICES.map(|(position, uv)| CommonVertex {
                    position: (Point2D::ZERO + Point2D::new(position.x * self.window_size.width, position.y * self.window_size.height)).extend(position.z),
                    color,
                    uv,
                });

                let indices: [u32; 6] = [0, 1, 2, 3, 2, 1];

                let vertex_buffer = VertexBuffer::new(display, &vertices).unwrap();
                let index_buffer = IndexBuffer::new(display, PrimitiveType::TrianglesList, &indices).unwrap();
                let matrix = matrix.unwrap_or_else(|| {
                    let (width, height) = surface.get_dimensions();

                    Transform3D::orthographic_rh_gl(0.0, width as f32, height as f32, 0.0, -1.0, 1.0)
                });

                let uniforms = uniform! {
                    atlas: layer
                        .sampled()
                        .minify_filter(MinifySamplerFilter::Nearest)
                        .magnify_filter(MagnifySamplerFilter::Nearest),
                    matrix: matrix.to_cols_array_2d(),
                };

                let vertices = vertex_buffer.len();

                surface
                    .draw(&vertex_buffer, &index_buffer, &self.common_renderer.shader, &uniforms, &DrawParameters {
                        blend: BLENDING,
                        ..DrawParameters::default()
                    })
                    .unwrap();

                self.common_renderer.buffers = buffers;

                Some(RenderInfo { draw_calls: 1, vertices })
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn finish<S: Surface>(self, display: &WindowDisplay, surface: &mut S) -> RenderInfo {
        self.common_renderer.render(surface, display, None).unwrap()
    }

    pub fn ui<F: FnOnce(&mut RenderContext, Rect2D)>(&mut self, func: F) {
        func(self, self.bounds);
    }

    pub fn transformed<F: FnOnce(&mut RenderContext, Rect2D)>(&mut self, transform: Transform3D, func: F) {
        self.add_transform(transform);

        func(self, self.bounds);

        self.remove_transform();
    }

    #[cfg(feature = "shape-rendering")]
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

const TEXT_BASE_VERTICES: [(Point3D, Point2D); 4] = [
    (Point3D::new(0.0, 1.0, 0.0), Point2D::new(0.0, 1.0)),
    (Point3D::new(0.0, 0.0, 0.0), Point2D::new(0.0, 0.0)),
    (Point3D::new(1.0, 1.0, 0.0), Point2D::new(1.0, 1.0)),
    (Point3D::new(1.0, 0.0, 0.0), Point2D::new(1.0, 0.0)),
];
