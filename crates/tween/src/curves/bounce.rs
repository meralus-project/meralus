use super::ParametricCurve;

fn bounce(mut t: f32) -> f32 {
    if t < 1.0 / 2.75 {
        7.5625 * t.powi(2)
    } else if t < 2.0 / 2.75 {
        t -= 1.5 / 2.75;

        7.5625f32.mul_add(t.powi(2), 0.75)
    } else if t < 2.5 / 2.75 {
        t -= 2.25 / 2.75;

        7.5625f32.mul_add(t.powi(2), 0.9375)
    } else {
        t -= 2.625 / 2.75;

        7.5625f32.mul_add(t.powi(2), 0.984_375)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BounceInCurve;

impl ParametricCurve<f32> for BounceInCurve {
    fn transform_internal(&self, t: f32) -> f32 {
        1.0 - bounce(1.0 - t)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BounceOutCurve;

impl ParametricCurve<f32> for BounceOutCurve {
    fn transform_internal(&self, t: f32) -> f32 {
        bounce(t)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BounceInOutCurve;

impl ParametricCurve<f32> for BounceInOutCurve {
    fn transform_internal(&self, t: f32) -> f32 {
        if t < 0.5 {
            (1.0 - bounce(t.mul_add(-2.0, 1.0))) * 0.5
        } else {
            bounce(t.mul_add(2.0, -1.0)).mul_add(0.5, 0.5)
        }
    }
}
