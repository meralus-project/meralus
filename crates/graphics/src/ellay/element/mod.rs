mod canvas;
mod column;
mod text;

use std::{any::Any, fmt::Debug};

use meralus_shared::{Point2D, RRect2D, Rect2D, Size2D};

pub use self::{
    canvas::{AnchorPoint, Canvas},
    column::Column,
    text::Text,
};
use super::Style;
use crate::{RenderContext, TextRenderer};

pub enum ElementChildren<'a> {
    None,
    Single(&'a Node),
    Multiple(Vec<&'a Node>),
}

pub enum ElementChildrenMut<'a> {
    None,
    Single(&'a mut Node),
    Multiple(Vec<&'a mut Node>),
}

impl<'a> ElementChildren<'a> {
    pub fn multiple<I: IntoIterator<Item = &'a Node>>(iter: I) -> Self {
        Self::Multiple(iter.into_iter().collect())
    }

    pub const fn len(&self) -> usize {
        match self {
            Self::None => 0,
            Self::Single(_) => 1,
            Self::Multiple(elements) => elements.len(),
        }
    }

    pub const fn is_empty(&self) -> bool {
        match self {
            Self::None => true,
            Self::Single(_) => false,
            Self::Multiple(elements) => elements.is_empty(),
        }
    }
}

impl<'a> ElementChildrenMut<'a> {
    pub fn multiple<I: IntoIterator<Item = &'a mut Node>>(iter: I) -> Self {
        Self::Multiple(iter.into_iter().collect())
    }

    pub const fn len(&self) -> usize {
        match self {
            Self::None => 0,
            Self::Single(_) => 1,
            Self::Multiple(elements) => elements.len(),
        }
    }

    pub const fn is_empty(&self) -> bool {
        match self {
            Self::None => true,
            Self::Single(_) => false,
            Self::Multiple(elements) => elements.is_empty(),
        }
    }
}

pub type Node = Box<dyn Element>;

/// Allows you to create GUI elements for subsequent use in the application.
/// Fully dyn-compatible, can also be casted from dyn to the original type.
pub trait Element: Any + Debug {
    /// Checks whether a point is inside an element.
    fn contains(&self, point: Point2D) -> bool {
        self.bounding_box().contains(point)
    }

    /// Sets the coordinates for the upper-left corner of the element.
    fn set_origin(&mut self, origin: Point2D) {
        self.bounding_box_mut().origin = origin;
    }

    // Sets the size of the element.
    fn set_size(&mut self, size: Size2D) {
        self.bounding_box_mut().size = size;
    }

    /// Returns a rectangle with possibly rounded corners.
    fn bounding_box(&self) -> RRect2D;
    /// Returns a mutable reference to rectangle with possibly rounded corners.
    fn bounding_box_mut(&mut self) -> &mut RRect2D;
    /// Returns a reference to the style of an element.
    fn style(&self) -> &Style;
    /// Returns a mutable reference to the style of an element.
    fn style_mut(&mut self) -> &mut Style;

    /// Removes children element.
    #[allow(unused_variables)]
    fn remove(&mut self, at: usize) {}

    /// Adds children element.
    #[allow(unused_variables)]
    fn insert(&mut self, at: usize, node: Node) {}

    /// Adds children element.
    #[allow(unused_variables)]
    fn push(&mut self, node: Node) {}

    /// Returns a reference to the children of an element.
    fn children(&self) -> ElementChildren<'_> {
        ElementChildren::None
    }

    /// Returns a reference to the children of an element.
    fn children_mut(&mut self) -> ElementChildrenMut<'_> {
        ElementChildrenMut::None
    }

    /// Auxiliary function for rendering the background of an element.
    fn draw_background(&self, context: &mut RenderContext) {
        if let Some(color) = self.style().background_color {
            let bounds = self.bounding_box();

            if bounds.corner_radius.any_above(0.0) {
                context.draw_rounded_rect(bounds, color);
            } else {
                context.draw_rect(bounds.as_rect(), color);
            }
        }
    }

    /// Calculates the size and position of an element.
    fn measure(&mut self, context: &RenderContext, parent: Rect2D);
    /// Draws an element on the screen.
    fn draw(&self, context: &mut RenderContext);

    fn translate(&mut self, point: Point2D) {
        self.bounding_box_mut().origin += point;

        match self.children_mut() {
            ElementChildrenMut::None => {}
            ElementChildrenMut::Single(element) => element.translate(point),
            ElementChildrenMut::Multiple(elements) => {
                for element in elements {
                    element.translate(point);
                }
            }
        }
    }

    /// Converts the element to the `Node` type.
    fn into_node(self) -> Node
    where
        Self: 'static + Sized,
    {
        Box::new(self) as Box<dyn Element>
    }
}

impl dyn Element {
    /// Returns `true` if the element type is the same as `T`.
    pub fn is<T: Element>(&self) -> bool {
        (self as &dyn Any).is::<T>()
    }

    /// Returns reference to the original element if it is of type `T`, or
    /// `None` if it isn't.
    pub fn try_as<T: Element>(&self) -> Option<&T> {
        (self as &dyn Any).downcast_ref()
    }

    /// Returns reference to the original element if it is of type `T`, or
    /// `None` if it isn't.
    pub fn try_as_mut<T: Element>(&mut self) -> Option<&mut T> {
        (self as &mut dyn Any).downcast_mut()
    }
}
