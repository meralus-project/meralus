use std::fmt;

use meralus_shared::{IPoint3D, Point2D, Point3D};
use serde::{Deserialize, Serialize};

#[allow(clippy::unsafe_derive_deserialize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize, Hash)]
#[serde(rename_all = "camelCase")]
pub enum Face {
    Bottom,
    Top,
    Left,
    Right,
    Front,
    Back,
}

impl fmt::Display for Face {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Top => f.write_str("Top"),
            Self::Bottom => f.write_str("Bottom"),
            Self::Left => f.write_str("Left"),
            Self::Right => f.write_str("Right"),
            Self::Front => f.write_str("Front"),
            Self::Back => f.write_str("Back"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Axis {
    X,
    Y,
    Z,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Corner {
    LeftTop,
    RightTop,
    LeftBottom,
    RightBottom,
}

impl Corner {
    pub const fn index(&self) -> usize {
        match self {
            Self::LeftTop => 0,
            Self::RightTop => 1,
            Self::LeftBottom => 2,
            Self::RightBottom => 3,
        }
    }

    pub const fn from_array([x, y]: [f32; 2]) -> Self {
        let [x, y] = [x > 0.0, y > 0.0];

        match [x, y] {
            [true, true] => Self::RightTop,
            [true, false] => Self::RightBottom,
            [false, true] => Self::LeftTop,
            [false, false] => Self::LeftBottom,
        }
    }

    pub const fn from_vec(face: Face, vec: Point3D) -> Self {
        Self::from_array(match face.as_axis() {
            Axis::X => [vec.y, vec.z], // only yz
            Axis::Y => [vec.x, vec.z], // only xz
            Axis::Z => [vec.x, vec.y], // only xy
        })
    }

    // const NEIGHBOURS: [[i32; 2]; 8] = [
    //     [-1, -1], // LEFT BOTTOM
    //     [-1, 0],  // LEFT
    //     [-1, 1],  // LEFT TOP
    //     [0, -1],  // BOTTOM
    //     [0, 1],   // TOP
    //     [1, -1],  // RIGHT BOTTOM
    //     [1, 0],   // RIGHT
    //     [1, 1],   // RIGHT TOP
    // ];

    pub const fn get_neighbours(self, face: Face) -> [IPoint3D; 3] {
        let neighbours = face.get_neighbours();

        match self {
            Self::LeftTop => [neighbours[1], neighbours[4], neighbours[2]],
            Self::RightTop => [neighbours[6], neighbours[4], neighbours[7]],
            Self::LeftBottom => [neighbours[1], neighbours[3], neighbours[0]],
            Self::RightBottom => [neighbours[6], neighbours[3], neighbours[5]],
        }
    }
}

impl Face {
    pub const ALL: [Self; 6] = [Self::Bottom, Self::Top, Self::Left, Self::Right, Self::Front, Self::Back];
    pub const BOOL_VERTICES: [[bool; 3]; 8] = [
        [false, false, true],  // 0 LEFT  BOTTOM FRONT
        [true, false, true],   // 1 RIGHT BOTTOM FRONT
        [false, true, true],   // 2 LEFT  TOP    FRONT
        [true, true, true],    // 3 RIGHT TOP    FRONT
        [false, false, false], // 4 LEFT  BOTTOM BACK
        [true, false, false],  // 5 RIGHT BOTTOM BACK
        [false, true, false],  // 6 LEFT  TOP    BACK
        [true, true, false],   // 7 RIGHT TOP    BACK
    ];
    pub const NORMALS: [IPoint3D; 6] = [IPoint3D::NEG_Y, IPoint3D::Y, IPoint3D::NEG_X, IPoint3D::X, IPoint3D::Z, IPoint3D::NEG_Z];
    // const NEIGHBOURS: [[i32; 2]; 8] = [
    //     [-1, -1], // LEFT BOTTOM
    //     [-1, 0],  // LEFT
    //     [-1, 1],  // LEFT TOP
    //     [0, -1],  // BOTTOM
    //     [0, 1],   // TOP
    //     [1, -1],  // RIGHT BOTTOM
    //     [1, 0],   // RIGHT
    //     [1, 1],   // RIGHT TOP
    // ];
    pub const VERTICES: [Point3D; 8] = [
        Point3D::new(0.0, 0.0, 1.0), // 0 LEFT  BOTTOM FRONT
        Point3D::new(1.0, 0.0, 1.0), // 1 RIGHT BOTTOM FRONT
        Point3D::new(0.0, 1.0, 1.0), // 2 LEFT  TOP    FRONT
        Point3D::new(1.0, 1.0, 1.0), // 3 RIGHT TOP    FRONT
        Point3D::new(0.0, 0.0, 0.0), // 4 LEFT  BOTTOM BACK
        Point3D::new(1.0, 0.0, 0.0), // 5 RIGHT BOTTOM BACK
        Point3D::new(0.0, 1.0, 0.0), // 6 LEFT  TOP    BACK
        Point3D::new(1.0, 1.0, 0.0), // 7 RIGHT TOP    BACK
    ];

    pub const fn get_light_level(self) -> f32 {
        match self {
            Self::Top => 1.0,
            Self::Bottom => 0.5,
            Self::Left | Self::Right => 0.6,
            Self::Front | Self::Back => 0.8,
        }
    }

    #[must_use]
    pub const fn opposite_normal_index(self) -> usize {
        match self {
            Self::Bottom => 1,
            Self::Top => 0,
            Self::Left => 3,
            Self::Right => 2,
            Self::Front => 5,
            Self::Back => 4,
        }
    }

    #[must_use]
    pub const fn opposite(self) -> Self {
        match self {
            Self::Bottom => Self::Top,
            Self::Top => Self::Bottom,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
            Self::Front => Self::Back,
            Self::Back => Self::Front,
        }
    }

    #[must_use]
    pub const fn get_neighbours(self) -> [IPoint3D; 8] {
        let normal = self.as_normal();
        let axis = self.as_axis();
        let v = match axis {
            Axis::X => normal.x,
            Axis::Y => normal.y,
            Axis::Z => normal.z,
        };

        // SAFETY: IVec3 is [i32; 3]
        unsafe {
            std::mem::transmute::<[[i32; 3]; 8], [IPoint3D; 8]>(match axis {
                Axis::X => [[v, -1, -1], [v, -1, 0], [v, -1, 1], [v, 0, -1], [v, 0, 1], [v, 1, -1], [v, 1, 0], [v, 1, 1]],
                Axis::Y => [[-1, v, -1], [-1, v, 0], [-1, v, 1], [0, v, -1], [0, v, 1], [1, v, -1], [1, v, 0], [1, v, 1]],
                Axis::Z => [[-1, -1, v], [-1, 0, v], [-1, 1, v], [0, -1, v], [0, 1, v], [1, -1, v], [1, 0, v], [1, 1, v]],
            })
        }
    }

    pub const fn normal_index(self) -> usize {
        self as usize
    }

    #[must_use]
    pub const fn from_axis_value(axis: Axis, is_positive: bool) -> Self {
        match (axis, is_positive) {
            (Axis::X, true) => Self::Right,
            (Axis::X, false) => Self::Left,
            (Axis::Y, true) => Self::Top,
            (Axis::Y, false) => Self::Bottom,
            (Axis::Z, true) => Self::Front,
            (Axis::Z, false) => Self::Back,
        }
    }

    pub const fn as_vertex_corners(self) -> [Corner; 4] {
        let vertices = self.as_vertices();

        [
            Corner::from_vec(self, vertices[0]),
            Corner::from_vec(self, vertices[1]),
            Corner::from_vec(self, vertices[2]),
            Corner::from_vec(self, vertices[3]),
        ]
    }

    pub const fn as_bool_vertices(self) -> [[bool; 3]; 4] {
        match self {
            Self::Top => [Self::BOOL_VERTICES[2], Self::BOOL_VERTICES[6], Self::BOOL_VERTICES[7], Self::BOOL_VERTICES[3]],
            Self::Bottom => [Self::BOOL_VERTICES[1], Self::BOOL_VERTICES[5], Self::BOOL_VERTICES[4], Self::BOOL_VERTICES[0]],
            Self::Left => [Self::BOOL_VERTICES[4], Self::BOOL_VERTICES[6], Self::BOOL_VERTICES[2], Self::BOOL_VERTICES[0]],
            Self::Right => [Self::BOOL_VERTICES[1], Self::BOOL_VERTICES[3], Self::BOOL_VERTICES[7], Self::BOOL_VERTICES[5]],
            Self::Front => [Self::BOOL_VERTICES[1], Self::BOOL_VERTICES[0], Self::BOOL_VERTICES[2], Self::BOOL_VERTICES[3]],
            Self::Back => [Self::BOOL_VERTICES[5], Self::BOOL_VERTICES[7], Self::BOOL_VERTICES[6], Self::BOOL_VERTICES[4]],
        }
    }

    #[must_use]
    pub const fn as_vertices(self) -> [Point3D; 4] {
        const X0Y0Z1: Point3D = Point3D::new(0.0, 0.0, 1.0); // LEFT  BOTTOM FRONT
        const X1Y0Z1: Point3D = Point3D::new(1.0, 0.0, 1.0); // RIGHT BOTTOM FRONT
        const X0Y1Z1: Point3D = Point3D::new(0.0, 1.0, 1.0); // LEFT  TOP    FRONT
        const X1Y1Z1: Point3D = Point3D::new(1.0, 1.0, 1.0); // RIGHT TOP    FRONT
        const X0Y0Z0: Point3D = Point3D::new(0.0, 0.0, 0.0); // LEFT  BOTTOM BACK
        const X1Y0Z0: Point3D = Point3D::new(1.0, 0.0, 0.0); // RIGHT BOTTOM BACK
        const X0Y1Z0: Point3D = Point3D::new(0.0, 1.0, 0.0); // LEFT  TOP    BACK
        const X1Y1Z0: Point3D = Point3D::new(1.0, 1.0, 0.0); // RIGHT TOP    BACK

        match self {
            Self::Top => [X0Y1Z0, X1Y1Z0, X0Y1Z1, X1Y1Z1],
            Self::Bottom => [X1Y0Z0, X0Y0Z0, X1Y0Z1, X0Y0Z1],
            Self::Left => [X0Y0Z1, X0Y0Z0, X0Y1Z1, X0Y1Z0],
            Self::Right => [X1Y0Z0, X1Y0Z1, X1Y1Z0, X1Y1Z1],
            Self::Front => [X1Y0Z1, X0Y0Z1, X1Y1Z1, X0Y1Z1],
            Self::Back => [X0Y0Z0, X1Y0Z0, X0Y1Z0, X1Y1Z0],
        }
    }

    #[must_use]
    pub const fn as_uv(self) -> [Point2D; 4] {
        [Point2D::ZERO, Point2D::X, Point2D::Y, Point2D::ONE]
    }

    #[must_use]
    pub const fn as_normal(self) -> IPoint3D {
        match self {
            Self::Top => IPoint3D::Y,
            Self::Bottom => IPoint3D::NEG_Y,
            Self::Right => IPoint3D::X,
            Self::Left => IPoint3D::NEG_X,
            Self::Front => IPoint3D::Z,
            Self::Back => IPoint3D::NEG_Z,
        }
    }

    pub const fn is_positive(self) -> bool {
        matches!(self, Self::Top | Self::Right | Self::Front)
    }

    pub const fn as_axis(self) -> Axis {
        match self {
            Self::Top | Self::Bottom => Axis::Y,
            Self::Left | Self::Right => Axis::X,
            Self::Front | Self::Back => Axis::Z,
        }
    }

    // #[must_use]
    // pub const fn add_position(self, mut position: U16Vec3) -> U16Vec3 {
    //     match self {
    //         Self::Top => position.y += 1,
    //         Self::Bottom => position.y = position.y.saturating_sub(1),
    //         Self::Right => position.x += 1,
    //         Self::Left => position.x = position.x.saturating_sub(1),
    //         Self::Front => position.z += 1,
    //         Self::Back => position.z = position.z.saturating_sub(1),
    //     }

    //     position
    // }
}

#[cfg(test)]
mod tests {
    use std::array;

    use super::Face;

    #[test]
    fn test_face_corners() {
        for face in Face::ALL {
            println!("{:#?}", face.as_vertex_corners());
        }
    }

    #[test]
    fn test_uh() {
        let top = Face::Top.as_vertices();
        let bottom = Face::Bottom.as_vertices();

        println!("{top:?} - {bottom:?} = {:?}", array::from_fn::<_, 4, _>(|i| top[i] - bottom[i]));
    }
}
