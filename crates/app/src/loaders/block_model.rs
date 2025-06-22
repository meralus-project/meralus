use std::path::Path;

use ahash::HashMap;
use glam::{Vec2, Vec3};
use meralus_shared::Cube3D;
use meralus_world::{
    ChunkManager, ElementRotation, Face, Faces, JsonError, TexturePath, TextureRef,
};
use owo_colors::OwoColorize;

use super::{LoadingResult, block::BlockManager, texture::TextureLoader};
use crate::loaders::LoadingError;

#[derive(Debug, PartialEq)]
pub struct FaceUV {
    pub offset: Vec2,
    pub scale: Vec2,
}

#[derive(Debug)]
pub struct BlockModelFace {
    pub texture_id: usize,
    pub face: Face,
    pub cull_face: Option<Face>,
    pub tint: bool,
    pub uv: FaceUV,
    pub is_opaque: bool,
}

impl BlockModelFace {
    pub fn culled(&self, chunk_manager: &ChunkManager, position: Vec3) -> bool {
        self.cull_face.is_some_and(|cull_face| {
            chunk_manager.contains_block(position + cull_face.as_normal().as_vec3())
        })
    }
}

#[derive(Debug)]
pub struct BlockModelElement {
    pub cube: Cube3D,
    pub rotation: Option<ElementRotation>,
    pub faces: [Option<BlockModelFace>; 6],
}

#[derive(Debug)]
pub struct BakedBlockModel {
    pub name: String,
    pub bounding_box: Cube3D,
    pub ambient_occlusion: bool,
    pub elements: Vec<BlockModelElement>,
}

const ERROR: [f32; 3] = [0.00001; 3];

impl BakedBlockModel {
    pub fn is_opaque(&self) -> bool {
        self.elements.iter().any(|BlockModelElement { cube, .. }| {
            (cube.size.to_raw() - Vec3::ONE).abs().to_array() < ERROR
        })
    }
}

#[derive(Debug, Default)]
pub struct BakedBlockModelLoader {
    models: Vec<BakedBlockModel>,
}

fn get_texture<T: AsRef<str>>(
    textures: &HashMap<String, TextureRef>,
    name: T,
) -> Option<&TexturePath> {
    textures
        .get(name.as_ref())
        .and_then(|texture_ref| match texture_ref {
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

impl BakedBlockModelLoader {
    #[allow(clippy::missing_const_for_fn)] // for MSRV compatibility
    pub fn count(&self) -> usize {
        self.models.len()
    }

    pub fn get(&self, value: usize) -> Option<&BakedBlockModel> {
        self.models.get(value)
    }

    /// # Errors
    ///
    /// An error will be returned if the passed path does not contain a filename
    /// or an error occurred while loading the block model (see
    /// [`BlockManager::load`]).
    pub fn load<P: AsRef<Path>, R: AsRef<Path>>(
        &mut self,
        textures: &mut TextureLoader,
        root: R,
        path: P,
    ) -> LoadingResult<&BakedBlockModel> {
        let path = path.as_ref();

        println!(
            "[{:18}] Loading model at {}",
            "INFO/ModelLoader".bright_green(),
            path.display().bright_blue().bold()
        );

        let name = path
            .file_stem()
            .ok_or(LoadingError::Model(ModelLoadingError::InvalidPath))?
            .to_string_lossy();

        let block = BlockManager::load(textures, root.as_ref(), path)?;

        let mut bounding_box: Option<Cube3D> = None;

        let elements = block
            .elements
            .into_iter()
            .map(|element| {
                let cube = Cube3D::new(element.start.into(), (element.end - element.start).into());

                if element.rotation.is_none() {
                    if let Some(bounding_box) = &mut bounding_box {
                        bounding_box.origin = bounding_box.origin.min(cube.origin);
                        bounding_box.size = bounding_box.size.max(cube.size);
                    } else {
                        bounding_box.replace(cube);
                    }
                }

                BlockModelElement {
                    cube,
                    rotation: element.rotation,
                    faces: match element.faces {
                        Faces::All(data) => Face::ALL.map(|face| {
                            let texture = get_texture(&block.textures, &data.texture).unwrap();
                            let (offset, scale, alpha) = textures
                                .get_texture(texture.1.file_stem().unwrap().to_string_lossy())
                                .unwrap();

                            let uv = if let Some([start, end]) = data.uv {
                                FaceUV {
                                    offset: offset + start,
                                    scale: scale * end,
                                }
                            } else {
                                FaceUV { offset, scale }
                            };

                            Some(BlockModelFace {
                                texture_id: 0,
                                face,
                                cull_face: data.cull_face,
                                uv,
                                tint: data.tint,
                                is_opaque: alpha == 255,
                            })
                        }),
                        Faces::Unique(face_map) => {
                            let mut faces = [const { None }; 6];

                            for (face, data) in face_map {
                                let texture = get_texture(&block.textures, &data.texture).unwrap();
                                let (offset, scale, alpha) = textures
                                    .get_texture(texture.1.file_stem().unwrap().to_string_lossy())
                                    .unwrap();

                                let uv = if let Some([start, end]) = data.uv {
                                    FaceUV {
                                        offset: offset + start,
                                        scale: scale * end,
                                    }
                                } else {
                                    FaceUV { offset, scale }
                                };

                                faces[face.normal_index()] = Some(BlockModelFace {
                                    texture_id: 0,
                                    face,
                                    cull_face: data.cull_face,
                                    uv,
                                    tint: data.tint,
                                    is_opaque: alpha == 255,
                                });
                            }

                            faces
                        }
                    },
                }
            })
            .collect();

        self.models.push(BakedBlockModel {
            name: name.to_string(),
            ambient_occlusion: block.ambient_occlusion,
            elements,
            bounding_box: bounding_box.unwrap_or(Cube3D::ONE),
        });

        Ok(self.models.last().unwrap())
    }
}
