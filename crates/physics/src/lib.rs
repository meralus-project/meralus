mod aabb;
mod body;
mod context;
mod raycast;

pub use self::{
    aabb::Aabb,
    body::{PhysicsBody, PhysicsConfig},
    context::PhysicsContext,
    raycast::{HitType, RayCastResult},
};

pub trait AabbSource {
    fn get_aabb(&self, position: glam::Vec3) -> Option<Aabb>;
    fn get_block_aabb(&self, position: glam::IVec3) -> Option<Aabb>;
}

impl<T: AabbSource> AabbSource for &T {
    fn get_aabb(&self, position: glam::Vec3) -> Option<Aabb> {
        T::get_aabb(self, position)
    }

    fn get_block_aabb(&self, position: glam::IVec3) -> Option<Aabb> {
        T::get_block_aabb(self, position)
    }
}
