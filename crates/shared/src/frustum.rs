use std::ops::{Index, IndexMut};

use crate::{Point3D, Transform2D, Transform3D, Vector3D, Vector4D};

pub trait Frustum {
    fn is_box_visible(&self, minp: Point3D, maxp: Point3D) -> bool;
}

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
    planes: [Vector4D; Plane::Count as usize],
    points: [Vector3D; 8],
}

impl Index<Plane> for FrustumCulling {
    type Output = Vector4D;

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
            planes: [Vector4D::ZERO; Plane::Count as usize],
            points: [Vector3D::ZERO; 8],
        }
    }

    pub fn update(&mut self, projection_view: Transform3D) {
        use Plane::{Bottom, Combinations, Far, Left, Near, Right, Top};

        let projection = projection_view.transpose();

        self[Left] = projection.col(3) + projection.col(0);
        self[Right] = projection.col(3) - projection.col(0);
        self[Bottom] = projection.col(3) + projection.col(1);
        self[Top] = projection.col(3) - projection.col(1);
        self[Near] = projection.col(3) + projection.col(2);
        self[Far] = projection.col(3) - projection.col(2);

        let crosses: [Vector3D; Combinations as usize] = [
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

    fn plane_cross(&self, a: Plane, b: Plane) -> Vector3D {
        self[a].truncate().cross(self[b].truncate())
    }

    fn intersection(&self, a: Plane, b: Plane, c: Plane, crosses: &[Vector3D]) -> Vector3D {
        let d = self[a].truncate().dot(crosses[b.k(c)]);
        let res = Transform2D::from_cols(crosses[b.k(c)], -crosses[a.k(c)], crosses[a.k(b)]) * Vector3D::new(self[a].w, self[b].w, self[c].w);

        res * (-1.0 / d)
    }
}

impl Frustum for FrustumCulling {
    fn is_box_visible(&self, minp: Point3D, maxp: Point3D) -> bool {
        // check box outside/inside of frustum
        for i in 0..(Plane::Count as usize) {
            if (self.planes[i].dot(Vector4D::new(minp.x, minp.y, minp.z, 1.0)) < 0.0)
                && (self.planes[i].dot(Vector4D::new(maxp.x, minp.y, minp.z, 1.0)) < 0.0)
                && (self.planes[i].dot(Vector4D::new(minp.x, maxp.y, minp.z, 1.0)) < 0.0)
                && (self.planes[i].dot(Vector4D::new(maxp.x, maxp.y, minp.z, 1.0)) < 0.0)
                && (self.planes[i].dot(Vector4D::new(minp.x, minp.y, maxp.z, 1.0)) < 0.0)
                && (self.planes[i].dot(Vector4D::new(maxp.x, minp.y, maxp.z, 1.0)) < 0.0)
                && (self.planes[i].dot(Vector4D::new(minp.x, maxp.y, maxp.z, 1.0)) < 0.0)
                && (self.planes[i].dot(Vector4D::new(maxp.x, maxp.y, maxp.z, 1.0)) < 0.0)
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
}
