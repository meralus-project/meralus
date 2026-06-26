#![allow(clippy::missing_errors_doc, clippy::cast_sign_loss)]

use std::{num::NonZeroU32, rc::Rc};

use glow::HasContext;
use glutin::{
    config::ConfigTemplate,
    context::{ContextApi, ContextAttributesBuilder, NotCurrentGlContext, PossiblyCurrentContext},
    display::{Display, DisplayApiPreference, GlDisplay},
    surface::{GlSurface, Surface, SurfaceAttributesBuilder, WindowSurface},
};
use raw_window_handle::{DisplayHandle, WindowHandle};

use crate::{ElementType, Error, GlPrimitive, IndexBuffer, Program, RenderInfo, RenderPass, Shader, Texture2d, Vertex, VertexBuffer};

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
        #[cfg(all(not(windows), not(target_os = "macos")))]
        let display = unsafe { Display::new(display.as_raw(), DisplayApiPreference::EglThenGlx(Box::new(|_| {}))) }?;
        #[cfg(windows)]
        let display = unsafe { Display::new(display.as_raw(), DisplayApiPreference::EglThenWgl(Some(window))) }?;
        #[cfg(target_os = "macos")]
        let display = unsafe { Display::new(display.as_raw(), DisplayApiPreference::Cgl) }?;
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

        surface.set_swap_interval(&context, glutin::surface::SwapInterval::Wait(unsafe { NonZeroU32::new_unchecked(1) }))?;

        Ok(Self {
            gl: Rc::new(unsafe { glow::Context::from_loader_function_cstr(|addr| display.get_proc_address(addr)) }),
            surface,
            context,
        })
    }

    pub fn set_vsync(&self, enabled: bool) -> Result<(), Error> {
        if enabled {
            self.surface
                .set_swap_interval(&self.context, glutin::surface::SwapInterval::Wait(unsafe { NonZeroU32::new_unchecked(1) }))?;
        } else {
            self.surface.set_swap_interval(&self.context, glutin::surface::SwapInterval::DontWait)?;
        }

        Ok(())
    }

    pub fn resize(&self, width: u32, height: u32) -> Result<(), Error> {
        self.surface.resize(
            &self.context,
            NonZeroU32::new(width).ok_or_else(|| glutin::error::Error::from(glutin::error::ErrorKind::NotSupported("window width can't be zero")))?,
            NonZeroU32::new(height).ok_or_else(|| glutin::error::Error::from(glutin::error::ErrorKind::NotSupported("window height can't be zero")))?,
        );

        unsafe { self.gl.viewport(0, 0, width.cast_signed(), height.cast_signed()) };

        Ok(())
    }

    #[inline]
    pub fn create_empty_texture2d(&self, width: u32, height: u32) -> Result<Texture2d, Error> {
        Texture2d::empty(&self.gl, width, height)
    }

    #[inline]
    pub fn create_empty_texture2d_with_mipmaps(&self, width: u32, height: u32, levels: usize) -> Result<Texture2d, Error> {
        Texture2d::empty_with_mipmaps(&self.gl, width, height, levels)
    }

    #[inline]
    pub fn create_vertex_buffer<V: Vertex, S: Shader>(&self, vertices: &[V], shader: &Program, is_dynamic: bool) -> Result<VertexBuffer<V, S>, Error> {
        VertexBuffer::new(&self.gl, shader, vertices, is_dynamic)
    }

    #[inline]
    pub fn create_index_buffer<I: GlPrimitive>(&self, element_type: ElementType, indices: &[I]) -> Result<IndexBuffer<I>, Error> {
        IndexBuffer::new(&self.gl, element_type, indices)
    }

    #[inline]
    pub fn create_empty_vertex_buffer<V: Vertex, S: Shader>(&self, vertices: usize, shader: &Program, is_dynamic: bool) -> Result<VertexBuffer<V, S>, Error> {
        VertexBuffer::empty(&self.gl, shader, vertices, is_dynamic)
    }

    #[inline]
    pub fn create_empty_index_buffer<I: GlPrimitive>(&self, element_type: ElementType, indices: usize, is_dynamic: bool) -> Result<IndexBuffer<I>, Error> {
        IndexBuffer::empty(&self.gl, element_type, indices, is_dynamic)
    }

    #[inline]
    pub fn create_program<T: Shader>(&self, source: &T) -> Result<Program, Error> {
        Program::new(&self.gl, source)
    }

    #[inline]
    pub fn get_opengl_version(&self) -> &glow::Version {
        self.gl.version()
    }

    #[inline]
    pub fn get_opengl_vendor_string(&self) -> String {
        unsafe { self.gl.get_parameter_string(glow::VENDOR) }
    }

    #[inline]
    pub fn get_opengl_version_string(&self) -> String {
        unsafe { self.gl.get_parameter_string(glow::VERSION) }
    }

    #[inline]
    pub fn get_opengl_renderer_string(&self) -> String {
        unsafe { self.gl.get_parameter_string(glow::RENDERER) }
    }

    pub fn get_free_video_memory(&self) -> Option<usize> {
        let extensions = self.gl.supported_extensions();

        if extensions.contains("GL_NVX_gpu_memory_info") {
            let value = unsafe { self.gl.get_parameter_i32(0x9049) };

            Some(value as usize * 1024)
        } else if extensions.contains("GL_ATI_meminfo") {
            let value = unsafe { self.gl.get_parameter_i32(0x87FC) };

            Some(value as usize * 1024)
        } else {
            None
        }
    }

    #[inline]
    pub fn begin_pass(&self) -> RenderPass {
        RenderPass {
            gl: self.gl.clone(),
            blend: None,
            depth: None,
            finished: false,
            culling: None,
            render_info: RenderInfo::default(),
        }
    }
}
