mod cube;
mod round_rect;
mod thickness;

pub use self::{cube::Cube3D, round_rect::RRect2D, thickness::Thickness};

pub type ISize2D = glamour::Size2<i32>;
pub type ISize3D = glamour::Size3<i32>;
pub type IPoint2D = glamour::Point2<i32>;
pub type IPoint3D = glamour::Point3<i32>;

pub type USize2D = glamour::Size2<u32>;
pub type USize3D = glamour::Size3<u32>;
pub type UPoint2D = glamour::Point2<u32>;
pub type UPoint3D = glamour::Point3<u32>;

pub type USizePoint2D = glam::USizeVec2;
pub type USizePoint3D = glam::USizeVec3;

pub type DSize3D = glamour::Size3<f64>;
pub type DPoint2D = glamour::Point2<f64>;
pub type DPoint3D = glamour::Point3<f64>;
pub type DVector2D = glamour::Vector2<f64>;
pub type DVector3D = glamour::Vector3<f64>;

pub type Size2D = glamour::Size2;
pub type Size3D = glamour::Size3;
pub type Point2D = glamour::Point2;
pub type Point3D = glamour::Point3;
pub type Vector2D = glamour::Vector2;
pub type Vector3D = glamour::Vector3;
pub type Vector4D = glamour::Vector4;
pub type Rect2D = glamour::Rect;
pub type Box2D = glamour::Box2;
pub type Transform2D = glamour::Matrix3<f32>;
pub type Transform3D = glamour::Matrix4<f32>;
pub type Quat = glam::Quat;
pub type Angle = glamour::Angle;

pub trait MatrixExt<T> {
    #[must_use]
    fn translate(self, position: T) -> Self;
    #[must_use]
    fn scale(self, value: T) -> Self;
    #[must_use]
    fn rotate_x(self, angle: f32) -> Self;
    #[must_use]
    fn rotate_y(self, angle: f32) -> Self;
    #[must_use]
    fn rotate_z(self, angle: f32) -> Self;
}

impl MatrixExt<Vector3D> for Transform3D {
    fn translate(self, position: Vector3D) -> Self {
        self * Self::from_translation(position)
    }

    fn scale(self, value: Vector3D) -> Self {
        self * Self::from_scale(value)
    }

    fn rotate_x(self, angle: f32) -> Self {
        self * Self::from_rotation_x(Angle::from_radians(angle))
    }

    fn rotate_y(self, angle: f32) -> Self {
        self * Self::from_rotation_y(Angle::from_radians(angle))
    }

    fn rotate_z(self, angle: f32) -> Self {
        self * Self::from_rotation_z(Angle::from_radians(angle))
    }
}
