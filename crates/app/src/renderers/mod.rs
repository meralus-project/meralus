use glium::Program;
use meralus_engine::WindowDisplay;

pub use self::{
    shape::{Line, Rectangle, ShapeTessellator, ShapeRenderer},
    text::{FONT, FONT_BOLD, TextRenderer},
    voxel::{Voxel, VoxelRenderer},
};

mod shape;
mod text;
mod voxel;

#[macro_export]
macro_rules! impl_vertex {
    ($struct_name:ident { $($field_name:ident: $field_ty:ty),+ }) => {
        impl $struct_name {
            const BINDINGS: &[(std::borrow::Cow<'static, str>, usize, i32, glium::vertex::AttributeType, bool)] = &[
                $((
                    std::borrow::Cow::Borrowed(stringify!($field_name)),
                    glium::__glium_offset_of!($struct_name, $field_name),
                    -1,
                    <$field_ty as glium::vertex::Attribute>::TYPE,
                    false,
                )),+
            ];
        }

        impl glium::Vertex for $struct_name {
            fn build_bindings() -> glium::VertexFormat {
                Self::BINDINGS
            }
        }
    };
}

pub trait Shader {
    const VERTEX: &str;
    const FRAGMENT: &str;
    const GEOMETRY: Option<&str> = None;

    fn program(display: &WindowDisplay) -> Program {
        Program::from_source(display, Self::VERTEX, Self::FRAGMENT, Self::GEOMETRY).unwrap()
    }
}
