mod block;
mod block_model;
mod block_states;
mod entity;
mod texture;

use std::{
    fs::{self, DirEntry},
    ops::Not,
    path::{Path, PathBuf, absolute},
};

use ahash::HashMap;
// use glam::{IPoint3D, Vec2, Vec3};
use image::RgbaImage;
use meralus_shared::{Point2D, Vector2D};
use meralus_world::BlockSource;

// use owo_colors::OwoColorize;
pub use self::{
    block::{Block, BlockData, BlockStorage},
    block_model::*,
    entity::EntityModelStorage,
    texture::{TextureLoadingError, TextureStorage},
};
// use crate::world::{EntityData, EntityManager};

pub type LoadingResult<T> = Result<T, LoadingError>;

#[derive(Debug)]
pub enum LoadingError {
    Texture(TextureLoadingError),
    Model(ModelLoadingError),
}

pub struct TexturePackInfo {
    pub name: String,
    pub description: String,
}

pub struct TexturePack {
    pub info: TexturePackInfo,
    pub textures: TextureStorage,
}

pub type Mappings = HashMap<String, PathBuf>;

pub struct ResourceStorage {
    pub textures: TextureStorage,
    pub blocks: BlockStorage,
    pub models: BakedBlockModelStorage,
    pub entity_models: EntityModelStorage,
    pub mappings: Mappings,
}

impl ResourceStorage {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let mut mappings = HashMap::default();

        mappings.insert(String::from("game"), absolute(root.into()).unwrap());

        Self {
            textures: TextureStorage::new(),
            blocks: BlockStorage::new(),
            models: BakedBlockModelStorage::default(),
            entity_models: EntityModelStorage::default(),
            mappings,
        }
    }

    pub fn register_block<T: Block + 'static>(&mut self, mapping: &str, block: T) {
        let id = block.id();

        if let Some(path) = self.mappings.get(mapping) {
            let path = path.join("models").join(id).with_extension("json");

            self.models.load(&mut self.textures, &self.mappings, path).unwrap();
        }

        self.blocks.register(block);
    }

    pub fn load_entity_model<T: AsRef<str>>(&mut self, mapping: &str, id: T) -> usize {
        let entity_id = self.entity_models.count();

        if let Some(path) = self.mappings.get(mapping) {
            let path = path.join("entity_models").join(id.as_ref()).with_extension("json");

            self.entity_models.load(&mut self.textures, &self.mappings, path).unwrap();
        }

        entity_id
    }

    pub fn load_buitlin_blocks(&mut self) {
        if let Some(path) = self.mappings.get("game")
            && let Ok(root) = path.join("models").read_dir()
            && let Ok(mut root) = root.collect::<Result<Vec<_>, _>>()
        {
            root.sort_by_key(DirEntry::file_name);

            for entry in root {
                if entry.metadata().is_ok_and(|metadata| metadata.is_file()) && !entry.file_name().to_string_lossy().starts_with("cuboid") {
                    self.models.load(&mut self.textures, &self.mappings, entry.path()).unwrap();
                }
            }
        }
    }

    pub fn generate_mipmaps(&mut self, level: usize) {
        self.textures.generate_mipmaps(level);
    }

    pub fn generate_mipmap(&mut self, level: usize) {
        self.textures.generate_mipmap(level);
    }

    pub fn load_texture<P: AsRef<Path>>(&mut self, path: P) {
        self.textures.load(path).unwrap();
    }

    pub fn get_texture_atlas(&self) -> &RgbaImage {
        self.textures.get_atlas()
    }

    pub fn get_mipmaps(&self) -> &[RgbaImage] {
        self.textures.get_mipmaps()
    }

    pub fn get_lightmap_mipmaps(&self) -> &[RgbaImage] {
        self.textures.get_lightmap_mipmaps()
    }

    pub fn get_texture_count(&self) -> usize {
        self.textures.get_texture_count()
    }

    pub fn get_texture<I: AsRef<str>>(&self, name: I) -> Option<(Point2D, Vector2D, u8)> {
        self.textures.get_texture(name.as_ref())
    }

    pub fn get_block(&self, id: usize) -> Option<&dyn Block> {
        self.blocks.get(id)
    }

    pub fn debug_save(&self) {
        let atlas = self.get_mipmaps();

        // println!(
        //     "[{:18}] Saving atlas ({} packed textures) with {} mipmap levels...",
        //     "INFO/AtlasManager".bright_green(),
        //     self.get_texture_count().bright_blue(),
        //     (atlas.len() - 1).bright_blue()
        // );

        if fs::exists("debug").is_ok_and(Not::not)
            && let Err(_error) = fs::create_dir("debug")
        {
            // println!("[{:18}] Failed to create debug directory: {error}", "
            // ERR/AtlasManager".bright_red());

            return;
        }

        for (level, image) in atlas.iter().enumerate() {
            let (_width, _height) = image.dimensions();

            if let Err(_error) = image.save(format!("debug/atlas_{level}.png")) {
                // println!(
                //     "[{:18}] Failed to save atlas (mipmap level: {}, size:
                // {}): {error}",     " ERR/AtlasManager".
                // bright_red(),     level.to_string().
                // bright_blue(),     format!("{width}x{height}"
                // ).bright_blue() );
            } else {
                // println!(
                //     "[{:18}] Successfully saved atlas (mipmap level: {},
                // size: {})",     "INFO/AtlasManager".
                // bright_green(),     level.to_string().
                // bright_blue(),     format!("{width}x{height}"
                // ).bright_blue() );
            }
        }

        let lightmap_atlas = self.get_lightmap_mipmaps();

        // println!(
        //     "[{:18}] Saving lightmap atlas ({} packed textures) with {} mipmap
        // levels...",     "INFO/AtlasManager".bright_green(),
        //     self.get_texture_count().bright_blue(),
        //     (lightmap_atlas.len() - 1).bright_blue()
        // );

        for (level, image) in lightmap_atlas.iter().enumerate() {
            let (_width, _height) = image.dimensions();

            if let Err(_error) = image.save(format!("debug/lightmap_atlas_{level}.png")) {
                // println!(
                //     "[{:18}] Failed to save atlas (mipmap level: {}, size:
                // {}): {error}",     " ERR/AtlasManager".
                // bright_red(),     level.to_string().
                // bright_blue(),     format!("{width}x{height}"
                // ).bright_blue() );
            } else {
                // println!(
                //     "[{:18}] Successfully saved atlas (mipmap level: {},
                // size: {})",     "INFO/AtlasManager".
                // bright_green(),     level.to_string().
                // bright_blue(),     format!("{width}x{height}"
                // ).bright_blue() );
            }
        }
    }
}

impl BlockSource for ResourceStorage {
    fn get_block_id(&self, name: &str) -> u8 {
        self.blocks.get_by_name(name) as u8
    }

    fn blocks_light(&self, block: u8) -> bool {
        unsafe { self.blocks.get(block as usize).unwrap_unchecked() }.blocks_light()
    }

    fn light_consumption(&self, block: u8) -> u8 {
        unsafe { self.blocks.get(block as usize).unwrap_unchecked() }.consume_light_level()
    }
}
