use std::rc::Rc;

use glow::HasContext;
use glutin::surface::GlSurface;

use crate::{ElementType, GlPrimitive, IndexBuffer, Program, RenderBackend, RenderInfo, Shader, Vertex, VertexBuffer};

pub struct RenderPass {
    pub(crate) gl: Rc<glow::Context>,
    pub(crate) blend: Option<Blend>,
    pub(crate) depth: Option<Depth>,
    pub(crate) culling: Option<BackfaceCullingMode>,
    pub(crate) finished: bool,
    pub render_info: RenderInfo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendingFactor {
    One,
    SourceAlpha,
    OneMinusSourceAlpha,
}

impl BlendingFactor {
    const fn as_gl(self) -> u32 {
        match self {
            Self::One => glow::ONE,
            Self::SourceAlpha => glow::SRC_ALPHA,
            Self::OneMinusSourceAlpha => glow::ONE_MINUS_SRC_ALPHA,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthTest {
    Never,
    Overwrite,
    IfEqual,
    IfNotEqual,
    IfGreater,
    IfGreaterOrEqual,
    IfLess,
    IfLessOrEqual,
}

impl DepthTest {
    const fn as_gl(self) -> u32 {
        match self {
            Self::Never => glow::NEVER,
            Self::Overwrite => glow::ALWAYS,
            Self::IfEqual => glow::EQUAL,
            Self::IfNotEqual => glow::NOTEQUAL,
            Self::IfGreater => glow::GREATER,
            Self::IfGreaterOrEqual => glow::GEQUAL,
            Self::IfLess => glow::LESS,
            Self::IfLessOrEqual => glow::LEQUAL,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Blend {
    pub color: (BlendingFactor, BlendingFactor),
    pub alpha: (BlendingFactor, BlendingFactor),
}

#[derive(Debug, Clone, Copy)]
pub struct Depth {
    pub test: DepthTest,
    pub write: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackfaceCullingMode {
    CullCounterClockwise,
    CullClockwise,
}

impl BackfaceCullingMode {
    const fn as_gl(self) -> u32 {
        match self {
            Self::CullCounterClockwise => glow::FRONT,
            Self::CullClockwise => glow::BACK,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DrawParams {
    pub blend: Option<Blend>,
    pub depth: Option<Depth>,
    pub culling: Option<BackfaceCullingMode>,
}

impl RenderPass {
    pub fn clear_color_and_depth(&self, color: [f32; 4], depth: f32) {
        unsafe {
            self.gl.clear_depth(f64::from(depth));
            self.gl.clear_depth_f32(depth);
            self.gl.clear_color(color[0], color[1], color[2], color[3]);
            self.gl.depth_mask(true);
            self.gl.clear(glow::DEPTH_BUFFER_BIT | glow::COLOR_BUFFER_BIT);
        }
    }

    pub fn clear_depth(&self, depth: f32) {
        unsafe {
            self.gl.clear_depth(f64::from(depth));
            self.gl.clear_depth_f32(depth);
            self.gl.depth_mask(true);
            self.gl.clear(glow::DEPTH_BUFFER_BIT);
        }
    }

    pub fn apply_params(&mut self, params: DrawParams) {
        unsafe {
            match (&mut self.blend, params.blend) {
                (None, None) => (),
                (None, Some(blend)) => {
                    self.gl.enable(glow::BLEND);
                    self.gl
                        .blend_func_separate(blend.color.0.as_gl(), blend.color.1.as_gl(), blend.alpha.0.as_gl(), blend.alpha.1.as_gl());

                    self.blend = Some(blend);
                }
                (Some(_), None) => {
                    self.gl.disable(glow::BLEND);

                    self.blend = None;
                }
                (Some(_), Some(blend)) => {
                    self.gl
                        .blend_func_separate(blend.color.0.as_gl(), blend.color.1.as_gl(), blend.alpha.0.as_gl(), blend.alpha.1.as_gl());

                    self.blend = Some(blend);
                }
            }

            match (&mut self.depth, params.depth) {
                (None, None) => (),
                (None, Some(depth)) => {
                    self.gl.enable(glow::DEPTH_TEST);
                    self.gl.depth_range(0.0, 1.0);
                    self.gl.depth_func(depth.test.as_gl());
                    self.gl.depth_mask(depth.write);

                    self.depth = Some(depth);
                }
                (Some(_), None) => {
                    self.gl.disable(glow::DEPTH_TEST);

                    self.depth = None;
                }
                (Some(old), Some(new)) => {
                    if old.test != new.test {
                        self.gl.depth_func(new.test.as_gl());

                        old.test = new.test;
                    }

                    if old.write != new.write {
                        self.gl.depth_mask(new.write);

                        old.write = new.write;
                    }
                }
            }

            match (self.culling, params.culling) {
                (None, None) => (),
                (None, Some(culling)) => {
                    self.gl.enable(glow::CULL_FACE);
                    self.gl.cull_face(culling.as_gl());

                    self.culling = Some(culling);
                }
                (Some(_), None) => {
                    self.gl.disable(glow::CULL_FACE);

                    self.culling = None;
                }
                (Some(old), Some(new)) => {
                    if old != new {
                        self.gl.cull_face(new.as_gl());

                        self.culling = Some(new);
                    }
                }
            }
        }
    }

    pub fn reset_params(&mut self) {
        unsafe {
            if self.blend.is_some() {
                self.gl.disable(glow::BLEND);
                self.gl.depth_func(glow::ALWAYS);
                self.gl.depth_mask(false);

                self.blend = None;
            }

            if self.depth.is_some() {
                self.gl.disable(glow::DEPTH_TEST);

                self.depth = None;
            }

            if self.culling.is_some() {
                self.gl.disable(glow::CULL_FACE);

                self.culling = None;
            }
        }
    }

    pub fn draw_elements_slice<V: Vertex, I: GlPrimitive, S: Shader>(
        &mut self,
        vertex_buffer: &VertexBuffer<V, S>,
        index_buffer: &IndexBuffer<I>,
        count: usize,
        offset: usize,
    ) {
        vertex_buffer.bind();
        index_buffer.bind();

        unsafe {
            self.gl
                .draw_elements(index_buffer.element_type.as_gl(), count as i32, I::gl_code(), offset as i32);
        }

        self.render_info.draw_calls += 1;
        self.render_info.vertices += count;
    }

    pub fn draw_arrays_slice<V: Vertex, S: Shader>(
        &mut self,
        program: &Program,
        vertex_buffer: &VertexBuffer<V, S>,
        element_type: &ElementType,
        count: usize,
        params: DrawParams,
    ) {
        self.apply_params(params);

        vertex_buffer.bind();

        unsafe {
            let stride = std::mem::size_of::<V>() as i32;

            for &(name, offset, (ty, size), normalized) in V::get_bindings() {
                if let Some(loc) = program.attributes.get(name).copied() {
                    self.gl.enable_vertex_attrib_array(loc);

                    if ty == glow::UNSIGNED_INT || ty == glow::INT || (ty == glow::UNSIGNED_BYTE && !normalized) {
                        self.gl.vertex_attrib_pointer_i32(loc, size, ty, stride, offset as i32);
                    } else {
                        self.gl.vertex_attrib_pointer_f32(loc, size, ty, normalized, stride, offset as i32);
                    }
                }
            }

            self.gl.draw_arrays(element_type.as_gl(), 0, count as i32);
        }

        vertex_buffer.unbind();

        self.reset_params();

        self.render_info.draw_calls += 1;
        self.render_info.vertices += count;
    }

    #[inline]
    pub fn draw_arrays<V: Vertex, S: Shader>(&mut self, program: &Program, vertex_buffer: &VertexBuffer<V, S>, element_type: &ElementType, params: DrawParams) {
        self.draw_arrays_slice(program, vertex_buffer, element_type, vertex_buffer.len, params);
    }

    #[inline]
    pub fn draw_elements<V: Vertex, I: GlPrimitive, S: Shader>(&mut self, vertex_buffer: &VertexBuffer<V, S>, index_buffer: &IndexBuffer<I>) {
        self.draw_elements_slice(vertex_buffer, index_buffer, index_buffer.len, 0);
    }

    #[inline]
    pub fn finish(mut self, backend: &RenderBackend) -> RenderInfo {
        if let Err(error) = backend.surface.swap_buffers(&backend.context) {
            eprintln!("failed to swap buffers: {error}");
        } else {
            self.finished = true;
        }

        self.render_info
    }
}

impl Drop for RenderPass {
    fn drop(&mut self) {
        assert!(self.finished, "RenderPass was dropped without `RenderPass::finish` being called");
    }
}
