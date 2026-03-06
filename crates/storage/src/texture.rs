use core::fmt;
use std::{
    io,
    path::Path,
};

use image::RgbaImage;
use meck::TextureAtlas;
use meralus_shared::{Point2D, USize2D, Vector2D};
use tracing::info;

use crate::LoadingError;
use crate::LoadingResult;

pub struct TextureStorage {
    regular_atlas: TextureAtlas<String>,
    lightmap_atlas: TextureAtlas<String>,
}

#[derive(Debug)]
pub enum TextureLoadingError {
    InvalidPath,
    Io(io::Error),
    Decode(image::error::ImageError),
}

impl fmt::Display for TextureLoadingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath => write!(f, "received invalid texture path"),
            Self::Io(error) => write!(f, "I/O error occurred while texture loading: {error}"),
            Self::Decode(error) => write!(f, "error occurred while texture image decoding: {error}"),
        }
    }
}

impl TextureStorage {
    pub const ATLAS_SIZE: u16 = 4096;

    pub fn new() -> Self {
        Self {
            regular_atlas: TextureAtlas::new(Self::ATLAS_SIZE.into()).with_mipmaps(4),
            lightmap_atlas: TextureAtlas::new(Self::ATLAS_SIZE.into()).with_mipmaps(4),
        }
    }

    pub fn contains_texture<T: AsRef<str>>(&self, name: T) -> bool {
        self.regular_atlas.contains_texture(name.as_ref())
    }

    pub fn get_texture<T: AsRef<str>>(&self, name: T) -> Option<(Point2D, Vector2D, u8)> {
        self.regular_atlas.get_texture_uv(name.as_ref())
    }

    pub fn get_lightmap<T: AsRef<str>>(&self, name: T) -> Option<(Point2D, Vector2D, u8)> {
        self.lightmap_atlas.get_texture_uv(name.as_ref())
    }

    pub fn get_atlas(&self) -> &RgbaImage {
        self.regular_atlas.main_texture()
    }

    pub fn get_mipmaps(&self) -> &[RgbaImage] {
        self.regular_atlas.mipmaps()
    }

    pub fn get_lightmap_mipmaps(&self) -> &[RgbaImage] {
        self.lightmap_atlas.mipmaps()
    }

    pub fn get_texture_count(&self) -> usize {
        self.regular_atlas.textures()
    }

    pub fn generate_mipmaps(&mut self, level: usize) {
        self.regular_atlas.generate_mipmaps(level);
        self.lightmap_atlas.generate_mipmaps(level);
    }

    pub fn generate_mipmap(&mut self, level: usize) {
        self.regular_atlas.generate_mipmap(level);
        self.lightmap_atlas.generate_mipmap(level);
    }

    /// # Errors
    ///
    /// An error will be returned if the passed path does not contain a filename
    /// or cannot be read.
    pub fn load_lightmap<P: AsRef<Path>>(&mut self, size: USize2D, path: P) -> LoadingResult<()> {
        let path = path.as_ref();

        // println!(
        //     "[{}] Loading lightmap at {}",
        //     "INFO/TextureLoader".bright_green(),
        //     path.display().bright_blue().bold()
        // );

        let name = path.file_stem().ok_or(LoadingError::Texture(TextureLoadingError::InvalidPath))?;
        let name = name.to_string_lossy();
        let name = name.to_string();

        if self.lightmap_atlas.contains_texture(&name) {
            return Ok(());
        }

        match image::ImageReader::open(path).and_then(image::ImageReader::with_guessed_format) {
            Ok(value) => {
                if let Ok(value) = value.decode() {
                    let image = value.to_rgba8();

                    self.lightmap_atlas.append(name, &image);
                } else {
                    self.lightmap_atlas.step_next(size);
                }
            }
            Err(err) => {
                self.lightmap_atlas.step_next(size);

                return Err(LoadingError::Texture(TextureLoadingError::Io(err)));
            }
        }

        Ok(())
    }

    /// # Errors
    ///
    /// An error will be returned if the passed path does not contain a filename
    /// or cannot be read.
    pub fn load<P: AsRef<Path>>(&mut self, path: P) -> LoadingResult<Option<USize2D>> {
        let path = path.as_ref();

        info!(target: "texture-loader", "Loading texture at {}", path.display());

        let name = path.file_stem().ok_or(LoadingError::Texture(TextureLoadingError::InvalidPath))?;
        let name = name.to_string_lossy();
        let name = name.to_string();

        if self.regular_atlas.contains_texture(&name) {
            return Ok(None);
        }

        match image::ImageReader::open(path).and_then(image::ImageReader::with_guessed_format) {
            Ok(value) => match value.decode() {
                Ok(value) => {
                    let image = value.to_rgba8();

                    Ok(Some(self.regular_atlas.special_append(name, &image)))
                }
                Err(error) => Err(LoadingError::Texture(TextureLoadingError::Decode(error))),
            },
            Err(err) => Err(LoadingError::Texture(TextureLoadingError::Io(err))),
        }
    }
}

impl Default for TextureStorage {
    fn default() -> Self {
        Self::new()
    }
}
