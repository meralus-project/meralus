use std::mem::{MaybeUninit, forget};

use glium::{
    DrawParameters, Program, Surface, Texture2d, VertexBuffer,
    backend::Facade,
    framebuffer::{SimpleFrameBuffer, ValidationError},
    index::{NoIndices, PrimitiveType},
    texture::TextureCreationError,
    uniform,
    uniforms::{MagnifySamplerFilter, MinifySamplerFilter},
    vertex::MultiVerticesSource,
};
use meralus_graphics::Shader;

use crate::posteffects::WorldScene;

struct DownscaleProgram;

impl Shader for DownscaleProgram {
    const FRAGMENT: &str = "./resources/shaders/downscale.fs";
    const VERTEX: &str = "./resources/shaders/downscale.vs";
}

struct UpscaleProgram;

impl Shader for UpscaleProgram {
    const FRAGMENT: &str = "./resources/shaders/upscale.fs";
    const VERTEX: &str = "./resources/shaders/upscale.vs";
}

pub struct DualKawase<const I: usize> {
    downscale_shader: Program,
    upscale_shader: Program,

    screen_rectangle: VertexBuffer<super::BasicVertex>,

    textures: [Texture2d; I],
}

fn assume_array_init<T, const I: usize>(arr: [MaybeUninit<T>; I]) -> [T; I] {
    let ptr = (&raw const arr).cast::<[T; I]>();
    let value = unsafe { ptr.read() };

    forget(arr);

    value
}

impl<const I: usize> DualKawase<I> {
    pub fn new<F: Facade>(facade: &F, width: u32, height: u32) -> Result<Self, Box<dyn std::error::Error + 'static>> {
        let mut textures = [const { MaybeUninit::zeroed() }; I];

        for (i, texture) in textures.iter_mut().enumerate() {
            let scale = 2u32.pow((i + 1) as u32);

            texture.write(Texture2d::empty(facade, width / scale, height / scale)?);
        }

        let textures = assume_array_init(textures);

        Ok(Self {
            downscale_shader: DownscaleProgram::program(facade),
            upscale_shader: UpscaleProgram::program(facade),

            screen_rectangle: VertexBuffer::new(facade, &super::SCREEN_RECTANGLE)?,

            textures,
        })
    }

    pub fn resize<F: Facade>(&mut self, facade: &F, [width, height]: [u32; 2]) -> Result<(), TextureCreationError> {
        for (i, texture) in self.textures.iter_mut().enumerate() {
            let scale = 2u32.pow((i + 1) as u32);

            *texture = Texture2d::empty(facade, width / scale, height / scale)?;
        }

        Ok(())
    }

    pub fn apply<'a, F: Facade>(&'a self, facade: &F, input: &'a WorldScene) -> Result<(), ValidationError> {
        fn render_fbo<'a, V: MultiVerticesSource<'a> + Copy>(input: &Texture2d, target: &mut SimpleFrameBuffer<'_>, vertices: V, shader: &Program) {
            let (width, height) = target.get_dimensions();
            let (width, height) = (width as f32, height as f32);
            let uniforms = uniform! {
                texture: input
                    .sampled()
                    .minify_filter(MinifySamplerFilter::Linear)
                    .magnify_filter(MagnifySamplerFilter::Linear),
                resolution: [width, height],
                half_pixel: [0.5 / width, 0.5 / height]
            };

            target
                .draw(vertices, NoIndices(PrimitiveType::TriangleStrip), shader, &uniforms, &DrawParameters::default())
                .unwrap();
        }

        let mut buffers = [const { MaybeUninit::zeroed() }; I];

        for (texture, buffer) in self.textures.iter().zip(buffers.iter_mut()) {
            buffer.write(SimpleFrameBuffer::new(facade, texture)?);
        }

        let mut main_buffer = SimpleFrameBuffer::new(facade, &input.bright_color_attachment)?;
        let mut buffers = assume_array_init(buffers);

        render_fbo(&input.bright_color_attachment, &mut buffers[0], &self.screen_rectangle, &self.downscale_shader);

        for i in 0..(I - 1) {
            render_fbo(&self.textures[i], &mut buffers[i + 1], &self.screen_rectangle, &self.downscale_shader);
        }

        for i in (1..=I).rev() {
            let texture = &self.textures[i - 1];
            let buffer = if i == 1 { &mut main_buffer } else { &mut buffers[i - 2] };

            render_fbo(texture, buffer, &self.screen_rectangle, &self.upscale_shader);
        }

        Ok(())
    }
}
