use std::{fs, path::Path};

use ahash::HashMap;
use meralus_shared::Color;
use meralus_world::{BlockModel, Property, TexturePath, TextureRef};

use crate::{LoadingError, LoadingResult, Mappings, ModelLoadingError, texture::TextureStorage};

pub trait Block: Send + Sync {
    fn id(&self) -> &'static str;

    fn tint_color(&self) -> Option<Color> {
        None
    }

    fn cull_if_same(&self) -> bool {
        false
    }

    fn blocks_light(&self) -> bool {
        true
    }

    fn consume_light_level(&self) -> u8 {
        0
    }

    fn light_level(&self) -> u8 {
        0
    }

    fn droppable(&self) -> bool {
        true
    }

    fn collidable(&self) -> bool {
        true
    }

    fn selectable(&self) -> bool {
        true
    }

    fn get_properties(&self) -> Vec<Property> {
        Vec::new()
    }
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct BlockData {
    pub id: &'static str,
    pub cull_if_same: bool,
    pub blocks_light: bool,
    pub consume_light_level: u8,
    pub light_level: u8,
    pub droppable: bool,
    pub tint_color: Option<Color>,
    pub collidable: bool,
    pub selectable: bool,
    pub properties: Vec<Property>,
}

impl Block for BlockData {
    fn id(&self) -> &'static str {
        self.id
    }

    fn tint_color(&self) -> Option<Color> {
        self.tint_color
    }

    fn cull_if_same(&self) -> bool {
        self.cull_if_same
    }

    fn blocks_light(&self) -> bool {
        self.blocks_light
    }

    fn consume_light_level(&self) -> u8 {
        self.consume_light_level
    }

    fn light_level(&self) -> u8 {
        self.light_level
    }

    fn droppable(&self) -> bool {
        self.droppable
    }

    fn collidable(&self) -> bool {
        self.collidable
    }

    fn selectable(&self) -> bool {
        self.selectable
    }

    fn get_properties(&self) -> Vec<Property> {
        self.properties.clone()
    }
}

pub struct BlockStorage {
    id_to_block: HashMap<&'static str, usize>,
    blocks: Vec<Box<dyn Block>>,
}

impl Default for BlockStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl BlockStorage {
    pub fn new() -> Self {
        Self {
            id_to_block: HashMap::default(),
            blocks: Vec::new(),
        }
    }

    pub fn get(&self, id: usize) -> Option<&dyn Block> {
        self.blocks.get(id).map(|block| block.as_ref())
    }

    pub fn get_unchecked(&self, id: usize) -> &dyn Block {
        unsafe { self.blocks.get_unchecked(id).as_ref() }
    }

    pub fn get_by_name(&self, name: &str) -> usize {
        self.id_to_block[&name]
    }

    pub fn register<T: Block + 'static>(&mut self, block: T) {
        self.id_to_block.insert(block.id(), self.blocks.len());
        self.blocks.push(Box::new(block));
    }

    fn load_block<P: AsRef<Path>>(root: &Mappings, path: P) -> LoadingResult<BlockModel> {
        let path = path.as_ref().with_extension("json");
        let data = fs::read(&path).map_err(|_| LoadingError::Model(ModelLoadingError::NotFound))?;
        let block = BlockModel::from_slice(&data).map_err(|err| LoadingError::Model(ModelLoadingError::ParsingFailed(err)))?;

        let block = if let Some(parent) = block.parent.as_ref()
            && let Some(mapping) = root.get(&parent.0)
        {
            let mut parent_block = Self::load_block(root, mapping.join("models").join(&parent.1))?;

            parent_block.ambient_occlusion = block.ambient_occlusion;
            parent_block.textures.extend(block.textures);
            parent_block.elements.extend(block.elements);

            parent_block
        } else {
            block
        };

        Ok(block)
    }

    /// # Errors
    ///
    /// An error will be returned if:
    /// - The passed path does not contain a filename.
    /// - The passed path cannot be read.
    /// - The passed path data cannot be successfully parsed.
    /// - An error occurred while loading some texture (see
    ///   [`TextureLoader::load`]).
    pub fn load<P: AsRef<Path>>(textures: &mut TextureStorage, root: &Mappings, path: P) -> LoadingResult<BlockModel> {
        let block = Self::load_block(root, path)?;

        for texture_ref in block.textures.values() {
            if let TextureRef::Path(TexturePath(mod_name, path)) = texture_ref
                && let Some(root) = root.get(mod_name)
                && let Some(regular_offset) = textures.load(root.join("textures").join(path).with_extension("png"))?
            {
                _ = textures.load_lightmap(regular_offset, root.join("lightmaps").join(path).with_extension("png"));
            }
        }

        Ok(block)
    }
}
