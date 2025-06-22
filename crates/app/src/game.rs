use std::{
    fs::DirEntry,
    path::{Path, PathBuf},
};

use ahash::HashMap;
use glam::{IVec2, Vec2, Vec3};
use image::RgbaImage;
use meralus_world::Face;

use crate::{BakedBlockModelLoader, Block, BlockManager, TextureLoader, renderers::Voxel};

pub struct ResourceManager {
    textures: TextureLoader,
    blocks: BlockManager,
    pub models: BakedBlockModelLoader,
    root: PathBuf,
}

pub type WorldMesh = HashMap<(IVec2, Face), [Vec<Voxel>; 2]>;

pub struct Player {
    pub position: Vec3,
    pub nickname: String,
    pub is_me: bool,
}

impl ResourceManager {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            textures: TextureLoader::new(),
            blocks: BlockManager::new(),
            models: BakedBlockModelLoader::default(),
            root: root.into(),
        }
    }

    // pub fn add_player(&mut self, player: Player) {
    //     self.players.push(player);
    // }

    // pub fn players(&self) -> &[Player] {
    //     &self.players
    // }
    
    pub fn register_block<T: Block + 'static>(&mut self, block: T) {
        let id = block.id();

        self.load_block(self.root.join("models").join(id).with_extension("json"));

        self.blocks.register(&block);
    }

    pub fn load_block<P: AsRef<Path>>(&mut self, path: P) {
        self.models
            .load(&mut self.textures, &self.root, path)
            .unwrap();
    }

    pub fn load_buitlin_blocks(&mut self) {
        if let Ok(root) = self.root.join("models").read_dir()
            && let Ok(mut root) = root.collect::<Result<Vec<_>, _>>()
        {
            root.sort_by_key(DirEntry::file_name);

            for entry in root {
                if entry.metadata().is_ok_and(|metadata| metadata.is_file())
                    && !entry.file_name().to_string_lossy().starts_with("cuboid")
                {
                    self.models
                        .load(&mut self.textures, &self.root, entry.path())
                        .unwrap();
                }
            }
        }
    }

    pub fn generate_mipmaps(&mut self, level: usize) {
        self.textures.generate_mipmaps(level);
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

    pub fn get_texture_count(&self) -> usize {
        self.textures.get_texture_count()
    }

    pub fn get_texture<I: AsRef<str>>(&self, name: I) -> Option<(Vec2, Vec2, u8)> {
        self.textures.get_texture(name.as_ref())
    }
}
