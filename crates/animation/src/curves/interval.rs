use super::{Curve, ICurve, ParametricCurve};

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
#[allow(dead_code)]
pub struct Interval {
    begin: f32,
    end: f32,
    curve: Curve,
}

impl ParametricCurve<f32> for Interval {
    fn transform_internal(&self, mut t: f32) -> f32 {
        assert!((0.0..=1.0).contains(&self.begin));
        assert!((0.0..=1.0).contains(&self.end));
        assert!(self.end >= self.begin);

        t = ((t - self.begin) / (self.end - self.begin)).clamp(0.0, 1.0);

        match t {
            0.0 | 1.0 => t,
            t => self.curve.transform(t),
        }
    }
}
