use meralus_shared::{Color, Thickness};

use super::Element;

#[derive(Debug)]
/// Simple style for elements, containing possible background color and padding.
pub struct Style {
    pub background_color: Option<Color>,
    pub padding: Thickness,
}

impl Style {
    pub const fn default() -> Self {
        Self {
            background_color: None,
            padding: Thickness::default(),
        }
    }
}

/// Contains functions for styling elements.
pub trait Styling {
    /// Sets the background color for the element.
    #[must_use]
    fn with_background(self, color: Color) -> Self;
    /// Sets the padding for the element.
    #[must_use]
    fn with_padding(self, value: Thickness) -> Self;
    /// Sets the corner radius for the element.
    #[must_use]
    fn with_corner_radius(self, radius: Thickness) -> Self;
}

impl<T: Element> Styling for T {
    fn with_background(mut self, color: Color) -> Self {
        self.style_mut().background_color.replace(color);

        self
    }

    fn with_padding(mut self, value: Thickness) -> Self {
        self.style_mut().padding = value;

        self
    }

    fn with_corner_radius(mut self, radius: Thickness) -> Self {
        self.bounding_box_mut().corner_radius = radius;

        self
    }
}
