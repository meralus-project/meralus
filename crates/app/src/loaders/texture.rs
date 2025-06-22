use std::{io, path::Path};

use glam::Vec2;
use image::RgbaImage;
use meck::TextureAtlas;
use owo_colors::OwoColorize;

use super::LoadingResult;
use crate::loaders::LoadingError;

pub struct TextureLoader {
    atlas: TextureAtlas<String>,
}

#[derive(Debug)]
pub enum TextureLoadingError {
    InvalidPath,
    DimensionMismatch,
    Io(io::Error),
}

impl TextureLoader {
    pub const ATLAS_SIZE: u32 = 4096;

    pub fn new() -> Self {
        Self {
            atlas: TextureAtlas::new(Self::ATLAS_SIZE).with_mipmaps(4),
        }
    }

    pub fn get_texture<T: AsRef<str>>(&self, name: T) -> Option<(Vec2, Vec2, u8)> {
        self.atlas.get_texture_uv(name.as_ref())
    }

    pub fn get_atlas(&self) -> &RgbaImage {
        self.atlas.main_texture()
    }

    pub fn get_mipmaps(&self) -> &[RgbaImage] {
        self.atlas.mipmaps()
    }

    pub fn get_texture_count(&self) -> usize {
        self.atlas.textures()
    }

    pub fn generate_mipmaps(&mut self, level: usize) {
        self.atlas.generate_mipmaps(level);
    }

    /// # Errors
    ///
    /// An error will be returned if the passed path does not contain a filename
    /// or cannot be read.
    pub fn load<P: AsRef<Path>>(&mut self, path: P) -> LoadingResult<()> {
        let path = path.as_ref();

        println!(
            "[{}] Loading texture at {}",
            "INFO/TextureLoader".bright_green(),
            path.display().bright_blue().bold()
        );

        let name = path
            .file_stem()
            .ok_or(LoadingError::Texture(TextureLoadingError::InvalidPath))?;
        let name = name.to_string_lossy();
        let name = name.to_string();

        if self.atlas.contains_texture(&name) {
            return Ok(());
        }

        match image::ImageReader::open(path).and_then(image::ImageReader::with_guessed_format) {
            Ok(value) => {
                if let Ok(value) = value.decode() {
                    let image = value.to_rgba8();

                    self.atlas.append(name, &image).map_err(|_| {
                        LoadingError::Texture(TextureLoadingError::DimensionMismatch)
                    })?;
                }
            }
            Err(err) => return Err(LoadingError::Texture(TextureLoadingError::Io(err))),
        }

        Ok(())
    }
}

impl Default for TextureLoader {
    fn default() -> Self {
        Self::new()
    }
}
