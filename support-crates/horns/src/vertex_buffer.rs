use std::{marker::PhantomData, rc::Rc};

use glow::HasContext;

use crate::Error;

pub trait Vertex: bytemuck::NoUninit {
    fn get_bindings() -> &'static [(&'static str, usize, (u32, i32), bool)];
}

pub struct VertexBuffer<V: Vertex> {
    gl: Rc<glow::Context>,
    pub(crate) ptr: glow::NativeBuffer,
    pub(crate) array_ptr: glow::NativeVertexArray,
    pub(crate) len: usize,
    _phantom: PhantomData<V>,
}

impl<V: Vertex> VertexBuffer<V> {
    pub(crate) fn new(gl: &Rc<glow::Context>, vertices: &[V], is_dynamic: bool) -> Result<Self, Error> {
        unsafe {
            let ptr = gl.create_buffer().map_err(Error::BufferCreation)?;
            let array_ptr = gl.create_vertex_array().map_err(Error::BufferCreation)?;

            gl.bind_buffer(glow::ARRAY_BUFFER, Some(ptr));
            gl.bind_vertex_array(Some(array_ptr));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(vertices),
                if is_dynamic { glow::DYNAMIC_DRAW } else { glow::STATIC_DRAW },
            );

            gl.bind_buffer(glow::ARRAY_BUFFER, None);
            gl.bind_vertex_array(None);

            Ok(Self {
                gl: gl.clone(),
                ptr,
                array_ptr,
                len: vertices.len(),
                _phantom: PhantomData,
            })
        }
    }

    pub fn bind(&self) {
        unsafe {
            self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.ptr));
            self.gl.bind_vertex_array(Some(self.array_ptr));
        }
    }

    pub fn unbind(&self) {
        unsafe {
            self.gl.bind_buffer(glow::ARRAY_BUFFER, None);
            self.gl.bind_vertex_array(None);
        }
    }
}

impl<V: Vertex> Drop for VertexBuffer<V> {
    fn drop(&mut self) {
        unsafe {
            self.gl.bind_buffer(glow::ARRAY_BUFFER, None);
            self.gl.bind_vertex_array(None);
            self.gl.delete_buffer(self.ptr);
            self.gl.delete_vertex_array(self.array_ptr);
        }
    }
}
