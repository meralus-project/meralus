use horns::{ElementType, IndexBuffer, Program, RenderBackend, RenderPass, Shader, Texture2d, VertexBuffer, impl_vertex};
use meralus_physics::{AabbSource, PhysicsBody, PhysicsContext, RayCastResult};
use meralus_shared::{Color, Point2D, Point3D, Size3D, Transform3D, Vector3D};

pub mod kawase;

#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
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

#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct ParticleVertex {
    position: Point3D,
}

impl_vertex! {
    ParticleVertex {
        position: [f32; 3]
    }
}

#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
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
    fn fragment(&self) -> String {
        std::fs::read_to_string("./resources/shaders/particle.fs").unwrap()
    }

    fn vertex(&self) -> String {
        std::fs::read_to_string("./resources/shaders/particle.vs").unwrap()
    }
}

pub struct ParticleSystem {
    particles: Vec<PhysicsBody>,
    base_particle: VertexBuffer<ParticleVertex>,
    base_particle_indices: IndexBuffer<u32>,
    particle_instances: VertexBuffer<ParticleInstance>,
    program: Program,
}

impl ParticleSystem {
    pub fn new(backend: &RenderBackend) -> Self {
        Self {
            particles: Vec::new(),
            base_particle: backend.create_vertex_buffer(&PARTICLE).unwrap(),
            base_particle_indices: backend
                .create_index_buffer(ElementType::TriangleStrip, &[0, 1, 4, 5, 6, 1, 3, 0, 2, 4, 7, 6, 2, 3])
                .unwrap(),
            particle_instances: VertexBuffer::empty_dynamic(backend, 10).unwrap(),
            program: backend.create_program(&ParticleShader).unwrap(),
        }
    }

    pub fn spawn(&mut self, backend: &RenderBackend, position: Point3D, color: Color) {
        self.particles.push(PhysicsBody::new(position, Size3D::splat(1.0)));

        let particle_instances = self
            .particles
            .iter()
            .map(|particle| ParticleInstance {
                world_position: particle.position,
                color: color.to_linear().into(),
            })
            .collect::<Vec<_>>();

        self.particle_instances = backend.create_vertex_buffer(&particle_instances, false).unwrap();
    }

    pub fn spawn_batch(&mut self, backend: &RenderBackend, positions: impl Iterator<Item = Point3D>, color: Color) {
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

        self.particle_instances = backend.create_vertex_buffer(&particle_instances, false).unwrap();
    }

    pub fn physics_update<S: AabbSource>(&mut self, context: &PhysicsContext<S>, delta: f32) {
        let mut mapping = self.particle_instances.map();

        for (body, particle) in self.particles.iter_mut().zip(mapping.iter_mut()) {
            body.velocity += Vector3D::Z * -3.8 * delta;

            let origin = body.position.as_dvec3();
            let speed = body.velocity.length();

            let (dist, norm) = context
                .raycast(origin, origin + body.velocity.as_dvec3() * delta as f64, true)
                .filter(RayCastResult::is_block)
                .map_or((0.0, Vector3D::ZERO), |raycast| {
                    (body.position.distance(raycast.position.as_vec3()), raycast.hit_side.as_normal().as_vec3())
                });

            if dist <= speed * delta {
                const BOUNCENESS: f32 = 0.05;
                const DAMPENING: f32 = 0.05;

                body.position += body.velocity / speed * (dist - 0.001);
                body.velocity = (body.velocity.reflect(norm) * 0.5f32.mul_add(BOUNCENESS, 0.5) + body.velocity * 0.5f32.mul_add(-BOUNCENESS, 0.5)).normalize()
                    * speed
                    * DAMPENING;
            } else {
                body.position += body.velocity * delta;
            }

            particle.world_position += body.position;
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
    fn fragment(&self) -> String {
        std::fs::read_to_string("./resources/shaders/bloom.fs").unwrap()
    }

    fn vertex(&self) -> String {
        std::fs::read_to_string("./resources/shaders/bloom.vs").unwrap()
    }
}

pub struct WorldScene {
    main_color_attachment: Texture2d,
    bright_color_attachment: Texture2d,
    depth_buffer: DepthRenderBuffer,

    screen_rectangle: VertexBuffer<BasicVertex>,

    program: Program,
}

impl WorldScene {
    pub fn new(backend: &RenderBackend, width: u32, height: u32) -> Result<Self, Box<dyn std::error::Error + 'static>> {
        let main_color_attachment = backend.create_empty_texture2d(width, height)?;
        let bright_color_attachment = backend.create_empty_texture2d(width, height)?;
        let depth_buffer = DepthRenderBuffer::new(backend, glium::texture::DepthFormat::F32, width, height)?;

        Ok(Self {
            main_color_attachment,
            bright_color_attachment,
            depth_buffer,

            screen_rectangle: backend.create_vertex_buffer(&SCREEN_RECTANGLE, false)?,

            program: backend.create_program(&BloomProgram),
        })
    }

    pub fn resize(&mut self, backend: &RenderBackend, [width, height]: [u32; 2]) -> Result<(), TextureCreationError> {
        self.main_color_attachment = backend.create_empty_texture2d(width, height)?;
        self.bright_color_attachment = backend.create_empty_texture2d(width, height)?;
        self.depth_buffer = DepthRenderBuffer::new(backend, glium::texture::DepthFormat::F32, width, height).unwrap();

        Ok(())
    }

    pub fn buffer(&self, facade: &RenderBackend) -> MultiOutputFrameBuffer<'_> {
        MultiOutputFrameBuffer::with_depth_buffer(
            facade,
            [("f_color", &self.main_color_attachment), ("f_bright_color", &self.bright_color_attachment)],
            &self.depth_buffer,
        )
        .unwrap()
    }

    pub fn render(&self, surface: &RenderPass) -> Result<(), DrawError> {
        self.program
            .bind()
            .with_uniform("scene", &self.main_color_attachment)
            .with_uniform("bright", &self.bright_color_attachment);

        surface.draw_arrays(&self.program, &self.screen_rectangle, ElementType::TriangleStrip);

        Ok(())
    }
}
