use glow::HasContext;
use glutin::surface::GlSurface;

use crate::{ElementType, GlPrimitive, IndexBuffer, Program, RenderBackend, Vertex, VertexBuffer};

pub struct RenderPass<'a> {
    pub(crate) backend: &'a RenderBackend,
}

impl RenderPass<'_> {
    pub fn draw_elements_slice<V: Vertex, I: GlPrimitive>(
        &self,
        program: &Program,
        vertex_buffer: &VertexBuffer<V>,
        index_buffer: &IndexBuffer<I>,
        count: usize,
        offset: usize,
    ) {
        vertex_buffer.bind();

        let stride = std::mem::size_of::<V>() as i32;

        for (name, offset, (ty, size), normalized) in V::get_bindings() {
            if let Some(loc) = program.attributes.get(*name).copied() {
                unsafe { self.backend.gl.vertex_attrib_pointer_f32(loc, *size, *ty, *normalized, stride, *offset as i32) };

                if *size > 1 {
                    unsafe { self.backend.gl.enable_vertex_attrib_array(loc) };
                }
            }
        }

        index_buffer.bind();

        unsafe {
            self.backend
                .gl
                .draw_elements(index_buffer.element_type.as_gl(), count as i32, I::gl_code(), offset as i32);
        }

        index_buffer.unbind();
        vertex_buffer.unbind();
    }

    pub fn draw_arrays_slice<V: Vertex>(&self, program: &Program, vertex_buffer: &VertexBuffer<V>, element_type: ElementType, count: usize) {
        vertex_buffer.bind();

        unsafe {
            let stride = std::mem::size_of::<V>() as i32;

            for (name, offset, (ty, size), normalized) in V::get_bindings() {
                if let Some(loc) = program.attributes.get(*name).copied() {
                    self.backend.gl.vertex_attrib_pointer_f32(loc, *size, *ty, *normalized, stride, *offset as i32);

                    if *size > 1 {
                        self.backend.gl.enable_vertex_attrib_array(loc);
                    }
                }
            }

            self.backend.gl.draw_arrays(element_type.as_gl(), 0, count as i32);
        }

        vertex_buffer.unbind();
    }

    pub fn draw_arrays<V: Vertex>(&self, program: &Program, vertex_buffer: &VertexBuffer<V>, element_type: ElementType) {
        self.draw_arrays_slice(program, vertex_buffer, element_type, vertex_buffer.len);
    }

    pub fn draw_elements<V: Vertex, I: GlPrimitive>(&self, program: &Program, vertex_buffer: &VertexBuffer<V>, index_buffer: &IndexBuffer<I>) {
        self.draw_elements_slice(program, vertex_buffer, index_buffer, index_buffer.len, 0);
    }
}

impl Drop for RenderPass<'_> {
    fn drop(&mut self) {
        if let Err(error) = self.backend.surface.swap_buffers(&self.backend.context) {
            eprintln!("failed to swap buffers: {error}");
        }
    }
}
