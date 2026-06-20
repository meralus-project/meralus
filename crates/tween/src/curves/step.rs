use super::ParametricCurve;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Stepped {
    pub(super) is_initial_step_single_frame: bool,
    pub(super) is_final_step_single_frame: bool,
    pub(super) step_count: u16,
}

impl Stepped {
    #[must_use]
    pub const fn new(step_count: u16) -> Self {
        Self {
            is_initial_step_single_frame: false,
            is_final_step_single_frame: false,
            step_count,
        }
    }

    pub const fn initial_step_single_frame(&mut self) {
        self.is_initial_step_single_frame = true;
    }

    pub const fn final_step_single_frame(&mut self) {
        self.is_final_step_single_frame = true;
    }
}

impl ParametricCurve<f32> for Stepped {
    fn transform_internal(&self, t: f32) -> f32 {
        let mut step_time = t * f32::from(self.step_count);

        if self.is_initial_step_single_frame && t > 0.0 {
            step_time = step_time.ceil();
        } else if self.is_final_step_single_frame && t < 1.0 {
            step_time = step_time.floor();
        } else {
            step_time = step_time.round();
        }

        step_time / f32::from(self.step_count)
    }
}
