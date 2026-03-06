use core::{fmt, ops::Add};

use super::{Point3D, Size3D};

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Cube3D {
    pub origin: Point3D,
    pub size: Size3D,
}

impl Cube3D {
    pub const ONE: Self = Self {
        origin: Point3D::ZERO,
        size: Size3D::ONE,
    };
    pub const ZERO: Self = Self {
        origin: Point3D::ZERO,
        size: Size3D::ZERO,
    };

    pub const fn new(origin: Point3D, size: Size3D) -> Self {
        Self { origin, size }
    }
}

impl fmt::Display for Cube3D {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}x{}x{} at {}x{}x{}",
            self.size.width, self.size.height, self.size.depth, self.origin.x, self.origin.y, self.origin.z
        )
    }
}

impl Add<Point3D> for Cube3D {
    type Output = Self;

    fn add(self, rhs: Point3D) -> Self::Output {
        Self {
            origin: self.origin + rhs,
            size: self.size,
        }
    }
}
