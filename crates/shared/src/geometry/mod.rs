mod box2d;
mod cube;
mod rect;
mod round_rect;
mod thickness;

pub use self::{box2d::Box2D, cube::Cube3D, rect::Rect, round_rect::RRect2D, thickness::Thickness};

pub type ISize2D = glam::IVec2;
pub type ISize3D = glam::IVec3;
pub type IPoint2D = glam::IVec2;
pub type IPoint3D = glam::IVec3;

pub type USize2D = glam::UVec2;
pub type USize3D = glam::UVec3;
pub type UPoint2D = glam::UVec2;
pub type UPoint3D = glam::UVec3;

pub type USizePoint2D = glam::USizeVec2;
pub type USizePoint3D = glam::USizeVec3;

pub type DSize3D = glam::DVec3;
pub type DPoint2D = glam::DVec2;
pub type DPoint3D = glam::DVec3;
pub type DVector2D = glam::DVec2;
pub type DVector3D = glam::DVec3;

pub type Size2D = glam::Vec2;
pub type Size3D = glam::Vec3;
pub type Point2D = glam::Vec2;
pub type Point3D = glam::Vec3;
pub type Vector2D = glam::Vec2;
pub type Vector3D = glam::Vec3;
pub type Vector4D = glam::Vec4;
pub type Transform2D = glam::Mat3;
pub type Transform3D = glam::Mat4;
pub type Quat = glam::Quat;

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
        self * Self::from_rotation_x(angle)
    }

    fn rotate_y(self, angle: f32) -> Self {
        self * Self::from_rotation_y(angle)
    }

    fn rotate_z(self, angle: f32) -> Self {
        self * Self::from_rotation_z(angle)
    }
}
