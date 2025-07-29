use glam::{Mat4, Vec2, Vec3, vec3};
use meralus_shared::FrustumCulling;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub position: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub fov: f32,
    pub aspect_ratio: f32,
    pub z_near: f32,
    pub z_far: f32,
    pub frustum: FrustumCulling,
}

impl Camera {
    pub const fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            target: Vec3::ZERO,
            aspect_ratio: 1024.0 / 768.0,
            up: vec3(0.0, 0.0, 1.0),
            fov: 55f32.to_radians(),
            z_near: 0.01,
            z_far: 10000.0,
            frustum: FrustumCulling::default(),
        }
    }

    pub fn unproject_position(&self, width: f32, height: f32, position: Vec3) -> Option<(Vec2, f32)> {
        let clip_space = self.matrix() * position.extend(1.0);

        if clip_space.w <= 0.0 {
            return None;
        }

        let ndc = clip_space.truncate() / clip_space.w;

        let x = (ndc.x + 1.0) * 0.5 * width;
        let y = (1.0 - ndc.y) * 0.5 * height;

        Some((Vec2::new(x, y), clip_space.w))
    }

    pub fn projection(&self) -> Mat4 {
        Mat4::perspective_rh_gl(self.fov, self.aspect_ratio, self.z_near, self.z_far)
    }

    pub fn view(&self) -> Mat4 {
        Mat4::look_at_rh(self.position, self.target, self.up)
    }

    pub fn matrix(&self) -> Mat4 {
        self.projection() * self.view()
    }

    pub fn update_frustum(&mut self) {
        self.frustum.update(self.matrix());
    }
}
