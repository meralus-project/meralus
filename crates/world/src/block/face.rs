use std::fmt;

use glam::{IVec3, U16Vec3, Vec2, Vec3, vec2, vec3};
use serde::{Deserialize, Serialize};

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

    pub const fn from_vec(face: Face, vec: Vec3) -> Self {
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

    pub fn get_neighbours(self, face: Face) -> [IVec3; 3] {
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
    pub const ALL: [Self; 6] = [
        Self::Bottom,
        Self::Top,
        Self::Left,
        Self::Right,
        Self::Front,
        Self::Back,
    ];
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
    const NEIGHBOURS: [[i32; 2]; 8] = [
        [-1, -1], // LEFT BOTTOM
        [-1, 0],  // LEFT
        [-1, 1],  // LEFT TOP
        [0, -1],  // BOTTOM
        [0, 1],   // TOP
        [1, -1],  // RIGHT BOTTOM
        [1, 0],   // RIGHT
        [1, 1],   // RIGHT TOP
    ];
    pub const VERTICES: [Vec3; 8] = [
        vec3(0.0, 0.0, 1.0), // 0 LEFT  BOTTOM FRONT
        vec3(1.0, 0.0, 1.0), // 1 RIGHT BOTTOM FRONT
        vec3(0.0, 1.0, 1.0), // 2 LEFT  TOP    FRONT
        vec3(1.0, 1.0, 1.0), // 3 RIGHT TOP    FRONT
        vec3(0.0, 0.0, 0.0), // 4 LEFT  BOTTOM BACK
        vec3(1.0, 0.0, 0.0), // 5 RIGHT BOTTOM BACK
        vec3(0.0, 1.0, 0.0), // 6 LEFT  TOP    BACK
        vec3(1.0, 1.0, 0.0), // 7 RIGHT TOP    BACK
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
    pub fn get_neighbours(self) -> [IVec3; 8] {
        let mut normal = self.as_normal();
        let axis = self.as_axis();

        match axis {
            Axis::X => Self::NEIGHBOURS.map(|neighbour| {
                normal.y = neighbour[0];
                normal.z = neighbour[1];

                normal
            }),
            Axis::Y => Self::NEIGHBOURS.map(|neighbour| {
                normal.x = neighbour[0];
                normal.z = neighbour[1];

                normal
            }),
            Axis::Z => Self::NEIGHBOURS.map(|neighbour| {
                normal.x = neighbour[0];
                normal.y = neighbour[1];

                normal
            }),
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

    pub fn as_vertice_corners(self) -> [Corner; 4] {
        self.as_vertices()
            .map(|vertice| Corner::from_vec(self, vertice))
    }

    pub const fn as_bool_vertices(self) -> [[bool; 3]; 4] {
        match self {
            Self::Top => [
                Self::BOOL_VERTICES[2],
                Self::BOOL_VERTICES[6],
                Self::BOOL_VERTICES[7],
                Self::BOOL_VERTICES[3],
            ],
            Self::Bottom => [
                Self::BOOL_VERTICES[1],
                Self::BOOL_VERTICES[5],
                Self::BOOL_VERTICES[4],
                Self::BOOL_VERTICES[0],
            ],
            Self::Left => [
                Self::BOOL_VERTICES[4],
                Self::BOOL_VERTICES[6],
                Self::BOOL_VERTICES[2],
                Self::BOOL_VERTICES[0],
            ],
            Self::Right => [
                Self::BOOL_VERTICES[1],
                Self::BOOL_VERTICES[3],
                Self::BOOL_VERTICES[7],
                Self::BOOL_VERTICES[5],
            ],
            Self::Front => [
                Self::BOOL_VERTICES[1],
                Self::BOOL_VERTICES[0],
                Self::BOOL_VERTICES[2],
                Self::BOOL_VERTICES[3],
            ],
            Self::Back => [
                Self::BOOL_VERTICES[5],
                Self::BOOL_VERTICES[7],
                Self::BOOL_VERTICES[6],
                Self::BOOL_VERTICES[4],
            ],
        }
    }

    #[must_use]
    pub const fn as_vertices(self) -> [Vec3; 4] {
        match self {
            Self::Top => [
                Self::VERTICES[2],
                Self::VERTICES[6],
                Self::VERTICES[7],
                Self::VERTICES[3],
            ],
            Self::Bottom => [
                Self::VERTICES[1],
                Self::VERTICES[5],
                Self::VERTICES[4],
                Self::VERTICES[0],
            ],
            Self::Left => [
                Self::VERTICES[4],
                Self::VERTICES[6],
                Self::VERTICES[2],
                Self::VERTICES[0],
            ],
            Self::Right => [
                Self::VERTICES[1],
                Self::VERTICES[3],
                Self::VERTICES[7],
                Self::VERTICES[5],
            ],
            Self::Front => [
                Self::VERTICES[1],
                Self::VERTICES[0],
                Self::VERTICES[2],
                Self::VERTICES[3],
            ],
            Self::Back => [
                Self::VERTICES[5],
                Self::VERTICES[7],
                Self::VERTICES[6],
                Self::VERTICES[4],
            ],
        }
    }

    #[must_use]
    pub const fn as_uv(self) -> [Vec2; 4] {
        match self {
            Self::Top | Self::Front => [
                vec2(0.0, 0.0),
                vec2(1.0, 0.0),
                vec2(1.0, 1.0),
                vec2(0.0, 1.0),
            ],
            Self::Bottom => [
                vec2(0.0, 1.0),
                vec2(1.0, 1.0),
                vec2(1.0, 0.0),
                vec2(0.0, 0.0),
            ],
            Self::Right => [
                vec2(1.0, 0.0),
                vec2(1.0, 1.0),
                vec2(0.0, 1.0),
                vec2(0.0, 0.0),
            ],
            Self::Left | Self::Back => [
                vec2(0.0, 0.0),
                vec2(0.0, 1.0),
                vec2(1.0, 1.0),
                vec2(1.0, 0.0),
            ],
        }
    }

    #[must_use]
    pub const fn as_normal(self) -> IVec3 {
        match self {
            Self::Top => IVec3::Y,
            Self::Bottom => IVec3::NEG_Y,
            Self::Right => IVec3::X,
            Self::Left => IVec3::NEG_X,
            Self::Front => IVec3::Z,
            Self::Back => IVec3::NEG_Z,
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

    #[must_use]
    pub const fn add_position(self, mut position: U16Vec3) -> U16Vec3 {
        match self {
            Self::Top => position.y += 1,
            Self::Bottom => position.y = position.y.saturating_sub(1),
            Self::Right => position.x += 1,
            Self::Left => position.x = position.x.saturating_sub(1),
            Self::Front => position.z += 1,
            Self::Back => position.z = position.z.saturating_sub(1),
        }

        position
    }
}

#[cfg(test)]
mod tests {
    use std::array;

    use super::Face;

    #[test]
    fn test_face_corners() {
        for face in Face::ALL {
            println!("{:#?}", face.as_vertice_corners());
        }
    }

    #[test]
    fn test_uh() {
        let top = Face::Top.as_vertices();
        let bottom = Face::Bottom.as_vertices();

        println!(
            "{top:?} - {bottom:?} = {:?}",
            array::from_fn::<_, 4, _>(|i| top[i] - bottom[i])
        );
    }
}
