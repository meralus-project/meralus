use meralus_physics::{AabbSource, PhysicsContext, RayCastResult};
use meralus_shared::{FrustumCulling, Point3D, Quat, Transform3D, Vector3D};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub position: Point3D,

    pub yaw: f32,
    pub pitch: f32,

    pub right: Vector3D,
    pub up: Vector3D,
    pub front: Vector3D,

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

        let front = Vector3D::new(yaw.cos() * pitch.cos(), pitch.sin(), yaw.sin() * pitch.cos()).normalize();

        let right = front.cross(Vector3D::Y).normalize();
        let up = right.cross(front).normalize();

        Self {
            yaw,
            pitch,
            position: Point3D::ZERO,
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

    pub fn new(position: Point3D) -> Self {
        Self { position, ..Self::default() }
    }

    pub const fn target(&self) -> Point3D {
        Point3D::new(self.position.x + self.front.x, self.position.y + self.front.y, self.position.z + self.front.z)
    }

    pub fn set_position<T: AabbSource>(&mut self, context: &PhysicsContext<T>, position: Point3D) {
        self.position = position;
        self.update_looking_at(context);
        self.update_frustum();
    }

    pub fn update_looking_at<T: AabbSource>(&mut self, context: &PhysicsContext<T>) {
        const BLOCK_REACH_DISTANCE: f32 = 20f32;

        let origin = self.position.as_dvec3();
        let target = origin + (self.front * BLOCK_REACH_DISTANCE).as_dvec3();

        self.looking_at = context.raycast(origin, target, true).filter(RayCastResult::is_block);
    }

    pub fn handle_mouse<T: AabbSource>(&mut self, context: &PhysicsContext<T>, (yaw, pitch): (f32, f32)) {
        self.front = Vector3D::new(yaw.cos() * pitch.cos(), pitch.sin(), yaw.sin() * pitch.cos()).normalize();
        self.right = self.front.cross(Vector3D::Y).normalize();
        self.up = self.right.cross(self.front).normalize();

        self.update_looking_at(context);
    }

    pub fn projection(&self) -> Transform3D {
        Transform3D::perspective_rh_gl(self.fov, self.aspect_ratio, self.z_near, self.z_far)
    }

    pub fn view(&self) -> Transform3D {
        Transform3D::look_at_rh(self.position, self.target(), self.up)
    }

    pub fn matrix(&self) -> Transform3D {
        self.projection() * self.view()
    }

    pub fn update_frustum(&mut self) {
        self.frustum.update(self.matrix());
    }
}
