use super::{Curve, ParametricCurve};

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Split {
    pub at: f32,
    pub begin: Curve,
    pub end: Curve,
}

impl ParametricCurve<f32> for Split {
    fn transform_internal(&self, t: f32) -> f32 {
        assert!((0.0..=1.0).contains(&t));
        assert!((0.0..=1.0).contains(&self.at));

        match t {
            0.0 | 1.0 => t,
            t if (t - self.at).abs() < f32::EPSILON => self.at,
            t if t < self.at => {
                let curve_progress = t / self.at;
                let transformed = self.begin.transform_internal(curve_progress);

                0f32.mul_add(1.0 - transformed, self.at * transformed)
            }
            t => {
                let curve_progress = (t - self.at) / (1.0 - self.at);
                let transformed = self.end.transform_internal(curve_progress);

                self.at.mul_add(1.0 - transformed, 1.0 * transformed)
            }
        }
    }
}
