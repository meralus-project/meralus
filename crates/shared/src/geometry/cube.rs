use core::{fmt, ops::Add};

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Cube {
    pub origin: glam::Vec3,
    pub size: glam::Vec3,
}

impl Cube {
    pub const ONE: Self = Self {
        origin: glam::Vec3::ZERO,
        size: glam::Vec3::ONE,
    };
    pub const ZERO: Self = Self {
        origin: glam::Vec3::ZERO,
        size: glam::Vec3::ZERO,
    };

    pub const fn new(origin: glam::Vec3, size: glam::Vec3) -> Self {
        Self { origin, size }
    }
}

impl fmt::Display for Cube {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}x{}x{} at {}x{}x{}",
            self.size.x, self.size.y, self.size.z, self.origin.x, self.origin.y, self.origin.z
        )
    }
}

impl Add<glam::Vec3> for Cube {
    type Output = Self;

    fn add(self, rhs: glam::Vec3) -> Self::Output {
        Self {
            origin: self.origin + rhs,
            size: self.size,
        }
    }
}
