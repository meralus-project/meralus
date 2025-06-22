#![allow(
    clippy::missing_errors_doc,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation
)]

mod color;
mod lerp;
#[cfg(feature = "network")] mod network;

use std::{fmt, ops::Add};

#[cfg(feature = "network")]
pub use self::network::{Client, IncomingPacket, OutgoingPacket, Player, ServerConnection};
pub use self::{color::Color, lerp::Lerp};

pub type Size2D = glamour::Size2;
pub type Size3D = glamour::Size3;
pub type Point2D = glamour::Point2;
pub type Point3D = glamour::Point3;
pub type Rect2D = glamour::Rect;

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
            self.size.width,
            self.size.height,
            self.size.depth,
            self.origin.x,
            self.origin.y,
            self.origin.z
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
