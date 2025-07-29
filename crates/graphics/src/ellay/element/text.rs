use meralus_shared::{Color, Point2D, RRect2D, Rect2D, Size2D};

use super::{Element, ElementChildren, RenderContext, Style};
use crate::TextRenderer;

/// Displays text
#[derive(Debug)]
pub struct Text {
    style: Style,
    bounding_box: RRect2D,
    data: String,
    size: f32,
    color: Color,
}

impl Text {
    pub fn new<T: Into<String>>(data: T) -> Self {
        Self {
            style: Style::default(),
            bounding_box: RRect2D::default(),
            data: data.into(),
            size: 12.0,
            color: Color::BLACK,
        }
    }

    #[must_use]
    pub const fn with_foreground(mut self, color: Color) -> Self {
        self.color = color;

        self
    }

    #[must_use]
    pub const fn with_text_size(mut self, size: f32) -> Self {
        self.size = size;

        self
    }
}

impl Element for Text {
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

    fn children(&self) -> ElementChildren<'_> {
        ElementChildren::None
    }

    fn measure(&mut self, text: &mut TextRenderer, context: &RenderContext, parent: Rect2D) {
        let padding = self.style.padding;

        self.set_origin(parent.origin);
        self.set_size(
            text.measure("default", &self.data, self.size, None).unwrap() + Size2D::new(padding.left() + padding.right(), padding.bottom() + padding.top()),
        );
    }

    fn draw(&self, context: &mut RenderContext) {
        self.draw_background(context);

        let origin = if self.style.padding.any_above(0.0) {
            self.bounding_box.origin + Point2D::new(self.style.padding.left(), self.style.padding.top())
        } else {
            self.bounding_box.origin
        };

        context.draw_text(origin, "default", &self.data, self.size, self.color, None);
    }
}
