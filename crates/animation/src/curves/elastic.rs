use std::f32::consts::PI;

use super::ParametricCurve;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct ElasticInCurve {
    period: f32,
}

impl ElasticInCurve {
    #[must_use]
    pub const fn default() -> Self {
        Self { period: 0.4 }
    }
}

impl ParametricCurve<f32> for ElasticInCurve {
    fn transform_internal(&self, mut t: f32) -> f32 {
        let s = self.period / 4.0;

        t -= 1.0;

        -(10.0 * t).exp2() * ((t - s) * (PI * 2.0) / self.period).cos()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct ElasticOutCurve {
    period: f32,
}

impl ElasticOutCurve {
    #[must_use]
    pub const fn default() -> Self {
        Self { period: 0.4 }
    }
}

impl ParametricCurve<f32> for ElasticOutCurve {
    fn transform_internal(&self, t: f32) -> f32 {
        let s = self.period / 4.0;

        (-10.0 * t).exp2().mul_add(((t - s) * (PI * 2.0) / self.period).sin(), 1.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct ElasticInOutCurve {
    period: f32,
}

impl ElasticInOutCurve {
    #[must_use]
    pub const fn default() -> Self {
        Self { period: 0.4 }
    }
}

impl ParametricCurve<f32> for ElasticInOutCurve {
    fn transform_internal(&self, mut t: f32) -> f32 {
        let s = self.period / 4.0;

        t = 2f32.mul_add(t, -1.0);

        if t < 0.0 {
            -0.5 * (10.0 * t).exp2() * ((t - s) * (PI * 2.0) / self.period).sin()
        } else {
            ((-10.0 * t).exp2() * ((t - s) * (PI * 2.0) / self.period).sin()).mul_add(0.5, 1.0)
        }
    }
}
