use glam::DVec3;
use meralus_shared::Cube3D;
use meralus_world::Face;

use crate::{raycast::RayCastResult, util::VecExt};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: DVec3,
    pub max: DVec3,
}

impl Aabb {
    pub const fn new(min: DVec3, max: DVec3) -> Self {
        Self { min, max }
    }

    pub const fn get_center(&self, size: DVec3) -> DVec3 {
        DVec3::new(self.min.x + size.x / 2.0, self.min.y + size.y / 2.0, self.min.z + size.z / 2.0)
    }

    pub const fn intersects_with_x(&self, against: Self) -> bool {
        self.min.x < against.max.x && self.max.x > against.min.x
    }

    pub const fn intersects_with_y(&self, against: Self) -> bool {
        self.min.y < against.max.y && self.max.y > against.min.y
    }

    pub const fn intersects_with_z(&self, against: Self) -> bool {
        self.min.z < against.max.z && self.max.z > against.min.z
    }

    pub const fn intersects_with_yz(&self, vec: DVec3) -> bool {
        vec.y >= self.min.y && vec.y <= self.max.y && vec.z >= self.min.z && vec.z <= self.max.z
    }

    pub const fn intersects_with_xz(&self, vec: DVec3) -> bool {
        vec.x >= self.min.x && vec.x <= self.max.x && vec.z >= self.min.z && vec.z <= self.max.z
    }

    pub const fn intersects_with_xy(&self, vec: DVec3) -> bool {
        vec.x >= self.min.x && vec.x <= self.max.x && vec.y >= self.min.y && vec.y <= self.max.y
    }

    fn collide_with_x_plane(&self, value: f64, a: DVec3, b: DVec3) -> Option<DVec3> {
        a.get_intermediate_with_x_value(b, value).filter(|vec3d| self.intersects_with_yz(*vec3d))
    }

    fn collide_with_y_plane(&self, value: f64, a: DVec3, b: DVec3) -> Option<DVec3> {
        a.get_intermediate_with_y_value(b, value).filter(|vec3d| self.intersects_with_xz(*vec3d))
    }

    fn collide_with_z_plane(&self, value: f64, a: DVec3, b: DVec3) -> Option<DVec3> {
        a.get_intermediate_with_z_value(b, value).filter(|vec3d| self.intersects_with_xy(*vec3d))
    }

    fn is_closest(a: DVec3, b: Option<DVec3>, c: DVec3) -> bool {
        b.is_none_or(|b| a.distance_squared(c) < a.distance_squared(b))
    }

    pub fn calculate_intercept(&self, vec_a: DVec3, vec_b: DVec3) -> Option<RayCastResult> {
        let mut a = self.collide_with_x_plane(self.min.x, vec_a, vec_b);
        let mut facing_at = Face::Left;
        let mut b = self.collide_with_x_plane(self.max.x, vec_a, vec_b);

        if b.is_some_and(|b| Self::is_closest(vec_a, a, b)) {
            a = b;

            facing_at = Face::Right;
        }

        b = self.collide_with_y_plane(self.min.y, vec_a, vec_b);

        if b.is_some_and(|b| Self::is_closest(vec_a, a, b)) {
            a = b;

            facing_at = Face::Bottom;
        }

        b = self.collide_with_y_plane(self.max.y, vec_a, vec_b);

        if b.is_some_and(|b| Self::is_closest(vec_a, a, b)) {
            a = b;

            facing_at = Face::Top;
        }

        b = self.collide_with_z_plane(self.min.z, vec_a, vec_b);

        if b.is_some_and(|b| Self::is_closest(vec_a, a, b)) {
            a = b;

            facing_at = Face::Front;
        }

        b = self.collide_with_z_plane(self.max.z, vec_a, vec_b);

        if b.is_some_and(|b| Self::is_closest(vec_a, a, b)) {
            a = b;

            facing_at = Face::Back;
        }

        a.map(|vec3d| RayCastResult::new2(vec3d, facing_at))
    }
}

impl From<Cube3D> for Aabb {
    fn from(value: Cube3D) -> Self {
        Self {
            min: value.origin.to_raw().as_dvec3(),
            max: (value.origin + value.size.to_vector()).to_raw().as_dvec3(),
        }
    }
}
