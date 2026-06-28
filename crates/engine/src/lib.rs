#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use std::{
    cell::Cell,
    fs::File,
    io::BufReader,
    time::{Duration, Instant},
};

use horns::RenderBackend;
use meralus_shared::{InspectMut, Point2D, USize2D, Vector2D};
use winit::{
    application::ApplicationHandler,
    error::EventLoopError,
    event::{ButtonSource, DeviceEvent, DeviceId, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    icon::RgbaIcon,
    keyboard::{ModifiersKeyState, PhysicalKey},
    raw_window_handle::{HasDisplayHandle, HasWindowHandle},
    window::{Window, WindowAttributes, WindowId},
};
pub use winit::{event::MouseButton, keyboard::KeyCode, window::CursorGrabMode};

#[derive(Debug, Clone, Copy)]
pub struct WindowContext<'a> {
    event_loop: &'a dyn ActiveEventLoop,
    window: &'a dyn Window,
    vsync: &'a Cell<bool>,
}

impl<'a> WindowContext<'a> {
    const fn new(event_loop: &'a dyn ActiveEventLoop, window: &'a dyn Window, vsync: &'a Cell<bool>) -> Self {
        Self { event_loop, window, vsync }
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

    fn new(context: WindowContext, display: &RenderBackend, args: Self::Args) -> Self;

    fn handle_window_resize(&mut self, display: &RenderBackend, size: USize2D, scale_factor: f64) {}
    fn handle_keyboard_modifiers(&mut self, modifiers: KeyboardModifiers) {}
    fn handle_keyboard_input(&mut self, key: KeyCode, is_pressed: bool, repeat: bool) {}
    fn handle_mouse_motion(&mut self, delta: Option<Vector2D>, position: Option<Point2D>) {}
    fn handle_mouse_wheel(&mut self, delta: Vector2D) {}
    fn handle_mouse_button(&mut self, button: MouseButton, is_pressed: bool) {}

    fn update(&mut self, context: WindowContext, display: &RenderBackend, delta: Duration) {}
    fn render(&mut self, context: WindowContext, display: &RenderBackend, delta: Duration);
}

pub struct ApplicationWindow<T: State> {
    state: T,
    window: Box<dyn Window>,
    backend: RenderBackend,
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

        let window_attrs = WindowAttributes::default().with_transparent(false).with_title(T::NAME).with_window_icon(icon);
        let window = event_loop.create_window(window_attrs).expect("failed to create window");
        let (width, height): (u32, u32) = window.surface_size().into();
        let backend = RenderBackend::new(window.display_handle().unwrap(), window.window_handle().unwrap(), width, height).unwrap();
        let refresh_rate = window
            .current_monitor()
            .and_then(|monitor| monitor.current_video_mode())
            .and_then(|video_mode| video_mode.refresh_rate_millihertz())
            .and_then(|refresh_rate| Duration::from_secs(1).checked_div(refresh_rate.get() / 1000))
            .unwrap_or(FALLBACK_RATE);
        let vsync = Cell::new(true);

        Self {
            state: T::new(WindowContext::new(event_loop, window.as_ref(), &vsync), &backend, args),
            window,
            backend,
            last_time: None,
            vsync: vsync.get(),
            refresh_rate,
        }
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
                window.backend.resize(physical_size.width, physical_size.height).unwrap();

                window.state.handle_window_resize(
                    &window.backend,
                    USize2D::new(physical_size.width, physical_size.height),
                    window.window.scale_factor(),
                );
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
                let context = WindowContext::new(event_loop, window.window.as_ref(), &vsync);

                window.state.update(context, &window.backend, delta);
                window.state.render(context, &window.backend, delta);
                window.vsync = vsync.get();
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

            let frame_time = window.last_time.map_or(Duration::ZERO, |time| time.elapsed());

            if window.vsync && window.refresh_rate > frame_time {
                let wait = window.refresh_rate.checked_sub(frame_time).unwrap();

                event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + wait));
            }
        });
    }
}
