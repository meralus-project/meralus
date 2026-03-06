mod index {
    use std::{
        marker::PhantomData,
        ops::{Index, IndexMut},
    };

    pub trait Idx {
        fn index(self) -> usize;
    }

    #[derive(Debug)]
    pub struct IndexVec<I: Idx, T> {
        raw: Vec<T>,
        _phantom: PhantomData<I>,
    }

    impl<I: Idx, T> IndexVec<I, T> {
        pub const fn len(&self) -> usize {
            self.raw.len()
        }

        pub fn push(&mut self, value: T) {
            self.raw.push(value);
        }
    }

    impl<I: Idx, T> Index<I> for IndexVec<I, T> {
        type Output = T;

        fn index(&self, i: I) -> &Self::Output {
            &self.raw[i.index()]
        }
    }

    impl<I: Idx, T> IndexMut<I> for IndexVec<I, T> {
        fn index_mut(&mut self, i: I) -> &mut Self::Output {
            &mut self.raw[i.index()]
        }
    }

    impl<I: Idx, T> Default for IndexVec<I, T> {
        fn default() -> Self {
            Self {
                raw: Vec::default(),
                _phantom: PhantomData,
            }
        }
    }
}

pub mod style {
    use meralus_shared::Color;

    #[derive(Debug, Clone, Copy)]
    pub struct Rounding(pub [f32; 4]);

    #[derive(Default, Debug, Clone, Copy)]
    pub struct Modifier {
        pub background: Option<Color>,
        pub rounding: Option<Rounding>,
    }

    impl Modifier {
        #[must_use]
        pub const fn with_background(mut self, color: Color) -> Self {
            self.background.replace(color);

            self
        }

        #[must_use]
        pub const fn with_rounding(mut self, rounding: Rounding) -> Self {
            self.rounding.replace(rounding);

            self
        }
    }
}

pub mod components {
    use meralus_shared::{Color, Point2D, Size2D};

    use crate::{Component, Constraints, LayoutContext, RenderCtx, Widget, WidgetId, style::Modifier};

    #[derive(Debug)]
    pub struct Button {
        pub content: Widget,
    }

    impl Button {
        pub const fn new(content: Widget) -> Self {
            Self { content }
        }
    }

    impl Component for Button {
        fn measure(&self, _: WidgetId, context: &mut LayoutContext, _: Modifier, constraints: Constraints) -> Size2D {
            self.content.measure(context, constraints)
        }

        fn layout(&self, _: WidgetId, context: &mut LayoutContext, _: Modifier, _: Size2D) {
            self.content.layout(context);
        }

        fn render(&self, _: WidgetId, context: &mut RenderCtx, _: Modifier) {
            self.content.render(context);
        }
    }

    #[derive(Debug)]
    pub struct Text {
        pub data: String,
        pub font_name: String,
        pub font_size: f32,
        pub color: Color,
    }

    impl Text {
        pub fn new<T: Into<String>, F: Into<String>>(data: T, font_name: F, font_size: f32, color: Color) -> Self {
            Self {
                data: data.into(),
                font_name: font_name.into(),
                font_size,
                color,
            }
        }
    }

    impl Component for Text {
        fn measure(&self, _: WidgetId, context: &mut LayoutContext, _: Modifier, constraints: Constraints) -> Size2D {
            context.measure_text(&self.font_name, self.font_size, &self.data, Some(constraints.max.width))
        }

        fn layout(&self, _: WidgetId, _: &mut LayoutContext, _: Modifier, _: Size2D) {}

        fn render(&self, _: WidgetId, context: &mut RenderCtx, _: Modifier) {
            context
                .common_renderer
                .draw_text(context.translation, &self.font_name, &self.data, self.color, self.font_size, None)
                .unwrap();
        }
    }

    #[derive(Debug)]
    pub enum Orientation {
        Vertical,
        Horizontal,
    }

    #[derive(Debug)]
    pub struct List {
        pub orientation: Orientation,
        pub spacing: f32,
        pub children: Vec<Widget>,
    }

    impl List {
        pub fn vertical<I: IntoIterator<Item = Widget>>(children: I) -> Self {
            Self {
                orientation: Orientation::Vertical,
                spacing: 0.0,
                children: children.into_iter().collect(),
            }
        }

        pub fn horizontal<I: IntoIterator<Item = Widget>>(children: I) -> Self {
            Self {
                orientation: Orientation::Horizontal,
                spacing: 0.0,
                children: children.into_iter().collect(),
            }
        }

        #[must_use]
        pub const fn with_spacing(mut self, spacing: f32) -> Self {
            self.spacing = spacing;

            self
        }
    }

    impl Component for List {
        fn measure(&self, _: WidgetId, context: &mut LayoutContext, _: Modifier, constraints: Constraints) -> Size2D {
            self.children
                .iter()
                .map(|c| c.measure(context, constraints))
                .reduce(|mut p, c| {
                    match self.orientation {
                        Orientation::Vertical => {
                            p.width = p.width.max(c.width);
                            p.height += c.height + self.spacing;
                        }
                        Orientation::Horizontal => {
                            p.height = p.height.max(c.height);
                            p.width += c.width + self.spacing;
                        }
                    }

                    p
                })
                .unwrap_or(Size2D::ZERO)
        }

        fn layout(&self, _: WidgetId, context: &mut LayoutContext, _: Modifier, _: Size2D) {
            let mut offset = 0.0;

            for child in &self.children {
                child.layout(context);

                match self.orientation {
                    Orientation::Vertical => {
                        context.place(child.id, Point2D { x: 0.0, y: offset });

                        offset += context.widgets[child.id].size.height + self.spacing;
                    }
                    Orientation::Horizontal => {
                        context.place(child.id, Point2D { x: offset, y: 0.0 });

                        offset += context.widgets[child.id].size.width + self.spacing;
                    }
                }
            }
        }

        fn render(&self, _: WidgetId, context: &mut RenderCtx, _: Modifier) {
            for widget in &self.children {
                widget.render(context);
            }
        }
    }
}

mod widget {
    use meralus_shared::Size2D;

    use crate::{Component, Constraints, EventHandler, LayoutContext, RenderCtx, WidgetId, style::Modifier};

    #[derive(Debug)]
    pub struct Widget {
        pub id: WidgetId,
        pub inner: Box<dyn Component>,
        pub handlers: Vec<EventHandler>,
        pub modifier: Modifier,
    }

    impl Widget {
        #[must_use]
        pub fn on_pointer_enter(mut self, handler: impl Fn() + 'static) -> Self {
            self.handlers.push(EventHandler::PointerEnter(Box::new(handler)));

            self
        }

        #[must_use]
        pub fn on_pointer_exit(mut self, handler: impl Fn() + 'static) -> Self {
            self.handlers.push(EventHandler::PointerExit(Box::new(handler)));

            self
        }

        #[must_use]
        pub fn on_pointer_down(mut self, handler: impl Fn() + 'static) -> Self {
            self.handlers.push(EventHandler::PointerDown(Box::new(handler)));

            self
        }

        #[must_use]
        pub fn on_pointer_up(mut self, handler: impl Fn() + 'static) -> Self {
            self.handlers.push(EventHandler::PointerUp(Box::new(handler)));

            self
        }

        pub fn measure(&self, context: &mut LayoutContext, constraints: Constraints) -> Size2D {
            let size = self.inner.measure(self.id, context, self.modifier, constraints);

            context.widgets[self.id].size = size;

            size
        }

        pub fn layout(&self, context: &mut LayoutContext) {
            self.inner.layout(self.id, context, self.modifier, context.widgets[self.id].size);

            context.widgets[self.id].needs_layout = false;
        }

        pub fn render(&self, context: &mut RenderCtx) {
            context.translation += context.widgets[self.id].position;

            if let Some(color) = self.modifier.background {
                context.render_widget_rect(self.id, color, self.modifier.rounding);
            }

            self.inner.render(self.id, context, self.modifier);

            context.translation -= context.widgets[self.id].position.to_vector();
        }
    }
}

use std::fmt;

use meralus_graphics::CommonRenderer;
use meralus_shared::{Color, Point2D, Size2D, Thickness};

pub use crate::widget::Widget;
use crate::{
    index::{Idx, IndexVec},
    style::{Modifier, Rounding},
};

#[derive(Debug, Default)]
pub struct VisualManager {
    widgets: IndexVec<WidgetId, WidgetData>,
}

impl VisualManager {
    pub fn new_widget<T: Component + 'static>(&mut self, inner: T, modifier: Modifier) -> Widget {
        let id = WidgetId(self.widgets.len());

        self.widgets.push(WidgetData {
            position: Point2D::ZERO,
            size: Size2D::ZERO,
            needs_layout: true,
        });

        Widget {
            id,
            inner: Box::new(inner),
            handlers: Vec::new(),
            modifier,
        }
    }

    pub const fn layout_ctx<'a>(&'a mut self, common_renderer: &'a CommonRenderer) -> LayoutContext<'a> {
        LayoutContext {
            widgets: &mut self.widgets,
            common_renderer,
        }
    }

    pub const fn render_ctx<'a>(&'a mut self, common_renderer: &'a mut CommonRenderer) -> RenderCtx<'a> {
        RenderCtx {
            translation: Point2D::ZERO,
            widgets: &mut self.widgets,
            common_renderer,
        }
    }
}

pub trait Component: fmt::Debug {
    fn measure(&self, id: WidgetId, context: &mut LayoutContext, modifier: Modifier, constraints: Constraints) -> Size2D;

    fn layout(&self, id: WidgetId, context: &mut LayoutContext, modifier: Modifier, size: Size2D);
    fn render(&self, id: WidgetId, context: &mut RenderCtx, modifier: Modifier);
}

#[derive(Debug, Clone, Copy)]
pub struct WidgetId(usize);

impl Idx for WidgetId {
    fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug)]
struct WidgetData {
    position: Point2D,
    size: Size2D,
    needs_layout: bool,
}

pub enum EventHandler {
    PointerEnter(Box<dyn Fn()>),
    PointerExit(Box<dyn Fn()>),
    PointerDown(Box<dyn Fn()>),
    PointerUp(Box<dyn Fn()>),
}

impl fmt::Debug for EventHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PointerEnter(_) => f.debug_tuple("PointerEnter").finish_non_exhaustive(),
            Self::PointerExit(_) => f.debug_tuple("PointerExit").finish_non_exhaustive(),
            Self::PointerDown(_) => f.debug_tuple("PointerDown").finish_non_exhaustive(),
            Self::PointerUp(_) => f.debug_tuple("PointerUp").finish_non_exhaustive(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Constraints {
    pub min: Size2D,
    pub max: Size2D,
}

pub struct LayoutContext<'a> {
    widgets: &'a mut IndexVec<WidgetId, WidgetData>,
    common_renderer: &'a CommonRenderer,
}

impl LayoutContext<'_> {
    fn place(&mut self, widget: WidgetId, position: Point2D) {
        self.widgets[widget].position = position;
    }

    fn measure_text(&self, font_name: &str, font_size: f32, text: &str, max_width: Option<f32>) -> Size2D {
        self.common_renderer.measure(font_name, text, font_size, max_width).unwrap_or_default()
    }
}

pub struct RenderCtx<'a> {
    translation: Point2D,
    widgets: &'a mut IndexVec<WidgetId, WidgetData>,
    common_renderer: &'a mut CommonRenderer,
}

impl RenderCtx<'_> {
    fn render_widget_rect(&mut self, id: WidgetId, color: Color, rounding: Option<Rounding>) {
        let WidgetData { size, .. } = self.widgets[id];

        if let Some(Rounding(rounding)) = rounding {
            let _ = self.common_renderer.draw_round_rect(
                self.translation,
                size,
                Thickness::new(rounding[0], rounding[1], rounding[2], rounding[3]),
                color,
            );
        } else {
            let _ = self.common_renderer.draw_rect(self.translation, size, color);
        }
    }
}
