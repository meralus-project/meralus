use super::{Box2D, Point2D, Rect2D, Size2D, Thickness};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RRect2D {
    /// Coordinates of the rectangle.
    pub origin: Point2D,
    /// Size of the rectangle.
    pub size: Size2D,
    /// Radius of all four corners.
    pub corner_radius: Thickness,
}

impl RRect2D {
    pub const EMPTY: Self = Self {
        origin: Point2D::ZERO,
        size: Size2D::ZERO,
        corner_radius: Thickness::default(),
    };

    pub const fn new(origin: Point2D, size: Size2D, corner_radius: Thickness) -> Self {
        Self { origin, size, corner_radius }
    }

    pub const fn as_rect(&self) -> Rect2D {
        Rect2D {
            origin: self.origin,
            size: self.size,
        }
    }

    pub const fn width(&self) -> f32 {
        self.size.width
    }

    pub const fn height(&self) -> f32 {
        self.size.height
    }

    pub fn center(&self) -> Point2D {
        self.origin + self.size.to_vector() / 2.0
    }

    pub fn as_box(&self) -> Box2D {
        Box2D {
            min: self.origin,
            max: self.origin + self.size.to_vector(),
        }
    }

    pub fn contains(&self, pt: Point2D) -> bool {
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
