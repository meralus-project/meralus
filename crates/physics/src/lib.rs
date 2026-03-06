mod aabb;
mod body;
mod context;
mod raycast;

use meralus_shared::{IPoint3D, Point3D};

pub use self::{
    aabb::Aabb,
    body::{PhysicsBody, PhysicsConfig},
    context::PhysicsContext,
    raycast::{HitType, RayCastResult},
};

pub trait AabbSource {
    fn get_aabb(&self, position: Point3D) -> Option<Aabb>;
    fn get_block_aabb(&self, position: IPoint3D) -> Option<Aabb>;
}

impl<T: AabbSource> AabbSource for &T {
    fn get_aabb(&self, position: Point3D) -> Option<Aabb> {
        T::get_aabb(self, position)
    }

    fn get_block_aabb(&self, position: IPoint3D) -> Option<Aabb> {
        T::get_block_aabb(self, position)
    }
}
