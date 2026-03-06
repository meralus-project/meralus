use glam::{Mat4, Vec3};
use meralus_world::Face;

use crate::plane::Plane;

pub struct BendApplier {
    pub matrix: Mat4,
    pub consumer: Box<dyn Fn(Vec3) -> Vec3>,
}

impl BendApplier {
    pub fn get_bend(
        bend: Vec3,
        base_plane: Plane,
        other_plane: Plane,
        is_bend_inverted: bool,
        mirror_bend: bool,
        bend_height: f32,
        mut bend_value: f32,
    ) -> Self {
        if mirror_bend {
            bend_value *= -1.0;
        }

        let matrix = Mat4::from_translation(bend) * Mat4::from_rotation_x(bend_value) * Mat4::from_translation(-bend);
        let half_size = bend_height / 2.0;

        Self {
            matrix,
            consumer: Box::new(move |mut pos| {
                let mut dist_from_base = base_plane.distance_to(pos).abs();
                let mut dist_from_other = other_plane.distance_to(pos).abs();

                let s = (bend_value / 2.0).tan() * pos.z;

                if mirror_bend || !is_bend_inverted {
                    std::mem::swap(&mut dist_from_base, &mut dist_from_other);
                }

                let v = half_size
                    - if (is_bend_inverted && s < 0.0) || (!is_bend_inverted && s >= 0.0) {
                        1.0f32.min(s.abs() / 2.0)
                    } else {
                        s.abs()
                    };

                if dist_from_base < dist_from_other {
                    if dist_from_base + dist_from_other <= bend_height && dist_from_base > v {
                        pos.y = bend.y + s;
                    }

                    pos = matrix.transform_point3(pos);
                } else if dist_from_base + dist_from_other <= bend_height && dist_from_other > v {
                    pos.y = bend.y - s;
                }

                pos
            }),
        }
    }

    pub fn get_bend_legacy(
        bend_direction: Face,
        bend: Vec3,
        base_plane: Plane,
        other_plane: Plane,
        is_bend_inverted: bool,
        mirror_bend: bool,
        bend_height: f32,
        mut bend_value: f32,
    ) -> Self {
        if mirror_bend {
            bend_value *= -1.0;
        }

        let final_bend = bend_value;
        let matrix = Mat4::from_translation(bend) * Mat4::from_rotation_x(bend_value) * Mat4::from_translation(-bend);
        let direction_unit = bend_direction.as_normal().to_raw().as_vec3().cross(Vec3::Z);
        let bend_plane = Plane::new(direction_unit, bend);
        let half_size = bend_height / 2.0;

        Self {
            matrix,
            consumer: Box::new(move |pos| {
                let mut dist_from_bend = if is_bend_inverted {
                    -bend_plane.distance_to(pos)
                } else {
                    bend_plane.distance_to(pos)
                };

                let mut dist_from_base = base_plane.distance_to(pos);
                let mut dist_from_other = other_plane.distance_to(pos);
                let x = bend_direction.as_normal().to_raw().as_vec3();

                if mirror_bend {
                    std::mem::swap(&mut dist_from_base, &mut dist_from_other);

                    dist_from_bend *= -1.0;
                }

                let s = (final_bend / 2.0).tan() * dist_from_bend;
                let is_in_bend_area = dist_from_base.abs() + dist_from_other.abs() <= bend_height.abs();

                if dist_from_base.abs() < dist_from_other.abs() {
                    if is_in_bend_area {
                        matrix.transform_point3(pos + x * (-dist_from_base / half_size * s))
                    } else {
                        matrix.transform_point3(pos)
                    }
                } else if is_in_bend_area {
                    pos + x * (-dist_from_other / half_size * s)
                } else {
                    pos
                }
            }),
        }
    }
}
