use super::{Cubic, ICurve, ParametricCurve};

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Offset {
    dx: f32,
    dy: f32,
}

impl Offset {
    pub const fn from_tuple((dx, dy): (f32, f32)) -> Self {
        Self { dx, dy }
    }
}

impl From<(f32, f32)> for Offset {
    fn from((dx, dy): (f32, f32)) -> Self {
        Self { dx, dy }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct ThreePointCubic {
    a1: Offset,
    b1: Offset,
    midpoint: Offset,
    a2: Offset,
    b2: Offset,
}

impl ThreePointCubic {
    pub const fn new(a1: (f32, f32), b1: (f32, f32), midpoint: (f32, f32), a2: (f32, f32), b2: (f32, f32)) -> Self {
        Self {
            a1: Offset::from_tuple(a1),
            b1: Offset::from_tuple(b1),
            midpoint: Offset::from_tuple(midpoint),
            a2: Offset::from_tuple(a2),
            b2: Offset::from_tuple(b2),
        }
    }
}

impl ParametricCurve<f32> for ThreePointCubic {
    fn transform_internal(&self, t: f32) -> f32 {
        let first_curve = t < self.midpoint.dx;
        let scale_x = if first_curve { self.midpoint.dx } else { 1.0 - self.midpoint.dx };

        let scale_y = if first_curve { self.midpoint.dy } else { 1.0 - self.midpoint.dy };

        let scaled_t = (t - (if first_curve { 0.0 } else { self.midpoint.dx })) / scale_x;

        if first_curve {
            Cubic::new(self.a1.dx / scale_x, self.a1.dy / scale_y, self.b1.dx / scale_x, self.b1.dy / scale_y).transform(scaled_t) * scale_y
        } else {
            Cubic::new(
                (self.a2.dx - self.midpoint.dx) / scale_x,
                (self.a2.dy - self.midpoint.dy) / scale_y,
                (self.b2.dx - self.midpoint.dx) / scale_x,
                (self.b2.dy - self.midpoint.dy) / scale_y,
            )
            .transform(scaled_t)
            .mul_add(scale_y, self.midpoint.dy)
        }
    }
}
