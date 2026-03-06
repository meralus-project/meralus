use glium::{
    DrawError, DrawParameters, Program, Surface, Texture2d, VertexBuffer, backend::Facade, framebuffer::{DepthRenderBuffer, MultiOutputFrameBuffer}, index::{NoIndices, PrimitiveType}, texture::TextureCreationError, uniform
};
use meralus_graphics::{Shader, impl_vertex};
use meralus_shared::{Point2D, Point3D};

pub mod kawase;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct BasicVertex {
    position: Point3D,
    uv: Point2D,
}

impl_vertex! {
    BasicVertex {
        position: [f32; 3],
        uv: [f32; 2]
    }
}

const SCREEN_RECTANGLE: [BasicVertex; 4] = [
    BasicVertex {
        position: Point3D::new(-1.0, 1.0, 0.0),
        uv: Point2D::new(0.0, 1.0),
    },
    BasicVertex {
        position: Point3D::new(-1.0, -1.0, 0.0),
        uv: Point2D::new(0.0, 0.0),
    },
    BasicVertex {
        position: Point3D::new(1.0, 1.0, 0.0),
        uv: Point2D::new(1.0, 1.0),
    },
    BasicVertex {
        position: Point3D::new(1.0, -1.0, 0.0),
        uv: Point2D::new(1.0, 0.0),
    },
];

struct BloomProgram;

impl Shader for BloomProgram {
    const FRAGMENT: &str = "./resources/shaders/bloom.fs";
    const VERTEX: &str = "./resources/shaders/bloom.vs";
}

pub struct WorldScene {
    main_color_attachment: Texture2d,
    bright_color_attachment: Texture2d,
    depth_buffer: DepthRenderBuffer,

    screen_rectangle: VertexBuffer<BasicVertex>,

    program: Program,
}

impl WorldScene {
    pub fn new<F: Facade>(facade: &F, width: u32, height: u32) -> Result<Self, Box<dyn std::error::Error + 'static>> {
        let main_color_attachment = Texture2d::empty(facade, width, height)?;
        let bright_color_attachment = Texture2d::empty(facade, width, height)?;
        let depth_buffer = DepthRenderBuffer::new(facade, glium::texture::DepthFormat::F32, width, height)?;

        Ok(Self {
            main_color_attachment,
            bright_color_attachment,
            depth_buffer,

            screen_rectangle: VertexBuffer::new(facade, &SCREEN_RECTANGLE)?,

            program: BloomProgram::program(facade),
        })
    }

    pub fn resize<F: Facade>(&mut self, facade: &F, [width, height]: [u32; 2]) -> Result<(), TextureCreationError> {
        self.main_color_attachment = Texture2d::empty(facade, width, height)?;
        self.bright_color_attachment = Texture2d::empty(facade, width, height)?;
        self.depth_buffer = DepthRenderBuffer::new(facade, glium::texture::DepthFormat::F32, width, height).unwrap();

        Ok(())
    }

    pub fn buffer<F: Facade>(&self, facade: &F) -> MultiOutputFrameBuffer<'_> {
        MultiOutputFrameBuffer::with_depth_buffer(
            facade,
            [("f_color", &self.main_color_attachment), ("f_bright_color", &self.bright_color_attachment)],
            &self.depth_buffer,
        )
        .unwrap()
    }

    pub fn render<S: Surface>(&self, surface: &mut S) -> Result<(), DrawError> {
        let uniforms = uniform! {
            scene: &self.main_color_attachment,
            bright: &self.bright_color_attachment
        };

        surface.draw(
            &self.screen_rectangle,
            NoIndices(PrimitiveType::TriangleStrip),
            &self.program,
            &uniforms,
            &DrawParameters::default(),
        )
    }
}
