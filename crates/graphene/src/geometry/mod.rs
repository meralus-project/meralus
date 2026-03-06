mod round_rect;
mod thickness;

pub use self::{round_rect::RRect, thickness::Thickness};

pub type Point2D = glamour::Point2<f32>;
pub type Box2D = glamour::Box2<f32>;
pub type Size2D = glamour::Size2<f32>;
pub type Rect = glamour::Rect<f32>;
