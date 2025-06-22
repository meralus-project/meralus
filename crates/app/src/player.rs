use std::f32;

use glam::{DVec3, FloatExt, Vec2, Vec3, dvec3, vec3};
use meralus_engine::KeyCode;
use meralus_world::ChunkManager;

use crate::{
    Aabb, BakedBlockModelLoader, Camera, KeyboardController, get_movement_direction,
    get_rotation_directions,
    raycast::{HitType, RayCastResult},
    util::ChunkManagerPhysics,
};

pub struct PlayerController {
    pub position: Vec3,
    // START CAMERA
    pub yaw: f32,
    pub pitch: f32,
    pub front: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    // END CAMERA
    pub velocity: Vec3,
    pub is_on_ground: bool,
    pub looking_at: Option<RayCastResult>,
}

impl Default for PlayerController {
    fn default() -> Self {
        let yaw: f32 = 0.0;
        let pitch: f32 = 0.0;

        let front = vec3(
            yaw.cos() * pitch.cos(),
            pitch.sin(),
            yaw.sin() * pitch.cos(),
        )
        .normalize();

        let right = front.cross(Vec3::Y).normalize();
        let up = right.cross(front).normalize();

        Self {
            position: Vec3::Y,
            yaw,
            pitch,
            front,
            right,
            up,
            velocity: Vec3::ZERO,
            is_on_ground: false,
            looking_at: None,
        }
    }
}

impl PlayerController {
    pub const AFFECTED_BY_PHYSICS: bool = false;
    pub const GRAVITY: f32 = 9.81 * 1.5;
    pub const LOOK_SPEED: f32 = 0.1;
    pub const MOUSE_SENSE: f32 = 0.05;
    pub const MOVE_SPEED: f32 = 4.;

    pub fn get_vector_for_rotation(&self) -> DVec3 {
        let f = (self.yaw - f32::consts::PI).cos();
        let f1 = (self.yaw - f32::consts::PI).sin();
        let f2 = -(self.pitch).cos();
        let f3 = (self.pitch).sin();

        DVec3::new(f64::from(f1 * f2), f64::from(f3), f64::from(f * f2))
    }

    pub fn handle_physics(
        &mut self,
        chunk_manager: &ChunkManager,
        models: &BakedBlockModelLoader,
        keyboard: &KeyboardController,
        camera: &mut Camera,
        delta: f32,
    ) {
        let direction = get_movement_direction(keyboard);

        let (front, right, _) = get_rotation_directions(self.yaw, 0.0);

        let velocity = ((front * direction.z) + (right * direction.x))
            * if keyboard.is_key_pressed(KeyCode::ShiftLeft) && direction.z > 0.0 {
                camera.fov = camera.fov.lerp(65.0_f32.to_radians(), 0.15);

                Self::MOVE_SPEED * 1.5
            } else {
                camera.fov = camera.fov.lerp(55.0_f32.to_radians(), 0.15);

                Self::MOVE_SPEED
            };

        self.velocity.x = velocity.x;
        self.velocity.z = velocity.z;

        if !self.is_on_ground && Self::AFFECTED_BY_PHYSICS {
            self.velocity.y -= Self::GRAVITY * delta;
        }

        if self.is_on_ground && self.velocity.y <= 0.0 && Self::AFFECTED_BY_PHYSICS {
            self.velocity.y = 0.0;
        }

        if keyboard.is_key_pressed(KeyCode::Space) && self.is_on_ground && Self::AFFECTED_BY_PHYSICS
        {
            self.velocity.y = 5.0;
        } else if keyboard.is_key_pressed(KeyCode::Space) && !Self::AFFECTED_BY_PHYSICS {
            self.position.y += 0.5;
        }

        if keyboard.is_key_pressed(KeyCode::ControlLeft) && !Self::AFFECTED_BY_PHYSICS {
            self.position.y -= 0.5;
        }

        self.move_and_collide(chunk_manager, models, delta);
    }

    pub fn update_looking_at(
        &mut self,
        chunk_manager: &ChunkManager,
        models: &BakedBlockModelLoader,
    ) {
        let block_reach_distance = 20.0f32;

        let origin = self.position;
        let target = origin + (self.front * block_reach_distance);

        self.looking_at = chunk_manager
            .raycast(models, origin.into(), target.into(), true)
            .filter(|result| result.hit_type == HitType::Block);
    }

    pub fn move_and_collide(
        &mut self,
        chunk_manager: &ChunkManager,
        models: &BakedBlockModelLoader,
        delta: f32,
    ) {
        let mut remaining_movement = self.velocity.as_dvec3() * f64::from(delta);
        let mut actual_movement = [0.0; 3];

        for axis in 0..3 {
            if remaining_movement[axis] == 0.0 {
                continue;
            }

            let mut test_pos = self.position.as_dvec3();

            test_pos[axis] += remaining_movement[axis];

            let test_aabb = Aabb::new(
                test_pos - dvec3(0.5, 2.0, 0.5),
                test_pos + dvec3(0.5, 0.0, 0.5),
            );

            if chunk_manager.collides(test_aabb) {
                self.is_on_ground = chunk_manager
                    .get_colliders(test_pos, test_aabb)
                    .bottom
                    .is_some();

                // Try smaller steps for more precision
                let mut step = remaining_movement[axis].abs();
                let direction = remaining_movement[axis].signum();

                #[allow(clippy::while_float)]
                while step > 0.001 {
                    test_pos[axis] = direction.mul_add(step, self.position[axis].into());

                    let test_aabb = Aabb::new(
                        test_pos - dvec3(0.5, 2.0, 0.5),
                        test_pos + dvec3(0.5, 0.0, 0.5),
                    );

                    if !chunk_manager.collides(test_aabb) {
                        self.position[axis] = test_pos[axis] as f32;

                        actual_movement[axis] += direction * step;
                        remaining_movement[axis] -= direction * step;

                        break;
                    }

                    step /= 2.0;
                }
            } else {
                self.position[axis] = test_pos[axis] as f32;
                self.is_on_ground = false;

                actual_movement[axis] = remaining_movement[axis];
                remaining_movement[axis] = 0.0;
            }
        }

        self.update_looking_at(chunk_manager, models);
    }

    pub fn handle_mouse(
        &mut self,
        chunk_manager: &ChunkManager,
        models: &BakedBlockModelLoader,
        mouse_delta: Vec2,
    ) {
        self.yaw += mouse_delta.x * Self::MOUSE_SENSE * Self::LOOK_SPEED;
        self.pitch += mouse_delta.y * Self::MOUSE_SENSE * -Self::LOOK_SPEED;

        self.pitch = if self.pitch > 1.5 { 1.5 } else { self.pitch };
        self.pitch = if self.pitch < -1.5 { -1.5 } else { self.pitch };

        self.front = vec3(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        )
        .normalize();

        self.update_looking_at(chunk_manager, models);

        self.right = self.front.cross(Vec3::Y).normalize();
        self.up = self.right.cross(self.front).normalize();

        // println!("{}", Self::calc_order(self.front));
    }

    pub const fn calc_order(direction: Vec3) -> usize {
        let sx = (direction.x < 0.0) as usize;
        let sy = (direction.y < 0.0) as usize;
        let sz = (direction.z < 0.0) as usize;
        let ax = direction.x.abs();
        let ay = direction.y.abs();
        let az = direction.z.abs();

        if ax > ay && ax > az {
            if ay > az {
                (sx << 2) | (sy << 1) | sz
            } else {
                8 + ((sx << 2) | (sz << 1) | sy)
            }
        } else if ay > az {
            if ax > az {
                16 + ((sy << 2) | (sx << 1) | sz)
            } else {
                24 + ((sy << 2) | (sz << 1) | sx)
            }
        } else if ax > ay {
            32 + ((sz << 2) | (sx << 1) | sy)
        } else {
            40 + ((sz << 2) | (sy << 1) | sx)
        }
    }
}
