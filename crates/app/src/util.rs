use glam::{DVec3, Mat4, Vec2, Vec3, ivec3, vec2, vec3};
use meralus_engine::KeyCode;
use meralus_graphics::Line;
use meralus_shared::{Color, Cube3D};
use meralus_world::{ChunkManager, Face};

use crate::{
    Aabb, BakedBlockModelLoader, Camera, KeyboardController,
    loaders::BakedBlockModel,
    raycast::{HitType, RayCastResult},
    world::Colliders,
};

const AMBIENT_OCCLUSION_VALUES: [f32; 4] = [0.4, 0.55, 0.75, 1.0];

#[must_use]
pub fn get_movement_direction(keyboard: &KeyboardController) -> Vec3 {
    let mut direction = Vec3::ZERO;

    if keyboard.is_key_pressed(KeyCode::KeyW) {
        direction.z += 1.0;
    }

    if keyboard.is_key_pressed(KeyCode::KeyS) {
        direction.z -= 1.0;
    }

    if keyboard.is_key_pressed(KeyCode::KeyA) {
        direction.x -= 1.0;
    }

    if keyboard.is_key_pressed(KeyCode::KeyD) {
        direction.x += 1.0;
    }

    direction
}

#[must_use]
pub fn get_rotation_directions(yaw: f32, pitch: f32) -> (Vec3, Vec3, Vec3) {
    let front: Vec3 = vec3(yaw.cos() * pitch.cos(), pitch.sin(), yaw.sin() * pitch.cos()).normalize();

    let right = front.cross(Vec3::Y).normalize();

    (front, right, right.cross(front).normalize())
}

#[must_use]
#[allow(clippy::fn_params_excessive_bools)]
pub fn vertex_ao(side1: bool, side2: bool, corner: bool) -> f32 {
    AMBIENT_OCCLUSION_VALUES[if side1 && side2 {
        0
    } else {
        3 - (usize::from(side1) + usize::from(side2) + usize::from(corner))
    }]
}

pub trait AsColor {
    fn as_color(&self) -> Color;
}

impl AsColor for Face {
    fn as_color(&self) -> Color {
        match self {
            Self::Top => Color::RED,
            Self::Bottom => Color::GREEN,
            Self::Left => Color::BLUE,
            Self::Right => Color::YELLOW,
            Self::Front => Color::BROWN,
            Self::Back => Color::PURPLE,
        }
    }
}

impl AsColor for Vec3 {
    fn as_color(&self) -> Color {
        for (pos, vertice) in Face::VERTICES.iter().enumerate() {
            if self == vertice {
                return Color::from_hsl(pos as f32 / 8.0, 1.0, 0.5);
            }
        }

        Color::BLACK
    }
}

pub trait VecExt<T>: Sized {
    fn get_intermediate_with_x_value(&self, vec: Self, x: T) -> Option<Self>;
    fn get_intermediate_with_y_value(&self, vec: Self, y: T) -> Option<Self>;
    fn get_intermediate_with_z_value(&self, vec: Self, z: T) -> Option<Self>;
}

impl VecExt<f64> for DVec3 {
    fn get_intermediate_with_x_value(&self, vec: Self, x: f64) -> Option<Self> {
        let d0 = vec.x - self.x;
        let d1 = vec.y - self.y;
        let d2 = vec.z - self.z;

        if d0 * d0 < 0.0000001 {
            None
        } else {
            let d3 = (x - self.x) / d0;

            if (0.0..=1.0).contains(&d3) {
                Some(Self::new(d0.mul_add(d3, self.x), d1.mul_add(d3, self.y), d2.mul_add(d3, self.z)))
            } else {
                None
            }
        }
    }

    fn get_intermediate_with_y_value(&self, vec: Self, y: f64) -> Option<Self> {
        let d0 = vec.x - self.x;
        let d1 = vec.y - self.y;
        let d2 = vec.z - self.z;

        if d1 * d1 < 1.0000000116860974E-7 {
            None
        } else {
            let d3 = (y - self.y) / d1;

            if (0.0..=1.0).contains(&d3) {
                Some(Self::new(d0.mul_add(d3, self.x), d1.mul_add(d3, self.y), d2.mul_add(d3, self.z)))
            } else {
                None
            }
        }
    }

    fn get_intermediate_with_z_value(&self, vec: Self, z: f64) -> Option<Self> {
        let d0 = vec.x - self.x;
        let d1 = vec.y - self.y;
        let d2 = vec.z - self.z;

        if d2 * d2 < 1.0000000116860974E-7 {
            None
        } else {
            let d3 = (z - self.x) / d2;

            if (0.0..=1.0).contains(&d3) {
                Some(Self::new(d0.mul_add(d3, self.x), d1.mul_add(d3, self.y), d2.mul_add(d3, self.z)))
            } else {
                None
            }
        }
    }
}

pub const SIZE_CAP: f32 = 960.0;

pub fn format_bytes(bytes: usize) -> String {
    let mut value = bytes as f32;

    for suffix in ["B", "kB", "MB"] {
        if value > SIZE_CAP {
            value /= 1024.0;
        } else {
            return format!("{value:.2}{suffix}");
        }
    }

    format!("{value:.2}GB")
}

pub fn cube_outline(Cube3D { origin, size }: Cube3D) -> [Line; 12] {
    [
        [[0.0, 0.0, 0.0], [0.0, size.height, 0.0]],
        [[size.width, 0.0, 0.0], [size.width, size.height, 0.0]],
        [[0.0, 0.0, size.depth], [0.0, size.height, size.depth]],
        [[size.width, 0.0, size.depth], [size.width, size.height, size.depth]],
        [[0.0, 0.0, 0.0], [size.width, 0.0, 0.0]],
        [[0.0, 0.0, 0.0], [0.0, 0.0, size.depth]],
        [[size.width, 0.0, 0.0], [size.width, 0.0, size.depth]],
        [[0.0, 0.0, size.depth], [size.width, 0.0, size.depth]],
        [[0.0, size.height, 0.0], [size.width, size.height, 0.0]],
        [[0.0, size.height, 0.0], [0.0, size.height, size.depth]],
        [[size.width, size.height, 0.0], [size.width, size.height, size.depth]],
        [[0.0, size.height, size.depth], [size.width, size.height, size.depth]],
    ]
    .map(|[start, end]| Line::new(origin.to_raw() + Vec3::from_array(start), origin.to_raw() + Vec3::from_array(end), Color::BLUE))
}

pub trait ChunkManagerPhysics {
    fn collides(&self, aabb: Aabb) -> bool;
    fn get_colliders(&self, collider_position: DVec3, aabb: Aabb) -> Colliders;
    fn raycast(&self, models: &BakedBlockModelLoader, origin: DVec3, target: DVec3, last_uncollidable_block: bool) -> Option<RayCastResult>;

    fn get_model_for<'a>(&self, models: &'a BakedBlockModelLoader, position: Vec3) -> Option<&'a BakedBlockModel>;
}

fn raycast_into(position: Vec3, start: DVec3, end: DVec3, aabb: Aabb) -> Option<RayCastResult> {
    aabb.calculate_intercept(start - position.as_dvec3(), end - position.as_dvec3())
        .map(|raytraceresult| RayCastResult::new3(raytraceresult.hit_vec + position.as_dvec3(), raytraceresult.hit_side, position))
}

impl ChunkManagerPhysics for ChunkManager {
    fn collides(&self, aabb: Aabb) -> bool {
        let min = aabb.min.floor().as_ivec3().to_array();
        let max = aabb.max.ceil().as_ivec3().to_array();

        for y in min[1]..max[1] {
            for z in min[2]..max[2] {
                for x in min[0]..max[0] {
                    let position = ivec3(x, y, z).as_dvec3();

                    if self.contains_block(position.as_vec3()) {
                        let block = Aabb::new(position, position + DVec3::ONE);

                        if aabb.intersects_with_x(block) && aabb.intersects_with_y(block) && aabb.intersects_with_z(block) {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    fn get_colliders(&self, collider_position: DVec3, aabb: Aabb) -> Colliders {
        let min = aabb.min.floor().as_ivec3().to_array();
        let max = aabb.max.ceil().as_ivec3().to_array();

        let mut colliders = Colliders::default();

        for y in min[1]..max[1] {
            for z in min[2]..max[2] {
                for x in min[0]..max[0] {
                    let position = ivec3(x, y, z).as_dvec3();

                    if self.contains_block(position.as_vec3()) {
                        let block = Aabb::new(position, position + DVec3::ONE);

                        if aabb.intersects_with_x(block) && aabb.intersects_with_y(block) && aabb.intersects_with_z(block) {
                            let colliding_position = position - collider_position.floor();

                            if colliding_position.x < 0.0 {
                                colliders.left = Some(position);
                            } else if colliding_position.x > 0.0 {
                                colliders.right = Some(position);
                            } else if colliding_position.y < 0.0 {
                                colliders.bottom = Some(position);
                            } else if colliding_position.y > 0.0 {
                                colliders.top = Some(position);
                            } else if colliding_position.z < 0.0 {
                                colliders.back = Some(position);
                            } else if colliding_position.z > 0.0 {
                                colliders.front = Some(position);
                            }
                        }
                    }
                }
            }
        }

        colliders
    }

    fn raycast(&self, models: &BakedBlockModelLoader, mut origin: DVec3, target: DVec3, last_uncollidable_block: bool) -> Option<RayCastResult> {
        if origin.is_nan() || target.is_nan() {
            return None;
        }

        let mut start = origin.floor();
        let end = target.floor();

        let mut position = start.as_vec3();
        let block = self.get_model_for(models, position);

        if let Some(block) = block {
            let result = raycast_into(position, origin, target, Aabb::from(block.bounding_box));

            if result.is_some() {
                return result;
            }
        }

        let mut result: Option<RayCastResult> = None;

        for _ in 0..200 {
            if origin.is_nan() {
                return None;
            }

            if (start.x - end.x).abs() < 0.0001 && (start.y - end.y).abs() < 0.0001 && (start.z - end.z).abs() < 0.0001 {
                return if last_uncollidable_block { result } else { None };
            }

            let mut modify_d3 = true;
            let mut modify_d4 = true;
            let mut modify_d5 = true;

            let mut d0 = 999f64;
            let mut d1 = 999f64;
            let mut d2 = 999f64;

            if end.x > start.x {
                d0 = start.x + 1.0;
            } else if end.x < start.x {
                d0 = start.x + 0.0;
            } else {
                modify_d3 = false;
            }

            if end.y > start.y {
                d1 = start.y + 1.0;
            } else if end.y < start.y {
                d1 = start.y + 0.0;
            } else {
                modify_d4 = false;
            }

            if end.z > start.z {
                d2 = start.z + 1.0;
            } else if end.z < start.z {
                d2 = start.z + 0.0;
            } else {
                modify_d5 = false;
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
                d3 = -0.0001;
            }

            if d4 == -0.0 {
                d4 = -0.0001;
            }

            if d5 == -0.0 {
                d5 = -0.0001;
            }

            let facing_at = if d3 < d4 && d3 < d5 {
                origin = DVec3::new(d0, d7.mul_add(d3, origin.y), d8.mul_add(d3, origin.z));

                if end.x > start.x { Face::Left } else { Face::Right }
            } else if d4 < d5 {
                origin = DVec3::new(d6.mul_add(d4, origin.x), d1, d8.mul_add(d4, origin.z));

                if end.y > start.y { Face::Bottom } else { Face::Top }
            } else {
                origin = DVec3::new(d6.mul_add(d5, origin.x), d7.mul_add(d5, origin.y), d2);

                if end.z > start.z { Face::Front } else { Face::Back }
            };

            start = origin.floor()
                - match facing_at {
                    Face::Right => DVec3::X,
                    Face::Top => DVec3::Y,
                    Face::Back => DVec3::Z,
                    Face::Bottom | Face::Left | Face::Front => DVec3::ZERO,
                };

            position = start.as_vec3();

            let block = self.get_model_for(models, position);

            if let Some(block) = block
                && let Some(result) = raycast_into(position, origin, target, Aabb::from(block.bounding_box))
            {
                return Some(result);
            }

            result.replace(RayCastResult::new(HitType::None, origin, facing_at, position));
        }

        if last_uncollidable_block { result } else { None }
    }

    fn get_model_for<'a>(&self, models: &'a BakedBlockModelLoader, position: Vec3) -> Option<&'a BakedBlockModel> {
        self.get_block(position).and_then(|block| models.get(block.into()))
    }
}

pub trait MatrixExt<T> {
    fn translate(self, position: T) -> Self;
    fn scale(self, value: T) -> Self;
    fn rotate_x(self, angle: f32) -> Self;
    fn rotate_y(self, angle: f32) -> Self;
    fn rotate_z(self, angle: f32) -> Self;
}

impl MatrixExt<Vec3> for Mat4 {
    fn translate(self, position: Vec3) -> Self {
        self * Self::from_translation(position)
    }

    fn scale(self, value: Vec3) -> Self {
        self * Self::from_scale(value)
    }

    fn rotate_x(self, angle: f32) -> Self {
        self * Self::from_rotation_x(angle)
    }

    fn rotate_y(self, angle: f32) -> Self {
        self * Self::from_rotation_y(angle)
    }

    fn rotate_z(self, angle: f32) -> Self {
        self * Self::from_rotation_z(angle)
    }
}
