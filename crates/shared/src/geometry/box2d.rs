use crate::Point2D;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Box2D {
    pub min: Point2D,
    pub max: Point2D,
}
