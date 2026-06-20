use std::mem::{MaybeUninit, forget};

use horns::{Error, Program, RenderBackend, Shader, Texture2d, VertexBuffer};

use crate::posteffects::WorldScene;

struct DownscaleProgram;

impl Shader for DownscaleProgram {
    fn fragment(&self) -> String {
        std::fs::read_to_string("./resources/shaders/downscale.fs").unwrap()
    }

    fn vertex(&self) -> String {
        std::fs::read_to_string("./resources/shaders/downscale.vs").unwrap()
    }
}

struct UpscaleProgram;

impl Shader for UpscaleProgram {
    fn fragment(&self) -> String {
        std::fs::read_to_string("./resources/shaders/upscale.fs").unwrap()
    }

    fn vertex(&self) -> String {
        std::fs::read_to_string("./resources/shaders/upscale.vs").unwrap()
    }
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
    pub fn new(backend: &RenderBackend, width: u32, height: u32) -> Result<Self, Box<dyn std::error::Error + 'static>> {
        let mut textures = [const { MaybeUninit::zeroed() }; I];

        for (i, texture) in textures.iter_mut().enumerate() {
            let scale = 2u32.pow((i + 1) as u32);

            texture.write(backend.create_empty_texture2d( width / scale, height / scale)?);
        }

        let textures = assume_array_init(textures);

        Ok(Self {
            downscale_shader: backend.create_program(&DownscaleProgram)?,
            upscale_shader: backend.create_program(&UpscaleProgram)?,

            screen_rectangle: backend.create_vertex_buffer(&super::SCREEN_RECTANGLE, false)?,

            textures,
        })
    }

    pub fn resize(&mut self, backend: &RenderBackend, [width, height]: [u32; 2]) -> Result<(), Error> {
        for (i, texture) in self.textures.iter_mut().enumerate() {
            let scale = 2u32.pow((i + 1) as u32);

            *texture = backend.create_empty_texture2d(width / scale, height / scale)?;
        }

        Ok(())
    }

    pub fn apply<'a>(&'a self, facade: &RenderBackend, input: &'a WorldScene) -> Result<(), ValidationError> {
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
