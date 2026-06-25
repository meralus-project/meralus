use std::rc::Rc;

use glow::HasContext;

use crate::Error;

pub struct Texture2d {
    gl: Rc<glow::Context>,
    pub(crate) ptr: glow::NativeTexture,
}

impl Texture2d {
    pub(crate) fn empty(gl: &Rc<glow::Context>, width: u32, height: u32) -> Result<Self, Error> {
        unsafe {
            let ptr = gl.create_texture().map_err(Error::TextureCreation)?;

            gl.bind_texture(glow::TEXTURE_2D, Some(ptr));
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::NEAREST.cast_signed());
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::NEAREST.cast_signed());
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA.cast_signed(),
                width.cast_signed(),
                height.cast_signed(),
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(None),
            );

            gl.bind_texture(glow::TEXTURE_2D, None);

            Ok(Self { gl: gl.clone(), ptr })
        }
    }

    pub(crate) fn empty_with_mipmaps(gl: &Rc<glow::Context>, width: u32, height: u32, levels: usize) -> Result<Self, Error> {
        unsafe {
            let ptr = gl.create_texture().map_err(Error::TextureCreation)?;

            gl.bind_texture(glow::TEXTURE_2D, Some(ptr));
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::NEAREST.cast_signed());
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::NEAREST.cast_signed());
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAX_LEVEL, levels as i32);
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA.cast_signed(),
                width.cast_signed(),
                height.cast_signed(),
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(None),
            );

            for level in 1..=levels {
                gl.tex_image_2d(
                    glow::TEXTURE_2D,
                    level as i32,
                    glow::RGBA.cast_signed(),
                    width.cast_signed() / 2i32.pow(level as u32),
                    height.cast_signed() / 2i32.pow(level as u32),
                    0,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(None),
                );
            }

            gl.bind_texture(glow::TEXTURE_2D, None);

            Ok(Self { gl: gl.clone(), ptr })
        }
    }

    pub const fn writable(&self) -> WritableTexture2d<'_> {
        WritableTexture2d { texture: self, level: 0 }
    }

    pub const fn writable_mipmap(&self, level: usize) -> WritableTexture2d<'_> {
        WritableTexture2d { texture: self, level }
    }

    pub fn with_filters(&self, minify_filter: MinifyFilter, magnify_filter: MagnifyFilter) -> SampledTexture2d<'_> {
        self.bind();

        unsafe {
            self.gl
                .tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, magnify_filter.as_gl().cast_signed());

            self.gl
                .tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, minify_filter.as_gl().cast_signed());
        }

        self.unbind();

        SampledTexture2d {
            texture: self,
            minify_filter,
            magnify_filter,
        }
    }

    pub fn bind(&self) {
        unsafe {
            self.gl.bind_texture(glow::TEXTURE_2D, Some(self.ptr));
        }
    }

    pub fn unbind(&self) {
        unsafe {
            self.gl.bind_texture(glow::TEXTURE_2D, None);
        }
    }
}

impl Drop for Texture2d {
    fn drop(&mut self) {
        unsafe { self.gl.delete_texture(self.ptr) };
    }
}

pub enum MinifyFilter {
    Nearest,
    Linear,
    NearestMipmapNearest,
    LinearMipmapNearest,
    NearestMipmapLinear,
    LinearMipmapLinear,
}

impl MinifyFilter {
    const fn as_gl(&self) -> u32 {
        match self {
            Self::Nearest => glow::NEAREST,
            Self::Linear => glow::LINEAR,
            Self::NearestMipmapNearest => glow::NEAREST_MIPMAP_NEAREST,
            Self::LinearMipmapNearest => glow::LINEAR_MIPMAP_NEAREST,
            Self::NearestMipmapLinear => glow::NEAREST_MIPMAP_LINEAR,
            Self::LinearMipmapLinear => glow::LINEAR_MIPMAP_LINEAR,
        }
    }
}

pub enum MagnifyFilter {
    Nearest,
    Linear,
}

impl MagnifyFilter {
    const fn as_gl(&self) -> u32 {
        match self {
            Self::Nearest => glow::NEAREST,
            Self::Linear => glow::LINEAR,
        }
    }
}

#[allow(dead_code)]
pub struct SampledTexture2d<'a> {
    pub(crate) texture: &'a Texture2d,
    minify_filter: MinifyFilter,
    magnify_filter: MagnifyFilter,
}

impl SampledTexture2d<'_> {
    pub fn bind(&self) {
        self.texture.bind();
    }

    pub fn unbind(&self) {
        self.texture.unbind();
    }
}

pub struct WritableTexture2d<'a> {
    texture: &'a Texture2d,
    level: usize,
}

impl WritableTexture2d<'_> {
    pub fn write(&self, x: u32, y: u32, width: u32, height: u32, data: &[u8]) {
        self.texture.bind();

        unsafe {
            self.texture.gl.tex_sub_image_2d(
                glow::TEXTURE_2D,
                self.level as i32,
                x.cast_signed(),
                y.cast_signed(),
                width.cast_signed(),
                height.cast_signed(),
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(data)),
            );

            let error = self.texture.gl.get_error();

            if error != glow::NO_ERROR {
                eprintln!(
                    "[WritableTexture2d::write] OpenGL error ({x}, {y}, {width}, {height}, {}): {:#X}",
                    self.level, error
                );
            }
        }
    }
}

impl Drop for WritableTexture2d<'_> {
    fn drop(&mut self) {
        unsafe { self.texture.gl.bind_texture(glow::TEXTURE_2D, None) };
    }
}
