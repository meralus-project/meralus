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
use meralus_shared::{Color, Point2D, Point3D, Rect2D, Size2D, Transform3D, Vector2D};
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

pub trait ArrangeStrategy {
    fn arrange(&mut self, context: &mut UiContext, widget: WidgetId);
}

pub trait MeasureStrategy {
    #[must_use = "size must be used"]
    fn measure(&mut self, context: &mut UiContext, widget: WidgetId) -> Size2D;
}

pub struct RowStrategy {
    spacing: f32,
}

impl ArrangeStrategy for RowStrategy {
    fn arrange(&mut self, context: &mut UiContext, widget: WidgetId) {
        let mut offset = Point2D::ZERO;

        for w in widget.into_iter(context.all_children(widget)) {
            if context.parent(w) == widget && !context.widgets[w.0].abs_pos {
                let item_size = context.layout_node(w).size;

                context.translate(w, offset);

                offset += Vector2D::new(item_size.width + self.spacing, 0.0);
            }
        }
    }
}

impl MeasureStrategy for RowStrategy {
    fn measure(&mut self, context: &mut UiContext, widget: WidgetId) -> Size2D {
        let mut size = Size2D::ZERO;

        for w in widget.into_iter(context.all_children(widget)) {
            if context.parent(w) == widget && !context.widgets[w.0].abs_pos {
                let item_size = context.layout_node(w).size;

                size.height = size.height.max(item_size.height);
                size.width += item_size.width + self.spacing;
            }
        }

        if size.width > 0.0 {
            size.width -= self.spacing;
        }

        size
    }
}

pub enum Arrangement {
    Start,
    End,
}

pub struct ColumnStrategy {
    spacing: f32,
    arrangement: Arrangement,
}

impl ArrangeStrategy for ColumnStrategy {
    fn arrange(&mut self, context: &mut UiContext, widget: WidgetId) {
        let mut offset = Point2D::ZERO;

        for w in widget.into_iter(context.all_children(widget)) {
            if context.parent(w) == widget && !context.widgets[w.0].abs_pos {
                let item_size = context.layout_node(w).size;

                context.translate(w, offset);

                offset += Vector2D::new(0.0, item_size.height + self.spacing);
            }
        }

        if matches!(self.arrangement, Arrangement::End) {
            let offset = Point2D::Y * (context.layout_node(widget).size.height - offset.y);

            for w in widget.into_iter(context.all_children(widget)) {
                if context.parent(w) == widget && !context.widgets[w.0].abs_pos {
                    context.translate(w, offset);
                }
            }
        }
    }
}

impl MeasureStrategy for ColumnStrategy {
    fn measure(&mut self, context: &mut UiContext, widget: WidgetId) -> Size2D {
        let mut size = Size2D::ZERO;

        for w in widget.into_iter(context.all_children(widget)) {
            if context.parent(w) == widget && !context.widgets[w.0].abs_pos {
                let item_size = context.layout_node(w).size;

                size.width = size.width.max(item_size.width);
                size.height += item_size.height + self.spacing;
            }
        }

        if size.height > 0.0 {
            size.height -= self.spacing;
        }

        size
    }
}

pub struct NoopStrategy;

impl ArrangeStrategy for NoopStrategy {
    fn arrange(&mut self, _: &mut UiContext, _: WidgetId) {}
}

impl MeasureStrategy for NoopStrategy {
    fn measure(&mut self, _: &mut UiContext, _: WidgetId) -> Size2D {
        Size2D::ZERO
    }
}

pub struct FillStrategy;

impl MeasureStrategy for FillStrategy {
    fn measure(&mut self, context: &mut UiContext, widget: WidgetId) -> Size2D {
        context.layout_node(context.parent(widget)).size
    }
}

pub struct SingleChildStrategy;

impl MeasureStrategy for SingleChildStrategy {
    fn measure(&mut self, context: &mut UiContext, widget: WidgetId) -> Size2D {
        context.layout_node(WidgetId(widget.0 + 1)).size
    }
}

pub struct CenterStrategy;

impl ArrangeStrategy for CenterStrategy {
    fn arrange(&mut self, context: &mut UiContext, widget: WidgetId) {
        let root = context.layout_node(widget);

        for w in widget.into_iter(context.all_children(widget)) {
            if context.parent(w) == widget && !context.widgets[w.0].abs_pos {
                let child = context.layout_node(w);

                context.translate(w, ((root.size - child.size) / 2.0).to_vector().to_point().max(Point2D::ZERO));
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct WidgetId(usize);

impl WidgetId {
    pub fn into_iter(self, count: usize) -> impl Iterator<Item = Self> {
        (1..=count).map(move |c| Self(self.0 + c))
    }
}

#[derive(Debug)]
pub enum Shape {
    Noop,
    RRect(Thickness, Color),
    Rect(Color),
    Text(String, f32, &'static str, Color),
}

impl Shape {
    fn paint(&self, renderer: &mut CommonRenderer, node: Rect2D) {
        match self {
            Self::Noop => (),
            &Self::RRect(rounding, color) => _ = renderer.draw_round_rect(node.origin, node.size, rounding, color),
            &Self::Rect(color) => _ = renderer.draw_rect(node.origin, node.size, color),
            Self::Text(text, font_size, font, color) => _ = renderer.draw_text(node.origin, font, text, *color, *font_size, Some(node.size.width)),
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct WidgetState {
    pub clicked: bool,
    pub pointer_entered: bool,
    pub pointer_inside: bool,
    pub pointer_leaved: bool,
}

#[derive(Debug)]
struct WidgetData {
    parent: WidgetId,
    layout_node: Rect2D,
    abs_pos: bool,
    children: usize,
    shape: Shape,
    state: WidgetState,
}

/// Holds UI-related data between UI functions calls
#[derive(Debug)]
pub struct UiContext {
    widgets: Vec<WidgetData>,
}

fn rect_contains(rect: &Rect2D, point: Point2D) -> bool {
    point.x > rect.origin.x && point.x < (rect.origin.x + rect.size.width) && point.y > rect.origin.y && point.y < (rect.origin.y + rect.size.height)
}

impl UiContext {
    pub fn new() -> Self {
        Self {
            widgets: vec![WidgetData {
                parent: WidgetId(0),
                layout_node: Rect2D::ZERO,
                abs_pos: false,
                children: 0,
                shape: Shape::Noop,
                state: WidgetState::default(),
            }],
        }
    }

    pub fn update(&mut self) {
        for w in &mut self.widgets {
            w.state.clicked = false;
        }
    }

    pub fn process_mouse_up(&mut self) {
        for w in &mut self.widgets {
            if w.state.pointer_inside {
                w.state.clicked = true;
            }
        }
    }

    pub fn process_mouse_move(&mut self, position: Point2D) {
        for w in &mut self.widgets {
            if rect_contains(&w.layout_node, position) {
                if w.state.pointer_inside {
                    w.state.pointer_entered = false;
                } else {
                    w.state.pointer_inside = true;
                    w.state.pointer_entered = true;
                }
            } else if w.state.pointer_inside {
                w.state.pointer_inside = false;
                w.state.pointer_entered = false;
                w.state.pointer_leaved = true;
            } else {
                w.state.pointer_leaved = false;
            }
        }
    }

    pub fn translate(&mut self, widget: WidgetId, offset: Point2D) {
        self.widgets[widget.0].layout_node.origin += offset.to_vector();

        for w in widget.into_iter(self.all_children(widget)) {
            if self.parent(w) == widget {
                self.translate(w, offset);
            }
        }
    }

    pub fn set_origin(&mut self, widget: WidgetId, origin: Point2D) {
        self.widgets[widget.0].abs_pos = true;
        self.translate(widget, origin);
    }

    pub fn set_size(&mut self, widget: WidgetId, size: Size2D) {
        self.widgets[widget.0].layout_node.size = size;
    }

    pub fn state(&self, widget: WidgetId) -> WidgetState {
        self.widgets[widget.0].state
    }

    pub fn layout_node(&self, widget: WidgetId) -> Rect2D {
        self.widgets[widget.0].layout_node
    }

    pub fn layout_node_mut(&mut self, widget: WidgetId) -> &mut Rect2D {
        &mut self.widgets[widget.0].layout_node
    }

    pub fn all_children(&self, widget: WidgetId) -> usize {
        self.widgets[widget.0].children
    }

    pub fn parent(&self, widget: WidgetId) -> WidgetId {
        self.widgets[widget.0].parent
    }

    pub fn paint_root(&self, renderer: &mut CommonRenderer) {
        self.paint(renderer, WidgetId(0));
    }

    pub fn paint(&self, renderer: &mut CommonRenderer, widget: WidgetId) {
        let data = &self.widgets[widget.0];

        data.shape.paint(renderer, data.layout_node);

        for w in 1..=self.all_children(widget) {
            self.paint(renderer, WidgetId(widget.0 + w));
        }
    }

    pub fn try_allocate_widget(&mut self, parent: WidgetId, id: WidgetId, shape: Shape, size: Size2D) {
        let widgets = self.widgets.len();

        self.widgets[parent.0].children += 1;

        // only allocate widget if it was not allocated before
        if widgets < (id.0 + 1) {
            self.widgets.push(WidgetData {
                parent,
                layout_node: Rect2D::new(Point2D::ZERO, size),
                abs_pos: false,
                children: 0,
                shape,
                state: WidgetState::default(),
            });
        } else {
            self.widgets[id.0].parent = parent;
            self.widgets[id.0].layout_node = Rect2D::new(Point2D::ZERO, size);
            self.widgets[id.0].abs_pos = false;
            self.widgets[id.0].children = 0;
            self.widgets[id.0].shape = shape;
        }
    }

    pub fn root<'a>(&'a mut self, renderer: &'a CommonRenderer, size: Size2D) -> UiSubcontext<'a, RowStrategy, RowStrategy> {
        self.widgets[0].layout_node = Rect2D::new(Point2D::ZERO, size);
        self.widgets[0].children = 0;
        self.widgets[0].abs_pos = false;
        self.widgets[0].shape = Shape::Noop;

        UiSubcontext {
            id: WidgetId(0),
            next_child_id: WidgetId(1),
            context: self,
            renderer,
            arrange_strategy: RowStrategy { spacing: 0.0 },
            measure_strategy: RowStrategy { spacing: 0.0 },
            explicit_pos: None,
            explicit_height: None,
            explicit_width: None,
        }
    }
}

pub struct UiSubcontext<'a, A: ArrangeStrategy, M: MeasureStrategy> {
    id: WidgetId,
    next_child_id: WidgetId,
    renderer: &'a CommonRenderer,
    pub context: &'a mut UiContext,
    arrange_strategy: A,
    measure_strategy: M,
    explicit_pos: Option<Point2D>,
    explicit_width: Option<f32>,
    explicit_height: Option<f32>,
}

impl UiSubcontext<'_, RowStrategy, RowStrategy> {
    pub const fn set_spacing(&mut self, pixels: f32) {
        self.arrange_strategy.spacing = pixels;
        self.measure_strategy.spacing = pixels;
    }
}

impl UiSubcontext<'_, ColumnStrategy, ColumnStrategy> {
    pub const fn set_arrangement(&mut self, arrangement: Arrangement) {
        self.arrange_strategy.arrangement = arrangement;
    }

    pub const fn set_spacing(&mut self, pixels: f32) {
        self.arrange_strategy.spacing = pixels;
        self.measure_strategy.spacing = pixels;
    }
}

impl<A: ArrangeStrategy, M: MeasureStrategy> UiSubcontext<'_, A, M> {
    const fn next_child(&mut self) -> WidgetId {
        let id = self.next_child_id;

        self.next_child_id.0 += 1;

        id
    }

    pub const fn abs_pos(&mut self, x: f32, y: f32) {
        self.explicit_pos.replace(Point2D::new(x, y));
    }

    pub fn set_width(&mut self, width: f32) {
        self.explicit_width.replace(width);
        self.context.layout_node_mut(self.id).size.width = width;
    }

    pub fn set_height(&mut self, height: f32) {
        self.explicit_height.replace(height);
        self.context.layout_node_mut(self.id).size.height = height;
    }

    pub fn parent_size(&self) -> Size2D {
        let parent = self.context.parent(self.id);

        self.context.layout_node(parent).size
    }

    pub fn part_of_parent_width(&mut self, ratio: f32) {
        let size = self.parent_size();

        self.set_width(size.width * ratio);
    }

    pub fn part_of_parent_height(&mut self, ratio: f32) {
        let size = self.parent_size();

        self.set_height(size.height * ratio);
    }

    pub fn fill_max_size(&mut self) {
        let size = self.parent_size();

        self.set_width(size.width);
        self.set_height(size.height);
    }

    pub fn child(&mut self, shape: Shape) -> WidgetId {
        self.sized_child(Size2D::ZERO, shape)
    }

    pub fn sized_child(&mut self, size: Size2D, shape: Shape) -> WidgetId {
        let id = self.next_child();

        self.context.try_allocate_widget(self.id, id, shape, size);

        id
    }

    fn perform_layout(&mut self) {
        let mut size = self.measure_strategy.measure(self.context, self.id);

        if let Some(width) = self.explicit_width {
            size.width = width;
        }

        if let Some(height) = self.explicit_height {
            size.height = height;
        }

        self.context.set_size(self.id, size);

        if let Some(pos) = self.explicit_pos {
            self.context.set_origin(self.id, pos);
        }

        self.arrange_strategy.arrange(self.context, self.id);
    }

    pub fn add_space(&mut self, space: Size2D) {
        self.sized_child(space, Shape::Noop);
    }

    pub fn set_background_color(&mut self, color: Color) {
        self.context.widgets[self.id.0].shape = Shape::Rect(color);
    }

    pub fn set_rounding(&mut self, thickness: Thickness, color: Color) {
        self.context.widgets[self.id.0].shape = Shape::RRect(thickness, color);
    }

    pub fn scope<SA: ArrangeStrategy, SM: MeasureStrategy>(
        &mut self,
        arrange_strategy: SA,
        measure_strategy: SM,
        ui: impl FnOnce(&mut UiSubcontext<'_, SA, SM>),
    ) -> WidgetState {
        let id = self.child(Shape::Noop);

        let mut scope = UiSubcontext {
            id,
            next_child_id: WidgetId(id.0 + 1),
            context: self.context,
            renderer: self.renderer,
            arrange_strategy,
            measure_strategy,
            explicit_pos: None,
            explicit_height: None,
            explicit_width: None,
        };

        ui(&mut scope);

        self.next_child_id = scope.next_child_id;

        drop(scope);

        self.context.widgets[self.id.0].children += self.context.all_children(id);
        self.context.state(id)
    }

    pub fn row(&mut self, ui: impl FnOnce(&mut UiSubcontext<'_, RowStrategy, RowStrategy>)) {
        self.scope(RowStrategy { spacing: 0.0 }, RowStrategy { spacing: 0.0 }, ui);
    }

    pub fn column(&mut self, ui: impl FnOnce(&mut UiSubcontext<'_, ColumnStrategy, ColumnStrategy>)) {
        self.scope(ColumnStrategy { spacing: 0.0, arrangement: Arrangement::Start }, ColumnStrategy { spacing: 0.0, arrangement: Arrangement::Start }, ui);
    }

    pub fn center(&mut self, ui: impl FnOnce(&mut UiSubcontext<'_, CenterStrategy, SingleChildStrategy>)) {
        self.scope(CenterStrategy, SingleChildStrategy, ui);
    }

    pub fn button(&mut self, ui: impl FnOnce(&mut UiSubcontext<'_, NoopStrategy, SingleChildStrategy>)) -> WidgetState {
        self.scope(NoopStrategy, SingleChildStrategy, ui)
    }

    pub fn rect(&mut self, size: Size2D, color: Color) {
        self.sized_child(size, Shape::Rect(color));
    }

    pub fn text<T: Into<String>>(&mut self, text: T, font_size: f32, font: &'static str, color: Color) {
        let text = text.into();
        let size = self.renderer.measure(font, &text, font_size, None).unwrap_or_default();

        self.sized_child(size, Shape::Text(text, font_size, font, color));
    }

    pub fn rrect(&mut self, size: Size2D, rounding: Thickness, color: Color) {
        self.sized_child(size, Shape::RRect(rounding, color));
    }
}

impl<A: ArrangeStrategy, M: MeasureStrategy> Drop for UiSubcontext<'_, A, M> {
    fn drop(&mut self) {
        self.perform_layout();
    }
}

pub struct RenderContext<'a> {
    common_renderer: &'a mut CommonRenderer,
    window_size: Size2D,
    clip: Option<Rect2D>,
    layers: HashMap<usize, (Texture2d, VertexBuffers<CommonVertex, u32>)>,
    current_layer: Option<usize>,

    pub bounds: Rect2D,
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

    // pub fn end_render_layer<S: Surface>(&mut self, display: &WindowDisplay,
    // surface: &mut S, color: Color, matrix: Option<Transform3D>) ->
    // Option<RenderInfo> {     if let Some(layer_idx) = self.current_layer.take()
    // {         let layer = self.layers.remove(&layer_idx);

    //         if layer_idx > 0 {
    //             self.current_layer = Some(layer_idx - 1);
    //         }

    //         if let Some((layer, buffers)) = layer {
    //             let mut buffer = SimpleFrameBuffer::new(display,
    // &layer).unwrap();             let window_matrix =
    // Transform3D::orthographic_rh_gl(0.0, self.window_size.width,
    // self.window_size.height, 0.0, -1.0, 1.0);             let _info =
    // self.common_renderer.render(&mut buffer, display,
    // Some(window_matrix)).unwrap();

    //             let vertices = TEXT_BASE_VERTICES.map(|(position, uv)|
    // CommonVertex {                 position: (Point2D::ZERO +
    // Point2D::new(position.x * self.window_size.width, position.y *
    // self.window_size.height)).extend(position.z),                 color,
    //                 uv,
    //             });

    //             let indices: [u32; 6] = [0, 1, 2, 3, 2, 1];

    //             let vertex_buffer = VertexBuffer::new(display,
    // &vertices).unwrap();             let index_buffer =
    // IndexBuffer::new(display, PrimitiveType::TrianglesList, &indices).unwrap();
    //             let matrix = matrix.unwrap_or_else(|| {
    //                 let (width, height) = surface.get_dimensions();

    //                 Transform3D::orthographic_rh_gl(0.0, width as f32, height as
    // f32, 0.0, -1.0, 1.0)             });

    //             let uniforms = uniform! {
    //                 atlas: layer
    //                     .sampled()
    //                     .minify_filter(MinifySamplerFilter::Nearest)
    //                     .magnify_filter(MagnifySamplerFilter::Nearest),
    //                 matrix: matrix.to_cols_array_2d(),
    //             };

    //             let vertices = vertex_buffer.len();

    //             surface
    //                 .draw(&vertex_buffer, &index_buffer,
    // &self.common_renderer.shader, &uniforms, &DrawParameters {
    // blend: BLENDING,                     ..DrawParameters::default()
    //                 })
    //                 .unwrap();

    //             self.common_renderer.buffers = buffers;

    //             Some(RenderInfo { draw_calls: 1, vertices })
    //         } else {
    //             None
    //         }
    //     } else {
    //         None
    //     }
    // }

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

        let mut bounds = bounds.to_box2();
        let max_y = bounds.max.y;
        let min_y = bounds.min.y;

        bounds.min.x /= self.window_size.width;
        bounds.min.y = 1.0 - (max_y / self.window_size.height);
        bounds.max.x /= self.window_size.width;
        bounds.max.y = 1.0 - (min_y / self.window_size.height);

        self.common_renderer.clip.replace(bounds.to_array().into());

        func(self, self.bounds);

        self.common_renderer.clip.take();
        self.clip.take();
    }

    pub fn clipped_bounds<F: FnOnce(&mut RenderContext, Rect2D)>(&mut self, bounds: Rect2D, func: F) {
        let tmp = self.bounds;

        self.bounds = bounds;
        self.clip.replace(bounds);

        let mut bounds = bounds.to_box2();
        let max_y = bounds.max.y;
        let min_y = bounds.min.y;

        bounds.min.x /= self.window_size.width;
        bounds.min.y = 1.0 - (max_y / self.window_size.height);
        bounds.max.x /= self.window_size.width;
        bounds.max.y = 1.0 - (min_y / self.window_size.height);

        self.common_renderer.clip.replace(bounds.to_array().into());

        func(self, self.bounds);

        self.common_renderer.clip.take();
        self.clip.take();
        self.bounds = tmp;
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
