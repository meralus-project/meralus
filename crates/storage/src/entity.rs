use std::{fs, path::Path};

use meralus_physics::Aabb;
use meralus_shared::{DPoint3D, Point2D, Vector2D};
use meralus_world::{EntityElementData, EntityModel, Face, TexturePath, TextureRef, vec_to_boxed_array};
use tracing::info;

use crate::{FaceData, FaceUV, LoadingError, Mappings, ModelLoadingError};
// use owo_colors::OwoColorize;
use crate::{LoadingResult, texture::TextureStorage};

#[derive(Debug)]
pub struct BakedEntityModelElement {
    pub cube: Aabb,
    pub faces: Box<[FaceData; 6]>,
}

#[derive(Debug)]
pub struct BakedEntityModel {
    pub name: String,
    pub bounding_box: Aabb,
    pub elements: Vec<BakedEntityModelElement>,
}

#[derive(Debug, Default)]
pub struct EntityModelStorage {
    models: Vec<BakedEntityModel>,
}

impl EntityModelStorage {
    pub const fn count(&self) -> usize {
        self.models.len()
    }

    pub fn get(&self, value: usize) -> Option<&BakedEntityModel> {
        self.models.get(value)
    }

    pub fn get_unchecked(&self, value: usize) -> &BakedEntityModel {
        unsafe { self.models.get_unchecked(value) }
    }

    /// # Errors
    ///
    /// An error will be returned if the passed path does not contain a filename
    /// or an error occurred while loading the block model (see
    /// [`BlockManager::load`]).
    pub fn load<P: AsRef<Path>>(&mut self, textures: &mut TextureStorage, root: &Mappings, path: P) -> LoadingResult<&BakedEntityModel> {
        let path = path.as_ref();

        info!(
            target: "model-loader",
            "Loading entity model at {}",
            path.display()
        );

        let name = path.file_stem().ok_or(LoadingError::Model(ModelLoadingError::InvalidPath))?.to_string_lossy();

        let path = path.with_extension("json");
        let data = fs::read(&path).map_err(|_| LoadingError::Model(ModelLoadingError::NotFound))?;
        let block = EntityModel::from_slice(&data).map_err(|err| LoadingError::Model(ModelLoadingError::ParsingFailed(err)))?;

        if let TextureRef::Path(TexturePath(mod_name, path)) = &block.texture.path
            && let Some(root) = root.get(mod_name)
            && let Some(regular_offset) = textures.load(root.join("textures").join(path).with_extension("png"))?
        {
            _ = textures.load_lightmap(regular_offset, root.join("lightmaps").join(path).with_extension("png"));
        }

        let mut bounding_box: Option<Aabb> = None;

        let elements: Vec<BakedEntityModelElement> = block
            .elements
            .into_iter()
            .map(|element| {
                let EntityElementData::Cube { start, end, mut faces } = element.data;
                let min = start / 48.0;
                let max = end / 48.0;
                let cube = Aabb::new(min.as_(), max.as_());

                if let Some(bounding_box) = &mut bounding_box {
                    bounding_box.min = bounding_box.min.min(min.as_());
                    bounding_box.max = bounding_box.max.max(max.as_());
                } else {
                    bounding_box.replace(Aabb::new(min.as_(), max.as_()));
                }

                BakedEntityModelElement {
                    cube,
                    faces: vec_to_boxed_array(
                        Face::ALL
                            .into_iter()
                            .map(|face| {
                                let data = faces.remove(&face).unwrap();
                                let (offset, ..) = if let TextureRef::Path(path) = &block.texture.path {
                                    textures.get_texture(path.1.file_stem().unwrap().to_string_lossy()).unwrap()
                                } else {
                                    (Point2D::ZERO, Vector2D::ZERO, 0)
                                };

                                let uv = FaceUV {
                                    offset: offset + data.from / f32::from(TextureStorage::ATLAS_SIZE),
                                    scale: ((data.to - data.from) / f32::from(TextureStorage::ATLAS_SIZE)),
                                };

                                FaceData::new(face, cube, uv, None)
                            })
                            .collect::<Vec<_>>(),
                    ),
                }
            })
            .collect();

        self.models.push(BakedEntityModel {
            name: name.to_string(),
            elements,
            bounding_box: bounding_box.unwrap_or(const { Aabb::new(DPoint3D::ZERO, DPoint3D::ONE) }),
        });

        Ok(self.models.last().unwrap())
    }

    pub fn get_aabb(&self, block_id: u8) -> Option<Aabb> {
        self.get(block_id.into()).map(|element| element.bounding_box)
    }
}
