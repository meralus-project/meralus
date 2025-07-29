//! Ellay is a submodule of `meralus-graphics` responsible for rendering GUI
//! using the `Element` trait.
//!
//! It is not a separate crate, as it relies heavily on some of the types from
//! `meralus-graphics`.

mod element;
mod style;

pub use self::{
    element::{AnchorPoint, Canvas, Column, Element, ElementChildren, ElementChildrenMut, Node, Text},
    style::{Style, Styling},
};
