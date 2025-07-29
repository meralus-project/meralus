use meralus_shared::{Point2D, RRect2D, Rect2D, Size2D};

use crate::TextRenderer;

use super::{Element, ElementChildren, ElementChildrenMut, Node, RenderContext, Style};

/// Vertically arranges elements.
#[derive(Debug)]
pub struct Column {
    style: Style,
    spacing: f32,
    bounding_box: RRect2D,
    children: Vec<Node>,
}

impl Column {
    pub const fn default() -> Self {
        Self {
            style: Style::default(),
            spacing: 0.0,
            bounding_box: RRect2D::default(),
            children: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_children<I: IntoIterator<Item = Node>>(mut self, children: I) -> Self {
        self.children = children.into_iter().collect();

        self
    }

    #[must_use]
    pub const fn with_spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;

        self
    }
}

impl Element for Column {
    fn bounding_box(&self) -> RRect2D {
        self.bounding_box
    }

    fn bounding_box_mut(&mut self) -> &mut RRect2D {
        &mut self.bounding_box
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }

    fn push(&mut self, node: Node) {
        self.children.push(node);
    }

    fn insert(&mut self, at: usize, node: Node) {
        self.children.insert(at, node);
    }

    fn remove(&mut self, at: usize) {
        self.children.remove(at);
    }

    fn children(&self) -> ElementChildren<'_> {
        ElementChildren::multiple(self.children.iter())
    }

    fn children_mut(&mut self) -> ElementChildrenMut<'_> {
        ElementChildrenMut::multiple(&mut self.children)
    }

    fn measure(&mut self, text: &mut TextRenderer, context: &RenderContext, parent: Rect2D) {
        self.set_origin(parent.origin);

        let origin = parent.origin + Point2D::new(self.style.padding.left(), self.style.padding.top());

        let mut size = Size2D::ZERO;
        let mut available_height = parent.size.height;

        for child in &mut self.children {
            let mut bounds = parent;

            bounds.origin = origin;
            bounds.origin.y += parent.size.height - available_height;
            bounds.size.height = available_height;

            child.measure(text, context, bounds);

            let bounds = child.bounding_box();

            size.width = size.width.max(bounds.size.width);
            size.height += bounds.size.height + self.spacing;
            available_height -= bounds.size.height + self.spacing;
        }

        if !self.children.is_empty() {
            size.height -= self.spacing;
        }

        self.set_size(
            size + Size2D::new(
                self.style.padding.left() + self.style.padding.right(),
                self.style.padding.top() + self.style.padding.bottom(),
            ),
        );
    }

    fn draw(&self, context: &mut RenderContext) {
        self.draw_background(context);

        for element in &self.children {
            element.draw(context);
        }
    }
}
