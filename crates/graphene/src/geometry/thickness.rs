#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Thickness([f32; 4]);

impl Thickness {
    pub const fn default() -> Self {
        Self::all(0.0)
    }

    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self([left, top, right, bottom])
    }

    pub const fn all(value: f32) -> Self {
        Self([value; 4])
    }

    pub const fn left(&self) -> f32 {
        self.0[0]
    }

    pub const fn top(&self) -> f32 {
        self.0[1]
    }

    pub const fn right(&self) -> f32 {
        self.0[2]
    }

    pub const fn bottom(&self) -> f32 {
        self.0[3]
    }

    pub const fn top_left(&self) -> f32 {
        self.0[0]
    }

    pub const fn top_right(&self) -> f32 {
        self.0[1]
    }

    pub const fn bottom_left(&self) -> f32 {
        self.0[2]
    }

    pub const fn bottom_right(&self) -> f32 {
        self.0[3]
    }

    pub const fn any_above(&self, value: f32) -> bool {
        self.0[0] > value || self.0[1] > value || self.0[2] > value || self.0[3] > value
    }
}

impl PartialEq<f32> for Thickness {
    fn eq(&self, other: &f32) -> bool {
        let values = self.0.map(|value| value.eq(other));

        if values[1] == values[0] && values[2] == values[0] && values[3] == values[0] {
            values[0]
        } else {
            false
        }
    }
}

impl PartialOrd<f32> for Thickness {
    fn partial_cmp(&self, other: &f32) -> Option<std::cmp::Ordering> {
        let values = self.0.map(|value| value.partial_cmp(other));

        if values[1] == values[0] && values[2] == values[0] && values[3] == values[0] {
            values[0]
        } else {
            None
        }
    }
}
