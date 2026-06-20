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

            Ok(Self { gl: gl.clone(), ptr })
        }
    }

    pub const fn writable(&self) -> WritableTexture2d<'_> {
        WritableTexture2d { texture: self }
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

pub struct WritableTexture2d<'a> {
    texture: &'a Texture2d,
}

impl WritableTexture2d<'_> {
    pub fn write(&self, x: u32, y: u32, width: u32, height: u32, data: &[u8]) {
        self.texture.bind();

        unsafe {
            self.texture.gl.tex_sub_image_2d(
                glow::TEXTURE_2D,
                0,
                x.cast_signed(),
                y.cast_signed(),
                width.cast_signed(),
                height.cast_signed(),
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(data)),
            );
        }
    }
}

impl Drop for WritableTexture2d<'_> {
    fn drop(&mut self) {
        unsafe { self.texture.gl.bind_texture(glow::TEXTURE_2D, None) };
    }
}
