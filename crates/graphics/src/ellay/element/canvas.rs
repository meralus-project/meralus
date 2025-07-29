use meralus_shared::{Point2D, RRect2D, Rect2D};

use crate::TextRenderer;

use super::{Element, ElementChildren, ElementChildrenMut, Node, RenderContext, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AnchorPoint {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Contains multiple elements with their own position.
#[derive(Debug)]
pub struct Canvas {
    style: Style,
    bounding_box: RRect2D,
    children: Vec<(Point2D, AnchorPoint, Node)>,
}

impl Canvas {
    pub const fn default() -> Self {
        Self {
            style: Style::default(),
            bounding_box: RRect2D::default(),
            children: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_children<I: IntoIterator<Item = (Point2D, AnchorPoint, Node)>>(mut self, children: I) -> Self {
        self.children = children.into_iter().collect();

        self
    }
}

impl Element for Canvas {
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
        self.children.push((Point2D::ZERO, AnchorPoint::TopLeft, node));
    }

    fn insert(&mut self, at: usize, node: Node) {
        self.children.insert(at, (Point2D::ZERO, AnchorPoint::TopLeft, node));
    }

    fn remove(&mut self, at: usize) {
        self.children.remove(at);
    }

    fn children(&self) -> ElementChildren<'_> {
        ElementChildren::multiple(self.children.iter().map(|(.., element)| element))
    }

    fn children_mut(&mut self) -> ElementChildrenMut<'_> {
        ElementChildrenMut::multiple(self.children.iter_mut().map(|(.., element)| element))
    }

    fn measure(&mut self, text: &mut TextRenderer, context: &RenderContext, parent: Rect2D) {
        self.set_origin(parent.origin);
        self.set_size(parent.size);

        for (origin, anchor, child) in &mut self.children {
            let mut bounds = parent;

            bounds.origin = *origin;

            child.measure(text, context, bounds);

            let bounds = child.bounding_box();

            match anchor {
                AnchorPoint::TopLeft => {}
                AnchorPoint::TopRight => child.translate(Point2D::new(parent.size.width + bounds.origin.x - bounds.size.width, 0.0)),
                AnchorPoint::BottomLeft => child.translate(Point2D::new(0.0, parent.size.height + bounds.origin.y - bounds.size.height)),
                AnchorPoint::BottomRight => child.translate(Point2D::new(
                    parent.size.width + bounds.origin.x - bounds.size.width,
                    parent.size.height + bounds.origin.y - bounds.size.height,
                )),
            }
        }
    }

    fn draw(&self, context: &mut RenderContext) {
        self.draw_background(context);

        for (.., element) in &self.children {
            element.draw(context);
        }
    }
}
