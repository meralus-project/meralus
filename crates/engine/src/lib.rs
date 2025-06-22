#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use std::{
    num::NonZeroU32,
    time::{Duration, Instant},
};

use glam::{UVec2, Vec2, uvec2, vec2};
use glium::Display;
use glutin::{
    config::ConfigTemplateBuilder,
    context::{ContextApi, ContextAttributesBuilder},
    display::GetGlDisplay,
    prelude::{GlDisplay, NotCurrentGlContext},
    surface::{SurfaceAttributesBuilder, WindowSurface},
};
use glutin_winit::DisplayBuilder;
use meralus_shared::InspectMut;
use winit::{
    application::ApplicationHandler,
    error::EventLoopError,
    event::{DeviceEvent, DeviceId, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::PhysicalKey,
    raw_window_handle::HasWindowHandle,
    window::{Window, WindowId},
};
pub use winit::{event::MouseButton, keyboard::KeyCode, window::CursorGrabMode};

pub type WindowDisplay = Display<WindowSurface>;

#[derive(Debug, Clone, Copy)]
pub struct WindowContext<'a> {
    event_loop: &'a ActiveEventLoop,
    window: &'a Window,
}

impl<'a> WindowContext<'a> {
    const fn new(event_loop: &'a ActiveEventLoop, window: &'a Window) -> Self {
        Self { event_loop, window }
    }

    #[allow(clippy::missing_panics_doc)]
    pub fn set_cursor_grab(&self, mode: CursorGrabMode) {
        self.window.set_cursor_grab(mode).unwrap();
    }

    pub fn set_cursor_visible(&self, visible: bool) {
        self.window.set_cursor_visible(visible);
    }

    pub fn close_window(&self) {
        self.event_loop.exit();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[allow(clippy::struct_excessive_bools)]
pub struct KeyboardModifiers {
    alt_key: bool,
    control_key: bool,
    shift_key: bool,
    super_key: bool,
}

#[allow(unused)]
pub trait State {
    fn new(context: WindowContext, display: &WindowDisplay) -> Self;

    fn handle_window_resize(&mut self, size: UVec2, scale_factor: f64) {}
    fn handle_keyboard_modifiers(&mut self, modifiers: KeyboardModifiers) {}
    fn handle_keyboard_input(&mut self, key: KeyCode, is_pressed: bool, repeat: bool) {}
    fn handle_mouse_motion(&mut self, position: Vec2) {}
    fn handle_mouse_wheel(&mut self, delta: Vec2) {}
    fn handle_mouse_button(&mut self, button: MouseButton, is_pressed: bool) {}

    // /// Runs every 50ms
    // fn tick(&mut self, event_loop: &ActiveEventLoop, display: &WindowDisplay,
    // delta: Duration) {} /// Runs every 16.66ms
    // fn fixed_update(&mut self, event_loop: &ActiveEventLoop, display:
    // &WindowDisplay, delta: f32) {}
    fn update(&mut self, context: WindowContext, display: &WindowDisplay, delta: Duration) {}
    fn render(&mut self, display: &WindowDisplay, delta: Duration);
}

pub struct ApplicationWindow<T: State> {
    state: T,
    window: Window,
    display: WindowDisplay,
    last_time: Option<Instant>,
    delta: Duration,
}

pub struct Application<T: State> {
    window: Option<ApplicationWindow<T>>,
}

impl<T: State> Application<T> {
    /// # Errors
    ///
    /// May return an error from event loop
    pub fn start(&mut self) -> Result<(), EventLoopError> {
        let event_loop = EventLoop::builder().build()?;

        event_loop.set_control_flow(ControlFlow::Poll);
        event_loop.run_app(self)?;

        Ok(())
    }
}

impl<T: State> Default for Application<T> {
    fn default() -> Self {
        Self { window: None }
    }
}

impl<T: State> ApplicationWindow<T> {
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub fn new(event_loop: &ActiveEventLoop) -> Self {
        let window_attrs = Window::default_attributes().with_transparent(false);

        let template_builder = ConfigTemplateBuilder::new().with_transparency(true);
        let display_builder = DisplayBuilder::new().with_window_attributes(Some(window_attrs));

        let (window, gl_config) = display_builder
            .build(event_loop, template_builder, |mut configs| {
                configs.next().expect("failed to retrieve configuration")
            })
            .expect("failed to build display");

        let window = window.expect("failed to get window");

        let window_handle = window.window_handle().expect("failed to get window handle");
        let context_attrs = ContextAttributesBuilder::new().build(Some(window_handle.into()));
        let fallback_context_attrs = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::Gles(None))
            .build(Some(window_handle.into()));

        let gl_context = unsafe {
            gl_config
                .display()
                .create_context(&gl_config, &context_attrs)
                .unwrap_or_else(|_| {
                    gl_config
                        .display()
                        .create_context(&gl_config, &fallback_context_attrs)
                        .expect("failed to create context")
                })
        };

        let (width, height): (u32, u32) = window.inner_size().into();
        let attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
            window_handle.into(),
            NonZeroU32::new(width).expect("failed to create window width"),
            NonZeroU32::new(height).expect("failed to create window height"),
        );

        let surface = unsafe {
            gl_config
                .display()
                .create_window_surface(&gl_config, &attrs)
                .expect("failed to create surface")
        };

        let current_context = gl_context
            .make_current(&surface)
            .expect("failed to obtain opengl context");

        let display = Display::from_context_surface(current_context, surface)
            .expect("failed to create display from context and surface");

        Self {
            state: T::new(WindowContext::new(event_loop, &window), &display),
            window,
            display,
            last_time: None,
            // tick_acceleration: Duration::ZERO,
            // fixed_acceleration: Duration::ZERO,
            delta: Duration::ZERO,
        }
    }
}

impl<T: State> ApplicationHandler for Application<T> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.window.replace(ApplicationWindow::new(event_loop));
    }

    fn suspended(&mut self, _: &ActiveEventLoop) {
        self.window.take();
    }

    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::Resized(physical_size) => self.window.inspect_mut(move |window| {
                window.display.resize(physical_size.into());

                window.state.handle_window_resize(
                    uvec2(physical_size.width, physical_size.height),
                    window.window.scale_factor(),
                );
            }),
            WindowEvent::ModifiersChanged(modifiers) => {
                let state = modifiers.state();

                self.window.inspect_mut(move |window| {
                    window.state.handle_keyboard_modifiers(KeyboardModifiers {
                        alt_key: state.alt_key(),
                        control_key: state.control_key(),
                        shift_key: state.shift_key(),
                        super_key: state.super_key(),
                    });
                });
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    self.window.inspect_mut(|window| {
                        window.state.handle_keyboard_input(
                            code,
                            event.state.is_pressed(),
                            event.repeat,
                        );
                    });
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let delta = match delta {
                    MouseScrollDelta::LineDelta(x, y) => vec2(x, y),
                    MouseScrollDelta::PixelDelta(delta) => vec2(delta.x as f32, delta.y as f32),
                };

                self.window.inspect_mut(|window| {
                    window.state.handle_mouse_wheel(delta);
                });
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.window.inspect_mut(|window| {
                    window.state.handle_mouse_button(button, state.is_pressed());
                });
            }
            _ => {}
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            self.window.inspect_mut(|window| {
                window
                    .state
                    .handle_mouse_motion(vec2(delta.0 as f32, delta.1 as f32));
            });
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.window.inspect_mut(|window| {
            // let update = Instant::now();

            window.state.update(
                WindowContext::new(event_loop, &window.window),
                &window.display,
                window.delta,
            );

            // println!("update took {:?}", update.elapsed());

            // let render = Instant::now();

            window.state.render(&window.display, window.delta);

            // println!("render took {:?}", render.elapsed());

            window.delta = window
                .last_time
                .map_or_else(|| Duration::ZERO, |last_time| last_time.elapsed());

            window.last_time.replace(Instant::now());
        });
    }
}
