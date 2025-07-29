use std::ops::{Index, IndexMut};

use glam::{Mat3, Mat4, Vec3, Vec4};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Plane {
    Left = 0,
    Right = 1,
    Bottom = 2,
    Top = 3,
    Near = 4,
    Far = 5,
    Count = 6,
    Combinations = Self::Count as isize * (Self::Count as isize - 1) / 2,
}

impl Plane {
    const fn k(self, other: Self) -> usize {
        self as usize * (9 - self as usize) / 2 + other as usize - 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrustumCulling {
    planes: [Vec4; Plane::Count as usize],
    points: [Vec3; 8],
}

impl Index<Plane> for FrustumCulling {
    type Output = Vec4;

    fn index(&self, index: Plane) -> &Self::Output {
        &self.planes[index as usize]
    }
}

impl IndexMut<Plane> for FrustumCulling {
    fn index_mut(&mut self, index: Plane) -> &mut Self::Output {
        &mut self.planes[index as usize]
    }
}

impl FrustumCulling {
    pub const fn default() -> Self {
        Self {
            planes: [Vec4::ZERO; Plane::Count as usize],
            points: [Vec3::ZERO; 8],
        }
    }

    fn plane_cross(&self, a: Plane, b: Plane) -> Vec3 {
        self[a].truncate().cross(self[b].truncate())
    }

    pub fn is_box_visible(&self, minp: Vec3, maxp: Vec3) -> bool {
        // check box outside/inside of frustum
        for i in 0..(Plane::Count as usize) {
            if (self.planes[i].dot(Vec4::new(minp.x, minp.y, minp.z, 1.0)) < 0.0)
                && (self.planes[i].dot(Vec4::new(maxp.x, minp.y, minp.z, 1.0)) < 0.0)
                && (self.planes[i].dot(Vec4::new(minp.x, maxp.y, minp.z, 1.0)) < 0.0)
                && (self.planes[i].dot(Vec4::new(maxp.x, maxp.y, minp.z, 1.0)) < 0.0)
                && (self.planes[i].dot(Vec4::new(minp.x, minp.y, maxp.z, 1.0)) < 0.0)
                && (self.planes[i].dot(Vec4::new(maxp.x, minp.y, maxp.z, 1.0)) < 0.0)
                && (self.planes[i].dot(Vec4::new(minp.x, maxp.y, maxp.z, 1.0)) < 0.0)
                && (self.planes[i].dot(Vec4::new(maxp.x, maxp.y, maxp.z, 1.0)) < 0.0)
            {
                return false;
            }
        }

        // check frustum outside/inside box
        let mut out;

        for axis in 0..3 {
            out = 0;

            for i in 0..8 {
                if self.points[i][axis] > maxp[axis] {
                    out += 1;
                }
            }

            if out == 8 {
                return false;
            }

            out = 0;

            for i in 0..8 {
                if self.points[i][axis] < minp[axis] {
                    out += 1;
                }
            }

            if out == 8 {
                return false;
            }
        }

        true
    }

    pub fn update(&mut self, projection_view: Mat4) {
        use Plane::{Bottom, Combinations, Far, Left, Near, Right, Top};

        let projection = projection_view.transpose();

        self[Left] = projection.col(3) + projection.col(0);
        self[Right] = projection.col(3) - projection.col(0);
        self[Bottom] = projection.col(3) + projection.col(1);
        self[Top] = projection.col(3) - projection.col(1);
        self[Near] = projection.col(3) + projection.col(2);
        self[Far] = projection.col(3) - projection.col(2);

        let crosses: [Vec3; Combinations as usize] = [
            self.plane_cross(Left, Right),
            self.plane_cross(Left, Bottom),
            self.plane_cross(Left, Top),
            self.plane_cross(Left, Near),
            self.plane_cross(Left, Far),
            self.plane_cross(Right, Bottom),
            self.plane_cross(Right, Top),
            self.plane_cross(Right, Near),
            self.plane_cross(Right, Far),
            self.plane_cross(Bottom, Top),
            self.plane_cross(Bottom, Near),
            self.plane_cross(Bottom, Far),
            self.plane_cross(Top, Near),
            self.plane_cross(Top, Far),
            self.plane_cross(Near, Far),
        ];

        self.points[0] = self.intersection(Left, Bottom, Near, &crosses);
        self.points[1] = self.intersection(Left, Top, Near, &crosses);
        self.points[2] = self.intersection(Right, Bottom, Near, &crosses);
        self.points[3] = self.intersection(Right, Top, Near, &crosses);
        self.points[4] = self.intersection(Left, Bottom, Far, &crosses);
        self.points[5] = self.intersection(Left, Top, Far, &crosses);
        self.points[6] = self.intersection(Right, Bottom, Far, &crosses);
        self.points[7] = self.intersection(Right, Top, Far, &crosses);
    }

    fn intersection(&self, a: Plane, b: Plane, c: Plane, crosses: &[Vec3]) -> Vec3 {
        let d = self[a].truncate().dot(crosses[b.k(c)]);
        let res = Mat3::from_cols(crosses[b.k(c)], -crosses[a.k(c)], crosses[a.k(b)]) * Vec3::new(self[a].w, self[b].w, self[c].w);

        res * (-1.0 / d)
    }
}
