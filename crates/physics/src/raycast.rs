use std::cmp::Ordering;

use meralus_shared::{DPoint3D, DVector3D, IPoint3D};
use meralus_world::Face;

use crate::{Aabb, AabbSource, PhysicsContext};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayCastResult {
    pub position: IPoint3D,
    pub hit_type: HitType,
    pub hit_side: Face,
    pub hit_vec: DPoint3D,
}

impl RayCastResult {
    pub const fn new(hit_type: HitType, hit_vec: DPoint3D, hit_side: Face, position: IPoint3D) -> Self {
        Self {
            position,
            hit_type,
            hit_side,
            hit_vec,
        }
    }

    pub const fn new2(hit_vec: DPoint3D, hit_side: Face) -> Self {
        Self::new(HitType::Block, hit_vec, hit_side, IPoint3D::ZERO)
    }

    pub const fn new3(hit_vec: DPoint3D, hit_side: Face, position: IPoint3D) -> Self {
        Self::new(HitType::Block, hit_vec, hit_side, position)
    }

    pub const fn is_block(&self) -> bool {
        matches!(self.hit_type, HitType::Block)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HitType {
    None,
    Block,
}

fn raycast_into(position: IPoint3D, start: DPoint3D, end: DPoint3D, aabb: Aabb) -> Option<RayCastResult> {
    aabb.calculate_intercept(start - position.to_vector().as_(), end - position.to_vector().as_())
        .map(|raytraceresult| RayCastResult::new3(raytraceresult.hit_vec + position.as_(), raytraceresult.hit_side, position))
}

impl<T: AabbSource> PhysicsContext<T> {
    pub fn raycast(&self, mut origin: DPoint3D, target: DPoint3D, last_uncollidable_block: bool) -> Option<RayCastResult> {
        if origin.is_nan() || target.is_nan() {
            return None;
        }

        let mut start_dvec3 = origin;
        let mut start = origin.floor().as_::<i32>();
        let end = target.floor().as_::<i32>();

        let mut position = start;

        if let Some(result) = self.get_block_aabb(position).and_then(|block| raycast_into(position, origin, target, block)) {
            return Some(result);
        }

        let mut result: Option<RayCastResult> = None;

        for _ in 0..200 {
            if origin.is_nan() {
                return None;
            }

            if start.x == end.x && start.y == end.y && start.z == end.z {
                // println!("return if {last_uncollidable_block} {{ {result:?} }} else {{ None
                // }}");

                return if last_uncollidable_block { result } else { None };
            }

            let mut modify_d3 = true;
            let mut modify_d4 = true;
            let mut modify_d5 = true;

            let mut d0 = 999f64;
            let mut d1 = 999f64;
            let mut d2 = 999f64;

            match end.x.cmp(&start.x) {
                Ordering::Greater => d0 = f64::from(start.x) + 1.0,
                Ordering::Less => d0 = f64::from(start.x) + 0.0,
                Ordering::Equal => modify_d3 = false,
            }

            match end.y.cmp(&start.y) {
                Ordering::Greater => d1 = f64::from(start.y) + 1.0,
                Ordering::Less => d1 = f64::from(start.y) + 0.0,
                Ordering::Equal => modify_d4 = false,
            }

            match end.z.cmp(&start.z) {
                Ordering::Greater => d2 = f64::from(start.z) + 1.0,
                Ordering::Less => d2 = f64::from(start.z) + 0.0,
                Ordering::Equal => modify_d5 = false,
            }

            let mut d3 = 999f64;
            let mut d4 = 999f64;
            let mut d5 = 999f64;

            let d6 = target.x - origin.x;
            let d7 = target.y - origin.y;
            let d8 = target.z - origin.z;

            if modify_d3 {
                d3 = (d0 - origin.x) / d6;
            }

            if modify_d4 {
                d4 = (d1 - origin.y) / d7;
            }

            if modify_d5 {
                d5 = (d2 - origin.z) / d8;
            }

            if d3 == -0.0 {
                d3 = -1.0E-4;
            }

            if d4 == -0.0 {
                d4 = -1.0E-4;
            }

            if d5 == -0.0 {
                d5 = -1.0E-4;
            }

            let facing_at = if d3 < d4 && d3 < d5 {
                origin = DPoint3D::new(d0, d7.mul_add(d3, origin.y), d8.mul_add(d3, origin.z));

                if target.x > start_dvec3.x { Face::Left } else { Face::Right }
            } else if d4 < d5 {
                origin = DPoint3D::new(d6.mul_add(d4, origin.x), d1, d8.mul_add(d4, origin.z));

                if target.y > start_dvec3.y { Face::Bottom } else { Face::Top }
            } else {
                origin = DPoint3D::new(d6.mul_add(d5, origin.x), d7.mul_add(d5, origin.y), d2);

                if target.z > start_dvec3.z { Face::Front } else { Face::Back }
            };

            start_dvec3 = origin.floor()
                - match facing_at {
                    Face::Right => DVector3D::X,
                    Face::Top => DVector3D::Y,
                    Face::Back => DVector3D::Z,
                    Face::Bottom | Face::Left | Face::Front => DVector3D::ZERO,
                };

            start = start_dvec3.as_();

            position = start;

            if let Some(result) = self.get_block_aabb(position).and_then(|block| raycast_into(position, origin, target, block)) {
                return Some(result);
            }

            result.replace(RayCastResult::new(HitType::None, origin, facing_at, position));
        }

        if last_uncollidable_block { result } else { None }
    }

    fn get_block_aabb(&self, position: IPoint3D) -> Option<Aabb> {
        self.source.get_block_aabb(position)
    }
}
