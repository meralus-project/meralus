use std::{
    collections::{HashMap, HashSet},
    hash::{DefaultHasher, Hash, Hasher},
};

use glam::{IVec2, Vec2, Vec3};
use meralus_world::Face;

use crate::{bend_applier::BendApplier, plane::Plane, quad::Quad};

#[derive(Debug)]
pub struct BCData {
    uv: IVec2,
    origin: Vec3,
    size: Vec3,
    extra: Vec3,
    mirror: bool,
    texture_size: Vec2,
    visible_faces: HashSet<Face>,
}

#[derive(Debug)]
pub struct Cuboid {
    pub min: Vec3,
    pub max: Vec3,
    pub data: BCData,
    pub base_plane: Plane,
    pub other_plane: Plane,
    pub bend: Vec3,
    pub bend_height: f32,
    pub sides: Vec<Quad>,
    pub positions: HashMap<u64, Vec3>,
    pub vertices: Vec<Vec3>,
    pub direction: Option<Face>,
    pub pivot: Option<i32>,
}

impl Default for Cuboid {
    fn default() -> Self {
        Self {
            min: Vec3::ZERO,
            max: Vec3::new(16.0, 64.0, 16.0),
            data: BCData {
                uv: IVec2::ZERO,
                origin: Vec3::ZERO,
                size: Vec3::new(16.0, 64.0, 16.0),
                extra: Vec3::ZERO,
                mirror: false,
                texture_size: Vec2::splat(16.0),
                visible_faces: HashSet::from_iter(Face::ALL),
            },
            base_plane: Plane::default(),
            other_plane: Plane::default(),
            bend: Vec3::ZERO,
            bend_height: 0.0,
            sides: Vec::new(),
            positions: HashMap::new(),
            vertices: Vec::new(),
            direction: None,
            pivot: None,
        }
    }
}

impl Cuboid {
    pub fn rebuild(&mut self, direction: Face, point: Option<i32>) {
        if self.sides.is_empty() {
            self.build();
        }

        if self.direction == Some(direction) && self.pivot == point {
            return;
        }

        self.direction.replace(direction);
        self.pivot = point;

        let direction = Face::Top;

        let mut pivot = Vec3::ZERO;

        if let Some(point) = point
            && point >= 0
        {
            let size = (direction.as_normal().to_raw().as_vec3() * self.data.size).length();

            if point as f32 <= size {
                pivot = direction.as_normal().to_raw().as_vec3() * (point as f32).mul_add(-2.0, size);

                self.vertices[6] -= pivot;
            }
        }

        self.base_plane = Plane::new(direction.as_normal().to_raw().as_vec3(), self.vertices[6]);
        self.other_plane = Plane::new(direction.as_normal().to_raw().as_vec3(), self.vertices[0]);
        self.bend_height = direction.as_normal().to_raw().as_vec3().dot(self.vertices[6]) - direction.as_normal().to_raw().as_vec3().dot(self.vertices[0]);
        self.bend = (self.data.size + self.min + self.min - pivot) / 2.0;
    }

    pub fn is_bend_inverted(&self) -> bool {
        self.direction.is_some_and(Face::is_positive)
    }

    pub fn apply_bend_legacy(&mut self, mut bend_value: f32) {
        if bend_value.abs() < 0.0001 {
            bend_value = 0.0;
        }

        let bend_applier = BendApplier::get_bend_legacy(
            self.direction.unwrap(),
            self.bend,
            self.base_plane,
            self.other_plane,
            self.is_bend_inverted(),
            self.direction == Some(Face::Top),
            self.bend_height,
            bend_value,
        );

        for side in &mut self.sides {
            for vertice in &mut side.vertices {
                vertice.position = (bend_applier.consumer)(vertice.original_position);
            }
        }
    }

    pub fn apply_bend(&mut self, mut bend_value: f32) {
        if bend_value.abs() < 0.0001 {
            bend_value = 0.0;
        }

        let bend_applier = BendApplier::get_bend(
            self.bend,
            self.base_plane,
            self.other_plane,
            self.is_bend_inverted(),
            false,
            self.bend_height,
            bend_value,
        );

        for side in &mut self.sides {
            for vertice in &mut side.vertices {
                vertice.position = (bend_applier.consumer)(vertice.original_position);
            }
        }
    }

    fn create_and_add_quads(
        quads: &mut Vec<Quad>,
        positions: &mut HashMap<u64, Vec3>,
        edges: [Vec3; 3],
        u1: f32,
        v1: f32,
        u2: f32,
        v2: f32,
        texture_width: f32,
        texture_height: f32,
        mirror: bool,
    ) {
        let positive_direction = v2 > v1;
        let total_tex_height = (v2 - v1).abs();

        let origin = edges[0];
        let vec_v = edges[2] - origin;
        let v_frac_scale = if v1 == v2 { 0.0 } else { 1.0 / (v2 - v1) };
        let vec_u = edges[1] - origin;
        let u_frac_scale = if u1 == u2 { 0.0 } else { 1.0 / (u2 - u1) };

        let mut v_pos = origin;
        let mut next_vpos = edges[1];
        let mut v_step;

        let mut quad_heights = None; // float[]
        let mut segment_height = 0;

        if total_tex_height > 0.0 && total_tex_height % 3.0 == 0.0 {
            segment_height = (total_tex_height / 3.0) as usize;

            if segment_height > 0 {
                let mut qh = vec![1.0; 2 + segment_height];

                qh[0] = segment_height as f32;
                qh[1 + segment_height] = segment_height as f32;

                quad_heights.replace(qh);
            }
        }

        let mut layer_index = 0;
        let mut local_v = v1;

        while (positive_direction && local_v < v2) || (!positive_direction && local_v > v2) {
            let dv;
            let mut is_middle_segment = false;

            if let Some(quad_heights) = &quad_heights {
                if layer_index >= quad_heights.len() {
                    break;
                }

                dv = if positive_direction {
                    quad_heights[layer_index]
                } else {
                    -quad_heights[layer_index]
                };

                if layer_index > 0 && layer_index <= segment_height {
                    is_middle_segment = true;
                }

                layer_index += 1;
            } else {
                dv = if positive_direction { 1.0 } else { -1.0 };
            }

            let mut local_v2 = local_v + dv;

            if quad_heights.is_none() && ((positive_direction && local_v2 > v2) || (!positive_direction && local_v2 < v2)) {
                local_v2 = v2;
            }

            let actual_dv = local_v2 - local_v;

            if actual_dv == 0.0 {
                break;
            }

            v_step = vec_v * (actual_dv * v_frac_scale);

            if is_middle_segment && (u2 - u1).abs() > 1.0 {
                let u_positive = u2 > u1;
                let du = if u_positive { 1.0 } else { -1.0 };

                let mut u_scan_pos_bottom = v_pos;
                let mut u_scan_pos_top = v_pos + v_step;

                let mut local_u = u2;

                while (u_positive && local_u > u1) || (!u_positive && local_u < u1) {
                    let mut local_u2 = local_u - du;

                    if (u_positive && local_u2 > u2) || (!u_positive && local_u2 < u2) {
                        local_u2 = u2;
                    }

                    if local_u == local_u2 {
                        break;
                    }

                    let actual_du = local_u2 - local_u;
                    let u_step = vec_u * (actual_du * u_frac_scale);

                    let bottom_left = Self::get_or_create(positions, u_scan_pos_bottom);
                    let top_left = Self::get_or_create(positions, u_scan_pos_top);

                    u_scan_pos_bottom -= u_step;
                    u_scan_pos_top -= u_step;

                    let bottom_right = Self::get_or_create(positions, u_scan_pos_bottom);
                    let top_right = Self::get_or_create(positions, u_scan_pos_top);

                    quads.push(Quad::new(
                        [bottom_left, bottom_right, top_right, top_left],
                        Vec2::new(local_u2 / texture_width, local_v / texture_height),
                        Vec2::new(local_u / texture_width, local_v2 / texture_height),
                        mirror,
                    ));

                    local_u -= du;
                }

                v_pos += v_step;
                next_vpos += v_step;
            } else {
                let rp3 = Self::get_or_create(positions, v_pos);
                let rp0 = Self::get_or_create(positions, next_vpos);

                v_pos += v_step;
                next_vpos += v_step;

                let rp2 = Self::get_or_create(positions, v_pos);
                let rp1 = Self::get_or_create(positions, next_vpos);

                quads.push(Quad::new(
                    [rp3, rp0, rp1, rp2],
                    Vec2::new(u1 / texture_width, local_v / texture_height),
                    Vec2::new(u2 / texture_width, local_v2 / texture_height),
                    mirror,
                ));
            }

            local_v = local_v2;
        }
    }

    pub fn build(&mut self) {
        let mut planes = Vec::new();
        let mut positions = HashMap::new();

        let mut pmin = self.min - self.data.extra;
        let mut pmax = self.max + self.data.extra;

        if self.data.mirror {
            std::mem::swap(&mut pmin.x, &mut pmax.x);
        }

        self.vertices = vec![
            pmin,
            Vec3::new(pmax.x, pmin.y, pmin.z),
            Vec3::new(pmax.x, pmax.y, pmin.z),
            Vec3::new(pmin.x, pmax.y, pmin.z),
            Vec3::new(pmin.x, pmin.y, pmax.z),
            Vec3::new(pmax.x, pmin.y, pmax.z),
            pmax,
            Vec3::new(pmin.x, pmax.y, pmax.z),
        ];

        let x = self.data.uv.x as f32;
        let xz = self.data.uv.x as f32 + self.data.size.z;
        let xzx = self.data.uv.x as f32 + self.data.size.z + self.data.size.x;
        let xzxx = self.data.uv.x as f32 + self.data.size.z + self.data.size.x + self.data.size.x;
        let xzxz = self.data.uv.x as f32 + self.data.size.z + self.data.size.x + self.data.size.z;
        let xzxzx = self.data.uv.x as f32 + self.data.size.z + self.data.size.x + self.data.size.z + self.data.size.x;
        let y = self.data.uv.y as f32;
        let yz = self.data.uv.y as f32 + self.data.size.z;
        let yzy = self.data.uv.y as f32 + self.data.size.z + self.data.size.y;
        let texture_width = self.data.texture_size.x;
        let texture_height = self.data.texture_size.y;
        let mirror = self.data.mirror;

        if self.data.visible_faces.contains(&Face::Bottom) {
            Self::create_and_add_quads(
                &mut planes,
                &mut positions,
                [self.vertices[5], self.vertices[4], self.vertices[1]],
                xz,
                y,
                xzx,
                yz,
                texture_width,
                texture_height,
                mirror,
            ); //down
        }
        if self.data.visible_faces.contains(&Face::Top) {
            Self::create_and_add_quads(
                &mut planes,
                &mut positions,
                [self.vertices[2], self.vertices[3], self.vertices[6]],
                xzx,
                yz,
                xzxx,
                y,
                texture_width,
                texture_height,
                mirror,
            ); //up
        }
        if self.data.visible_faces.contains(&Face::Left) {
            Self::create_and_add_quads(
                &mut planes,
                &mut positions,
                [self.vertices[0], self.vertices[4], self.vertices[3]],
                x,
                yz,
                xz,
                yzy,
                texture_width,
                texture_height,
                mirror,
            ); //west
        }
        if self.data.visible_faces.contains(&Face::Front) {
            Self::create_and_add_quads(
                &mut planes,
                &mut positions,
                [self.vertices[1], self.vertices[0], self.vertices[2]],
                xz,
                yz,
                xzx,
                yzy,
                texture_width,
                texture_height,
                mirror,
            ); //north
        }
        if self.data.visible_faces.contains(&Face::Right) {
            Self::create_and_add_quads(
                &mut planes,
                &mut positions,
                [self.vertices[5], self.vertices[1], self.vertices[6]],
                xzx,
                yz,
                xzxz,
                yzy,
                texture_width,
                texture_height,
                mirror,
            ); //east
        }
        if self.data.visible_faces.contains(&Face::Back) {
            Self::create_and_add_quads(
                &mut planes,
                &mut positions,
                [self.vertices[4], self.vertices[5], self.vertices[7]],
                xzxz,
                yz,
                xzxzx,
                yzy,
                texture_width,
                texture_height,
                mirror,
            ); //south
        }

        self.sides = planes;
        // this.positions = positions.values().toArray(new RememberingPos[0]);
    }

    fn get_or_create(positions: &mut HashMap<u64, Vec3>, pos: Vec3) -> Vec3 {
        let mut hasher = DefaultHasher::new();

        pos.x.to_bits().hash(&mut hasher);
        pos.y.to_bits().hash(&mut hasher);
        pos.z.to_bits().hash(&mut hasher);

        let result = hasher.finish();

        *positions.entry(result).or_insert(pos)
    }
}
