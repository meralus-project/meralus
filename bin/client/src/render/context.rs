use mavelin_shared::{Color, Rect, Thickness};

use crate::render::common::CommonRenderer;

pub trait ArrangeStrategy {
    fn arrange(&mut self, context: &mut UiContext, widget: WidgetId);
}

pub trait MeasureStrategy {
    #[must_use = "size must be used"]
    fn measure(&mut self, context: &mut UiContext, widget: WidgetId) -> glam::Vec2;
}

pub struct RowStrategy {
    spacing: f32,
}

impl ArrangeStrategy for RowStrategy {
    fn arrange(&mut self, context: &mut UiContext, widget: WidgetId) {
        let mut offset = glam::Vec2::ZERO;

        for w in widget.into_iter(context.all_children(widget)) {
            if context.parent(w) == widget && !context.widgets[w.0].abs_pos {
                let item_size = context.layout_node(w).size;

                context.translate(w, offset);

                offset += glam::Vec2::new(item_size.x + self.spacing, 0.0);
            }
        }
    }
}

impl MeasureStrategy for RowStrategy {
    fn measure(&mut self, context: &mut UiContext, widget: WidgetId) -> glam::Vec2 {
        let mut size = glam::Vec2::ZERO;

        for w in widget.into_iter(context.all_children(widget)) {
            if context.parent(w) == widget && !context.widgets[w.0].abs_pos {
                let item_size = context.layout_node(w).size;

                size.y = size.y.max(item_size.y);
                size.x += item_size.x + self.spacing;
            }
        }

        if size.x > 0.0 {
            size.x -= self.spacing;
        }

        size
    }
}

#[allow(dead_code)]
pub enum Arrangement {
    Start,
    Center,
    End,
    Stretch,
}

pub struct ColumnStrategy {
    spacing: f32,
    v_arrangement: Arrangement,
    h_arrangement: Arrangement,
}

impl ArrangeStrategy for ColumnStrategy {
    fn arrange(&mut self, context: &mut UiContext, widget: WidgetId) {
        let mut offset = glam::Vec2::ZERO;

        for w in widget.into_iter(context.all_children(widget)) {
            if context.parent(w) == widget && !context.widgets[w.0].abs_pos {
                let item_size = context.layout_node(w).size;

                context.translate(w, offset);

                offset += glam::Vec2::new(0.0, item_size.y + self.spacing);
            }
        }

        if matches!(self.v_arrangement, Arrangement::End) {
            let offset = glam::Vec2::Y * (context.layout_node(widget).size.y - offset.y);

            for w in widget.into_iter(context.all_children(widget)) {
                if context.parent(w) == widget && !context.widgets[w.0].abs_pos {
                    context.translate(w, offset);
                }
            }
        }

        match self.h_arrangement {
            Arrangement::End => {
                let offset = glam::Vec2::X * (context.layout_node(widget).size.x - offset.x);

                for w in widget.into_iter(context.all_children(widget)) {
                    if context.parent(w) == widget && !context.widgets[w.0].abs_pos {
                        context.translate(w, offset - context.layout_node(w).size.with_y(0.0));
                    }
                }
            }
            Arrangement::Center => {
                let parent_size = context.layout_node(widget).size.with_y(0.0);

                for w in widget.into_iter(context.all_children(widget)) {
                    if context.parent(w) == widget && !context.widgets[w.0].abs_pos {
                        context.translate(w, (parent_size - context.layout_node(w).size.with_y(0.0)) / 2.0);
                    }
                }
            }
            Arrangement::Stretch => {
                let parent_size = context.layout_node(widget).size.x;

                for w in widget.into_iter(context.all_children(widget)) {
                    if context.parent(w) == widget && !context.widgets[w.0].abs_pos {
                        context.layout_node_mut(w).size.x = parent_size;
                    }
                }
            }
            Arrangement::Start => (),
        }
    }
}

impl MeasureStrategy for ColumnStrategy {
    fn measure(&mut self, context: &mut UiContext, widget: WidgetId) -> glam::Vec2 {
        let mut size = glam::Vec2::ZERO;

        for w in widget.into_iter(context.all_children(widget)) {
            if context.parent(w) == widget && !context.widgets[w.0].abs_pos {
                let item_size = context.layout_node(w).size;

                size.x = size.x.max(item_size.x);
                size.y += item_size.y + self.spacing;
            }
        }

        if size.y > 0.0 {
            size.y -= self.spacing;
        }

        size
    }
}

pub struct NoopStrategy;

impl ArrangeStrategy for NoopStrategy {
    fn arrange(&mut self, _: &mut UiContext, _: WidgetId) {}
}

impl MeasureStrategy for NoopStrategy {
    fn measure(&mut self, _: &mut UiContext, _: WidgetId) -> glam::Vec2 {
        glam::Vec2::ZERO
    }
}

#[allow(dead_code)]
pub struct FillStrategy;

impl MeasureStrategy for FillStrategy {
    fn measure(&mut self, context: &mut UiContext, widget: WidgetId) -> glam::Vec2 {
        context.layout_node(context.parent(widget)).size
    }
}

pub struct SingleChildStrategy;

impl MeasureStrategy for SingleChildStrategy {
    fn measure(&mut self, context: &mut UiContext, widget: WidgetId) -> glam::Vec2 {
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

                context.translate(w, ((root.size - child.size) / 2.0).max(glam::Vec2::ZERO));
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
    #[allow(dead_code)]
    RRect(Thickness, Color),
    Rect(Color),
    Text(String, f32, &'static str, Color),
}

impl Shape {
    fn paint(&self, renderer: &mut CommonRenderer, queue: &wgpu::Queue, node: Rect) {
        match self {
            Self::Noop => (),
            &Self::RRect(rounding, color) => renderer.draw_round_rect(node.origin, node.size, rounding, color),
            &Self::Rect(color) => renderer.draw_rect(node.origin, node.size, color),
            Self::Text(text, font_size, font, color) => renderer.draw_text(queue, node.origin, font, text, *color, *font_size, Some(node.size.x)),
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
#[allow(clippy::struct_excessive_bools)]
pub struct WidgetState {
    pub clicked: bool,
    pub pointer_entered: bool,
    pub pointer_inside: bool,
    pub pointer_leaved: bool,
}

#[derive(Debug)]
struct WidgetData {
    parent: WidgetId,
    layout_node: Rect,
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

fn rect_contains(rect: &Rect, point: glam::Vec2) -> bool {
    point.x > rect.origin.x && point.x < (rect.origin.x + rect.size.x) && point.y > rect.origin.y && point.y < (rect.origin.y + rect.size.y)
}

impl UiContext {
    pub fn new() -> Self {
        Self {
            widgets: vec![WidgetData {
                parent: WidgetId(0),
                layout_node: Rect::ZERO,
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

    pub fn process_mouse_move(&mut self, position: glam::Vec2) {
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

    pub fn translate(&mut self, widget: WidgetId, offset: glam::Vec2) {
        self.widgets[widget.0].layout_node.origin += offset;

        for w in widget.into_iter(self.all_children(widget)) {
            if self.parent(w) == widget {
                self.translate(w, offset);
            }
        }
    }

    pub fn set_origin(&mut self, widget: WidgetId, origin: glam::Vec2) {
        self.widgets[widget.0].abs_pos = true;
        self.translate(widget, origin);
    }

    pub fn set_size(&mut self, widget: WidgetId, size: glam::Vec2) {
        self.widgets[widget.0].layout_node.size = size;
    }

    pub fn state(&self, widget: WidgetId) -> WidgetState {
        self.widgets[widget.0].state
    }

    pub fn layout_node(&self, widget: WidgetId) -> Rect {
        self.widgets[widget.0].layout_node
    }

    pub fn layout_node_mut(&mut self, widget: WidgetId) -> &mut Rect {
        &mut self.widgets[widget.0].layout_node
    }

    pub fn all_children(&self, widget: WidgetId) -> usize {
        self.widgets[widget.0].children
    }

    pub fn parent(&self, widget: WidgetId) -> WidgetId {
        self.widgets[widget.0].parent
    }

    pub fn paint_root(&self, renderer: &mut CommonRenderer, queue: &wgpu::Queue) {
        self.paint(renderer, queue, WidgetId(0));
    }

    pub fn paint(&self, renderer: &mut CommonRenderer, queue: &wgpu::Queue, widget: WidgetId) {
        let data = &self.widgets[widget.0];

        data.shape.paint(renderer, queue, data.layout_node);

        for w in 1..=self.all_children(widget) {
            self.paint(renderer, queue, WidgetId(widget.0 + w));
        }
    }

    pub fn try_allocate_widget(&mut self, parent: WidgetId, id: WidgetId, shape: Shape, size: glam::Vec2) {
        let widgets = self.widgets.len();

        self.widgets[parent.0].children += 1;

        // only allocate widget if it was not allocated before
        if widgets < (id.0 + 1) {
            self.widgets.push(WidgetData {
                parent,
                layout_node: Rect::new(glam::Vec2::ZERO, size),
                abs_pos: false,
                children: 0,
                shape,
                state: WidgetState::default(),
            });
        } else {
            self.widgets[id.0].parent = parent;
            self.widgets[id.0].layout_node = Rect::new(glam::Vec2::ZERO, size);
            self.widgets[id.0].abs_pos = false;
            self.widgets[id.0].children = 0;
            self.widgets[id.0].shape = shape;
        }
    }

    pub fn root<'a>(&'a mut self, renderer: &'a CommonRenderer, size: glam::Vec2) -> UiSubcontext<'a, RowStrategy, RowStrategy> {
        self.widgets[0].layout_node = Rect::new(glam::Vec2::ZERO, size);
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
    explicit_pos: Option<glam::Vec2>,
    explicit_width: Option<f32>,
    explicit_height: Option<f32>,
}

impl UiSubcontext<'_, RowStrategy, RowStrategy> {
    #[allow(dead_code)]
    pub const fn set_spacing(&mut self, pixels: f32) {
        self.arrange_strategy.spacing = pixels;
        self.measure_strategy.spacing = pixels;
    }
}

impl UiSubcontext<'_, ColumnStrategy, ColumnStrategy> {
    pub const fn set_v_arrangement(&mut self, arrangement: Arrangement) {
        self.arrange_strategy.v_arrangement = arrangement;
    }

    pub const fn set_h_arrangement(&mut self, arrangement: Arrangement) {
        self.arrange_strategy.h_arrangement = arrangement;
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
        self.explicit_pos.replace(glam::Vec2::new(x, y));
    }

    pub fn set_width(&mut self, width: f32) {
        self.explicit_width.replace(width);
        self.context.layout_node_mut(self.id).size.x = width;
    }

    pub fn set_height(&mut self, height: f32) {
        self.explicit_height.replace(height);
        self.context.layout_node_mut(self.id).size.y = height;
    }

    pub fn parent_size(&self) -> glam::Vec2 {
        let parent = self.context.parent(self.id);

        self.context.layout_node(parent).size
    }

    pub fn part_of_parent_width(&mut self, ratio: f32) {
        let size = self.parent_size();

        self.set_width(size.x * ratio);
    }

    #[allow(dead_code)]
    pub fn part_of_parent_height(&mut self, ratio: f32) {
        let size = self.parent_size();

        self.set_height(size.y * ratio);
    }

    pub fn fill_max_size(&mut self) {
        let size = self.parent_size();

        self.set_width(size.x);
        self.set_height(size.y);
    }

    pub fn child(&mut self, shape: Shape) -> WidgetId {
        self.sized_child(glam::Vec2::ZERO, shape)
    }

    pub fn sized_child(&mut self, size: glam::Vec2, shape: Shape) -> WidgetId {
        let id = self.next_child();

        self.context.try_allocate_widget(self.id, id, shape, size);

        id
    }

    fn perform_layout(&mut self) {
        let mut size = self.measure_strategy.measure(self.context, self.id);

        if let Some(width) = self.explicit_width {
            size.x = width;
        }

        if let Some(height) = self.explicit_height {
            size.y = height;
        }

        self.context.set_size(self.id, size);

        if let Some(pos) = self.explicit_pos {
            self.context.set_origin(self.id, pos);
        }

        self.arrange_strategy.arrange(self.context, self.id);
    }

    pub fn add_space(&mut self, space: glam::Vec2) {
        self.sized_child(space, Shape::Noop);
    }

    pub fn set_background_color(&mut self, color: Color) {
        self.context.widgets[self.id.0].shape = Shape::Rect(color);
    }

    #[allow(dead_code)]
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
        self.scope(
            ColumnStrategy {
                spacing: 0.0,
                v_arrangement: Arrangement::Start,
                h_arrangement: Arrangement::Start,
            },
            ColumnStrategy {
                spacing: 0.0,
                v_arrangement: Arrangement::Start,
                h_arrangement: Arrangement::Start,
            },
            ui,
        );
    }

    pub fn center(&mut self, ui: impl FnOnce(&mut UiSubcontext<'_, CenterStrategy, SingleChildStrategy>)) {
        self.scope(CenterStrategy, SingleChildStrategy, ui);
    }

    pub fn button(&mut self, ui: impl FnOnce(&mut UiSubcontext<'_, NoopStrategy, SingleChildStrategy>)) -> WidgetState {
        self.scope(NoopStrategy, SingleChildStrategy, ui)
    }

    pub fn rect(&mut self, size: glam::Vec2, color: Color) {
        self.sized_child(size, Shape::Rect(color));
    }

    pub fn text<T: Into<String>>(&mut self, text: T, font_size: f32, font: &'static str, color: Color) {
        let text = text.into();
        let size = self.renderer.measure(font, &text, font_size, None).unwrap_or_default();

        self.sized_child(size, Shape::Text(text, font_size, font, color));
    }

    #[allow(dead_code)]
    pub fn rrect(&mut self, size: glam::Vec2, rounding: Thickness, color: Color) {
        self.sized_child(size, Shape::RRect(rounding, color));
    }
}

impl<A: ArrangeStrategy, M: MeasureStrategy> Drop for UiSubcontext<'_, A, M> {
    fn drop(&mut self) {
        self.perform_layout();
    }
}
