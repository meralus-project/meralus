use meralus_graphics::CommonVertex;
use meralus_physics::Aabb;
use meralus_shared::{Color, Cube3D, Point2D, Point3D, Vector3D};
use meralus_world::Face;

use crate::input::Input;

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

impl AsColor for Point3D {
    fn as_color(&self) -> Color {
        for (pos, vertice) in Face::VERTICES.iter().enumerate() {
            if self == vertice {
                return Color::from_hsl(pos as f32 / 8.0, 1.0, 0.5);
            }
        }

        Color::BLACK
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

pub fn cube_outline(Cube3D { origin, size }: Cube3D, white_pixel_uv: Point2D) -> Vec<CommonVertex> {
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
    .into_iter()
    .fold(Vec::new(), |mut vertices, [start, end]| {
        vertices.extend([
            CommonVertex {
                position: origin + Point3D::from_array(start),
                color: Color::BLUE,
                uv: white_pixel_uv,
            },
            CommonVertex {
                position: origin + Point3D::from_array(end),
                color: Color::BLUE,
                uv: white_pixel_uv,
            },
        ]);

        vertices
    })
}

pub fn aabb_outline(Aabb { min, max }: Aabb, white_pixel_uv: Point2D) -> Vec<CommonVertex> {
    let size = (max - min).to_size().as_::<f32>();

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
    .into_iter()
    .fold(Vec::new(), |mut vertices, [start, end]| {
        vertices.extend([
            CommonVertex {
                position: min.as_() + Point3D::from_array(start),
                color: Color::BLUE,
                uv: white_pixel_uv,
            },
            CommonVertex {
                position: min.as_() + Point3D::from_array(end),
                color: Color::BLUE,
                uv: white_pixel_uv,
            },
        ]);

        vertices
    })
}
