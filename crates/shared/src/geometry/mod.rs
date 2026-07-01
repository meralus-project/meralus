mod box2d;
mod cube;
mod rect;
mod round_rect;
mod thickness;

pub use self::{box2d::Box2, cube::Cube, rect::Rect, round_rect::RRect, thickness::Thickness};

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

impl MatrixExt<glam::Vec3> for glam::Mat4 {
    fn translate(self, position: glam::Vec3) -> Self {
        self * Self::from_translation(position)
    }

    fn scale(self, value: glam::Vec3) -> Self {
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
