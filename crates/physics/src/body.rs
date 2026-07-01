use crate::Aabb;

pub struct PhysicsConfig {
    /// A multiplier that determines how strongly an object is affected by
    /// gravity. Default value is `1.0`.
    pub gravity_scale: f32,
    /// A multiplier that determines how strongly an object sticks to a surface.
    /// The higher the value, the faster the object's relative speed decreases
    /// after it stops moving. Values above `1.0` are meaningless. The default
    /// value is `0.5`.
    pub linear_damping: f32,
    pub vertical_damping: f32,
    pub friction: f32,
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        Self {
            gravity_scale: 1.0,
            linear_damping: 0.5,
            vertical_damping: 1.0,
            friction: 1.0,
        }
    }
}

pub struct PhysicsBody {
    pub config: PhysicsConfig,
    pub is_on_ground: bool,
    pub size: glam::Vec3,
    pub position: glam::Vec3,
    pub velocity: glam::Vec3,
}

impl PhysicsBody {
    pub fn new(position: glam::Vec3, size: glam::Vec3) -> Self {
        Self {
            config: PhysicsConfig::default(),
            is_on_ground: false,
            size,
            position,
            velocity: glam::Vec3::ZERO,
        }
    }

    pub fn aabb(&self) -> Aabb {
        let half_size = self.size / 2.0;

        Aabb::new((self.position - half_size).as_dvec3(), (self.position + half_size).as_dvec3())
    }
}
