#![allow(clippy::missing_errors_doc, clippy::cast_sign_loss, clippy::cast_possible_truncation)]

mod color;
mod frustum;
mod lerp;
#[cfg(feature = "network")] mod network;

use core::f32;
use std::{
    fmt::{self},
    ops::{Add, AddAssign, SubAssign},
};

#[cfg(feature = "network")]
pub use self::network::{Client, IncomingPacket, OutgoingPacket, Player, ServerConnection};
pub use self::{color::Color, frustum::FrustumCulling, lerp::Lerp};

pub type Size2D = glamour::Size2;
pub type Size3D = glamour::Size3;
pub type Point2D = glamour::Point2;
pub type Point3D = glamour::Point3;
pub type Rect2D = glamour::Rect;
pub type Box2D = glamour::Box2;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Thickness([f32; 4]);

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

    pub const fn any_above(&self, value: f32) -> bool {
        self.0[0] > value || self.0[1] > value || self.0[2] > value || self.0[3] > value
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RRect2D {
    /// Coordinates of the rectangle.
    pub origin: Point2D,
    /// Size of the rectangle.
    pub size: Size2D,
    /// Radius of all four corners.
    pub corner_radius: Thickness,
}

impl RRect2D {
    pub const fn default() -> Self {
        Self {
            origin: Point2D { x: 0.0, y: 0.0 },
            size: Size2D { width: 0.0, height: 0.0 },
            corner_radius: Thickness::default(),
        }
    }

    pub const fn new(origin: Point2D, size: Size2D, corner_radius: Thickness) -> Self {
        Self { origin, size, corner_radius }
    }

    pub const fn as_rect(&self) -> Rect2D {
        Rect2D {
            origin: self.origin,
            size: self.size,
        }
    }

    pub const fn width(&self) -> f32 {
        self.size.width
    }

    pub const fn height(&self) -> f32 {
        self.size.height
    }

    pub const fn center(&self) -> Point2D {
        Point2D {
            x: self.origin.x + self.size.width / 2.0,
            y: self.origin.y + self.size.height / 2.0,
        }
    }

    pub const fn as_box(&self) -> Box2D {
        Box2D {
            min: self.origin,
            max: Point2D {
                x: self.origin.x + self.size.width,
                y: self.origin.y + self.size.height,
            },
        }
    }
}

impl RRect2D {
    pub const fn contains(&self, pt: Point2D) -> bool {
        let center = self.center();
        let pt = Point2D {
            x: pt.x - center.x,
            y: pt.y - center.y,
        };

        let radius = match pt {
            pt if pt.x < 0.0 && pt.y < 0.0 => self.corner_radius.left(),
            pt if pt.x >= 0.0 && pt.y < 0.0 => self.corner_radius.top(),
            pt if pt.x >= 0.0 && pt.y >= 0.0 => self.corner_radius.right(),
            pt if pt.x < 0.0 && pt.y >= 0.0 => self.corner_radius.bottom(),
            _ => 0.0,
        };

        let px = (pt.x.abs() - (self.width() / 2.0 - radius).max(0.0)).max(0.0);
        let py = (pt.y.abs() - (self.height() / 2.0 - radius).max(0.0)).max(0.0);

        px * px + py * py <= radius * radius
    }
}

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

pub trait AsValue<T> {
    fn as_value(&self) -> T;
}

pub trait FromValue<T> {
    fn from_value(value: &T) -> Self;
}

impl<A, T> FromValue<T> for A
where
    T: AsValue<A>,
{
    fn from_value(value: &T) -> Self {
        value.as_value()
    }
}

pub trait InspectMut<T> {
    fn inspect_mut<F: FnOnce(&mut T)>(&mut self, func: F);
}

impl<T> InspectMut<T> for Option<T> {
    fn inspect_mut<F: FnOnce(&mut T)>(&mut self, func: F) {
        if let Some(data) = self {
            func(data);
        }
    }
}

pub trait Num {
    fn one() -> Self;
}

impl Num for usize {
    fn one() -> Self {
        1
    }
}

impl Num for u8 {
    fn one() -> Self {
        1
    }
}

pub struct Ranged<T> {
    pub min: T,
    pub max: T,
    pub value: T,
}

impl<T: Num + PartialOrd + SubAssign + AddAssign + Copy> Ranged<T> {
    pub const fn new(default_value: T, min: T, max: T) -> Self {
        Self {
            min,
            max,
            value: default_value,
        }
    }

    pub fn increase(&mut self) {
        if self.value == self.max {
            self.value = self.min;
        } else {
            self.value += T::one();
        }
    }

    pub fn decrease(&mut self) {
        if self.value == self.min {
            self.value = self.max;
        } else {
            self.value -= T::one();
        }
    }
}
