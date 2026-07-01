use super::{Box2, Rect, Thickness};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RRect {
    /// Coordinates of the rectangle.
    pub origin: glam::Vec2,
    /// Size of the rectangle.
    pub size: glam::Vec2,
    /// Radius of all four corners.
    pub corner_radius: Thickness,
}

impl RRect {
    pub const EMPTY: Self = Self {
        origin: glam::Vec2::ZERO,
        size: glam::Vec2::ZERO,
        corner_radius: Thickness::default(),
    };

    pub const fn new(origin: glam::Vec2, size: glam::Vec2, corner_radius: Thickness) -> Self {
        Self { origin, size, corner_radius }
    }

    pub const fn as_rect(&self) -> Rect {
        Rect {
            origin: self.origin,
            size: self.size,
        }
    }

    pub const fn width(&self) -> f32 {
        self.size.x
    }

    pub const fn height(&self) -> f32 {
        self.size.y
    }

    pub fn center(&self) -> glam::Vec2 {
        self.origin + self.size / 2.0
    }

    pub fn as_box(&self) -> Box2 {
        Box2 {
            min: self.origin,
            max: self.origin + self.size,
        }
    }

    pub fn contains(&self, pt: glam::Vec2) -> bool {
        let center = self.center();
        let pt = pt - center;
        let radius = match pt {
            pt if pt.x < 0.0 && pt.y < 0.0 => self.corner_radius.left(),
            pt if pt.x >= 0.0 && pt.y < 0.0 => self.corner_radius.top(),
            pt if pt.x >= 0.0 && pt.y >= 0.0 => self.corner_radius.right(),
            pt if pt.x < 0.0 && pt.y >= 0.0 => self.corner_radius.bottom(),
            _ => 0.0,
        };

        let px = (pt.x.abs() - (self.width() / 2.0 - radius).max(0.0)).max(0.0);
        let py = (pt.y.abs() - (self.height() / 2.0 - radius).max(0.0)).max(0.0);

        px * px + py * py <= radius * radius
    }
}
