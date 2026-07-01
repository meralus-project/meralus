use crate::Box2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub origin: glam::Vec2,
    pub size: glam::Vec2,
}

impl Rect {
    pub const ZERO: Self = Self::new(glam::Vec2::ZERO, glam::Vec2::ZERO);

    pub const fn new(origin: glam::Vec2, size: glam::Vec2) -> Self {
        Self { origin, size }
    }

    pub fn center(&self) -> glam::Vec2 {
        self.origin + self.size / 2.0
    }

    pub fn to_box2(self) -> Box2 {
        Box2 {
            min: self.origin,
            max: self.origin + self.size,
        }
    }
}
