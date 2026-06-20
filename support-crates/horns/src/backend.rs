use std::{num::NonZeroU32, rc::Rc};

use glutin::{
    config::ConfigTemplate,
    context::{ContextApi, ContextAttributesBuilder, NotCurrentGlContext, PossiblyCurrentContext},
    display::{Display, DisplayApiPreference, GlDisplay},
    surface::{GlSurface, Surface, SurfaceAttributesBuilder, WindowSurface},
};
use raw_window_handle::{DisplayHandle, WindowHandle};

use crate::{ElementType, Error, GlPrimitive, IndexBuffer, Program, RenderPass, Shader, Texture2d, Vertex, VertexBuffer};

pub struct RenderBackend {
    pub(crate) gl: Rc<glow::Context>,
    pub(crate) surface: Surface<WindowSurface>,
    pub(crate) context: PossiblyCurrentContext,
}

impl RenderBackend {
    /// # Errors
    ///
    /// Will return [`Error`] if backend creation failed.
    ///
    /// [`Error`]: crate::Error
    pub fn new(display: DisplayHandle, window: WindowHandle, width: u32, height: u32) -> Result<Self, Error>
    where
        Self: Sized,
    {
        let window = window.as_raw();
        let display = unsafe { Display::new(display.as_raw(), DisplayApiPreference::Egl) }?;
        let config = unsafe { display.find_configs(ConfigTemplate::default()) }?
            .next()
            .ok_or_else(|| glutin::error::Error::from(glutin::error::ErrorKind::NotSupported("no config")))?;

        let context = unsafe {
            display
                .create_context(&config, &ContextAttributesBuilder::new().build(Some(window)))
                .or_else(|_| {
                    display.create_context(
                        &config,
                        &ContextAttributesBuilder::new().with_context_api(ContextApi::Gles(None)).build(Some(window)),
                    )
                })
        }?;

        let surface = unsafe {
            display.create_window_surface(
                &config,
                &SurfaceAttributesBuilder::<WindowSurface>::new().build(
                    window,
                    NonZeroU32::new(width).ok_or_else(|| glutin::error::Error::from(glutin::error::ErrorKind::NotSupported("window width can't be zero")))?,
                    NonZeroU32::new(height).ok_or_else(|| glutin::error::Error::from(glutin::error::ErrorKind::NotSupported("window height can't be zero")))?,
                ),
            )?
        };

        let context = context.make_current(&surface)?;

        surface.set_swap_interval(&context, glutin::surface::SwapInterval::DontWait)?;

        Ok(Self {
            gl: Rc::new(unsafe { glow::Context::from_loader_function_cstr(|addr| display.get_proc_address(addr)) }),
            surface,
            context,
        })
    }

    pub fn resize(&self, width: u32, height: u32) -> Result<(), Error> {
        self.surface.resize(
            &self.context,
            NonZeroU32::new(width).ok_or_else(|| glutin::error::Error::from(glutin::error::ErrorKind::NotSupported("window width can't be zero")))?,
            NonZeroU32::new(height).ok_or_else(|| glutin::error::Error::from(glutin::error::ErrorKind::NotSupported("window height can't be zero")))?,
        );

        Ok(())
    }

    pub fn create_empty_texture2d(&self, width: u32, height: u32) -> Result<Texture2d, Error> {
        Texture2d::empty(&self.gl, width, height)
    }

    pub fn create_vertex_buffer<V: Vertex>(&self, vertices: &[V], is_dynamic: bool) -> Result<VertexBuffer<V>, Error> {
        VertexBuffer::new(&self.gl, vertices, is_dynamic)
    }

    pub fn create_index_buffer<I: GlPrimitive>(&self, element_type: ElementType, indices: &[I]) -> Result<IndexBuffer<I>, Error> {
        IndexBuffer::new(&self.gl, element_type, indices)
    }

    pub fn create_program<T: Shader>(&self, source: &T) -> Result<Program, Error> {
        Program::new(&self.gl, source)
    }

    pub fn begin_pass(&self) -> RenderPass {
        RenderPass { backend: self }
    }
}
