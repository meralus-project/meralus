use meralus_shared::{Point3D, Size3D, Vector3D};

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
    pub size: Size3D,
    pub position: Point3D,
    pub velocity: Vector3D,
}

impl PhysicsBody {
    pub fn new(position: Point3D, size: Size3D) -> Self {
        Self {
            config: PhysicsConfig::default(),
            is_on_ground: false,
            size,
            position,
            velocity: Vector3D::ZERO,
        }
    }

    pub fn aabb(&self) -> Aabb {
        let half_size = self.size.to_vector() / 2.0;

        Aabb::new((self.position - half_size).as_::<f64>(), (self.position + half_size).as_::<f64>())
    }
}
