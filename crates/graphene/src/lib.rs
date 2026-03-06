mod color;
mod common;
mod geometry;

use std::{collections::HashMap, marker::PhantomData, rc::Rc};

use glow::HasContext;

pub use self::{
    color::Color,
    common::{AtlasKey, CommonRenderer, CommonTessellator, CommonVertex, FilterMode, ObjectFit, Path, ShapeGeometryBuilder, ShapeShader},
    geometry::{Box2D, Point2D, RRect, Rect, Size2D, Thickness},
};

pub const FONT: &[u8] = include_bytes!("../../../resources/fonts/Monocraft.ttf");

pub struct RenderInfo {
    pub draw_calls: usize,
    pub vertices: usize,
}

impl RenderInfo {
    pub const fn default() -> Self {
        Self { draw_calls: 0, vertices: 0 }
    }

    pub const fn extend(&mut self, other: &Self) {
        self.draw_calls += other.draw_calls;
        self.vertices += other.vertices;
    }

    #[must_use]
    pub const fn take(&mut self) -> Self {
        Self {
            draw_calls: std::mem::replace(&mut self.draw_calls, 0),
            vertices: std::mem::replace(&mut self.vertices, 0),
        }
    }
}

// pub const BLENDING: glium::Blend = glium::Blend {
//     color: glium::BlendingFunction::Addition {
//         source: glium::LinearBlendingFactor::SourceAlpha,
//         destination: glium::LinearBlendingFactor::OneMinusSourceAlpha,
//     },
//     alpha: glium::BlendingFunction::Addition {
//         source: glium::LinearBlendingFactor::One,
//         destination: glium::LinearBlendingFactor::OneMinusSourceAlpha,
//     },
//     constant_value: (0.0, 0.0, 0.0, 0.0),
// };

#[macro_export]
macro_rules! impl_vertex {
    ($struct_name:ident { $($field_name:ident: $field_ty:expr),+ }) => {
        impl $struct_name {
            const BINDINGS: &[(std::borrow::Cow<'static, str>, usize, (u32, i32), bool)] = &[
                $((
                    std::borrow::Cow::Borrowed(stringify!($field_name)),
                    core::mem::offset_of!($struct_name, $field_name),
                    $field_ty,
                    false,
                )),+
            ];
        }

        impl $crate::Vertex for $struct_name {
            fn get_bindings() -> &'static [(std::borrow::Cow<'static, str>, usize, (u32, i32), bool)] {
                Self::BINDINGS
            }
        }
    };
}

#[derive(Debug)]
pub struct Program<T: HasContext> {
    gl: Rc<T>,
    pub ptr: T::Program,
    pub attributes: HashMap<String, u32>,
    pub uniforms: HashMap<String, T::UniformLocation>,
}

pub enum UniformValue<'a, T: HasContext> {
    I32(i32),
    I32x2([i32; 2]),
    I32x3([i32; 3]),
    I32x4([i32; 4]),
    F32(f32),
    F32x2([f32; 2]),
    F32x3([f32; 3]),
    F32x4([f32; 4]),
    Mat4x4([f32; 16]),
    Mat3x3([f32; 9]),
    Texture(&'a T::Texture),
}

macro_rules! uniform_values {
    ($([$typo:ty => $variant:ident]($name:ident): $code:expr),*) => {
        $(
            impl<'a, T: HasContext> From<$typo> for UniformValue<'a, T> {
                fn from($name: $typo) -> Self {
                    Self::$variant($code)
                }
            }
        )*
    };
}

uniform_values! {
    [i32               => I32    ](value): value,
    [[i32; 2]          => I32x2  ](value): value,
    [[i32; 3]          => I32x3  ](value): value,
    [[i32; 4]          => I32x4  ](value): value,
    [f32               => F32    ](value): value,
    [[f32; 2]          => F32x2  ](value): value,
    [[f32; 3]          => F32x3  ](value): value,
    [[f32; 4]          => F32x4  ](value): value,
    [[f32; 9]          => Mat3x3 ](value): value,
    [[f32; 16]         => Mat4x4 ](value): value,
    [glam::IVec2       => I32x2  ](value): value.to_array(),
    [glam::IVec3       => I32x3  ](value): value.to_array(),
    [glam::IVec4       => I32x4  ](value): value.to_array(),
    [glam::Vec2        => F32x2  ](value): value.to_array(),
    [glam::Vec3        => F32x3  ](value): value.to_array(),
    [glam::Vec4        => F32x4  ](value): value.to_array(),
    [glam::Mat3        => Mat3x3 ](value): value.to_cols_array(),
    [glam::Mat4        => Mat4x4 ](value): value.to_cols_array(),
    [&'a Texture2d<T>  => Texture](value): &value.ptr
}

pub struct ProgramBinder<'a, T: HasContext> {
    program: &'a Program<T>,
    texture_id: (i32, u32),
}

impl<T: HasContext> ProgramBinder<'_, T> {
    #[must_use]
    pub fn with_uniform<'a, N: AsRef<str>, V: Into<UniformValue<'a, T>>>(mut self, name: N, value: V) -> Self
    where
        <T as glow::HasContext>::Texture: 'a,
    {
        if let Some(location) = self.program.uniforms.get(name.as_ref()) {
            let location = Some(location);

            unsafe {
                match value.into() {
                    UniformValue::I32(x) => self.program.gl.uniform_1_i32(location, x),
                    UniformValue::I32x2([x, y]) => self.program.gl.uniform_2_i32(location, x, y),
                    UniformValue::I32x3([x, y, z]) => self.program.gl.uniform_3_i32(location, x, y, z),
                    UniformValue::I32x4([x, y, z, w]) => self.program.gl.uniform_4_i32(location, x, y, z, w),
                    UniformValue::F32(x) => self.program.gl.uniform_1_f32(location, x),
                    UniformValue::F32x2([x, y]) => self.program.gl.uniform_2_f32(location, x, y),
                    UniformValue::F32x3([x, y, z]) => self.program.gl.uniform_3_f32(location, x, y, z),
                    UniformValue::F32x4([x, y, z, w]) => self.program.gl.uniform_4_f32(location, x, y, z, w),
                    UniformValue::Mat4x4(value) => self.program.gl.uniform_matrix_4_f32_slice(location, false, &value),
                    UniformValue::Mat3x3(value) => self.program.gl.uniform_matrix_3_f32_slice(location, false, &value),
                    UniformValue::Texture(ptr) => {
                        self.program.gl.active_texture(self.texture_id.1);
                        self.program.gl.bind_texture(glow::TEXTURE_2D, Some(*ptr));
                        self.program.gl.uniform_1_i32(location, self.texture_id.0);

                        self.texture_id.0 += 1;
                        self.texture_id.1 += 1;
                    }
                }
            }
        }

        self
    }
}

impl<T: HasContext> Program<T> {
    pub fn bind(&self) -> ProgramBinder<'_, T> {
        unsafe { self.gl.use_program(Some(self.ptr)) };

        ProgramBinder {
            program: self,
            texture_id: (0, glow::TEXTURE0),
        }
    }
}

impl<T: HasContext> Drop for Program<T> {
    fn drop(&mut self) {
        unsafe { self.gl.delete_program(self.ptr) };
    }
}

pub trait Shader {
    const VERTEX: &str;
    const FRAGMENT: &str;
    const GEOMETRY: Option<&str> = None;

    fn program<T: HasContext>(context: &Rc<T>) -> Result<Program<T>, String> {
        unsafe {
            let vertex = context.create_shader(glow::VERTEX_SHADER).unwrap();

            context.shader_source(vertex, Self::VERTEX);
            context.compile_shader(vertex);

            if !context.get_shader_compile_status(vertex) {
                return Err(context.get_shader_info_log(vertex));
            }

            let fragment = context.create_shader(glow::FRAGMENT_SHADER).unwrap();

            context.shader_source(fragment, Self::FRAGMENT);
            context.compile_shader(fragment);

            if !context.get_shader_compile_status(fragment) {
                return Err(context.get_shader_info_log(fragment));
            }

            let geometry = if let Some(geometry) = Self::GEOMETRY {
                let id = context.create_shader(glow::GEOMETRY_SHADER).unwrap();

                context.shader_source(id, geometry);
                context.compile_shader(id);

                if !context.get_shader_compile_status(id) {
                    return Err(context.get_shader_info_log(id));
                }

                Some(id)
            } else {
                None
            };

            let program = context.create_program().unwrap();

            context.attach_shader(program, vertex);
            context.attach_shader(program, fragment);

            if let Some(geometry) = geometry {
                context.attach_shader(program, geometry);
            }

            context.link_program(program);

            let active_attributes = context.get_program_parameter_i32(program, glow::ACTIVE_ATTRIBUTES).cast_unsigned();

            let mut attributes = HashMap::with_capacity(active_attributes as usize);

            for attribute_id in 0..active_attributes {
                if let Some(attribute) = context.get_active_attribute(program, attribute_id) {
                    if attribute.name.starts_with("gl_") {
                        // ignoring everything built-in
                        continue;
                    }

                    if attribute.name.is_empty() {
                        // Some spirv compilers add an empty attribute to shaders. Most drivers
                        // don't expose this attribute, but some do.
                        // Since we can't do anything with empty attribute names, we simply skip
                        // them in this reflection code.
                        continue;
                    }

                    let location = context.get_attrib_location(program, attribute.name.as_str()).unwrap();

                    attributes.insert(attribute.name, location);
                }
            }

            let active_uniforms = context.get_program_parameter_i32(program, glow::ACTIVE_UNIFORMS).cast_unsigned();

            let mut uniforms = HashMap::with_capacity(active_uniforms as usize);

            for uniform_id in 0..active_uniforms {
                if let Some(uniform) = context.get_active_uniform(program, uniform_id) {
                    let location = context.get_uniform_location(program, uniform.name.as_str()).unwrap();

                    uniforms.insert(uniform.name, location);
                }
            }

            context.detach_shader(program, vertex);
            context.detach_shader(program, fragment);

            if let Some(geometry) = geometry {
                context.detach_shader(program, geometry);
            }

            context.delete_shader(vertex);
            context.delete_shader(fragment);

            if let Some(geometry) = geometry {
                context.delete_shader(geometry);
            }

            if context.get_program_link_status(program) {
                Ok(Program {
                    gl: context.clone(),
                    ptr: program,
                    attributes,
                    uniforms,
                })
            } else {
                Err(context.get_program_info_log(program))
            }
        }
    }
}

pub struct Texture2d<T: HasContext> {
    gl: Rc<T>,
    pub ptr: T::Texture,
}

impl<T: HasContext> Texture2d<T> {
    pub fn empty(gl: &Rc<T>, width: u32, height: u32) -> Result<Self, String> {
        unsafe {
            let ptr = gl.create_texture()?;

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

    pub const fn writable(&self) -> WritableTexture2d<'_, T> {
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

pub struct WritableTexture2d<'a, T: HasContext> {
    texture: &'a Texture2d<T>,
}

impl<T: HasContext> WritableTexture2d<'_, T> {
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

impl<T: HasContext> Drop for WritableTexture2d<'_, T> {
    fn drop(&mut self) {
        unsafe { self.texture.gl.bind_texture(glow::TEXTURE_2D, None) };
    }
}

impl<T: HasContext> Drop for Texture2d<T> {
    fn drop(&mut self) {
        unsafe { self.gl.delete_texture(self.ptr) };
    }
}

pub trait Vertex {
    fn get_bindings() -> &'static [(std::borrow::Cow<'static, str>, usize, (u32, i32), bool)];
}

pub struct VertexBuffer<T: HasContext, V: Vertex + bytemuck::NoUninit> {
    gl: Rc<T>,
    pub ptr: T::Buffer,
    pub array_ptr: T::VertexArray,
    _phantom: PhantomData<V>,
}

impl<T: HasContext, V: Vertex + bytemuck::NoUninit> VertexBuffer<T, V> {
    pub fn new(gl: &Rc<T>, program: &Program<T>, vertices: &[V]) -> Result<Self, String> {
        unsafe {
            let ptr = gl.create_buffer()?;
            let array_ptr = gl.create_vertex_array()?;

            gl.bind_buffer(glow::ARRAY_BUFFER, Some(ptr));
            gl.bind_vertex_array(Some(array_ptr));
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytemuck::cast_slice(vertices), glow::STATIC_DRAW);

            let stride = std::mem::size_of::<V>() as i32;

            for (name, offset, (ty, size), normalized) in V::get_bindings() {
                if let Some(loc) = program.attributes.get(name.as_ref()).copied() {
                    gl.vertex_attrib_pointer_f32(loc, *size, *ty, *normalized, stride, *offset as i32);

                    if *size > 1 {
                        gl.enable_vertex_attrib_array(loc);
                    }
                }
            }

            gl.bind_buffer(glow::ARRAY_BUFFER, None);
            gl.bind_vertex_array(None);

            Ok(Self {
                gl: gl.clone(),
                ptr,
                array_ptr,
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

impl<T: HasContext, V: Vertex + bytemuck::NoUninit> Drop for VertexBuffer<T, V> {
    fn drop(&mut self) {
        unsafe {
            self.gl.bind_buffer(glow::ARRAY_BUFFER, None);
            self.gl.bind_vertex_array(None);
            self.gl.delete_buffer(self.ptr);
            self.gl.delete_vertex_array(self.array_ptr);
        }
    }
}

pub enum ElementType {
    Triangles,
    TriangleStrip,
    Lines,
    LineStrip,
}

impl ElementType {
    const fn as_gl(&self) -> u32 {
        match self {
            Self::Triangles => glow::TRIANGLES,
            Self::TriangleStrip => glow::TRIANGLE_STRIP,
            Self::Lines => glow::LINES,
            Self::LineStrip => glow::LINE_STRIP,
        }
    }
}

pub trait GlPrimitive {
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

pub struct IndexBuffer<T: HasContext, I: GlPrimitive + bytemuck::NoUninit> {
    gl: Rc<T>,
    pub ptr: T::Buffer,
    pub element_type: ElementType,
    _phantom: PhantomData<I>,
}

impl<T: HasContext, I: GlPrimitive + bytemuck::NoUninit> IndexBuffer<T, I> {
    pub fn new(gl: &Rc<T>, element_type: ElementType, indices: &[I]) -> Result<Self, String> {
        unsafe {
            let ptr = gl.create_buffer()?;

            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ptr));
            gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, bytemuck::cast_slice(indices), glow::STATIC_DRAW);
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, None);

            Ok(Self {
                gl: gl.clone(),
                ptr,
                element_type,
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

impl<T: HasContext, I: GlPrimitive + bytemuck::NoUninit> Drop for IndexBuffer<T, I> {
    fn drop(&mut self) {
        unsafe {
            self.gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, None);
            self.gl.delete_buffer(self.ptr);
        }
    }
}

pub struct VertexBuffers<V, I> {
    pub vertices: Vec<V>,
    pub indices: Vec<I>,
}

impl<V, I> VertexBuffers<V, I> {
    pub const fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    pub fn with_capacity(vertices: usize, indices: usize) -> Self {
        Self {
            vertices: Vec::with_capacity(vertices),
            indices: Vec::with_capacity(indices),
        }
    }

    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
    }
}

impl<V, I> Default for VertexBuffers<V, I> {
    fn default() -> Self {
        Self::new()
    }
}
