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
    #[inline]
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
    #[inline]
    fn gl_code() -> u32 {
        glow::UNSIGNED_BYTE
    }
}

impl GlPrimitive for u16 {
    #[inline]
    fn gl_code() -> u32 {
        glow::UNSIGNED_SHORT
    }
}

impl GlPrimitive for u32 {
    #[inline]
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
    pub(crate) fn empty(gl: &Rc<glow::Context>, element_type: ElementType, indices: usize, is_dynamic: bool) -> Result<Self, Error> {
        unsafe {
            let ptr = gl.create_buffer().map_err(Error::BufferCreation)?;

            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ptr));
            gl.buffer_data_size(
                glow::ELEMENT_ARRAY_BUFFER,
                (indices * size_of::<I>()) as i32,
                if is_dynamic { glow::DYNAMIC_DRAW } else { glow::STATIC_DRAW },
            );

            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, None);

            Ok(Self {
                gl: gl.clone(),
                ptr,
                element_type,
                len: indices,
                _phantom: PhantomData,
            })
        }
    }

    pub(crate) fn new(gl: &Rc<glow::Context>, element_type: ElementType, indices: &[I], is_dynamic: bool) -> Result<Self, Error> {
        unsafe {
            let ptr = gl.create_buffer().map_err(Error::BufferCreation)?;

            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ptr));
            gl.buffer_data_u8_slice(
                glow::ELEMENT_ARRAY_BUFFER,
                bytemuck::cast_slice(indices),
                if is_dynamic { glow::DYNAMIC_DRAW } else { glow::STATIC_DRAW },
            );
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

    pub fn dynamic_write(&self, data: &[I]) {
        let data = bytemuck::cast_slice(data);

        self.bind();

        unsafe {
            let ptr = self.gl.map_buffer_range(
                glow::ELEMENT_ARRAY_BUFFER,
                0,
                data.len() as i32,
                glow::MAP_WRITE_BIT | glow::MAP_INVALIDATE_BUFFER_BIT,
            );

            if ptr.is_null() {
                eprintln!(
                    "[warn] map_buffer_range returned null (current-size = {}, data-size = {})",
                    self.len * size_of::<I>(),
                    data.len()
                );
            } else {
                std::ptr::copy_nonoverlapping(data.as_ptr(), ptr, data.len());

                self.gl.unmap_buffer(glow::ELEMENT_ARRAY_BUFFER);
            }
        }

        self.unbind();
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    pub fn bind(&self) {
        unsafe {
            self.gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.ptr));
        }
    }

    #[inline]
    pub fn unbind(&self) {
        unsafe {
            self.gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, None);
        }
    }
}

impl<I: GlPrimitive> Drop for IndexBuffer<I> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            self.gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, None);
            self.gl.delete_buffer(self.ptr);
        }
    }
}
