use glam::{Mat4, Vec3};
use glium::{
    DrawParameters, Frame, IndexBuffer, Program, Surface, VertexBuffer,
    index::{IndicesSource, NoIndices, PrimitiveType},
    uniform,
};
use lyon_tessellation::{
    FillBuilder, FillGeometryBuilder, FillOptions, FillTessellator, FillVertex, GeometryBuilder, GeometryBuilderError, StrokeGeometryBuilder, StrokeVertex,
    TessellationError, VertexBuffers, VertexId,
    math::Transform,
    path::builder::{NoAttributes, Transformed},
};
use meralus_engine::WindowDisplay;
use meralus_shared::Color;

use super::Shader;
use crate::{BLENDING, RenderInfo, impl_vertex};

struct ShapeShader;

impl Shader for ShapeShader {
    const FRAGMENT: &str = include_str!("../../app/resources/shaders/shape.fs");
    const VERTEX: &str = include_str!("../../app/resources/shaders/shape.vs");
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapeVertex {
    pub position: Vec3,
    pub color: Color,
    pub transform: Mat4,
}

impl_vertex! {
    ShapeVertex {
        position: [f32; 3],
        color: [u8; 4],
        transform: [[f32; 4]; 4]
    }
}

pub struct Line {
    pub start: Vec3,
    pub end: Vec3,
    pub color: Color,
}

impl Line {
    pub const fn new(start: Vec3, end: Vec3, color: Color) -> Self {
        Self { start, end, color }
    }

    pub const fn as_vertices(&self) -> [ShapeVertex; 2] {
        [
            ShapeVertex {
                position: self.start,
                color: self.color,
                transform: Mat4::IDENTITY,
            },
            ShapeVertex {
                position: self.end,
                color: self.color,
                transform: Mat4::IDENTITY,
            },
        ]
    }
}

pub struct ShapeGeometryBuilder {
    buffers: VertexBuffers<ShapeVertex, u32>,
    first_vertex: u32,
    first_index: u32,
    vertex_offset: u32,
    color: Color,
}

impl ShapeGeometryBuilder {
    pub const fn new(buffers: VertexBuffers<ShapeVertex, u32>, color: Color) -> Self {
        let first_vertex = buffers.vertices.len() as u32;
        let first_index = buffers.indices.len() as u32;

        Self {
            buffers,
            first_vertex,
            first_index,
            vertex_offset: 0,
            color,
        }
    }

    pub const fn set_color(&mut self, color: Color) {
        self.color = color;
    }

    const fn get_mesh(&self) -> &VertexBuffers<ShapeVertex, u32> {
        &self.buffers
    }
}

impl GeometryBuilder for ShapeGeometryBuilder {
    fn begin_geometry(&mut self) {
        self.first_vertex = self.buffers.vertices.len() as u32;
        self.first_index = self.buffers.indices.len() as u32;
    }

    fn add_triangle(&mut self, a: VertexId, b: VertexId, c: VertexId) {
        debug_assert!(a != b);
        debug_assert!(a != c);
        debug_assert!(b != c);
        debug_assert!(a != VertexId::INVALID);
        debug_assert!(b != VertexId::INVALID);
        debug_assert!(c != VertexId::INVALID);

        self.buffers.indices.push((a + self.vertex_offset).into());
        self.buffers.indices.push((b + self.vertex_offset).into());
        self.buffers.indices.push((c + self.vertex_offset).into());
    }

    fn abort_geometry(&mut self) {
        self.buffers.vertices.truncate(self.first_vertex as usize);
        self.buffers.indices.truncate(self.first_index as usize);
    }
}

impl FillGeometryBuilder for ShapeGeometryBuilder {
    fn add_fill_vertex(&mut self, vertex: FillVertex) -> Result<VertexId, GeometryBuilderError> {
        self.buffers.vertices.push(ShapeVertex {
            position: Vec3::from_array(vertex.position().extend(0.0).to_array()),
            color: self.color,
            transform: Mat4::IDENTITY,
        });

        let len = self.buffers.vertices.len();

        if len > u32::MAX as usize {
            return Err(GeometryBuilderError::TooManyVertices);
        }

        Ok(VertexId((len - 1) as u32))
    }
}

impl StrokeGeometryBuilder for ShapeGeometryBuilder {
    fn add_stroke_vertex(&mut self, vertex: StrokeVertex) -> Result<VertexId, GeometryBuilderError> {
        self.buffers.vertices.push(ShapeVertex {
            position: Vec3::from_array(vertex.position().extend(0.0).to_array()),
            color: self.color,
            transform: Mat4::IDENTITY,
        });

        let len = self.buffers.vertices.len();

        if len > u32::MAX as usize {
            return Err(GeometryBuilderError::TooManyVertices);
        }

        Ok(VertexId((len - 1) as u32))
    }
}

pub struct ShapeTessellator {
    builder: ShapeGeometryBuilder,
    tessellator: FillTessellator,
    options: FillOptions,
}

impl ShapeTessellator {
    pub fn new() -> Self {
        let builder = ShapeGeometryBuilder::new(VertexBuffers::new(), Color::RED);
        let tessellator = FillTessellator::new();
        let options = FillOptions::default();

        Self { builder, tessellator, options }
    }

    pub fn transformed_tessellate_with_color<F: FnOnce(&mut NoAttributes<Transformed<FillBuilder, Transform>>)>(
        &mut self,
        color: Color,
        transform: Transform,
        tessellate: F,
    ) -> Result<(), TessellationError> {
        self.builder.set_color(color);

        let mut builder = self.tessellator.builder(&self.options, &mut self.builder).transformed(transform);

        tessellate(&mut builder);

        builder.build()
    }

    pub fn tessellate_with_color<F: FnOnce(NoAttributes<FillBuilder>) -> Result<(), TessellationError>>(
        &mut self,
        color: Color,
        tessellate: F,
    ) -> Result<(), TessellationError> {
        self.builder.set_color(color);

        let builder = self.tessellator.builder(&self.options, &mut self.builder);

        tessellate(builder)
    }

    pub fn build(self, display: &WindowDisplay) -> (VertexBuffer<ShapeVertex>, IndexBuffer<u32>) {
        let mesh = self.builder.get_mesh();

        (
            glium::VertexBuffer::new(display, &mesh.vertices).expect("Could not create vertex buffer"),
            glium::IndexBuffer::new(display, glium::index::PrimitiveType::TrianglesList, &mesh.indices).expect("Could not create index buffer"),
        )
    }
}

pub struct ShapeRenderer {
    shader: Program,
    matrix: Option<Mat4>,
}

impl ShapeRenderer {
    pub fn new(display: &WindowDisplay) -> Self {
        Self {
            shader: ShapeShader::program(display),
            matrix: None,
        }
    }

    pub const fn set_matrix(&mut self, matrix: Mat4) {
        self.matrix = Some(matrix);
    }

    pub const fn set_default_matrix(&mut self) {
        self.matrix = None;
    }

    pub fn draw<'a, I: Into<IndicesSource<'a>>>(&self, frame: &mut Frame, display: &WindowDisplay, vertex_buffer: &VertexBuffer<ShapeVertex>, index_buffer: I) {
        let (width, height) = display.get_framebuffer_dimensions();

        let matrix = self
            .matrix
            .unwrap_or_else(|| Mat4::orthographic_rh_gl(0.0, width as f32, height as f32, 0.0, -1.0, 1.0));

        let uniforms = uniform! {
            matrix: matrix.to_cols_array_2d(),
        };

        frame
            .draw(vertex_buffer, index_buffer, &self.shader, &uniforms, &DrawParameters {
                blend: BLENDING,
                ..DrawParameters::default()
            })
            .expect("failed to draw!");
    }

    pub fn draw_shape_vertices(&self, frame: &mut Frame, display: &WindowDisplay, vertices: &[ShapeVertex], ty: PrimitiveType) {
        let vertex_buffer = VertexBuffer::new(display, vertices).unwrap();

        let (width, height) = display.get_framebuffer_dimensions();

        let matrix = self
            .matrix
            .unwrap_or_else(|| Mat4::orthographic_rh_gl(0.0, width as f32, height as f32, 0.0, -1.0, 1.0));

        let uniforms = uniform! {
            matrix: matrix.to_cols_array_2d(),
        };

        frame
            .draw(&vertex_buffer, NoIndices(ty), &self.shader, &uniforms, &DrawParameters {
                blend: BLENDING,
                ..DrawParameters::default()
            })
            .expect("failed to draw!");
    }

    // pub fn draw_rects(
    //     &self,
    //     frame: &mut Frame,
    //     display: &WindowDisplay,
    //     rects: &[Rectangle],
    //     draw_calls: &mut usize,
    //     rendered_vertices: &mut usize,
    // ) {
    //     let vertices = rects.iter().fold(Vec::new(), |mut vertices, rect| {
    //         vertices.extend(rect.as_vertices());

    //         vertices
    //     });

    //     self.draw_shape_vertices(frame, display, &vertices,
    // PrimitiveType::TrianglesList);

    //     *draw_calls += 1;
    //     *rendered_vertices += vertices.len();
    // }

    pub fn draw_lines(&self, frame: &mut Frame, display: &WindowDisplay, lines: &[Line]) -> RenderInfo {
        let vertices = lines.iter().fold(Vec::new(), |mut vertices, line| {
            vertices.extend(line.as_vertices());

            vertices
        });

        self.draw_shape_vertices(frame, display, &vertices, PrimitiveType::LinesList);

        RenderInfo {
            draw_calls: 1,
            vertices: vertices.len(),
        }
    }
}
