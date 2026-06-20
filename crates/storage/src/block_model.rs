use std::path::Path;

use ahash::HashMap;
use meralus_io::{BlockFace, Faces, JsonError, TexturePath, TextureRef};
use meralus_physics::Aabb;
use meralus_shared::{Axis, DPoint3D, Face, IPoint3D, Point2D, Point3D, Transform3D, Vector2D, Vector3D};

use crate::{LoadingError, LoadingResult, Mappings, block::BlockStorage, texture::TextureStorage};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Corner {
    LeftTop,
    RightTop,
    LeftBottom,
    RightBottom,
}

impl Corner {
    #[inline]
    pub const fn index(&self) -> usize {
        match self {
            Self::LeftTop => 0,
            Self::RightTop => 1,
            Self::LeftBottom => 2,
            Self::RightBottom => 3,
        }
    }

    #[inline]
    pub const fn from_array([x, y]: [f32; 2]) -> Self {
        let [x, y] = [x > 0.0, y > 0.0];

        match [x, y] {
            [true, true] => Self::RightTop,
            [true, false] => Self::RightBottom,
            [false, true] => Self::LeftTop,
            [false, false] => Self::LeftBottom,
        }
    }

    #[inline]
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaceUV {
    pub offset: Point2D,
    pub scale: Vector2D,
}

#[derive(Debug)]
pub struct FaceData {
    pub face: Face,
    pub normal: IPoint3D,
    pub vertices: [Point3D; 4],
    pub corners: [[IPoint3D; 3]; 4],
    pub uvs: [Point2D; 4],
}

#[inline]
const fn as_vertex_corners(face: Face) -> [Corner; 4] {
    let vertices = face.as_vertices();

    [
        Corner::from_vec(face, vertices[0]),
        Corner::from_vec(face, vertices[1]),
        Corner::from_vec(face, vertices[2]),
        Corner::from_vec(face, vertices[3]),
    ]
}

impl FaceData {
    pub fn new(face: Face, aabb: Aabb, uv: FaceUV, rotation: Option<&(Transform3D, Point3D, Point3D)>) -> Self {
        let mut vertices = face.as_vertices();

        let aabb_size = aabb.size().as_vec3();

        for vertex in &mut vertices {
            let vert = aabb.min.as_vec3() + *vertex * aabb_size;

            *vertex = if let Some((matrix, origin, scale)) = rotation {
                let point = matrix.transform_point3(vert - origin);

                Point3D::new(point.x * scale.x, point.y * scale.y, point.z * scale.z) + origin
            } else {
                vert
            };
        }

        // match self {
        //     Self::Top => [X0Y1Z0, X1Y1Z0, X0Y1Z1, X1Y1Z1],
        //     Self::Bottom => [X1Y0Z0, X0Y0Z0, X1Y0Z1, X0Y0Z1],
        //     Self::Left => [X0Y0Z1, X0Y0Z0, X0Y1Z1, X0Y1Z0],
        //     Self::Right => [X1Y0Z0, X1Y0Z1, X1Y1Z0, X1Y1Z1],

        //     >>Self::Front => [X1Y0Z1, X0Y0Z1, X1Y1Z1, X0Y1Z1],

        //     Self::Back => [X0Y0Z0, X1Y0Z0, X0Y1Z0, X1Y1Z0],
        // }

        Self {
            face,
            normal: face.as_normal(),
            vertices,
            corners: as_vertex_corners(face).map(|corner| corner.get_neighbours(face)),
            uvs: [Point2D::X, Point2D::ZERO, Point2D::ONE, Point2D::Y].map(|face_uv| {
                let face_uv = Point2D::new(face_uv.x * uv.scale.x, face_uv.y * uv.scale.y);

                uv.offset + face_uv
            }),
        }
    }
}

#[derive(Debug)]
pub struct BlockModelFace {
    pub texture_id: usize,
    pub face_data: FaceData,
    pub cull_face: Option<(usize, IPoint3D, Face, usize)>,
    pub tint: bool,
    pub uv: FaceUV,
    pub is_opaque: bool,
}

impl BlockModelFace {
    fn new(
        texture_storage: &TextureStorage,
        textures: &HashMap<String, TextureRef>,
        aabb: Aabb,
        rotation: Option<&(Transform3D, Point3D, Point3D)>,
        data: &BlockFace,
        face: Face,
    ) -> Self {
        let texture = get_texture(textures, &data.texture).unwrap();
        let (offset, scale, alpha) = texture_storage.get_texture(texture.1.file_stem().unwrap().to_string_lossy()).unwrap();

        let uv = if let Some([start, end]) = data.uv {
            FaceUV {
                offset: offset + start / f32::from(TextureStorage::ATLAS_SIZE),
                scale: ((end - start) / f32::from(TextureStorage::ATLAS_SIZE)),
            }
        } else {
            FaceUV { offset, scale }
        };

        Self {
            texture_id: 0,
            face_data: FaceData::new(face, aabb, uv, rotation),
            cull_face: data
                .cull_face
                .map(|face| (face.normal_index(), face.as_normal(), face, face.opposite_normal_index())),
            uv,
            tint: data.tint,
            is_opaque: alpha == 255,
        }
    }
}

#[derive(Debug)]
pub struct BlockModelElement {
    pub cube: Aabb,
    pub faces: Vec<BlockModelFace>,
}

#[derive(Debug)]
pub struct BakedBlockModel {
    pub name: String,
    pub bounding_box: Aabb,
    pub ambient_occlusion: bool,
    pub elements: Vec<BlockModelElement>,
    pub is_opaque: bool,
}

impl BakedBlockModel {
    pub fn is_opaque(&self, opposite_face: usize) -> bool {
        self.is_opaque
            && self
                .elements
                .iter()
                .any(|element| element.faces.get(opposite_face).as_ref().is_some_and(|face| face.is_opaque))
    }
}

const ERROR: [f32; 3] = [0.00001; 3];

#[derive(Debug, Default)]
pub struct BakedBlockModelStorage {
    models: Vec<BakedBlockModel>,
}

fn get_texture<T: AsRef<str>>(textures: &HashMap<String, TextureRef>, name: T) -> Option<&TexturePath> {
    textures.get(name.as_ref()).and_then(|texture_ref| match texture_ref {
        TextureRef::Id(id) => get_texture(textures, &id.0),
        TextureRef::Path(path) => Some(path),
    })
}

#[derive(Debug)]
pub enum ModelLoadingError {
    InvalidPath,
    NotFound,
    ParsingFailed(JsonError),
}

impl BakedBlockModelStorage {
    pub const fn count(&self) -> usize {
        self.models.len()
    }

    pub fn get(&self, value: usize) -> Option<&BakedBlockModel> {
        self.models.get(value)
    }

    pub fn get_unchecked(&self, value: usize) -> &BakedBlockModel {
        unsafe { self.models.get_unchecked(value) }
    }

    /// # Errors
    ///
    /// An error will be returned if the passed path does not contain a filename
    /// or an error occurred while loading the block model (see
    /// [`BlockManager::load`]).
    pub fn load<P: AsRef<Path>>(&mut self, textures: &mut TextureStorage, root: &Mappings, path: P) -> LoadingResult<usize> {
        let path = path.as_ref();

        // println!(
        //     "[{:18}] Loading model at {}",
        //     "INFO/ModelLoader".bright_green(),
        //     path.display().bright_blue().bold()
        // );

        let name = path.file_stem().ok_or(LoadingError::Model(ModelLoadingError::InvalidPath))?.to_string_lossy();
        let block = BlockStorage::load(textures, root, path)?;
        let mut bounding_box: Option<Aabb> = None;

        let elements: Vec<BlockModelElement> = block
            .elements
            .into_iter()
            .map(|element| {
                let cube = Aabb::new(element.start.as_dvec3(), element.end.as_dvec3());

                if element.rotation.is_none() {
                    if let Some(bounding_box) = &mut bounding_box {
                        bounding_box.min = bounding_box.min.min(cube.min);
                        bounding_box.max = bounding_box.max.max(cube.max);
                    } else {
                        bounding_box.replace(cube);
                    }
                }

                let rotation = element.rotation.map(|rotation| {
                    let angle = rotation.angle.to_radians();
                    let scale = Point3D::ONE;
                    let matrix = match rotation.axis {
                        Axis::X => Transform3D::from_rotation_x(angle),
                        Axis::Y => Transform3D::from_rotation_y(angle),
                        Axis::Z => Transform3D::from_rotation_z(angle),
                    };

                    (matrix, rotation.origin, scale)
                });

                BlockModelElement {
                    cube,
                    faces: match element.faces {
                        Faces::All(data) => Face::ALL
                            .into_iter()
                            .map(|face| BlockModelFace::new(textures, &block.textures, cube, rotation.as_ref(), &data, face))
                            .collect(),
                        Faces::Unique(face_map) => {
                            let mut face_map = face_map
                                .into_iter()
                                .map(|(face, data)| BlockModelFace::new(textures, &block.textures, cube, rotation.as_ref(), &data, face))
                                .collect::<Vec<_>>();

                            face_map.sort_by_key(|face| face.face_data.face.normal_index());

                            face_map
                        }
                    },
                }
            })
            .collect();

        let is_opaque = elements
            .iter()
            .any(|element| (element.cube.size().as_vec3() - Vector3D::ONE).abs().to_array() < ERROR);

        let index = self.models.len();

        self.models.push(BakedBlockModel {
            name: name.to_string(),
            ambient_occlusion: block.ambient_occlusion.unwrap_or(true),
            elements,
            bounding_box: bounding_box.unwrap_or(const { Aabb::new(DPoint3D::ZERO, DPoint3D::ONE) }),
            is_opaque,
        });

        Ok(index)
    }
}
