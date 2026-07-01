use core::fmt;

use crate::Axis;

#[allow(clippy::unsafe_derive_deserialize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
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

impl Face {
    pub const ALL: [Self; 6] = [Self::Bottom, Self::Top, Self::Left, Self::Right, Self::Front, Self::Back];
    #[cfg(feature = "geometry")]
    pub const BOOL_VERTICES: [glam::BVec3; 8] = [
        glam::BVec3::new(false, false, true),  // 0 LEFT  BOTTOM FRONT
        glam::BVec3::new(true, false, true),   // 1 RIGHT BOTTOM FRONT
        glam::BVec3::new(false, true, true),   // 2 LEFT  TOP    FRONT
        glam::BVec3::new(true, true, true),    // 3 RIGHT TOP    FRONT
        glam::BVec3::new(false, false, false), // 4 LEFT  BOTTOM BACK
        glam::BVec3::new(true, false, false),  // 5 RIGHT BOTTOM BACK
        glam::BVec3::new(false, true, false),  // 6 LEFT  TOP    BACK
        glam::BVec3::new(true, true, false),   // 7 RIGHT TOP    BACK
    ];
    #[cfg(feature = "geometry")]
    pub const NORMALS: [glam::IVec3; 6] = [
        glam::IVec3::NEG_Y,
        glam::IVec3::Y,
        glam::IVec3::NEG_X,
        glam::IVec3::X,
        glam::IVec3::Z,
        glam::IVec3::NEG_Z,
    ];

    #[inline]
    pub const fn get_light_level(self) -> f32 {
        const LIGHT_LEVEL: [f32; 6] = [0.5, 1.0, 0.6, 0.6, 0.8, 0.8];

        LIGHT_LEVEL[self.normal_index()]
    }

    #[must_use]
    #[inline]
    pub const fn opposite_normal_index(self) -> usize {
        const OPPOSITE: [usize; 6] = [
            Face::Top.normal_index(),
            Face::Bottom.normal_index(),
            Face::Right.normal_index(),
            Face::Left.normal_index(),
            Face::Back.normal_index(),
            Face::Front.normal_index(),
        ];

        OPPOSITE[self.normal_index()]
    }

    #[must_use]
    #[inline]
    pub const fn opposite(self) -> Self {
        const OPPOSITE: [Face; 6] = [Face::Top, Face::Bottom, Face::Right, Face::Left, Face::Back, Face::Front];

        OPPOSITE[self.normal_index()]
    }

    #[must_use]
    #[inline]
    #[cfg(feature = "geometry")]
    pub const fn get_neighbours(self) -> [glam::IVec3; 8] {
        let normal = self.as_normal();
        let axis = self.as_axis();
        let v = match axis {
            Axis::X => normal.x,
            Axis::Y => normal.y,
            Axis::Z => normal.z,
        };

        // SAFETY: IVec3 is [i32; 3]
        unsafe {
            std::mem::transmute::<[[i32; 3]; 8], [glam::IVec3; 8]>(match axis {
                Axis::X => [[v, -1, -1], [v, -1, 0], [v, -1, 1], [v, 0, -1], [v, 0, 1], [v, 1, -1], [v, 1, 0], [v, 1, 1]],
                Axis::Y => [[-1, v, -1], [-1, v, 0], [-1, v, 1], [0, v, -1], [0, v, 1], [1, v, -1], [1, v, 0], [1, v, 1]],
                Axis::Z => [[-1, -1, v], [-1, 0, v], [-1, 1, v], [0, -1, v], [0, 1, v], [1, -1, v], [1, 0, v], [1, 1, v]],
            })
        }
    }

    #[inline]
    pub const fn normal_index(self) -> usize {
        self as usize
    }

    #[must_use]
    #[inline]
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

    #[inline]
    #[cfg(feature = "geometry")]
    pub const fn as_bool_vertices(self) -> [glam::BVec3; 4] {
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
    #[inline]
    #[cfg(feature = "geometry")]
    pub const fn as_vertices(self) -> [glam::Vec3; 4] {
        const X0Y0Z1: glam::Vec3 = glam::Vec3::new(0.0, 0.0, 1.0); // LEFT  BOTTOM FRONT
        const X1Y0Z1: glam::Vec3 = glam::Vec3::new(1.0, 0.0, 1.0); // RIGHT BOTTOM FRONT
        const X0Y1Z1: glam::Vec3 = glam::Vec3::new(0.0, 1.0, 1.0); // LEFT  TOP    FRONT
        const X1Y1Z1: glam::Vec3 = glam::Vec3::new(1.0, 1.0, 1.0); // RIGHT TOP    FRONT
        const X0Y0Z0: glam::Vec3 = glam::Vec3::new(0.0, 0.0, 0.0); // LEFT  BOTTOM BACK
        const X1Y0Z0: glam::Vec3 = glam::Vec3::new(1.0, 0.0, 0.0); // RIGHT BOTTOM BACK
        const X0Y1Z0: glam::Vec3 = glam::Vec3::new(0.0, 1.0, 0.0); // LEFT  TOP    BACK
        const X1Y1Z0: glam::Vec3 = glam::Vec3::new(1.0, 1.0, 0.0); // RIGHT TOP    BACK
        const VERTICES: [[glam::Vec3; 4]; 6] = [
            [X1Y0Z0, X0Y0Z0, X1Y0Z1, X0Y0Z1], // Bottom
            [X0Y1Z0, X1Y1Z0, X0Y1Z1, X1Y1Z1], // Top
            [X0Y0Z1, X0Y0Z0, X0Y1Z1, X0Y1Z0], // Left
            [X1Y0Z0, X1Y0Z1, X1Y1Z0, X1Y1Z1], // Right
            [X1Y0Z1, X0Y0Z1, X1Y1Z1, X0Y1Z1], // Front
            [X0Y0Z0, X1Y0Z0, X0Y1Z0, X1Y1Z0], // Back
        ];

        VERTICES[self.normal_index()]
    }

    #[must_use]
    #[inline]
    #[cfg(feature = "geometry")]
    pub const fn as_uv(self) -> [glam::Vec2; 4] {
        const { [glam::Vec2::ZERO, glam::Vec2::X, glam::Vec2::Y, glam::Vec2::ONE] }
    }

    #[must_use]
    #[inline]
    #[cfg(feature = "geometry")]
    pub const fn as_normal(self) -> glam::IVec3 {
        Self::NORMALS[self.normal_index()]
    }

    #[inline]
    pub const fn is_positive(self) -> bool {
        matches!(self, Self::Top | Self::Right | Self::Front)
    }

    #[inline]
    pub const fn as_axis(self) -> Axis {
        match self {
            Self::Top | Self::Bottom => Axis::Y,
            Self::Left | Self::Right => Axis::X,
            Self::Front | Self::Back => Axis::Z,
        }
    }
}
