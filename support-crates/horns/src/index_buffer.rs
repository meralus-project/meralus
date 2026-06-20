use std::{marker::PhantomData, rc::Rc};

use glow::HasContext;

use crate::Error;

pub enum ElementType {
    Triangles,
    TriangleStrip,
    Lines,
    LineStrip,
}

impl ElementType {
    pub const fn as_gl(&self) -> u32 {
        match self {
            Self::Triangles => glow::TRIANGLES,
            Self::TriangleStrip => glow::TRIANGLE_STRIP,
            Self::Lines => glow::LINES,
            Self::LineStrip => glow::LINE_STRIP,
        }
    }
}

pub trait GlPrimitive: bytemuck::NoUninit {
    fn gl_code() -> u32;
}

impl GlPrimitive for u8 {
    fn gl_code() -> u32 {
        glow::UNSIGNED_BYTE
    }
}

impl GlPrimitive for u16 {
    fn gl_code() -> u32 {
        glow::UNSIGNED_SHORT
    }
}

impl GlPrimitive for u32 {
    fn gl_code() -> u32 {
        glow::UNSIGNED_INT
    }
}

pub struct IndexBuffer<I: GlPrimitive> {
    gl: Rc<glow::Context>,
    pub(crate) ptr: glow::NativeBuffer,
    pub(crate) element_type: ElementType,
    pub(crate) len: usize,
    _phantom: PhantomData<I>,
}

impl<I: GlPrimitive> IndexBuffer<I> {
    pub(crate) fn new(gl: &Rc<glow::Context>, element_type: ElementType, indices: &[I]) -> Result<Self, Error> {
        unsafe {
            let ptr = gl.create_buffer().map_err(Error::BufferCreation)?;

            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ptr));
            gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, bytemuck::cast_slice(indices), glow::STATIC_DRAW);
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, None);

            Ok(Self {
                gl: gl.clone(),
                ptr,
                element_type,
                len: indices.len(),
                _phantom: PhantomData,
            })
        }
    }

    pub fn bind(&self) {
        unsafe {
            self.gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.ptr));
        }
    }

    pub fn unbind(&self) {
        unsafe {
            self.gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, None);
        }
    }
}

impl<I: GlPrimitive> Drop for IndexBuffer<I> {
    fn drop(&mut self) {
        unsafe {
            self.gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, None);
            self.gl.delete_buffer(self.ptr);
        }
    }
}
