use crate::Color;

pub trait Lerp {
    #[must_use]
    fn lerp(&self, end: &Self, x: f32) -> Self;
}

impl Lerp for f32 {
    #[inline]
    fn lerp(&self, end: &Self, x: f32) -> Self {
        self * (1.0 - x) + end * x
    }
}

impl Lerp for Color {
    #[inline]
    fn lerp(&self, end: &Self, x: f32) -> Self {
        Self::new(
            f32::from(self.get_red()).lerp(&f32::from(end.get_red()), x) as u8,
            f32::from(self.get_green()).lerp(&f32::from(end.get_green()), x) as u8,
            f32::from(self.get_blue()).lerp(&f32::from(end.get_blue()), x) as u8,
            f32::from(self.get_alpha()).lerp(&f32::from(end.get_alpha()), x) as u8,
        )
    }
}
