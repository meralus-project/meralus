use std::{collections::HashMap, rc::Rc};

use glow::HasContext;

use crate::Error;

#[derive(Debug)]
pub struct Program {
    gl: Rc<glow::Context>,
    pub(crate) ptr: glow::NativeProgram,
    pub(crate) attributes: HashMap<String, u32>,
    pub(crate) uniforms: HashMap<String, glow::NativeUniformLocation>,
}

pub enum UniformValue<'a> {
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
    Texture(&'a glow::NativeTexture),
}

macro_rules! uniform_values {
    ($([$typo:ty => $variant:ident]($name:ident): $code:expr),*) => {
        $(
            impl<'a> From<$typo> for UniformValue<'a> {
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
    [&'a crate::Texture2d  => Texture](value): &value.ptr
}

pub struct ProgramBinder<'a> {
    program: &'a Program,
    texture_id: (i32, u32),
}

impl ProgramBinder<'_> {
    #[must_use]
    pub fn with_uniform<'a, N: AsRef<str>, V: Into<UniformValue<'a>>>(mut self, name: N, value: V) -> Self {
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

impl Program {
    pub(crate) fn new<T: Shader>(context: &Rc<glow::Context>, source: &T) -> Result<Self, Error> {
        unsafe {
            let vertex = context.create_shader(glow::VERTEX_SHADER).unwrap();

            context.shader_source(vertex, &source.vertex());
            context.compile_shader(vertex);

            if !context.get_shader_compile_status(vertex) {
                return Err(Error::ShaderCreation(context.get_shader_info_log(vertex)));
            }

            let fragment = context.create_shader(glow::FRAGMENT_SHADER).unwrap();

            context.shader_source(fragment, &source.fragment());
            context.compile_shader(fragment);

            if !context.get_shader_compile_status(fragment) {
                return Err(Error::ShaderCreation(context.get_shader_info_log(fragment)));
            }

            let geometry = if let Some(source) = source.geometry() {
                let id = context.create_shader(glow::GEOMETRY_SHADER).unwrap();

                context.shader_source(id, &source);
                context.compile_shader(id);

                if !context.get_shader_compile_status(id) {
                    return Err(Error::ShaderCreation(context.get_shader_info_log(id)));
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
                Ok(Self {
                    gl: context.clone(),
                    ptr: program,
                    attributes,
                    uniforms,
                })
            } else {
                Err(Error::ShaderCreation(context.get_program_info_log(program)))
            }
        }
    }

    pub fn bind(&self) -> ProgramBinder<'_> {
        unsafe { self.gl.use_program(Some(self.ptr)) };

        ProgramBinder {
            program: self,
            texture_id: (0, glow::TEXTURE0),
        }
    }
}

impl Drop for Program {
    fn drop(&mut self) {
        unsafe { self.gl.delete_program(self.ptr) };
    }
}

pub trait Shader {
    fn vertex(&self) -> String;
    fn fragment(&self) -> String;
    fn geometry(&self) -> Option<String> {
        None
    }
}
