use super::ParametricCurve;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Threshold {
    pub(super) threshold: f32,
}

impl ParametricCurve<f32> for Threshold {
    fn transform_internal(&self, t: f32) -> f32 {
        assert!((0.0..=1.0).contains(&self.threshold));

        if t < self.threshold { 0.0 } else { 1.0 }
    }
}
