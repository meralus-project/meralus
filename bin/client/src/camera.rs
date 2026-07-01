use mavelin_physics::{AabbSource, PhysicsContext, RayCastResult};
use mavelin_shared::FrustumCulling;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub position: glam::Vec3,

    pub yaw: f32,
    pub pitch: f32,

    pub right: glam::Vec3,
    pub up: glam::Vec3,
    pub front: glam::Vec3,

    pub looking_at: Option<RayCastResult>,

    pub fov: f32,
    pub aspect_ratio: f32,
    pub z_near: f32,
    pub z_far: f32,

    pub frustum: FrustumCulling,
}

impl Camera {
    pub fn default() -> Self {
        let yaw = 0f32;
        let pitch = 0f32;

        let front = glam::Vec3::new(yaw.cos() * pitch.cos(), pitch.sin(), yaw.sin() * pitch.cos()).normalize();

        let right = front.cross(glam::Vec3::Y).normalize();
        let up = right.cross(front).normalize();

        Self {
            yaw,
            pitch,
            position: glam::Vec3::ZERO,
            right,
            up,
            front,
            looking_at: None,
            fov: 55f32.to_radians(),
            z_near: 0.01,
            z_far: 10000.0,
            aspect_ratio: 1024.0 / 768.0,
            frustum: FrustumCulling::default(),
        }
    }

    #[inline]
    pub fn new(position: glam::Vec3) -> Self {
        Self { position, ..Self::default() }
    }

    #[inline]
    pub const fn target(&self) -> glam::Vec3 {
        glam::Vec3::new(self.position.x + self.front.x, self.position.y + self.front.y, self.position.z + self.front.z)
    }

    #[inline]
    pub fn set_position<T: AabbSource>(&mut self, context: &PhysicsContext<T>, position: glam::Vec3) {
        self.position = position;
        self.update_looking_at(context);
        self.update_frustum();
    }

    #[inline]
    pub fn update_looking_at<T: AabbSource>(&mut self, context: &PhysicsContext<T>) {
        const BLOCK_REACH_DISTANCE: f32 = 20f32;

        let origin = self.position.as_dvec3();
        let target = origin + (self.front * BLOCK_REACH_DISTANCE).as_dvec3();

        self.looking_at = context.raycast(origin, target, true).filter(RayCastResult::is_block);
    }

    #[inline]
    pub fn handle_mouse<T: AabbSource>(&mut self, context: &PhysicsContext<T>, (yaw, pitch): (f32, f32)) {
        self.front = glam::Vec3::new(yaw.cos() * pitch.cos(), pitch.sin(), yaw.sin() * pitch.cos()).normalize();
        self.right = self.front.cross(glam::Vec3::Y).normalize();
        self.up = self.right.cross(self.front).normalize();

        self.update_looking_at(context);
    }

    #[inline]
    pub fn projection(&self) -> glam::Mat4 {
        glam::camera::rh::proj::directx::perspective(self.fov, self.aspect_ratio, self.z_near, self.z_far)
    }

    #[inline]
    pub fn view(&self) -> glam::Mat4 {
        glam::camera::rh::view::look_at_mat4(glam::Vec3::ZERO, self.front, self.up)
    }

    #[inline]
    pub fn world_view(&self) -> glam::Mat4 {
        glam::camera::rh::view::look_at_mat4(self.position, self.target(), self.up)
    }

    #[inline]
    #[allow(dead_code)]
    pub fn matrix(&self) -> glam::Mat4 {
        self.projection() * self.view()
    }

    #[inline]
    pub fn world_matrix(&self) -> glam::Mat4 {
        self.projection() * self.world_view()
    }

    #[inline]
    pub fn update_frustum(&mut self) {
        self.frustum.update(self.world_matrix());
    }
}
