use glium::{
    DrawError, DrawParameters, IndexBuffer, Program, Surface, Texture2d, VertexBuffer,
    backend::Facade,
    framebuffer::{DepthRenderBuffer, MultiOutputFrameBuffer},
    index::{NoIndices, PrimitiveType},
    texture::TextureCreationError,
    uniform,
};
use meralus_graphics::{Shader, impl_vertex};
use meralus_physics::{AabbSource, PhysicsBody, PhysicsContext};
use meralus_shared::{Color, Point2D, Point3D, Size3D, Transform3D, Vector3D};

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

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct ParticleVertex {
    position: Point3D,
}

impl_vertex! {
    ParticleVertex {
        position: [f32; 3]
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct ParticleInstance {
    pub world_position: Point3D,
    pub color: Vector3D,
}

impl_vertex! {
    ParticleInstance {
        world_position: [f32; 3],
        color: [f32; 3]
    }
}

const PARTICLE: [ParticleVertex; 8] = [
    ParticleVertex {
        position: Point3D::new(0.2, 0.2, 0.0),
    }, // 0
    ParticleVertex {
        position: Point3D::new(0.0, 0.2, 0.0),
    }, // 1
    ParticleVertex {
        position: Point3D::new(0.2, 0.2, 0.2),
    }, // 2
    ParticleVertex {
        position: Point3D::new(0.0, 0.2, 0.2),
    }, // 3
    ParticleVertex {
        position: Point3D::new(0.2, 0.0, 0.0),
    }, // 4
    ParticleVertex {
        position: Point3D::new(0.0, 0.0, 0.0),
    }, // 5
    ParticleVertex {
        position: Point3D::new(0.0, 0.0, 0.2),
    }, // 6
    ParticleVertex {
        position: Point3D::new(0.2, 0.0, 0.2),
    }, // 7
];

struct ParticleShader;

impl Shader for ParticleShader {
    const FRAGMENT: &str = "./resources/shaders/particle.fs";
    const VERTEX: &str = "./resources/shaders/particle.vs";
}

pub struct ParticleSystem {
    particles: Vec<PhysicsBody>,
    base_particle: VertexBuffer<ParticleVertex>,
    base_particle_indices: IndexBuffer<u32>,
    particle_instances: VertexBuffer<ParticleInstance>,
    program: Program,
}

impl ParticleSystem {
    pub fn new<F: Facade>(facade: &F) -> Self {
        Self {
            particles: Vec::new(),
            base_particle: VertexBuffer::new(facade, &PARTICLE).unwrap(),
            base_particle_indices: IndexBuffer::new(facade, PrimitiveType::TriangleStrip, &[0, 1, 4, 5, 6, 1, 3, 0, 2, 4, 7, 6, 2, 3]).unwrap(),
            particle_instances: VertexBuffer::empty_dynamic(facade, 10).unwrap(),
            program: ParticleShader::program(facade),
        }
    }

    pub fn spawn<F: Facade>(&mut self, facade: &F, position: Point3D, color: Color) {
        self.particles.push(PhysicsBody::new(position, Size3D::splat(1.0)));

        let particle_instances = self
            .particles
            .iter()
            .map(|particle| ParticleInstance {
                world_position: particle.position,
                color: color.to_linear().into(),
            })
            .collect::<Vec<_>>();

        self.particle_instances = VertexBuffer::dynamic(facade, &particle_instances).unwrap();
    }

    pub fn spawn_batch<F: Facade>(&mut self, facade: &F, positions: impl Iterator<Item = Point3D>, color: Color) {
        for position in positions {
            self.particles.push(PhysicsBody::new(position, Size3D::splat(1.0)));
        }

        let particle_instances = self
            .particles
            .iter()
            .map(|particle| ParticleInstance {
                world_position: particle.position,
                color: color.to_linear().into(),
            })
            .collect::<Vec<_>>();

        self.particle_instances = VertexBuffer::dynamic(facade, &particle_instances).unwrap();
    }

    pub fn physics_update<S: AabbSource>(&mut self, context: &PhysicsContext<S>, delta: f32) {
        let mut mapping = self.particle_instances.map();

        for (body, particle) in self.particles.iter_mut().zip(mapping.iter_mut()) {
            // context.physics_step(body, delta);

            particle.world_position.y += 0.001;
        }
    }

    pub fn render<S: Surface>(&self, surface: &mut S, matrix: Transform3D) -> Result<(), DrawError> {
        surface.draw(
            (&self.base_particle, self.particle_instances.per_instance().unwrap()),
            &self.base_particle_indices,
            &self.program,
            &uniform! { matrix: matrix.to_cols_array_2d() },
            &DrawParameters::default(),
        )
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
