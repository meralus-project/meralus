use crate::{Point2D, Size2D};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub origin: Point2D,
    pub size: Size2D,
}
