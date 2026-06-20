use super::ParametricCurve;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DecelerateCurve;

impl ParametricCurve<f32> for DecelerateCurve {
    fn transform_internal(&self, mut t: f32) -> f32 {
        t = 1.0 - t;

        t.mul_add(-t, 1.0)
    }
}
