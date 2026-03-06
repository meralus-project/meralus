mod bounce;
mod cubic;
mod decelerate;
mod elastic;
mod interval;
mod linear;
mod saw_tooth;
mod split;
mod step;
mod three_point_cubic;
mod threshold;

use three_point_cubic::ThreePointCubic;

pub use self::{
    bounce::{BounceInCurve, BounceInOutCurve, BounceOutCurve},
    cubic::Cubic,
    decelerate::DecelerateCurve,
    elastic::{ElasticInCurve, ElasticInOutCurve, ElasticOutCurve},
    linear::Linear,
    saw_tooth::SawTooth,
    step::Stepped,
    threshold::Threshold,
};

trait ParametricCurve<T> {
    fn transform_internal(&self, t: f32) -> T;
}

pub trait ICurve {
    fn transform(&self, t: f32) -> f32;
}

impl<T: ParametricCurve<f32>> ICurve for T {
    fn transform(&self, t: f32) -> f32
    where
        Self: Sized,
    {
        match t {
            0.0 | 1.0 => t,
            t => self.transform_internal(t),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum Curve {
    None,
    Linear(Linear),
    Cubic(Cubic),
    ThreePointCubic(ThreePointCubic),
    Threshold(Threshold),
    SawTooth(SawTooth),
    BounceIn(BounceInCurve),
    BounceOut(BounceOutCurve),
    BounceInOut(BounceInOutCurve),
    ElasticIn(ElasticInCurve),
    ElasticOut(ElasticOutCurve),
    ElasticInOut(ElasticInOutCurve),
    Decelerate(DecelerateCurve),
    Stepped(Stepped),
}

impl Curve {
    pub const BOUNCE_IN: Self = Self::BounceIn(BounceInCurve);
    pub const BOUNCE_IN_OUT: Self = Self::BounceInOut(BounceInOutCurve);
    pub const BOUNCE_OUT: Self = Self::BounceOut(BounceOutCurve);
    pub const DECELERATE: Self = Self::Decelerate(DecelerateCurve);
    pub const EASE: Self = Self::cubic(0.25, 0.1, 0.25, 1.0);
    pub const EASE_IN: Self = Self::cubic(0.42, 0.0, 1.0, 1.0);
    pub const EASE_IN_BACK: Self = Self::cubic(0.6, -0.28, 0.735, 0.045);
    pub const EASE_IN_CIRC: Self = Self::cubic(0.6, 0.04, 0.98, 0.335);
    pub const EASE_IN_CUBIC: Self = Self::cubic(0.55, 0.055, 0.675, 0.19);
    pub const EASE_IN_EXPO: Self = Self::cubic(0.95, 0.05, 0.795, 0.035);
    pub const EASE_IN_OUT: Self = Self::cubic(0.42, 0.0, 0.58, 1.0);
    pub const EASE_IN_OUT_BACK: Self = Self::cubic(0.68, -0.55, 0.265, 1.55);
    pub const EASE_IN_OUT_CIRC: Self = Self::cubic(0.785, 0.135, 0.15, 0.86);
    pub const EASE_IN_OUT_CUBIC: Self = Self::cubic(0.645, 0.045, 0.355, 1.0);
    pub const EASE_IN_OUT_CUBIC_EMPHASIZED: Self = Self::three_point_cubic((0.05, 0.0), (0.133_333, 0.06), (0.166_666, 0.4), (0.208_333, 0.82), (0.25, 1.0));
    pub const EASE_IN_OUT_EXPO: Self = Self::cubic(1.0, 0.0, 0.0, 1.0);
    pub const EASE_IN_OUT_QUAD: Self = Self::cubic(0.455, 0.03, 0.515, 0.955);
    pub const EASE_IN_OUT_QUART: Self = Self::cubic(0.77, 0.0, 0.175, 1.0);
    pub const EASE_IN_OUT_QUINT: Self = Self::cubic(0.86, 0.0, 0.07, 1.0);
    pub const EASE_IN_OUT_SINE: Self = Self::cubic(0.445, 0.05, 0.55, 0.95);
    pub const EASE_IN_QUAD: Self = Self::cubic(0.55, 0.085, 0.68, 0.53);
    pub const EASE_IN_QUART: Self = Self::cubic(0.895, 0.03, 0.685, 0.22);
    pub const EASE_IN_QUINT: Self = Self::cubic(0.755, 0.05, 0.855, 0.06);
    pub const EASE_IN_SINE: Self = Self::cubic(0.47, 0.0, 0.745, 0.715);
    pub const EASE_IN_TO_LINEAR: Self = Self::cubic(0.67, 0.03, 0.65, 0.09);
    pub const EASE_OUT: Self = Self::cubic(0.0, 0.0, 0.58, 1.0);
    pub const EASE_OUT_BACK: Self = Self::cubic(0.175, 0.885, 0.32, 1.275);
    pub const EASE_OUT_CIRC: Self = Self::cubic(0.075, 0.82, 0.165, 1.0);
    pub const EASE_OUT_CUBIC: Self = Self::cubic(0.215, 0.61, 0.355, 1.0);
    pub const EASE_OUT_EXPO: Self = Self::cubic(0.19, 1.0, 0.22, 1.0);
    pub const EASE_OUT_QUAD: Self = Self::cubic(0.25, 0.46, 0.45, 0.94);
    pub const EASE_OUT_QUART: Self = Self::cubic(0.165, 0.84, 0.44, 1.0);
    pub const EASE_OUT_QUINT: Self = Self::cubic(0.23, 1.0, 0.32, 1.0);
    pub const EASE_OUT_SINE: Self = Self::cubic(0.39, 0.575, 0.565, 1.0);
    pub const ELASTIC_IN: Self = Self::ElasticIn(ElasticInCurve::default());
    pub const ELASTIC_IN_OUT: Self = Self::ElasticInOut(ElasticInOutCurve::default());
    pub const ELASTIC_OUT: Self = Self::ElasticOut(ElasticOutCurve::default());
    pub const FAST_EASE_IN_TO_SLOW_EASE_OUT: Self = Self::three_point_cubic((0.056, 0.024), (0.108, 0.3085), (0.198, 0.541), (0.3655, 1.0), (0.5465, 0.989));
    pub const FAST_LINEAR_TO_SLOW_EASE_IN: Self = Self::cubic(0.18, 1.0, 0.04, 1.0);
    pub const FAST_OUT_SLOW_IN: Self = Self::cubic(0.4, 0.0, 0.2, 1.0);
    pub const LINEAR: Self = Self::Linear(Linear);
    pub const LINEAR_TO_EASE_OUT: Self = Self::cubic(0.35, 0.91, 0.33, 0.97);
    pub const NONE: Self = Self::None;
    pub const SLOW_MIDDLE: Self = Self::cubic(0.15, 0.85, 0.85, 0.15);

    #[must_use]
    pub const fn three_point_cubic(a1: (f32, f32), b1: (f32, f32), midpoint: (f32, f32), a2: (f32, f32), b2: (f32, f32)) -> Self {
        Self::ThreePointCubic(ThreePointCubic::new(a1, b1, midpoint, a2, b2))
    }

    #[must_use]
    pub const fn cubic(a: f32, b: f32, c: f32, d: f32) -> Self {
        Self::Cubic(Cubic::new(a, b, c, d))
    }

    #[must_use]
    pub const fn threshold(threshold: f32) -> Self {
        Self::Threshold(Threshold { threshold })
    }

    #[must_use]
    pub const fn saw_tooth(count: f32) -> Self {
        Self::SawTooth(SawTooth { count })
    }

    #[must_use]
    pub const fn stepped(step_count: u16, is_initial_step_single_frame: bool, is_final_step_single_frame: bool) -> Self {
        Self::Stepped(Stepped {
            is_initial_step_single_frame,
            is_final_step_single_frame,
            step_count,
        })
    }
}

impl ParametricCurve<f32> for Curve {
    fn transform_internal(&self, t: f32) -> f32 {
        match self {
            Self::None => 1.0,
            Self::Linear(curve) => curve.transform_internal(t),
            Self::Cubic(curve) => curve.transform_internal(t),
            Self::ThreePointCubic(curve) => curve.transform_internal(t),
            Self::Threshold(curve) => curve.transform_internal(t),
            Self::SawTooth(curve) => curve.transform_internal(t),
            Self::BounceIn(curve) => curve.transform_internal(t),
            Self::BounceOut(curve) => curve.transform_internal(t),
            Self::BounceInOut(curve) => curve.transform_internal(t),
            Self::ElasticIn(curve) => curve.transform_internal(t),
            Self::ElasticOut(curve) => curve.transform_internal(t),
            Self::ElasticInOut(curve) => curve.transform_internal(t),
            Self::Decelerate(curve) => curve.transform_internal(t),
            Self::Stepped(curve) => curve.transform_internal(t),
        }
    }
}
