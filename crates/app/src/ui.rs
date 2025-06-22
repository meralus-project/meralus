use glam::Mat4;
use glium::{Frame, Rect};
use lessor::{LayoutContext, LayoutNode, Measurable, TextMeasurer, Thickness, ThicknessRule};
use meralus_animation::{AnimationPlayer, Transition, TweenValue};
use meralus_engine::WindowDisplay;
use meralus_shared::{Color, Point2D, Rect2D, Size2D};

use crate::renderers::{Rectangle, ShapeRenderer, TextRenderer};

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

pub struct UiContext<'a> {
    window_size: Size2D,
    bounds: Rect2D,
    animation_player: &'a mut AnimationPlayer,
    shape_renderer: &'a mut ShapeRenderer,
    text_renderer: &'a mut TextRenderer,
    display: &'a WindowDisplay,
    frame: &'a mut Frame,
    rectangles: Vec<Rectangle>,
    texts: Vec<Text>,
    clip: Option<Rect2D>,
    matrix: Option<Mat4>,
}

impl<'a> UiContext<'a> {
    pub fn new(
        animation_player: &'a mut AnimationPlayer,
        shape_renderer: &'a mut ShapeRenderer,
        text_renderer: &'a mut TextRenderer,
        display: &'a WindowDisplay,
        frame: &'a mut Frame,
    ) -> Self {
        let (width, height) = display.get_framebuffer_dimensions();

        Self {
            window_size: Size2D::new(width as f32, height as f32),
            bounds: Rect2D::new(Point2D::ZERO, Size2D::new(width as f32, height as f32)),
            animation_player,
            shape_renderer,
            text_renderer,
            display,
            frame,
            rectangles: Vec::new(),
            texts: Vec::new(),
            clip: None,
            matrix: None,
        }
    }

    pub const fn text_renderer(&self) -> &TextRenderer {
        self.text_renderer
    }

    pub const fn get_bounds(&self) -> Rect2D {
        self.bounds
    }

    pub fn measure_text<F: AsRef<str>, T: AsRef<str>>(
        &self,
        font: F,
        text: T,
        size: f32,
    ) -> Option<Size2D> {
        self.text_renderer.measure(font, text, size, None)
    }

    pub fn draw_text<F: Into<String>, T: Into<String>>(
        &mut self,
        position: Point2D,
        font: F,
        text: T,
        size: f32,
        color: Color,
        max_width: Option<f32>,
    ) {
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

    pub fn animations(&self) -> usize {
        self.animation_player.len()
    }

    pub fn get_animation_at(&self, index: usize) -> Option<(&str, &Transition)> {
        self.animation_player.get_at(index)
    }

    pub fn get_animation_value<K: AsRef<str>, V: From<TweenValue>>(&self, name: K) -> Option<V> {
        self.animation_player.get_value(name)
    }

    pub const fn add_transform(&mut self, transform: Mat4) {
        self.matrix.replace(transform);
    }

    pub const fn remove_transform(&mut self) {
        self.matrix.take();
    }

    pub fn draw_rect(&mut self, position: Point2D, size: Size2D, color: Color) {
        self.rectangles.push(
            Rectangle::new(position.x, position.y, size.width, size.height, color)
                .with_matrix(self.matrix),
        );
    }

    pub fn finish(self, window_matrix: Mat4, draw_calls: &mut usize, vertices: &mut usize) {
        self.shape_renderer.draw_rects(
            self.frame,
            self.display,
            &self.rectangles,
            draw_calls,
            vertices,
        );

        for text in self.texts {
            self.text_renderer.render(
                self.frame,
                &(window_matrix * text.matrix.unwrap_or_default()),
                text.position,
                text.font,
                text.data,
                text.size,
                text.max_width,
                text.color,
                text.clip.map(|area| Rect {
                    left: area.origin.x.floor() as u32,
                    bottom: (self.window_size.height - area.origin.y - area.size.height).floor()
                        as u32,
                    width: area.size.width.floor() as u32,
                    height: area.size.height.floor() as u32,
                }),
                draw_calls,
            );
        }
    }

    pub fn ui<F: FnOnce(&mut UiContext, Rect2D)>(&mut self, func: F) {
        func(self, self.bounds);
    }

    pub fn fill(&mut self, color: Color) {
        self.draw_rect(self.bounds.origin, self.bounds.size, color);
    }

    pub fn clipped<F: FnOnce(&mut UiContext, Rect2D)>(&mut self, bounds: Rect2D, func: F) {
        self.clip.replace(bounds);

        func(self, self.bounds);

        self.clip.take();
    }

    pub fn bounds<F: FnOnce(&mut UiContext, Rect2D)>(&mut self, bounds: Rect2D, func: F) {
        let tmp = self.bounds;

        self.bounds = bounds;

        func(self, self.bounds);

        self.bounds = tmp;
    }

    pub fn padding<F: FnOnce(&mut UiContext, Rect2D)>(&mut self, value: f32, func: F) {
        self.bounds.origin += Point2D::ONE * value;
        self.bounds.size -= Size2D::ONE * value * 2.0;
        self.bounds.size = self.bounds.size.max(Size2D::ZERO);

        func(self, self.bounds);

        self.bounds.origin -= Point2D::ONE.to_vector() * value;
        self.bounds.size += Size2D::ONE * value * 2.0;
    }
}

pub enum SizeUnit {
    Auto,
    PartOf(f32),
    Pixels(f32),
}

impl SizeUnit {
    const fn resolve(&self, current_size: f32, available_size: f32) -> f32 {
        match *self {
            Self::Auto => current_size,
            Self::PartOf(value) => available_size * value,
            Self::Pixels(value) => value,
        }
    }

    /// Returns `true` if the size unit is [`Auto`].
    ///
    /// [`Auto`]: SizeUnit::Auto
    #[must_use]
    pub const fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }
}

pub struct Style {
    foreground: Option<Color>,
    background: Option<Color>,
    padding: Option<Thickness>,
    text_family: Option<String>,
    text_size: f32,
    width: SizeUnit,
    height: SizeUnit,
    min_width: Option<SizeUnit>,
    min_height: Option<SizeUnit>,
    horizontal_align: Align,
}

impl Style {
    pub const fn default() -> Self {
        Self {
            foreground: None,
            background: None,
            padding: None,
            text_family: None,
            text_size: 12.0,
            width: SizeUnit::Auto,
            height: SizeUnit::Auto,
            min_width: None,
            min_height: None,
            horizontal_align: Align::Start,
        }
    }

    pub const fn with_foreground(mut self, color: Color) -> Self {
        self.foreground = Some(color);

        self
    }

    pub const fn with_background(mut self, color: Color) -> Self {
        self.background = Some(color);

        self
    }

    pub fn with_text_family<T: Into<String>>(mut self, family: T) -> Self {
        self.text_family = Some(family.into());

        self
    }

    pub const fn with_text_size(mut self, value: f32) -> Self {
        self.text_size = value;

        self
    }

    pub const fn with_padding(mut self, value: Thickness) -> Self {
        self.padding = Some(value);

        self
    }

    pub const fn with_width(mut self, value: SizeUnit) -> Self {
        self.width = value;

        self
    }

    pub const fn with_height(mut self, value: SizeUnit) -> Self {
        self.height = value;

        self
    }

    pub const fn with_min_width(mut self, value: SizeUnit) -> Self {
        self.min_width = Some(value);

        self
    }

    pub const fn with_min_height(mut self, value: SizeUnit) -> Self {
        self.min_height = Some(value);

        self
    }

    pub const fn with_horizontal_align(mut self, value: Align) -> Self {
        self.horizontal_align = value;

        self
    }
}

pub enum Align {
    Start,
    Center,
    End,
}

pub enum Element {
    Column { children: Vec<Node>, spacing: f32 },
    Text(String),
    Noop,
}

pub struct Node {
    style: Style,
    element: Element,
}

impl Node {
    pub const fn new(element: Element) -> Self {
        Self {
            style: Style::default(),
            element,
        }
    }

    pub fn text<T: Into<String>>(value: T) -> Self {
        Self::new(Element::Text(value.into()))
    }

    pub const fn noop() -> Self {
        Self::new(Element::Noop)
    }

    pub const fn column() -> Self {
        Self::new(Element::Column {
            children: Vec::new(),
            spacing: 0.0,
        })
    }

    pub const fn with_spacing(mut self, value: f32) -> Self {
        if let Element::Column { spacing, .. } = &mut self.element {
            *spacing = value;
        }

        self
    }

    pub fn with_style(mut self, style: Style) -> Self {
        self.style = style;

        self
    }

    pub fn with_children<I: IntoIterator<Item = Self>>(mut self, nodes: I) -> Self {
        if let Element::Column { children, .. } = &mut self.element {
            *children = nodes
                .into_iter()
                .map(|mut node| {
                    if let Some(foreground) = self.style.foreground
                        && node.style.foreground.is_none()
                    {
                        node.style.foreground.replace(foreground);
                    }

                    if let Some(family) = &self.style.text_family
                        && node.style.text_family.is_none()
                    {
                        node.style.text_family.replace(family.clone());
                    }

                    node
                })
                .collect();
        }

        self
    }

    pub fn render(
        &self,
        context: &mut UiContext,
        layout_context: &LayoutContext,
        node_index: usize,
    ) {
        let node = layout_context.get_node(node_index);

        if let Some(background) = self.style.background {
            context.draw_rect(node.outer.origin, node.outer.size, background);
        }

        match &self.element {
            Element::Column { children, .. } => {
                for (element, node_index) in children
                    .iter()
                    .zip(layout_context.get_node_children(node_index))
                {
                    element.render(context, layout_context, *node_index);
                }
            }
            Element::Text(text) => {
                context.draw_text(
                    node.inner.origin,
                    self.style.text_family.as_deref().unwrap_or_default(),
                    text,
                    self.style.text_size,
                    self.style.foreground.unwrap_or(Color::BLACK),
                    if self.style.width.is_auto() {
                        None
                    } else {
                        Some(node.inner.size.width)
                    },
                );
            }
            Element::Noop => {}
        }
    }
}

impl TextMeasurer for TextRenderer {
    fn measure_text<F: AsRef<str>, T: AsRef<str>>(
        &self,
        font: F,
        text: T,
        size: f32,
        width: Option<f32>,
    ) -> lessor::Size2D {
        self.measure(font, text, size, width).unwrap_or_default()
    }
}

impl<T: TextMeasurer> Measurable<T> for Node {
    fn measure(
        &self,
        text_measurer: &T,
        context: &mut LayoutContext,
        bounds: Rect2D,
    ) -> LayoutNode {
        let mut node = LayoutNode::new(bounds.origin, Size2D::ZERO);

        node.set_width(self.style.width.resolve(node.width(), bounds.size.width));
        node.set_height(self.style.height.resolve(node.height(), bounds.size.height));

        if let Some(padding) = self.style.padding {
            node.add_padding(padding, ThicknessRule::Enlarge);
        }

        if let Some(min_width) = &self.style.min_width {
            node.try_set_width(min_width.resolve(node.width(), bounds.size.width));
        }

        if let Some(min_height) = &self.style.min_height {
            node.try_set_height(min_height.resolve(node.height(), bounds.size.height));
        }

        match &self.element {
            Element::Column { children, spacing } => {
                let mut available_height = if self.style.height.is_auto() {
                    bounds.size.height
                } else {
                    node.inner.size.height
                };

                for element in children {
                    let mut bounds = bounds;

                    bounds.origin = node.inner.origin;

                    if self.style.height.is_auto() {
                        bounds.origin.y += node.inner.size.height;
                    } else {
                        bounds.origin.y += node.inner.size.height - available_height;
                    }

                    if !self.style.width.is_auto() {
                        bounds.size.width = node.inner.size.width;
                    }

                    bounds.size.height = available_height;

                    let (child_node, node_index) = context.measure(text_measurer, bounds, element);

                    if !self.style.width.is_auto()
                        && matches!(self.style.horizontal_align, Align::Center)
                    {
                        context.set_origin(
                            node_index,
                            Point2D::new(
                                bounds.origin.x + (node.inner.size.width - child_node.width()) / 2.0,
                                child_node.outer.origin.y,
                            ),
                        );
                    }

                    if self.style.width.is_auto() {
                        node.set_width(node.width().max(child_node.outer.size.width));
                    }

                    if self.style.height.is_auto() {
                        node.add_height(child_node.outer.size.height + spacing);
                    }

                    available_height -= child_node.outer.size.height;
                }

                if !children.is_empty() && self.style.height.is_auto() {
                    node.sub_height(*spacing);
                }
            }
            Element::Text(text) => {
                let size = text_measurer.measure_text(
                    self.style.text_family.as_deref().unwrap_or_default(),
                    text,
                    self.style.text_size,
                    if self.style.width.is_auto() {
                        None
                    } else {
                        Some(node.inner.size.width)
                    },
                );

                if self.style.width.is_auto() {
                    node.set_width(size.width);
                }

                if self.style.height.is_auto() {
                    node.set_height(size.height);
                }
            }
            Element::Noop => {}
        }

        node
    }
}
