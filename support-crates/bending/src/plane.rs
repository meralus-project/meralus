use glam::Vec3;

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Plane {
    pub normal: Vec3,
    pub norm_distance: f32,
}

impl Plane {
    pub fn new(normal: Vec3, pos: Vec3) -> Self {
        let normal = normal.normalize();

        Self {
            normal,
            norm_distance: -normal.dot(pos),
        }
    }

    pub fn distance_to(&self, pos: Vec3) -> f32 {
        self.normal.dot(pos) + self.norm_distance
    }

    pub fn distance_to_plane(&self, other: Self) -> f32 {
        let tmp = self.normal.cross(other.normal);

        if tmp.dot(tmp) < 0.01 {
            self.normal.dot(other.normal).mul_add(other.norm_distance, self.norm_distance)
        } else {
            0.0
        }
    }
}
