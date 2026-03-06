use meralus_shared::{Point3D, Size3D, Vector3D};

use crate::{Aabb, AabbSource, PhysicsBody};

pub struct PhysicsContext<T: AabbSource> {
    pub(crate) source: T,
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
impl<T: AabbSource> PhysicsContext<T> {
    const E: f32 = 0.03;
    const GRAVITY: Vector3D = Vector3D::new(0.0, -9.81 * 1.75, 0.0);
    const MAX_FIX: f32 = 0.01;
    const S: f32 = 2.0 / 16.0;

    pub const fn new(source: T) -> Self {
        Self { source }
    }

    fn is_obstacle(&self, position: Point3D) -> Option<Aabb> {
        self.source.get_aabb(position)
    }

    fn calc_collision_neg<const NX: usize, const NY: usize, const NZ: usize>(&self, body: &mut PhysicsBody, half: Size3D, step_height: f32) -> bool {
        if body.velocity[NX] >= 0.0 {
            return false;
        }

        let offset = Vector3D::new(0.0, step_height, 0.0);
        let max_y = (((half.to_vector() - offset * 0.5)[NY] - Self::E) * 2.0 / Self::S) as i32;

        for iy in 0..=max_y {
            let mut coord = Point3D::ZERO;

            coord[NY] = (iy as f32).mul_add(Self::S, (body.position + offset)[NY] - half[NY] + Self::E);

            let max_z = ((half[NZ] - Self::E) * 2.0 / Self::S) as i32;

            for iz in 0..=max_z {
                coord[NZ] = (iz as f32).mul_add(Self::S, body.position[NZ] - half[NZ] + Self::E);
                coord[NX] = body.position[NX] - half[NX] - Self::E;

                if let Some(aabb) = self.is_obstacle(coord) {
                    body.velocity[NX] = 0.0;

                    let newx = coord[NX].floor() + half[NX] + aabb.max[NX] as f32 - Self::E;

                    if (newx - body.position[NX]).abs() <= Self::MAX_FIX {
                        body.position[NX] = newx;
                    }

                    return true;
                }
            }
        }

        false
    }

    fn calc_collision_pos<const NX: usize, const NY: usize, const NZ: usize>(&self, body: &mut PhysicsBody, half: Size3D, step_height: f32) {
        if body.velocity[NX] <= 0.0 {
            return;
        }

        let offset = Vector3D::new(0.0, step_height, 0.0);
        let max_y = (((half.to_vector() - offset * 0.5)[NY] - Self::E) * 2.0 / Self::S) as i32;

        for iy in 0..=max_y {
            let mut coord = Point3D::ZERO;

            coord[NY] = (iy as f32).mul_add(Self::S, (body.position + offset)[NY] - half[NY] + Self::E);

            let max_z = ((half[NZ] - Self::E) * 2.0 / Self::S) as i32;

            for iz in 0..=max_z {
                coord[NZ] = (iz as f32).mul_add(Self::S, body.position[NZ] - half[NZ] + Self::E);
                coord[NX] = body.position[NX] + half[NX] + Self::E;

                if let Some(aabb) = self.is_obstacle(coord) {
                    body.velocity[NX] = 0.0;

                    let newx = coord[NX].floor() - half[NX] + aabb.min[NX] as f32 - Self::E;

                    if (newx - body.position[NX]).abs() <= Self::MAX_FIX {
                        body.position[NX] = newx;
                    }

                    return;
                }
            }
        }
    }

    fn calc_step_height(&self, pos: Point3D, half: Size3D, step_height: f32) -> f32 {
        if step_height > 0.0 {
            for ix in 0..=((half.width - Self::E) * 2.0 / Self::S) as i32 {
                let x = (ix as f32).mul_add(Self::S, pos.x - half.width + Self::E);

                for iz in 0..=((half.depth - Self::E) * 2.0 / Self::S) as i32 {
                    let z = (iz as f32).mul_add(Self::S, pos.z - half.depth + Self::E);

                    if self.is_obstacle(Point3D::new(x, pos.y + half.height + step_height, z)).is_some() {
                        return 0.0;
                    }
                }
            }
        }

        step_height
    }

    fn collision_calc(&self, body: &mut PhysicsBody, half: Size3D, step_height: f32) {
        let step_height = self.calc_step_height(body.position, half, step_height);

        self.calc_collision_neg::<0, 1, 2>(body, half, step_height);
        self.calc_collision_pos::<0, 1, 2>(body, half, step_height);
        self.calc_collision_neg::<2, 1, 0>(body, half, step_height);
        self.calc_collision_pos::<2, 1, 0>(body, half, step_height);

        if self.calc_collision_neg::<1, 0, 2>(body, half, step_height) {
            body.is_on_ground = true;
        }

        if step_height > 0.0 && body.velocity.y <= 0.0 {
            for ix in 0..=((half.width - Self::E) * 2.0 / Self::S) as i32 {
                let x = (ix as f32).mul_add(Self::S, body.position.x - half.width + Self::E);

                for iz in 0..=((half.depth - Self::E) * 2.0 / Self::S) as i32 {
                    let z = (iz as f32).mul_add(Self::S, body.position.z - half.depth + Self::E);
                    let y = body.position.y - half.height + Self::E;

                    if let Some(aabb) = self.is_obstacle(Point3D::new(x, y, z)) {
                        body.velocity.y = 0.0;

                        let newy = y.floor() + aabb.max.y as f32 + half.height;

                        if (newy - body.position.y).abs() <= Self::MAX_FIX + step_height {
                            body.position.y = newy;
                        }

                        break;
                    }
                }
            }
        }

        if body.velocity.y > 0.0 {
            for ix in 0..=((half.width - Self::E) * 2.0 / Self::S) as i32 {
                let x = (ix as f32).mul_add(Self::S, body.position.x - half.width + Self::E);

                for iz in 0..=((half.depth - Self::E) * 2.0 / Self::S) as i32 {
                    let z = (iz as f32).mul_add(Self::S, body.position.z - half.depth + Self::E);
                    let y = body.position.y + half.height + Self::E;

                    if let Some(aabb) = self.is_obstacle(Point3D::new(x, y, z)) {
                        body.velocity.y = 0.0;

                        let newy = y.floor() - half.height + aabb.min.y as f32 - Self::E;

                        if (newy - body.position.y).abs() <= Self::MAX_FIX {
                            body.position.y = newy;
                        }

                        break;
                    }
                }
            }
        }
    }

    pub fn physics_step(&self, body: &mut PhysicsBody, delta: f32) {
        let substeps = ((delta * body.velocity.length() * 20.0) as i32).clamp(2, 100);

        if body.position.y <= -48.0 {
            body.position.y = 300.0;
        }

        let dt = delta / substeps as f32;
        let linear_damping = body.config.linear_damping * body.config.friction;

        let half = body.size * 0.5;
        let prev_grounded = body.is_on_ground;

        body.is_on_ground = false;

        for _ in 0..substeps {
            // let px = pos.x;
            let py = body.position.y;
            // let pz = pos.z;

            body.velocity += Self::GRAVITY * dt * body.config.gravity_scale;

            self.collision_calc(body, half, if prev_grounded && body.config.gravity_scale > 0.0 { 0.5 } else { 0.0 });

            body.position += body.velocity * dt * 1.25 + Self::GRAVITY * body.config.gravity_scale * dt * dt * 0.5;

            if body.is_on_ground && body.position.y < py {
                body.position.y = py;
            }
        }

        body.velocity.x /= delta.mul_add(linear_damping, 1.0);
        body.velocity.z /= delta.mul_add(linear_damping, 1.0);

        if body.config.vertical_damping > 0.0 {
            body.velocity.y /= (delta * linear_damping).mul_add(body.config.vertical_damping, 1.0);
        }
    }
}
