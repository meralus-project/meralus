use meralus_shared::{Cube3D, DPoint3D, DSize3D};
use meralus_world::Face;

use crate::raycast::RayCastResult;

fn get_intermediate_with_x_value(a: DPoint3D, b: DPoint3D, x: f64) -> Option<DPoint3D> {
    let d0 = b.x - a.x;
    let d1 = b.y - a.y;
    let d2 = b.z - a.z;

    if d0 * d0 < 1.000_000_011_686_097_4E-7 {
        None
    } else {
        let d3 = (x - a.x) / d0;

        if (0.0..=1.0).contains(&d3) {
            Some(DPoint3D::new(d0.mul_add(d3, a.x), d1.mul_add(d3, a.y), d2.mul_add(d3, a.z)))
        } else {
            None
        }
    }
}

fn get_intermediate_with_y_value(a: DPoint3D, b: DPoint3D, y: f64) -> Option<DPoint3D> {
    let d0 = b.x - a.x;
    let d1 = b.y - a.y;
    let d2 = b.z - a.z;

    if d1 * d1 < 1.000_000_011_686_097_4E-7 {
        None
    } else {
        let d3 = (y - a.y) / d1;

        if (0.0..=1.0).contains(&d3) {
            Some(DPoint3D::new(d0.mul_add(d3, a.x), d1.mul_add(d3, a.y), d2.mul_add(d3, a.z)))
        } else {
            None
        }
    }
}

fn get_intermediate_with_z_value(a: DPoint3D, b: DPoint3D, z: f64) -> Option<DPoint3D> {
    let d0 = b.x - a.x;
    let d1 = b.y - a.y;
    let d2 = b.z - a.z;

    if d2 * d2 < 1.000_000_011_686_097_4E-7 {
        None
    } else {
        let d3 = (z - a.x) / d2;

        if (0.0..=1.0).contains(&d3) {
            Some(DPoint3D::new(d0.mul_add(d3, a.x), d1.mul_add(d3, a.y), d2.mul_add(d3, a.z)))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: DPoint3D,
    pub max: DPoint3D,
}

impl Aabb {
    pub const fn new(min: DPoint3D, max: DPoint3D) -> Self {
        Self { min, max }
    }

    pub fn size(&self) -> DSize3D {
        (self.max - self.min).to_size()
    }

    #[must_use]
    pub fn extended(mut self, point: DPoint3D) -> Self {
        self.min += point;
        self.max += point;

        self
    }

    pub fn contains(&self, pos: DPoint3D) -> bool {
        !(pos.x < self.min.x || pos.y < self.min.y || pos.z < self.min.z || pos.x >= self.max.x || pos.y >= self.max.y || pos.z >= self.max.z)
    }

    pub const fn get_center(&self, size: DPoint3D) -> DPoint3D {
        DPoint3D::new(self.min.x + size.x / 2.0, self.min.y + size.y / 2.0, self.min.z + size.z / 2.0)
    }

    pub const fn intersects_with_x(&self, against: &Self) -> bool {
        self.min.x < against.max.x && self.max.x > against.min.x
    }

    pub const fn intersects_with_y(&self, against: &Self) -> bool {
        self.min.y < against.max.y && self.max.y > against.min.y
    }

    pub const fn intersects_with_z(&self, against: &Self) -> bool {
        self.min.z < against.max.z && self.max.z > against.min.z
    }

    pub const fn intersects(&self, against: &Self) -> bool {
        self.intersects_with_x(against) && self.intersects_with_y(against) && self.intersects_with_z(against)
    }

    pub const fn intersects_with_yz(&self, vec: DPoint3D) -> bool {
        vec.y >= self.min.y && vec.y <= self.max.y && vec.z >= self.min.z && vec.z <= self.max.z
    }

    pub const fn intersects_with_xz(&self, vec: DPoint3D) -> bool {
        vec.x >= self.min.x && vec.x <= self.max.x && vec.z >= self.min.z && vec.z <= self.max.z
    }

    pub const fn intersects_with_xy(&self, vec: DPoint3D) -> bool {
        vec.x >= self.min.x && vec.x <= self.max.x && vec.y >= self.min.y && vec.y <= self.max.y
    }

    fn collide_with_x_plane(&self, value: f64, a: DPoint3D, b: DPoint3D) -> Option<DPoint3D> {
        get_intermediate_with_x_value(a, b, value).filter(|vec3d| self.intersects_with_yz(*vec3d))
    }

    fn collide_with_y_plane(&self, value: f64, a: DPoint3D, b: DPoint3D) -> Option<DPoint3D> {
        get_intermediate_with_y_value(a, b, value).filter(|vec3d| self.intersects_with_xz(*vec3d))
    }

    fn collide_with_z_plane(&self, value: f64, a: DPoint3D, b: DPoint3D) -> Option<DPoint3D> {
        get_intermediate_with_z_value(a, b, value).filter(|vec3d| self.intersects_with_xy(*vec3d))
    }

    fn is_closest(a: DPoint3D, b: Option<DPoint3D>, c: DPoint3D) -> bool {
        b.is_none_or(|b| a.distance_squared(c) < a.distance_squared(b))
    }

    pub fn calculate_intercept(&self, vec_a: DPoint3D, vec_b: DPoint3D) -> Option<RayCastResult> {
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
        let half_size = value.size.to_vector() / 2.0;

        Self {
            min: (value.origin - half_size).as_::<f64>(),
            max: (value.origin + half_size).as_::<f64>(),
        }
    }
}
