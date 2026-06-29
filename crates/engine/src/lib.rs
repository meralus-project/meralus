#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, unused_crate_dependencies)]

use std::{
    cell::Cell,
    fs::File,
    io::BufReader,
    sync::Arc,
    time::{Duration, Instant},
};

use mavelin_shared::{InspectMut, Point2D, USize2D, Vector2D};
use winit::{
    application::ApplicationHandler,
    error::EventLoopError,
    event::{ButtonSource, DeviceEvent, DeviceId, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    icon::RgbaIcon,
    keyboard::{ModifiersKeyState, PhysicalKey},
    window::{Window, WindowAttributes, WindowId},
};
pub use winit::{event::MouseButton, keyboard::KeyCode, window::CursorGrabMode};

#[derive(Debug)]
pub struct Texture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl Texture {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    // 1.

    pub fn create_depth_texture(device: &wgpu::Device, width: u32, height: u32, label: &str) -> Self {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let desc = wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };

        let texture = device.create_texture(&desc);

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual), // 5.
            lod_min_clamp: 0.0,
            lod_max_clamp: 100.0,
            ..Default::default()
        });

        Self { texture, view, sampler }
    }
}
#[derive(Debug, Clone, Copy)]
pub struct WindowContext<'a> {
    pub instance: &'a wgpu::Instance,
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub depth_texture: &'a Texture,
    pub surface: &'a wgpu::Surface<'static>,
    pub surface_format: &'a wgpu::TextureFormat,
    pub adapter: &'a wgpu::Adapter,
    event_loop: &'a dyn ActiveEventLoop,
    window: &'a dyn Window,
    vsync: &'a Cell<bool>,
}

impl WindowContext<'_> {
    pub fn get_surface_texture(&self) -> Option<wgpu::SurfaceTexture> {
        match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture) => Some(texture),
            wgpu::CurrentSurfaceTexture::Occluded | wgpu::CurrentSurfaceTexture::Timeout => None,
            wgpu::CurrentSurfaceTexture::Suboptimal(texture) => {
                drop(texture);

                self.configure_surface();

                None
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.configure_surface();

                None
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                unreachable!("No error scope registered, so validation errors will panic")
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                // self.surface = self.instance.create_surface(self.window.clone()).unwrap();
                // self.configure_surface();

                None
            }
        }
    }

    fn configure_surface(&self) {
        let (width, height) = self.window.surface_size().into();

        self.surface.configure(self.device, &wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: *self.surface_format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            view_formats: vec![self.surface_format.add_srgb_suffix()],
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            width,
            height,
            desired_maximum_frame_latency: 2,
            present_mode: if self.vsync.get() {
                wgpu::PresentMode::AutoVsync
            } else {
                wgpu::PresentMode::AutoNoVsync
            },
        });
    }

    #[allow(clippy::missing_panics_doc)]
    pub fn set_cursor_grab(&self, mode: CursorGrabMode) {
        self.window.set_cursor_grab(mode).unwrap();
    }

    pub fn set_cursor_visible(&self, visible: bool) {
        self.window.set_cursor_visible(visible);
    }

    pub fn set_vsync(&self, enabled: bool) {
        self.vsync.set(enabled);
    }

    pub fn window_size(&self) -> USize2D {
        let size = self.window.surface_size();

        USize2D::new(size.width, size.height)
    }

    pub fn window_scale_factor(&self) -> f64 {
        self.window.scale_factor()
    }

    pub fn pre_present_notify(&self) {
        self.window.pre_present_notify();
    }

    pub fn close_window(&self) {
        self.event_loop.exit();
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[allow(clippy::struct_excessive_bools)]
pub struct KeyboardModifiers {
    pub alt_key: bool,
    pub control_key: bool,
    pub shift_key: bool,
    pub meta_key: bool,
}

#[allow(unused)]
pub trait State {
    type Args;

    const ICON: Option<&str>;
    const NAME: &str;

    fn new(context: WindowContext, args: Self::Args) -> Self;

    fn handle_window_resize(&mut self, context: WindowContext, size: USize2D, scale_factor: f64) {}
    fn handle_keyboard_modifiers(&mut self, modifiers: KeyboardModifiers) {}
    fn handle_keyboard_input(&mut self, key: KeyCode, is_pressed: bool, repeat: bool) {}
    fn handle_mouse_motion(&mut self, delta: Option<Vector2D>, position: Option<Point2D>) {}
    fn handle_mouse_wheel(&mut self, delta: Vector2D) {}
    fn handle_mouse_button(&mut self, button: MouseButton, is_pressed: bool) {}

    fn update(&mut self, context: WindowContext, delta: Duration) {}
    fn render(&mut self, context: WindowContext, delta: Duration);
}

pub struct ApplicationWindow<T: State> {
    state: T,
    window: Arc<dyn Window>,
    instance: wgpu::Instance,
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter: wgpu::Adapter,
    surface: wgpu::Surface<'static>,
    surface_format: wgpu::TextureFormat,
    depth_texture: Texture,
    last_time: Option<Instant>,
    vsync: bool,
    refresh_rate: Duration,
}

pub struct Application<T: State> {
    window: Option<ApplicationWindow<T>>,
    args: Option<T::Args>,
}

impl<T: State + 'static> Application<T> {
    /// # Errors
    ///
    /// May return an error from event loop
    pub fn start(self) -> Result<(), EventLoopError> {
        let event_loop = EventLoop::builder().build()?;

        event_loop.set_control_flow(ControlFlow::Poll);
        event_loop.run_app(self)?;

        Ok(())
    }
}

impl<T: State<Args = ()>> Default for Application<T> {
    fn default() -> Self {
        Self { window: None, args: Some(()) }
    }
}

impl<T: State> Application<T> {
    pub const fn new(args: T::Args) -> Self {
        Self {
            window: None,
            args: Some(args),
        }
    }
}

impl<T: State> ApplicationWindow<T> {
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub fn new(event_loop: &dyn ActiveEventLoop, args: T::Args) -> Self {
        const FALLBACK_RATE: Duration = Duration::from_secs(1).checked_div(60).unwrap();

        let icon = T::ICON.and_then(|icon| {
            let decoder = png::Decoder::new(BufReader::new(File::open(icon).unwrap()));
            let mut reader = decoder.read_info().unwrap();
            let mut buf = vec![0; reader.output_buffer_size().unwrap()];
            let info = reader.next_frame(&mut buf).unwrap();

            RgbaIcon::new(buf[..info.buffer_size()].to_vec(), info.width, info.height).map(Into::into).ok()
        });

        let window: Arc<dyn Window> = Arc::from(
            event_loop
                .create_window(WindowAttributes::default().with_transparent(false).with_title(T::NAME).with_window_icon(icon))
                .expect("failed to create window"),
        );

        let (width, height): (u32, u32) = window.surface_size().into();

        let instance = wgpu::Instance::new(
            wgpu::InstanceDescriptor::new_without_display_handle()
                .with_display_handle(Box::new(event_loop.owned_display_handle()))
                .with_window_handle(Box::new(window.clone())),
        );

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).unwrap();

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_limits: wgpu::Limits {
                max_compute_workgroup_size_x: 0,
                max_compute_workgroup_size_y: 0,
                max_compute_workgroup_size_z: 0,
                max_compute_workgroups_per_dimension: 0,
                max_compute_invocations_per_workgroup: 0,
                max_compute_workgroup_storage_size: 0,
                max_storage_buffer_binding_size: 0,
                max_storage_buffers_per_shader_stage: 0,
                max_storage_textures_per_shader_stage: 0,
                max_dynamic_storage_buffers_per_pipeline_layout: 0,
                max_texture_dimension_1d: 8192,
                max_texture_dimension_2d: 4096,
                max_immediate_size: 96,
                ..wgpu::Limits::downlevel_defaults()
            },
            required_features: wgpu::Features::IMMEDIATES,
            ..wgpu::DeviceDescriptor::default()
        }))
        .unwrap();

        let surface = instance.create_surface(window.clone()).unwrap();
        let cap = surface.get_capabilities(&adapter);
        let format = cap.formats[0];

        // let backend = RenderBackend::new(window.display_handle().unwrap(),
        // window.window_handle().unwrap(), width, height).unwrap();
        let refresh_rate = window
            .current_monitor()
            .and_then(|monitor| monitor.current_video_mode())
            .and_then(|video_mode| video_mode.refresh_rate_millihertz())
            .and_then(|refresh_rate| Duration::from_secs(1).checked_div(refresh_rate.get() / 1000))
            .unwrap_or(FALLBACK_RATE);
        let vsync = Cell::new(false);

        let depth_texture = Texture::create_depth_texture(&device, width, height, "Mavelin Depth Texture");

        // backend.set_vsync(false).unwrap();

        let state = T::new(
            WindowContext {
                instance: &instance,
                device: &device,
                queue: &queue,
                surface: &surface,
                surface_format: &format,
                event_loop,
                window: window.as_ref(),
                vsync: &vsync,
                depth_texture: &depth_texture,
                adapter: &adapter,
            },
            args,
        );

        let mut this = Self {
            state,
            window,
            last_time: None,
            vsync: vsync.get(),
            refresh_rate,
            instance,
            device,
            queue,
            surface,
            surface_format: format,
            depth_texture,
            adapter,
        };

        this.configure_surface(width, height);

        this
    }

    fn configure_surface(&mut self, width: u32, height: u32) {
        self.depth_texture = Texture::create_depth_texture(&self.device, width, height, "Mavelin Depth Texture");
        self.surface.configure(&self.device, &wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: self.surface_format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            view_formats: vec![self.surface_format.add_srgb_suffix()],
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            width,
            height,
            desired_maximum_frame_latency: 2,
            present_mode: 
                wgpu::PresentMode::AutoNoVsync
            ,
        });
    }
}

impl<T: State> ApplicationHandler for Application<T> {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        if let Some(args) = self.args.take() {
            self.window.replace(ApplicationWindow::new(event_loop, args));
        }
    }

    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::SurfaceResized(physical_size) => self.window.inspect_mut(move |window| {
                let vsync = Cell::new(window.vsync);

                window.state.handle_window_resize(
                    WindowContext {
                        instance: &window.instance,
                        device: &window.device,
                        queue: &window.queue,
                        surface: &window.surface,
                        surface_format: &window.surface_format,
                        event_loop,
                        window: window.window.as_ref(),
                        vsync: &vsync,
                        depth_texture: &window.depth_texture,
                        adapter: &window.adapter,
                    },
                    USize2D::new(physical_size.width, physical_size.height),
                    window.window.scale_factor(),
                );

                window.vsync = vsync.get();
                window.configure_surface(physical_size.width, physical_size.height);
            }),
            WindowEvent::ModifiersChanged(modifiers) => {
                let state = modifiers.state();

                self.window.inspect_mut(move |window| {
                    window.state.handle_keyboard_modifiers(KeyboardModifiers {
                        alt_key: matches!(modifiers.lalt_state(), ModifiersKeyState::Pressed)
                            | matches!(modifiers.ralt_state(), ModifiersKeyState::Pressed)
                            | state.alt_key(),
                        control_key: matches!(modifiers.lcontrol_state(), ModifiersKeyState::Pressed)
                            | matches!(modifiers.rcontrol_state(), ModifiersKeyState::Pressed)
                            | state.control_key(),
                        shift_key: matches!(modifiers.lshift_state(), ModifiersKeyState::Pressed)
                            | matches!(modifiers.rshift_state(), ModifiersKeyState::Pressed)
                            | state.shift_key(),
                        meta_key: matches!(modifiers.lsuper_state(), ModifiersKeyState::Pressed)
                            | matches!(modifiers.rsuper_state(), ModifiersKeyState::Pressed)
                            | state.meta_key(),
                    });
                });
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    self.window.inspect_mut(|window| {
                        window.state.handle_keyboard_input(code, event.state.is_pressed(), event.repeat);
                    });
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let delta = match delta {
                    MouseScrollDelta::LineDelta(x, y) => Vector2D::new(x, y),
                    MouseScrollDelta::PixelDelta(delta) => Vector2D::new(delta.x as f32, delta.y as f32),
                };

                self.window.inspect_mut(|window| {
                    window.state.handle_mouse_wheel(delta);
                });
            }
            WindowEvent::PointerMoved { position, .. } => {
                self.window.inspect_mut(|window| {
                    window.state.handle_mouse_motion(None, Some(Point2D::new(position.x as f32, position.y as f32)));
                });
            }
            WindowEvent::PointerButton {
                state,
                button: ButtonSource::Mouse(button),
                ..
            } => {
                self.window.inspect_mut(|window| {
                    window.state.handle_mouse_button(button, state.is_pressed());
                });
            }
            WindowEvent::RedrawRequested => self.window.inspect_mut(|window| {
                let now = Instant::now();
                let delta = now.duration_since(window.last_time.unwrap_or_else(Instant::now));

                window.last_time.replace(now);

                let vsync = Cell::new(window.vsync);
                let context = WindowContext {
                    instance: &window.instance,
                    device: &window.device,
                    queue: &window.queue,
                    surface: &window.surface,
                    surface_format: &window.surface_format,
                    event_loop,
                    window: window.window.as_ref(),
                    vsync: &vsync,
                    depth_texture: &window.depth_texture,
                    adapter: &window.adapter,
                };

                window.state.update(context, delta);
                window.state.render(context, delta);
                
                let prev_vsync = window.vsync;

                window.vsync = vsync.get();

                if window.vsync != window.vsync {
                    let (width, height) = window.window.surface_size().into();

                    window.configure_surface(width, height);
                }
            }),
            WindowEvent::CloseRequested => event_loop.exit(),
            _ => {}
        }
    }

    fn device_event(&mut self, _: &dyn ActiveEventLoop, _: Option<DeviceId>, event: DeviceEvent) {
        if let DeviceEvent::PointerMotion { delta } = event {
            self.window.inspect_mut(|window| {
                window.state.handle_mouse_motion(Some(Vector2D::new(delta.0 as f32, delta.1 as f32)), None);
            });
        }
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.window.inspect_mut(|window| {
            window.window.request_redraw();

            // let frame_time = window.last_time.map_or(Duration::ZERO, |time| time.elapsed());

            // if window.vsync && window.refresh_rate > frame_time {
            //     let wait = window.refresh_rate.checked_sub(frame_time).unwrap();

            //     event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + wait));
            // }
        });
    }
}
