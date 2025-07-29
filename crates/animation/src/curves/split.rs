use super::{Curve, ICurve};

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Split {
    at: f32,
    begin: Curve,
    end: Curve,
}

impl ICurve for Split {
    fn transform(&self, t: f32) -> f32 {
        assert!((0.0..=1.0).contains(&t));
        assert!((0.0..=1.0).contains(&self.at));

        match t {
            0.0 | 1.0 => t,
            t if (t - self.at).abs() < f32::EPSILON => self.at,
            t if t < self.at => {
                let curve_progress = t / self.at;
                let transformed = ICurve::transform(&self.begin, curve_progress);

                0f32.mul_add(1.0 - transformed, self.at * transformed)
            }
            t => {
                let curve_progress = (t - self.at) / (1.0 - self.at);
                let transformed = ICurve::transform(&self.end, curve_progress);

                self.at.mul_add(1.0 - transformed, 1.0 * transformed)
            }
        }
    }
}
