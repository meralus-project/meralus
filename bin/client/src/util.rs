use meralus_physics::Aabb;
use meralus_shared::{AsValue, Color, Cube3D, Face, Point2D, Point3D, Vector3D, Vector4D};

use crate::{input::Input, render::common::CommonVertex};

const AMBIENT_OCCLUSION_VALUES: [f32; 4] = [0.55, 0.65, 0.8, 1.0];

#[must_use]
pub fn get_movement_direction(binds: &Input) -> Point3D {
    let mut direction = Point3D::ZERO;

    if binds.is_pressed("walk.forward") {
        direction.z += 1.0;
    }

    if binds.is_pressed("walk.backward") {
        direction.z -= 1.0;
    }

    if binds.is_pressed("walk.left") {
        direction.x -= 1.0;
    }

    if binds.is_pressed("walk.right") {
        direction.x += 1.0;
    }

    direction
}

#[must_use]
pub fn get_rotation_directions(yaw: f32, pitch: f32) -> (Vector3D, Vector3D, Vector3D) {
    let front = Vector3D::new(yaw.cos() * pitch.cos(), pitch.sin(), yaw.sin() * pitch.cos()).normalize();
    let right = front.cross(Vector3D::Y).normalize();

    (front, right, right.cross(front).normalize())
}

#[must_use]
#[allow(clippy::fn_params_excessive_bools)]
pub const fn vertex_ao(side1: bool, side2: bool, corner: bool) -> f32 {
    AMBIENT_OCCLUSION_VALUES[if side1 && side2 {
        0
    } else {
        3 - (side1 as usize + side2 as usize + corner as usize)
    }]
}

#[allow(dead_code)]
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

#[allow(dead_code)]
pub fn cube_outline(Cube3D { origin, size }: Cube3D, white_pixel_uv: Point2D) -> Vec<CommonVertex> {
    [
        [[0.0, 0.0, 0.0], [0.0, size.y, 0.0]],
        [[size.x, 0.0, 0.0], [size.x, size.y, 0.0]],
        [[0.0, 0.0, size.z], [0.0, size.y, size.z]],
        [[size.x, 0.0, size.z], [size.x, size.y, size.z]],
        [[0.0, 0.0, 0.0], [size.x, 0.0, 0.0]],
        [[0.0, 0.0, 0.0], [0.0, 0.0, size.z]],
        [[size.x, 0.0, 0.0], [size.x, 0.0, size.z]],
        [[0.0, 0.0, size.z], [size.x, 0.0, size.z]],
        [[0.0, size.y, 0.0], [size.x, size.y, 0.0]],
        [[0.0, size.y, 0.0], [0.0, size.y, size.z]],
        [[size.x, size.y, 0.0], [size.x, size.y, size.z]],
        [[0.0, size.y, size.z], [size.x, size.y, size.z]],
    ]
    .into_iter()
    .fold(Vec::new(), |mut vertices, [start, end]| {
        vertices.extend([
            CommonVertex {
                position: origin + Point3D::from_array(start),
                color: Color::BLUE.as_value(),
                uv: white_pixel_uv,
                clip: Vector4D::new(0.0, 0.0, 1.0, 1.0),
                _pad: [0; 8],
            },
            CommonVertex {
                position: origin + Point3D::from_array(end),
                color: Color::BLUE.as_value(),
                uv: white_pixel_uv,
                clip: Vector4D::new(0.0, 0.0, 1.0, 1.0),
                _pad: [0; 8],
            },
        ]);

        vertices
    })
}

#[allow(dead_code)]
pub fn aabb_outline(Aabb { min, max }: Aabb, white_pixel_uv: Point2D) -> Vec<CommonVertex> {
    let size = (max - min).as_vec3();

    [
        [[0.0, 0.0, 0.0], [0.0, size.y, 0.0]],
        [[size.x, 0.0, 0.0], [size.x, size.y, 0.0]],
        [[0.0, 0.0, size.z], [0.0, size.y, size.z]],
        [[size.x, 0.0, size.z], [size.x, size.y, size.z]],
        [[0.0, 0.0, 0.0], [size.x, 0.0, 0.0]],
        [[0.0, 0.0, 0.0], [0.0, 0.0, size.z]],
        [[size.x, 0.0, 0.0], [size.x, 0.0, size.z]],
        [[0.0, 0.0, size.z], [size.x, 0.0, size.z]],
        [[0.0, size.y, 0.0], [size.x, size.y, 0.0]],
        [[0.0, size.y, 0.0], [0.0, size.y, size.z]],
        [[size.x, size.y, 0.0], [size.x, size.y, size.z]],
        [[0.0, size.y, size.z], [size.x, size.y, size.z]],
    ]
    .into_iter()
    .fold(Vec::new(), |mut vertices, [start, end]| {
        vertices.extend([
            CommonVertex {
                position: min.as_vec3() + Point3D::from_array(start),
                color: Color::BLUE.as_value(),
                uv: white_pixel_uv,
                clip: Vector4D::new(0.0, 0.0, 1.0, 1.0),
                _pad: [0; 8],
            },
            CommonVertex {
                position: min.as_vec3() + Point3D::from_array(end),
                color: Color::BLUE.as_value(),
                uv: white_pixel_uv,
                clip: Vector4D::new(0.0, 0.0, 1.0, 1.0),
                _pad: [0; 8],
            },
        ]);

        vertices
    })
}
