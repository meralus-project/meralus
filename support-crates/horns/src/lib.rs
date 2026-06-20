mod backend;
mod error;
mod index_buffer;
mod render_pass;
mod shader;
mod texture;
mod vertex_buffer;

pub use self::{
    backend::RenderBackend,
    error::Error,
    index_buffer::{ElementType, GlPrimitive, IndexBuffer},
    render_pass::RenderPass,
    shader::{Program, ProgramBinder, Shader, UniformValue},
    texture::{Texture2d, WritableTexture2d},
    vertex_buffer::{Vertex, VertexBuffer},
};

#[macro_export]
macro_rules! impl_vertex {
    ($struct_name:ident { $($field_name:ident: [$field_ty:ident; $count:literal]),+ }) => {
        impl $struct_name {
            const BINDINGS: &[(&'static str, usize, (u32, i32), bool)] = &[
                $((
                    stringify!($field_name),
                    core::mem::offset_of!($struct_name, $field_name),
                    ($crate::__vertex_impl::BindingType::$field_ty.as_glow_ty(), $count),
                    false,
                )),+
            ];
        }

        impl $crate::Vertex for $struct_name {
            fn get_bindings() -> &'static [(&'static str, usize, (u32, i32), bool)] {
                Self::BINDINGS
            }
        }
    };
}

#[doc(hidden)]
pub mod __vertex_impl {
    #[allow(non_camel_case_types)]
    #[doc(hidden)]
    pub enum BindingType {
        f32,
        f64,
        i8,
        i16,
        i32,
        u8,
        u16,
        u32,
        bool,
    }

    impl BindingType {
        #[doc(hidden)]
        pub const fn as_glow_ty(self) -> u32 {
            match self {
                Self::f32 => glow::FLOAT,
                Self::f64 => glow::DOUBLE,
                Self::i8 => glow::BYTE,
                Self::i16 => glow::SHORT,
                Self::i32 => glow::INT,
                Self::u8 => glow::UNSIGNED_BYTE,
                Self::u16 => glow::UNSIGNED_SHORT,
                Self::u32 => glow::UNSIGNED_INT,
                Self::bool => glow::BOOL,
            }
        }
    }
}
