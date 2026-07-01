use std::time::Duration;

use mavelin_shared::{Color, Face, Lerp};

use crate::input::Input;

const AMBIENT_OCCLUSION_VALUES: [f32; 4] = [0.55, 0.65, 0.8, 1.0];

#[must_use]
#[inline]
pub fn get_movement_direction(binds: &Input) -> glam::Vec3 {
    let mut direction = glam::Vec3::ZERO;

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
#[inline]
pub fn get_rotation_directions(yaw: f32, pitch: f32) -> (glam::Vec3, glam::Vec3, glam::Vec3) {
    let front = glam::Vec3::new(yaw.cos() * pitch.cos(), pitch.sin(), yaw.sin() * pitch.cos()).normalize();
    let right = front.cross(glam::Vec3::Y).normalize();

    (front, right, right.cross(front).normalize())
}

#[must_use]
#[allow(clippy::fn_params_excessive_bools)]
#[inline]
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

// #[allow(dead_code)]
// pub fn cube_outline(Cube3D { origin, size }: Cube3D, white_pixel_uv:
// glam::Vec2) -> Vec<CommonVertex> {     [
//         [[0.0, 0.0, 0.0], [0.0, size.y, 0.0]],
//         [[size.x, 0.0, 0.0], [size.x, size.y, 0.0]],
//         [[0.0, 0.0, size.z], [0.0, size.y, size.z]],
//         [[size.x, 0.0, size.z], [size.x, size.y, size.z]],
//         [[0.0, 0.0, 0.0], [size.x, 0.0, 0.0]],
//         [[0.0, 0.0, 0.0], [0.0, 0.0, size.z]],
//         [[size.x, 0.0, 0.0], [size.x, 0.0, size.z]],
//         [[0.0, 0.0, size.z], [size.x, 0.0, size.z]],
//         [[0.0, size.y, 0.0], [size.x, size.y, 0.0]],
//         [[0.0, size.y, 0.0], [0.0, size.y, size.z]],
//         [[size.x, size.y, 0.0], [size.x, size.y, size.z]],
//         [[0.0, size.y, size.z], [size.x, size.y, size.z]],
//     ]
//     .into_iter()
//     .fold(Vec::new(), |mut vertices, [start, end]| {
//         vertices.extend([
//             CommonVertex {
//                 position: origin + glam::Vec3::from_array(start),
//                 color: Color::BLUE.as_value(),
//                 uv: white_pixel_uv,
//                 clip: glam::Vec4::new(0.0, 0.0, 1.0, 1.0),
//                 _pad: [0; 8],
//             },
//             CommonVertex {
//                 position: origin + glam::Vec3::from_array(end),
//                 color: Color::BLUE.as_value(),
//                 uv: white_pixel_uv,
//                 clip: glam::Vec4::new(0.0, 0.0, 1.0, 1.0),
//                 _pad: [0; 8],
//             },
//         ]);

//         vertices
//     })
// }

// #[allow(dead_code)]
// pub fn aabb_outline(Aabb { min, max }: Aabb, white_pixel_uv: glam::Vec2) ->
// Vec<CommonVertex> {     let size = (max - min).as_vec3();

//     [
//         [[0.0, 0.0, 0.0], [0.0, size.y, 0.0]],
//         [[size.x, 0.0, 0.0], [size.x, size.y, 0.0]],
//         [[0.0, 0.0, size.z], [0.0, size.y, size.z]],
//         [[size.x, 0.0, size.z], [size.x, size.y, size.z]],
//         [[0.0, 0.0, 0.0], [size.x, 0.0, 0.0]],
//         [[0.0, 0.0, 0.0], [0.0, 0.0, size.z]],
//         [[size.x, 0.0, 0.0], [size.x, 0.0, size.z]],
//         [[0.0, 0.0, size.z], [size.x, 0.0, size.z]],
//         [[0.0, size.y, 0.0], [size.x, size.y, 0.0]],
//         [[0.0, size.y, 0.0], [0.0, size.y, size.z]],
//         [[size.x, size.y, 0.0], [size.x, size.y, size.z]],
//         [[0.0, size.y, size.z], [size.x, size.y, size.z]],
//     ]
//     .into_iter()
//     .fold(Vec::new(), |mut vertices, [start, end]| {
//         vertices.extend([
//             CommonVertex {
//                 position: min.as_vec3() + glam::Vec3::from_array(start),
//                 color: Color::BLUE.as_value(),
//                 uv: white_pixel_uv,
//                 clip: glam::Vec4::new(0.0, 0.0, 1.0, 1.0),
//                 _pad: [0; 8],
//             },
//             CommonVertex {
//                 position: min.as_vec3() + glam::Vec3::from_array(end),
//                 color: Color::BLUE.as_value(),
//                 uv: white_pixel_uv,
//                 clip: glam::Vec4::new(0.0, 0.0, 1.0, 1.0),
//                 _pad: [0; 8],
//             },
//         ]);

//         vertices
//     })
// }
#[allow(dead_code)]
pub fn get_sky_color((after_day, progress): (bool, f32), weather: f32) -> Color {
    let day_color: Color = Color::from_hsl(220.0, 0.2f32.mul_add(weather, 0.5), 0.6f32.mul_add(-weather, 0.75));
    let night_color: Color = Color::from_hsl(220.0, 0.1f32.mul_add(weather, 0.35), 0.15f32.mul_add(-weather, 0.25));

    if after_day {
        day_color.lerp(&night_color, progress)
    } else {
        night_color.lerp(&day_color, progress)
    }
}

pub struct Interval {
    rate: Duration,
    accel: Duration,
}

impl Interval {
    pub const fn new(rate: Duration) -> Self {
        Self { rate, accel: Duration::ZERO }
    }

    pub fn update(&mut self, delta: Duration) -> usize {
        self.accel += delta;

        let mut times = 0;

        while self.accel >= self.rate {
            self.accel -= self.rate;

            times += 1;
        }

        times
    }
}
