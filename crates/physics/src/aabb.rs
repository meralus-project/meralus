use mavelin_shared::{Cube, Face};

use crate::raycast::RayCastResult;

fn get_intermediate_with_x_value(a: glam::DVec3, b: glam::DVec3, x: f64) -> Option<glam::DVec3> {
    let diff = b - a;

    if diff.x * diff.x < 1.000_000_011_686_097_4E-7 {
        None
    } else {
        let f = (x - a.x) / diff.x;

        if (0.0..=1.0).contains(&f) {
            Some(glam::DVec3::new(diff.x.mul_add(f, a.x), diff.x.mul_add(f, a.y), diff.x.mul_add(f, a.z)))
        } else {
            None
        }
    }
}

fn get_intermediate_with_y_value(a: glam::DVec3, b: glam::DVec3, y: f64) -> Option<glam::DVec3> {
    let diff = b - a;

    if diff.y * diff.y < 1.000_000_011_686_097_4E-7 {
        None
    } else {
        let f = (y - a.y) / diff.y;

        if (0.0..=1.0).contains(&f) {
            Some(glam::DVec3::new(diff.y.mul_add(f, a.x), diff.y.mul_add(f, a.y), diff.y.mul_add(f, a.z)))
        } else {
            None
        }
    }
}

fn get_intermediate_with_z_value(a: glam::DVec3, b: glam::DVec3, z: f64) -> Option<glam::DVec3> {
    let diff = b - a;

    if diff.z * diff.z < 1.000_000_011_686_097_4E-7 {
        None
    } else {
        let f = (z - a.z) / diff.z;

        if (0.0..=1.0).contains(&f) {
            Some(glam::DVec3::new(diff.x.mul_add(f, a.x), diff.y.mul_add(f, a.y), diff.z.mul_add(f, a.z)))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: glam::DVec3,
    pub max: glam::DVec3,
}

impl Aabb {
    #[inline]
    pub const fn new(min: glam::DVec3, max: glam::DVec3) -> Self {
        Self { min, max }
    }

    #[inline]
    pub const fn cube(origin: glam::DVec3) -> Self {
        Self::new(origin, glam::DVec3::new(origin.x + 1.0, origin.y + 1.0, origin.z + 1.0))
    }

    #[inline]
    #[must_use]
    pub fn min_max(self, other: Self) -> Self {
        Self::new(self.min.min(other.min), self.max.max(other.max))
    }

    #[inline]
    pub fn size(&self) -> glam::DVec3 {
        self.max - self.min
    }

    #[must_use]
    #[inline]
    pub fn extended(mut self, point: glam::DVec3) -> Self {
        self.min += point;
        self.max += point;

        self
    }

    #[inline]
    pub fn contains(&self, pos: glam::DVec3) -> bool {
        !(pos.x < self.min.x || pos.y < self.min.y || pos.z < self.min.z || pos.x >= self.max.x || pos.y >= self.max.y || pos.z >= self.max.z)
    }

    #[inline]
    pub const fn get_center(&self, size: glam::DVec3) -> glam::DVec3 {
        glam::DVec3::new(self.min.x + size.x / 2.0, self.min.y + size.y / 2.0, self.min.z + size.z / 2.0)
    }

    #[inline]
    pub const fn intersects_with_x(&self, against: &Self) -> bool {
        self.min.x < against.max.x && self.max.x > against.min.x
    }

    #[inline]
    pub const fn intersects_with_y(&self, against: &Self) -> bool {
        self.min.y < against.max.y && self.max.y > against.min.y
    }

    #[inline]
    pub const fn intersects_with_z(&self, against: &Self) -> bool {
        self.min.z < against.max.z && self.max.z > against.min.z
    }

    #[inline]
    pub const fn intersects(&self, against: &Self) -> bool {
        self.intersects_with_x(against) && self.intersects_with_y(against) && self.intersects_with_z(against)
    }

    #[inline]
    pub const fn intersects_with_yz(&self, vec: glam::DVec3) -> bool {
        vec.y >= self.min.y && vec.y <= self.max.y && vec.z >= self.min.z && vec.z <= self.max.z
    }

    #[inline]
    pub const fn intersects_with_xz(&self, vec: glam::DVec3) -> bool {
        vec.x >= self.min.x && vec.x <= self.max.x && vec.z >= self.min.z && vec.z <= self.max.z
    }

    #[inline]
    pub const fn intersects_with_xy(&self, vec: glam::DVec3) -> bool {
        vec.x >= self.min.x && vec.x <= self.max.x && vec.y >= self.min.y && vec.y <= self.max.y
    }

    #[inline]
    fn collide_with_x_plane(&self, value: f64, a: glam::DVec3, b: glam::DVec3) -> Option<glam::DVec3> {
        get_intermediate_with_x_value(a, b, value).filter(|vec3d| self.intersects_with_yz(*vec3d))
    }

    #[inline]
    fn collide_with_y_plane(&self, value: f64, a: glam::DVec3, b: glam::DVec3) -> Option<glam::DVec3> {
        get_intermediate_with_y_value(a, b, value).filter(|vec3d| self.intersects_with_xz(*vec3d))
    }

    #[inline]
    fn collide_with_z_plane(&self, value: f64, a: glam::DVec3, b: glam::DVec3) -> Option<glam::DVec3> {
        get_intermediate_with_z_value(a, b, value).filter(|vec3d| self.intersects_with_xy(*vec3d))
    }

    #[inline]
    fn is_closest(a: glam::DVec3, b: Option<glam::DVec3>, c: glam::DVec3) -> bool {
        b.is_none_or(|b| a.distance_squared(c) < a.distance_squared(b))
    }

    pub fn calculate_intercept(&self, vec_a: glam::DVec3, vec_b: glam::DVec3) -> Option<RayCastResult> {
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

            facing_at = Face::Back;
        }

        b = self.collide_with_z_plane(self.max.z, vec_a, vec_b);

        if b.is_some_and(|b| Self::is_closest(vec_a, a, b)) {
            a = b;

            facing_at = Face::Front;
        }

        a.map(|vec3d| RayCastResult::new2(vec3d, facing_at))
    }
}

impl From<Cube> for Aabb {
    #[inline]
    fn from(value: Cube) -> Self {
        let half_size = value.size / 2.0;

        Self {
            min: (value.origin - half_size).as_dvec3(),
            max: (value.origin + half_size).as_dvec3(),
        }
    }
}
