#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc
)]

mod common;
mod context;
#[cfg(feature = "ellay")] pub mod ellay;
#[cfg(feature = "voxel-rendering")] mod voxel;

use std::{fmt, fs};

#[doc(hidden)] pub use glium as __glium;
use glium::{
    Blend, BlendingFunction, IndexBuffer, LinearBlendingFactor, Program, Vertex, VertexBuffer,
    index::{Index, IndexBufferSlice, PrimitiveType},
    vertex::VertexBufferSlice,
};
use meralus_engine::WindowDisplay;

#[cfg(feature = "shape-rendering")]
pub use self::common::{CommonTessellator, Path};
#[cfg(feature = "voxel-rendering")] pub use self::voxel::*;
pub use self::{
    common::{CommonRenderer, CommonVertex, ObjectFit},
    context::{RenderContext, RenderInfo},
};

pub const FONT: &[u8] = include_bytes!("../../../resources/fonts/Monocraft.ttf");
pub const FONT_BOLD: &[u8] = include_bytes!("../../../resources/fonts/Monocraft-Bold.ttf");
pub const BLENDING: Blend = Blend {
    color: BlendingFunction::Addition {
        source: LinearBlendingFactor::SourceAlpha,
        destination: LinearBlendingFactor::OneMinusSourceAlpha,
    },
    alpha: BlendingFunction::Addition {
        source: LinearBlendingFactor::One,
        destination: LinearBlendingFactor::OneMinusSourceAlpha,
    },
    constant_value: (0.0, 0.0, 0.0, 0.0),
};

#[macro_export]
macro_rules! impl_vertex {
    ($struct_name:ident { $($field_name:ident: $field_ty:ty),+ }) => {
        impl $struct_name {
            const BINDINGS: &[(std::borrow::Cow<'static, str>, usize, i32, $crate::__glium::vertex::AttributeType, bool)] = &[
                $((
                    std::borrow::Cow::Borrowed(stringify!($field_name)),
                    $crate::__glium::__glium_offset_of!($struct_name, $field_name),
                    -1,
                    <$field_ty as $crate::__glium::vertex::Attribute>::TYPE,
                    false,
                )),+
            ];
        }

        impl $crate::__glium::Vertex for $struct_name {
            fn build_bindings() -> $crate::__glium::VertexFormat {
                Self::BINDINGS
            }
        }
    };
}

pub trait Shader {
    const VERTEX: &str;
    const FRAGMENT: &str;
    const GEOMETRY: Option<&str> = None;

    fn program<F: glium::backend::Facade>(facade: &F) -> Program {
        let vertex = fs::read_to_string(Self::VERTEX).unwrap();
        let fragment = fs::read_to_string(Self::FRAGMENT).unwrap();

        Program::from_source(facade, &vertex, &fragment, Self::GEOMETRY).unwrap()
    }
}

pub struct CachedBuffers<V: Copy + Vertex, I: Index> {
    vertices: VertexBuffer<V>,
    indices: IndexBuffer<I>,
}

#[derive(Debug, Clone, Copy)]
pub enum CreationError {
    Vertex(glium::vertex::BufferCreationError),
    Index(glium::index::BufferCreationError),
}

impl fmt::Display for CreationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Vertex(creation_error) => creation_error.fmt(f),
            Self::Index(creation_error) => creation_error.fmt(f),
        }
    }
}

impl std::error::Error for CreationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Vertex(creation_error) => creation_error.source(),
            Self::Index(creation_error) => creation_error.source(),
        }
    }
}

impl From<glium::vertex::BufferCreationError> for CreationError {
    fn from(value: glium::vertex::BufferCreationError) -> Self {
        Self::Vertex(value)
    }
}

impl From<glium::index::BufferCreationError> for CreationError {
    fn from(value: glium::index::BufferCreationError) -> Self {
        Self::Index(value)
    }
}

impl<V: Copy + Vertex, I: Index> CachedBuffers<V, I> {
    pub fn new(display: &WindowDisplay, vertices: &[V], primitive_type: PrimitiveType, indices: &[I]) -> Result<Self, CreationError> {
        Ok(Self {
            vertices: VertexBuffer::new(display, vertices)?,
            indices: IndexBuffer::new(display, primitive_type, indices)?,
        })
    }

    pub fn try_write<'a>(
        &'a mut self,
        display: &WindowDisplay,
        vertices: &[V],
        indices: &[I],
    ) -> Result<(VertexBufferSlice<'a, V>, IndexBufferSlice<'a, I>), CreationError> {
        if self.vertices.len() < vertices.len() {
            self.vertices = VertexBuffer::new(display, vertices)?;
        } else if let Some(buffer) = self.vertices.slice_mut(0..vertices.len()) {
            buffer.write(vertices);
        }

        if self.indices.len() < indices.len() {
            let primitive_type = self.indices.get_primitives_type();

            self.indices = IndexBuffer::new(display, primitive_type, indices)?;
        } else if let Some(buffer) = self.indices.slice_mut(0..indices.len()) {
            buffer.write(indices);
        }

        Ok((self.vertices.slice(0..vertices.len()).unwrap(), self.indices.slice(0..indices.len()).unwrap()))
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
