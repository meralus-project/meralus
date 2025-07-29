#![allow(
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::unreadable_literal,
    clippy::missing_panics_doc
)]

mod aabb;
mod bfs_light;
mod blocks;
mod camera;
mod clock;
mod game;
mod input;
mod loaders;
mod player;
mod progress;
mod raycast;
mod util;
mod world;

use std::{
    env::consts::{ARCH, OS},
    fmt, fs,
    mem::replace,
    net::SocketAddrV4,
    ops::Not,
    sync::Arc,
    time::Duration,
};

use blocks::{AirBlock, DirtBlock, GrassBlock};
use camera::Camera;
use clap::Parser;
use glam::{IVec2, Mat4, Quat, UVec2, Vec2, Vec3};
use glamour::Vector2;
use glium::{
    Surface, Texture2d,
    texture::MipmapsOption,
    uniforms::{MagnifySamplerFilter, MinifySamplerFilter},
};
use input::KeyboardController;
use meralus_animation::{AnimationPlayer, Curve, FinishBehaviour, RepeatMode, RestartBehaviour, Transition};
use meralus_engine::{Application, CursorGrabMode, KeyCode, MouseButton, State, WindowContext, WindowDisplay};
use meralus_graphics::{
    FONT, FONT_BOLD, Line, RenderContext, RenderInfo, ShapeRenderer, TextRenderer,
    ellay::{self, Element, ElementChildren, ElementChildrenMut, Styling},
};
use meralus_shared::{Color, Lerp, Point2D, Point3D, RRect2D, Rect2D, Size2D, Thickness};
use meralus_world::{CHUNK_SIZE_U16, ChunkManager};
use mollie_compiler::Compiler;
use mollie_vm::{Chunk, ObjectValue, StructType, TypeVariant, Value, Vm, float, function, integer, void};
use owo_colors::OwoColorize;
use parking_lot::RwLock;
use polymorpher::{Morph, geometry::Vector};
use tokio::sync::mpsc;
use util::cube_outline;

use self::{
    aabb::Aabb,
    bfs_light::{BfsLight, LightNode},
    game::ResourceManager,
    loaders::{BakedBlockModelLoader, Block, BlockManager, TextureLoader},
    player::PlayerController,
    util::{AsColor, get_movement_direction, get_rotation_directions, vertex_ao},
};
use crate::{
    input::MouseController,
    progress::{Progress, ProgressInfo, ProgressSender},
    util::{ChunkManagerPhysics, MatrixExt},
    world::World,
};

pub const TICK_RATE: Duration = Duration::from_millis(50);
pub const TPS: usize = (1000 / TICK_RATE.as_millis()) as usize;
pub const FIXED_FRAMERATE: Duration = Duration::from_secs(1).checked_div(60).expect("failed to calculate fixed framerate somehow");

const TEXT_COLOR: Color = Color::from_hsl(120.0, 0.5, 0.4);
const BG_COLOR: Color = Color::from_hsl(120.0, 0.4, 0.75);
const DAY_COLOR: Color = Color::from_hsl(220.0, 0.5, 0.75);
const NIGHT_COLOR: Color = Color::from_hsl(220.0, 0.35, 0.25);

fn get_sky_color((after_day, progress): (bool, f32)) -> Color {
    if after_day {
        DAY_COLOR.lerp(&NIGHT_COLOR, progress)
    } else {
        NIGHT_COLOR.lerp(&DAY_COLOR, progress)
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long, requires = "net")]
    host: Option<SocketAddrV4>,
    #[arg(short, long, group = "net")]
    nickname: Option<String>,
}

#[allow(clippy::struct_excessive_bools)]
struct Debugging {
    time_paused: bool,
    overlay: bool,
    wireframe: bool,
    draw_borders: bool,
    inventory_open: bool,
    chunk_borders: Vec<Line>,
    render_info: RenderInfo,
}

enum Action {
    UpdateChunkMesh(IVec2),
}

struct GameLoop {
    resource_manager: Arc<RwLock<ResourceManager>>,
    texture_atlas: Texture2d,
    keyboard: KeyboardController,
    mouse: MouseController,
    window_matrix: Mat4,
    debugging: Debugging,
    animation_player: AnimationPlayer,
    text_renderer: TextRenderer,
    shape_renderer: ShapeRenderer,
    accel: Duration,
    action_queue: Vec<Action>,
    fixed_accel: Duration,
    world: Option<World>,
    root: ellay::Canvas,
    m: Morph,
    vm: Vm,
    program: Chunk,

    progress: Progress,
}

const INVENTORY_HOTBAR_SLOTS: u8 = 10;

impl GameLoop {
    fn destroy_looking_at(&mut self) {
        if let Some(world) = self.world.as_mut()
            && let Some(looking_at) = world.player.looking_at
        {
            let local = world.chunk_manager.to_chunk_local(looking_at.position);

            if let Some(local) = local {
                world.chunk_manager.set_block(looking_at.position, 0);

                if local.y >= 255 {
                    world.chunk_manager.set_sky_light(looking_at.position, 15);
                }

                world.update_block_sky_light(&self.resource_manager.read().models, looking_at.position);

                let chunk = ChunkManager::to_local(looking_at.position);

                if local.x == 0 {
                    let chunk = chunk - IVec2::X;

                    if world.chunk_manager.contains_chunk(&chunk) {
                        self.action_queue.push(Action::UpdateChunkMesh(chunk));
                    }
                } else if local.x == (CHUNK_SIZE_U16 - 1) {
                    let chunk = chunk + IVec2::X;

                    if world.chunk_manager.contains_chunk(&chunk) {
                        self.action_queue.push(Action::UpdateChunkMesh(chunk));
                    }
                }

                if local.z == 0 {
                    let chunk = chunk - IVec2::Y;

                    if world.chunk_manager.contains_chunk(&chunk) {
                        self.action_queue.push(Action::UpdateChunkMesh(chunk));
                    }
                } else if local.z == (CHUNK_SIZE_U16 - 1) {
                    let chunk = chunk + IVec2::Y;

                    if world.chunk_manager.contains_chunk(&chunk) {
                        self.action_queue.push(Action::UpdateChunkMesh(chunk));
                    }
                }

                self.action_queue.push(Action::UpdateChunkMesh(chunk));

                world.player.update_looking_at(&world.chunk_manager, &self.resource_manager.read().models);
            }
        }
    }

    fn fixed_update(&mut self) {
        if let Some(world) = self.world.as_mut()
            && world.player_controllable
        {
            world.player.handle_physics(
                &world.chunk_manager,
                &self.resource_manager.read().models,
                &self.keyboard,
                &mut world.camera,
                FIXED_FRAMERATE.as_secs_f32(),
            );

            world.camera.position = world.player.position;
            world.camera.up = world.player.up;
            world.camera.target = world.player.position + world.player.front;

            world.camera.update_frustum();
        }
    }

    fn world_mut<F: FnOnce(&mut World)>(&mut self, func: F) {
        if let Some(world) = self.world.as_mut() {
            func(world);
        }
    }
}

const SLOT_SIZE: f32 = 48f32;

fn init_animation_player(animation_player: &mut AnimationPlayer) {
    animation_player.enable();

    animation_player.add("loading-screen", Transition::new(1.0, 0.0, 1000, Curve::LINEAR, RepeatMode::Once));
    animation_player.add("show-settings", Transition::new(0.0, 1.0, 500, Curve::EASE_IN_OUT_EXPO, RepeatMode::Once));
    animation_player.add("overlay-width", Transition::new(0.0, 1.0, 400, Curve::EASE_IN_OUT_EXPO, RepeatMode::Once));
    animation_player.add(
        "text-scaling",
        Transition::new(0.5, 1.0, 600, Curve::EASE_IN_OUT, RepeatMode::Infinite).with_restart_behaviour(RestartBehaviour::EndValue),
    );

    animation_player.add(
        "shape-morph",
        Transition::new(0.0, 1.0, 600, Curve::EASE_IN_OUT_EXPO, RepeatMode::Infinite).with_restart_behaviour(RestartBehaviour::EndValue),
    );

    animation_player.add(
        "stage-substage-translation",
        Transition::new(0.0, -1.0, 400, Curve::EASE_IN_OUT_EXPO, RepeatMode::Once).with_finish_behaviour(FinishBehaviour::Reset),
    );

    animation_player.add("progress-opacity", Transition::new(1.0, 1.0, 400, Curve::LINEAR, RepeatMode::Once));
    animation_player.add(
        "stage-previous-progress",
        Transition::new(0.0, 1.0, 400, Curve::EASE_IN_OUT_EXPO, RepeatMode::Once),
    );

    animation_player.add("stage-progress", Transition::new(0.0, 1.0, 400, Curve::EASE_IN_OUT_EXPO, RepeatMode::Once));
    animation_player.add(
        "stage-substage-progress",
        Transition::new(0.0, 1.0, 400, Curve::EASE_IN_OUT_EXPO, RepeatMode::Once),
    );
}

impl State for GameLoop {
    fn new(context: WindowContext, display: &WindowDisplay) -> Self {
        context.set_cursor_grab(CursorGrabMode::Confined);
        context.set_cursor_visible(false);

        let (tx, rx) = mpsc::channel(8);

        let resource_manager = Arc::new(RwLock::new(ResourceManager::new("./resources")));

        let resources = resource_manager.clone();

        tokio::spawn(async move {
            let sender = ProgressSender(tx);

            sender.set_visible(true).await?;
            sender.set_initial_info(ProgressInfo::new(2, 0, 1, 0)).await?;

            sender.new_stage("Blocks loading", 3).await?;

            resources.write().register_block(AirBlock);
            sender.complete_task().await?;

            resources.write().register_block(DirtBlock);
            sender.complete_task().await?;

            resources.write().register_block(GrassBlock);
            sender.complete_task().await?;

            sender.new_stage("Mip-maps generation", 1).await?;

            resources.write().generate_mipmaps(4);
            sender.complete_task().await?;

            sender.set_visible(false).await
        });

        // resource_manager.generate_world(12723);
        // resource_manager.generate_lights();
        // resource_manager.set_block_light(vec3(-13.0, 217.0, 0.0), 15);

        // println!(
        //     "[{:18}] Generated {} chunks",
        //     "INFO/WorldGen".bright_green(),
        //     resource_manager.chunk_manager().len().bright_blue().bold(),
        // );

        // let world_mesh = resource_manager.compute_world_mesh();

        // println!(
        //     "[{:18}] Generated {} meshes for chunks",
        //     "INFO/Rendering".bright_green(),
        //     (world_mesh.len() * 6).bright_blue().bold()
        // );

        // let player = PlayerController {
        //     position: vec3(2.0, 275.0, 2.0),
        //     ..Default::default()
        // };

        let mut text_renderer = TextRenderer::new(display, 4096 / 2).unwrap();

        text_renderer.add_font(display, "default", FONT).unwrap();
        text_renderer.add_font(display, "default_bold", FONT_BOLD).unwrap();

        let mut animation_player = AnimationPlayer::default();

        init_animation_player(&mut animation_player);

        let mut root = ellay::Canvas::default().with_children([(
            Point2D::new(48.0, 0.0),
            ellay::AnchorPoint::BottomLeft,
            ellay::Column::default()
                .with_children(["aa", "bbbb", "ccc"].into_iter().enumerate().map(|(i, text)| {
                    ellay::Text::new(text)
                        .with_text_size(24.0)
                        .with_foreground(Color::from_u32_rgb(0x1E4D52))
                        .with_background(Color::from_u32_rgb(0xBCEBF0))
                        .with_corner_radius(if i == 0 {
                            Thickness::new(8.0, 8.0, 4.0, 4.0)
                        } else if i == 2 {
                            Thickness::new(4.0, 4.0, 8.0, 8.0)
                        } else {
                            Thickness::all(4.0)
                        })
                        .with_padding(Thickness::all(4.0))
                        .into_node()
                }))
                .with_background(Color::from_u32_rgb(0x3C4B38))
                .with_spacing(4.0)
                .with_corner_radius(Thickness::all(12.0))
                .with_padding(Thickness::all(4.0))
                .into_node(),
        )]);

        println!("{root:#?}");

        if let ElementChildrenMut::Multiple(mut elements) = root.children_mut() {
            elements[0].remove(0);
        }

        let mut compiler = Compiler::default();

        // draw_round_rect(24.0, 80.0, 128.0, 48.0, 12.0, 0x00FFFF);
        // draw_round_rect(24.0, 136.0, 128.0, 48.0, 12.0, 0xFFFF00);

        compiler.var_value(
            "draw_round_rect",
            function(false, [float(), float(), float(), float(), float(), integer()], void()),
            Value::object(ObjectValue::NativeFunc(|vm, args| {
                let x = args[0].as_float()?;
                let y = args[1].as_float()?;
                let w = args[2].as_float()?;
                let h = args[3].as_float()?;
                let roundness = args[4].as_float()?;
                let color = args[5].as_integer()? as u32;

                let context = vm.state.downcast_mut::<RenderContext>()?;

                context.draw_rounded_rect(
                    RRect2D::new(Point2D::new(x, y), Size2D::new(w, h), Thickness::all(roundness)),
                    Color::from_u32_rgb(color),
                );

                None
            })),
        );

        let context_ty = TypeVariant::complex(mollie_vm::ComplexType::Struct(StructType { properties: Vec::new() }));

        compiler.add_type("DrawContext", context_ty.clone());
        compiler.vtable_func(
            context_ty.clone(),
            "draw_rect",
            function(true, [float(), float(), float(), float(), integer()], void()).into(),
            Value::object(ObjectValue::NativeFunc(|vm, args| {
                let x = args[1].as_float()?;
                let y = args[2].as_float()?;
                let w = args[3].as_float()?;
                let h = args[4].as_float()?;
                let color = args[5].as_integer()? as u32;

                let context = vm.state.downcast_mut::<RenderContext>()?;

                context.draw_rect(Rect2D::new(Point2D::new(x, y), Size2D::new(w, h)), Color::from_u32_rgb(color));

                None
            })),
        );

        compiler.add_trait("Drawable").method("draw", [context_ty], void());

        let chunk = compiler.compile_program_text(fs::read_to_string("/home/aiving/meralus.mol").unwrap()).unwrap();

        let vm = compiler.as_vm();

        Self {
            keyboard: KeyboardController::default(),
            animation_player,
            text_renderer,
            shape_renderer: ShapeRenderer::new(display),
            window_matrix: Mat4::IDENTITY,
            debugging: Debugging {
                time_paused: true,
                overlay: false,
                wireframe: false,
                draw_borders: false,
                inventory_open: false,
                chunk_borders: Vec::new(), /* resource_manager.chunk_manager().chunks().fold(
                                            * Vec::new(),
                                            * |mut lines, Chunk { origin, .. }| {
                                            * let origin = origin.as_vec2() * CHUNK_SIZE_F32;
                                            *
                                            * lines.extend(cube_outline(Cube3D::new(
                                            * Point3D::new(origin.x, 0.0, origin.y),
                                            * Size3D::new(CHUNK_SIZE_F32, CHUNK_HEIGHT_F32,
                                            * CHUNK_SIZE_F32),
                                            * )));
                                            *
                                            * lines
                                            * },
                                            * ) */
                render_info: RenderInfo::default(),
            },
            resource_manager,
            accel: Duration::ZERO,
            fixed_accel: Duration::ZERO,
            action_queue: Vec::new(),
            world: None,
            progress: Progress::new(rx),
            texture_atlas: Texture2d::empty_with_mipmaps(display, MipmapsOption::EmptyMipmapsMax(4), TextureLoader::ATLAS_SIZE, TextureLoader::ATLAS_SIZE)
                .unwrap(),
            root,
            mouse: MouseController::default(),
            m: Morph::new(
                polymorpher::shapes::triangle()
                    .transformed(|point| point * 128.0)
                    .transformed(|point| point + Vector::splat(128.0)),
                polymorpher::shapes::arrow()
                    .transformed(|point| point * 128.0)
                    .transformed(|point| point + Vector::splat(128.0)),
            ),
            vm,
            program: chunk,
        }
    }

    fn handle_window_resize(&mut self, size: UVec2, scale_factor: f64) {
        let size = size.as_vec2();

        self.window_matrix = Mat4::orthographic_rh_gl(0.0, size.x / scale_factor as f32, size.y / scale_factor as f32, 0.0, -1.0, 1.0);

        self.world_mut(|world| world.camera.aspect_ratio = size.x / size.y);
    }

    fn handle_keyboard_input(&mut self, key: KeyCode, is_pressed: bool, repeat: bool) {
        self.keyboard.handle_keyboard_input(key, is_pressed, repeat);
    }

    fn handle_mouse_button(&mut self, button: MouseButton, is_pressed: bool) {
        self.mouse.handle_mouse_button(button, is_pressed);

        if button == MouseButton::Left && is_pressed {
            self.destroy_looking_at();
        }
    }

    fn handle_mouse_motion(&mut self, delta: Option<Vec2>, position: Option<Vec2>) {
        if let Some(delta) = delta
            && let Some(world) = self.world.as_mut()
            && world.player_controllable
        {
            world.player.handle_mouse(&world.chunk_manager, &self.resource_manager.read().models, delta);
        } else if let Some(position) = position {
            self.mouse.handle_mouse_motion(position);
        }
    }

    fn handle_mouse_wheel(&mut self, delta: Vec2) {
        self.world_mut(|world| {
            if delta.y > 0.0 {
                world.inventory_slot.increase();
            } else if delta.y < 0.0 {
                world.inventory_slot.decrease();
            }
        });
    }

    #[allow(clippy::too_many_lines, clippy::significant_drop_tightening)]
    fn update(&mut self, context: WindowContext, display: &WindowDisplay, delta: Duration) {
        self.accel += delta;
        self.fixed_accel += delta;

        self.world_mut(|world| world.tick_accel += delta);

        while self.fixed_accel > FIXED_FRAMERATE {
            self.fixed_accel -= FIXED_FRAMERATE;

            self.fixed_update();
        }

        self.progress.update(&mut self.animation_player, &self.texture_atlas, &self.resource_manager);

        let paused = self.debugging.time_paused;

        if let Some(world) = self.world.as_mut() {
            while world.tick_accel > TICK_RATE {
                world.tick_accel -= TICK_RATE;

                world.tick(&mut self.animation_player, paused);
            }
        }

        if self.accel >= Duration::from_secs(1) {
            self.accel = Duration::ZERO;
            self.world_mut(|world| {
                world.ticks = world.tick_sum;
                world.tick_sum = 0;
            });
        }

        if self.keyboard.is_key_pressed_once(KeyCode::Tab) {
            self.world_mut(|world| world.player_controllable = !world.player_controllable);

            if self.world.as_ref().is_some_and(|world| world.player_controllable) {
                context.set_cursor_grab(CursorGrabMode::Confined);
                context.set_cursor_visible(false);
            } else {
                context.set_cursor_grab(CursorGrabMode::None);
                context.set_cursor_visible(true);
            }
        }

        if self.keyboard.is_key_pressed_once(KeyCode::Escape) && self.animation_player.get_value("show-settings") > Some(0f32) {
            self.animation_player.play_transition_to("show-settings", 0.0);
        }

        self.animation_player.advance(delta.as_secs_f32());

        if self.keyboard.is_key_pressed_once(KeyCode::KeyT) {
            self.debugging.wireframe = !self.debugging.wireframe;
        }

        if self.keyboard.is_key_pressed_once(KeyCode::KeyV) {
            self.debugging.inventory_open = !self.debugging.inventory_open;

            if self.debugging.inventory_open {
                let scale = self.animation_player.get_mut("scale").unwrap();

                scale.set_delay(0);
                scale.to(1.0);

                let scale_vertical = self.animation_player.get_mut("scale-vertical").unwrap();

                scale_vertical.set_delay(400);
                scale_vertical.to(1.0);

                let opacity = self.animation_player.get_mut("opacity").unwrap();

                opacity.set_delay(0);
                opacity.to(1.0);
            } else {
                let scale = self.animation_player.get_mut("scale").unwrap();

                scale.set_delay(400);
                scale.to(0.0);

                let scale_vertical = self.animation_player.get_mut("scale-vertical").unwrap();

                scale_vertical.set_delay(0);
                scale_vertical.to(0.0);

                let opacity = self.animation_player.get_mut("opacity").unwrap();

                opacity.set_delay(400);
                opacity.to(0.0);
            }

            self.animation_player.play("scale");
            self.animation_player.play("opacity");
            self.animation_player.play("scale-vertical");
        }

        if self.keyboard.is_key_pressed_once(KeyCode::KeyP) {
            self.debugging.time_paused = !self.debugging.time_paused;
        }

        if self.keyboard.is_key_pressed_once(KeyCode::KeyO) {
            self.debugging.overlay = !self.debugging.overlay;

            self.animation_player
                .play_transition_to("overlay-width", if self.debugging.overlay { 1.0 } else { 0.0 });
        }

        while let Some(action) = self.action_queue.pop() {
            match action {
                Action::UpdateChunkMesh(origin) => {
                    if let Some(world) = self.world.as_mut()
                        && let Some(chunk) = world.compute_chunk_mesh_at(&self.resource_manager.read().models, origin)
                    {
                        world.voxel_renderer.set_chunk(display, origin, chunk);
                    }
                }
            }
        }

        if self.keyboard.is_key_pressed_once(KeyCode::KeyB) {
            self.debugging.draw_borders = !self.debugging.draw_borders;
        }

        if self.keyboard.is_key_pressed_once(KeyCode::KeyL) {
            let resource_manager = self.resource_manager.read();
            let atlas = resource_manager.get_mipmaps();

            println!(
                "[{:18}] Saving atlas ({} packed textures) with {} mipmap levels...",
                "INFO/AtlasManager".bright_green(),
                resource_manager.get_texture_count().bright_blue(),
                (atlas.len() - 1).bright_blue()
            );

            for (i, font_info) in self.text_renderer.fonts().iter().enumerate() {
                if fs::exists("debug-fonts").is_ok_and(Not::not)
                    && let Err(error) = fs::create_dir("debug-fonts")
                {
                    println!("[{:18}] Failed to create debug directory: {error}", " ERR/FontManager".bright_red(),);

                    break;
                }

                let name = font_info.font.name().map_or_else(|| i.to_string(), ToString::to_string);

                if let Err(error) = font_info.atlas.main_texture().save(format!("debug-fonts/{name}.png")) {
                    println!("[{:18}] Failed to save {name} font: {error}", " ERR/AtlasManager".bright_red(),);
                } else {
                    println!("[{:18}] Successfully saved {name} font", "INFO/AtlasManager".bright_green(),);
                }
            }

            for (level, image) in atlas.iter().enumerate() {
                let (width, height) = image.dimensions();

                if fs::exists("debug").is_ok_and(Not::not)
                    && let Err(error) = fs::create_dir("debug")
                {
                    println!("[{:18}] Failed to create debug directory: {error}", " ERR/AtlasManager".bright_red(),);

                    break;
                }

                if let Err(error) = image.save(format!("debug/atlas_{level}.png")) {
                    println!(
                        "[{:18}] Failed to save atlas (mipmap level: {}, size: {}): {error}",
                        " ERR/AtlasManager".bright_red(),
                        level.to_string().bright_blue(),
                        format!("{width}x{height}").bright_blue()
                    );
                } else {
                    println!(
                        "[{:18}] Successfully saved atlas (mipmap level: {}, size: {})",
                        "INFO/AtlasManager".bright_green(),
                        level.to_string().bright_blue(),
                        format!("{width}x{height}").bright_blue()
                    );
                }
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn render(&mut self, window_context: WindowContext, display: &WindowDisplay, delta: Duration) {
        let RenderInfo { draw_calls, vertices } = self.debugging.render_info.take();

        let (width, height) = display.get_framebuffer_dimensions();
        let mut frame = display.draw();

        if let Some(world) = self.world.as_mut() {
            let [r, g, b] = get_sky_color(world.clock.get_visual_progress()).to_linear();

            frame.clear_color_and_depth((r, g, b, 1.0), 1.0);

            world.voxel_renderer.render(
                &mut frame,
                &world.camera.frustum,
                world.camera.matrix(),
                self.texture_atlas
                    .sampled()
                    .minify_filter(MinifySamplerFilter::NearestMipmapLinear)
                    .magnify_filter(MagnifySamplerFilter::Nearest),
                self.debugging.wireframe,
            );

            self.debugging.render_info.extend(&world.voxel_renderer.get_debug_info());

            if self.debugging.draw_borders {
                self.shape_renderer.set_matrix(world.camera.matrix());
                self.debugging
                    .render_info
                    .extend(&self.shape_renderer.draw_lines(&mut frame, display, &self.debugging.chunk_borders));

                self.shape_renderer.set_default_matrix();
            }

            if let Some(result) = world.player.looking_at
                && let Some(model) = world.chunk_manager.get_model_for(&self.resource_manager.read().models, result.position)
            {
                self.shape_renderer.set_matrix(world.camera.matrix());
                self.debugging.render_info.extend(&self.shape_renderer.draw_lines(
                    &mut frame,
                    display,
                    &cube_outline(model.bounding_box + Point3D::from_raw(result.position)),
                ));

                self.shape_renderer.set_default_matrix();
            }

            // {
            //     let sun_position = {
            //         let angle: f32 = self.animation_player.get_value("sun").unwrap();

            //         [angle.cos(), angle.sin()]
            //     };

            //     self.shape_renderer.set_matrix(self.camera.matrix());
            //     self.shape_renderer.draw_rects(
            //         &mut frame,
            //         display,
            //         &[Rectangle::new_3d(
            //             -4.0,
            //             (256.0 + 64.0) * sun_position[1],
            //             (256.0 + 64.0) * sun_position[0],
            //             8.0,
            //             8.0,
            //             Color::RED,
            //         )
            //         .with_rotation_matrix(Some(Mat4::look_at_rh(
            //             Vec3::new(
            //                 0.0,
            //                 (256.0 + 64.0) * sun_position[1],
            //                 (256.0 + 64.0) * sun_position[0],
            //             ),
            //             Vec3::ZERO,
            //             Vec3::Z,
            //         )))],
            //         &mut self.debugging.draw_calls,
            //         &mut self.debugging.vertices,
            //     );
            //     self.shape_renderer.set_default_matrix();
            // }

            let mut context = RenderContext::new(display);

            context.ui(|context, bounds| {
                let hotbar_width = f32::from(INVENTORY_HOTBAR_SLOTS) * SLOT_SIZE;

                let origin = Point2D::new((bounds.size.width / 2.0) - (hotbar_width / 2.0), bounds.size.height - SLOT_SIZE - 8.0);

                let offset = f32::from(world.inventory_slot.value) * SLOT_SIZE;

                context.draw_rect(Rect2D::new(origin, Size2D::new(hotbar_width, SLOT_SIZE)), Color::from_hsl(0.0, 0.0, 0.5));

                context.draw_rect(
                    Rect2D::new(origin + Point2D::new(offset, 0.0), Size2D::new(SLOT_SIZE, SLOT_SIZE)),
                    Color::from_hsl(0.0, 0.0, 0.8),
                );

                context.draw_rect(
                    Rect2D::new(
                        origin + Point2D::new(4.0, 4.0) + Point2D::new(offset, 0.0),
                        Size2D::new(SLOT_SIZE - 8.0, SLOT_SIZE - 8.0),
                    ),
                    Color::from_hsl(0.0, 0.0, 0.5),
                );
            });

            context.ui(|context, bounds| {
                let opacity: f32 = self.animation_player.get_value("opacity").unwrap();
                let scale: f32 = self.animation_player.get_value("scale").unwrap();
                let scale_vertical: f32 = self.animation_player.get_value("scale-vertical").unwrap();

                let screen_center = bounds.center();

                let size = Size2D::new(bounds.size.width * 0.65, bounds.size.height.mul_add(0.4, 320.0 * scale_vertical));

                let center = screen_center - (size / 2.0).to_vector();

                context.add_transform(Mat4::from_scale_rotation_translation(
                    Vec3::from_array([scale; 3]),
                    Quat::IDENTITY,
                    screen_center.to_raw().extend(0.0) * (1.0 - scale),
                ));

                context.bounds(Rect2D::new(center, size), |context, _| {
                    context.fill(Color::from_hsl(130.0, 0.35, 0.25).with_alpha(opacity));

                    context.padding(2.0, |context, bounds| {
                        context.clipped(bounds, |context, bounds| {
                            let measured = self.text_renderer.measure("default_bold", "Inventory", 18.0, None).unwrap();

                            context.draw_text(bounds.origin, "default_bold", "Inventory", 18.0, Color::WHITE.with_alpha(opacity), None);

                            let size = bounds.size - Size2D::new(0.0, measured.height + 4.0);
                            let origin = bounds.origin + Point2D::new(0.0, measured.height + 2.0);

                            let inner_origin = origin + Point2D::new(2.0, 2.0);
                            let inner_size = size - Size2D::new(4.0, 4.0);

                            let tile_count = 24usize;
                            let tile_gap = 2f32;
                            let tile_size =
                                (inner_size - Size2D::new((tile_count as f32 - 1.0) * tile_gap, (tile_count as f32 - 1.0) * tile_gap)) / tile_count as f32;

                            context.draw_rect(Rect2D::new(origin, size), Color::from_hsl(130.0, 0.5, 0.75).with_alpha(opacity));

                            for x in 0..tile_count {
                                for y in 0..tile_count {
                                    context.draw_rect(
                                        Rect2D::new(
                                            inner_origin + Point2D::new((tile_gap + tile_size.width) * x as f32, (tile_gap + tile_size.height) * y as f32),
                                            tile_size,
                                        ),
                                        Color::from_hsl(130.0, 0.25, 0.5).with_alpha(opacity),
                                    );
                                }
                            }
                        });
                    });
                });

                context.remove_transform();
            });

            let animation_progress: f32 = self.animation_player.get_value("loading-screen").unwrap();

            {
                let chunk = ChunkManager::to_local(world.player.position);

                let (hours, minutes) = {
                    let time = world.clock.time().as_secs();
                    let seconds = time % 60;
                    let minutes = (time - seconds) / 60 % 60;
                    let hours = (time - seconds - minutes * 60) / 60 / 60;

                    (hours, minutes)
                };

                let version = display.get_opengl_version();
                let rendered_chunks = world.voxel_renderer.rendered_chunks();
                let total_chunks = world.voxel_renderer.total_chunks();

                let text = format!(
                    "OpenGL {}.{}
    Free GPU memory: {}
    Window size: {width}x{height}
    Player position: {:.2}
    Chunk: {} {}
    Game Time: {hours:02}:{minutes:02}
    FPS: {:.0} ({:.2}ms)
    TPS: {}
    Looking at {}
    Draw calls: {draw_calls}
    Rendered chunks: {rendered_chunks} / {total_chunks}
    Rendered vertices: {vertices}
    Animation player:",
                    version.1,
                    version.2,
                    display.get_free_video_memory().map_or_else(|| String::from("unknown"), util::format_bytes),
                    world.player.position,
                    chunk.x,
                    chunk.y,
                    1.0 / delta.as_secs_f32(),
                    delta.as_secs_f32() * 1000.0,
                    world.ticks,
                    world
                        .player
                        .looking_at
                        .and_then(|result| world.chunk_manager.get_block(result.position).and_then(|block| self
                            .resource_manager
                            .read()
                            .get_block(block.into())
                            .map(|block| format!("{block} (at {})", result.hit_side))))
                        .unwrap_or_else(|| String::from("nothing")),
                );

                let text_size = self.text_renderer.measure("default", &text, 18.0, None).unwrap();
                let overlay_width = self.animation_player.get_value::<_, f32>("overlay-width").unwrap();

                let text_bounds = Rect2D::new(Point2D::new(12.0, 12.0), Size2D::new((522.0 + 4.0) * overlay_width, text_size.height + 4.0));

                context.bounds(text_bounds, |context, _| {
                    context.fill(Color::BLACK.with_alpha(0.25));

                    context.padding(2.0, |context, bounds| {
                        context.clipped(bounds, |context, bounds| {
                            context.draw_text(bounds.origin, "default", text, 18.0, Color::WHITE, None);
                        });
                    });
                });

                let mut offset = 0.0;

                for i in 0..self.animation_player.len() {
                    let (finished, elapsed, duration, text) = {
                        let (name, animation) = self.animation_player.get_at(i).unwrap();
                        let elapsed = animation.get_elapsed();
                        let duration = animation.get_duration();

                        (
                            animation.is_finished(),
                            elapsed,
                            duration,
                            format!(
                                "#{name}: {:.2}, {:.1}% ({:.2}ms/{:.2}ms)",
                                animation.get::<f32>(),
                                (elapsed / duration) * 100.0,
                                elapsed * 1000.0,
                                duration * 1000.0
                            ),
                        )
                    };

                    let text_size = self.text_renderer.measure("default", &text, 18.0, None).unwrap();

                    context.bounds(
                        Rect2D::new(
                            Point2D::new(12.0, text_bounds.origin.y + 2.0 + text_bounds.size.height + offset),
                            Size2D::new((522.0 + 4.0) * overlay_width, text_size.height + 12.0),
                        ),
                        |context, root| {
                            context.fill(Color::BLACK.with_alpha(0.25));

                            context.padding(2.0, |context, bounds| {
                                context.clipped(bounds, |context, bounds| {
                                    context.draw_text(bounds.origin, "default", text, 18.0, Color::WHITE, None);

                                    context.draw_rect(
                                        Rect2D::new(
                                            root.origin + Point2D::new(4.0, text_size.height + 4.0),
                                            Size2D::new((root.size.width - 8.0) * (elapsed / duration), 2.0),
                                        ),
                                        if finished {
                                            Color::new(120, 255, 155, 255)
                                        } else {
                                            Color::new(120, 167, 255, 255)
                                        },
                                    );
                                });
                            });
                        },
                    );

                    offset += text_size.height + 14.0;
                }
            }

            context.ui(|context, bounds| {
                context.fill(BG_COLOR.with_alpha(animation_progress));

                let measured = self.text_renderer.measure("default_bold", "Meralus", 64.0, None).unwrap();
                let text_pos = Point2D::from_tuple(((bounds.size - measured) / 2.0).to_tuple());

                let progress_width = bounds.size.width * 0.5;
                let progress_position = (bounds.size.width - progress_width) / 2.0;
                let offset = Point2D::new(progress_position, text_pos.y + 12.0 + measured.height);

                context.bounds(Rect2D::new(bounds.origin + offset, Size2D::new(progress_width, 48.0)), |context, _| {
                    context.fill(TEXT_COLOR.with_alpha(animation_progress));

                    context.padding(2.0, |context, _| {
                        context.fill(BG_COLOR.with_alpha(animation_progress));

                        context.padding(2.0, |context, bounds| {
                            context.draw_rect(
                                Rect2D::new(bounds.origin, bounds.size.with_width(bounds.size.width * (1.0 - animation_progress))),
                                TEXT_COLOR.with_alpha(animation_progress),
                            );
                        });
                    });
                });

                context.draw_text(text_pos, "default_bold", "Meralus", 64.0, TEXT_COLOR.with_alpha(animation_progress), None);
            });

            self.debugging
                .render_info
                .extend(&context.finish(&mut self.shape_renderer, &mut self.text_renderer, display, &mut frame, self.window_matrix));
        } else {
            let [r, g, b] = Color::from_u32_rgb(0x1D211B).to_linear();

            frame.clear_color_and_depth((r, g, b, 1.0), 1.0);

            let context = RenderContext::new(display);

            self.vm.set_state(context);
            self.vm.execute(&self.program);

            let mut context = replace(&mut self.vm.state, Box::new(())).downcast::<RenderContext>().unwrap();

            context.ui(|context, bounds| {
                let text_scaling: f32 = self.animation_player.get_value("text-scaling").unwrap();
                let text_opacity = 1.0 - self.animation_player.get_value::<_, f32>("progress-opacity").unwrap_or(0.0);

                let size = self.text_renderer.measure("default", "Meralus", 64.0, None).unwrap();
                let offset = Point2D::new(bounds.size.width / 2.0 - size.width / 2.0, 24.0);

                context.draw_text(
                    bounds.origin + offset,
                    "default",
                    "Meralus",
                    64.0,
                    Color::from_hsl(110.0, 0.4, 0.7).with_alpha(text_opacity),
                    None,
                );

                let mut origin = bounds.origin + offset + Point2D::new(size.width, 0.0);

                let size = self.text_renderer.measure("default", "hiii wrld!!", 36.0, None).unwrap().to_raw();

                origin += Vector2::from_raw(size * Vec2::new(-0.4, 0.45));

                let center = Mat4::from_rotation_z(20f32.to_radians()).transform_point3(((size / 2.0) * (1.0 - text_scaling)).extend(0.0));

                context.transformed(
                    Mat4::from_translation(origin.to_raw().extend(0.0) + center)
                        .scale(Vec3::splat(text_scaling))
                        .rotate_z(20f32.to_radians())
                        .translate(-origin.to_raw().extend(0.0)),
                    |context, _| {
                        context.draw_text(
                            origin,
                            "default",
                            "hiii wrld!!",
                            36.0,
                            Color::from_hsl(200.0, 0.6, 0.6).with_alpha(text_opacity),
                            None,
                        );
                    },
                );

                context.draw_text(
                    bounds.origin + Point2D::new(8.0, bounds.size.height - 24.0),
                    "default",
                    format!("developer build for {OS} (arch: {ARCH}), v{}", env!("CARGO_PKG_VERSION")),
                    16.0,
                    Color::from_hsl(110.0, 0.6, 0.6).with_alpha(text_opacity),
                    None,
                );

                let button_width = (bounds.size.width * 0.4).max(192.0);
                let mut start = bounds.origin + Point2D::new(bounds.size.width / 2.0 - button_width / 2.0, bounds.size.height / 2.0 - 68.0);

                for (i, button) in MenuButton::ALL.into_iter().enumerate() {
                    let animation = format!("menu-button-{i}");

                    if !self.animation_player.contains(&animation) {
                        self.animation_player.add(
                            &animation,
                            Transition::new(
                                Color::from_u32_rgb(0x3C4B38),
                                Color::from_u32_rgb(0x3C4B38),
                                200,
                                Curve::LINEAR,
                                RepeatMode::Once,
                            ),
                        );
                    }

                    let box_bounds = RRect2D::new(start, Size2D::new(button_width, 40.0), Thickness::all(8.0));

                    if box_bounds.contains(self.mouse.position) {
                        if self.mouse.entered.insert(i) {
                            self.animation_player.get_mut(&animation).unwrap().to(Color::from_u32_rgb(0x5E7558));
                            self.animation_player.play(&animation);
                        }
                    } else if self.mouse.entered.remove(&i) {
                        self.animation_player.get_mut(&animation).unwrap().to(Color::from_u32_rgb(0x3C4B38));
                        self.animation_player.play(&animation);
                    }

                    if self.mouse.is_pressed_once(MouseButton::Left) && box_bounds.contains(self.mouse.position) {
                        println!("{button} clicked");

                        match button {
                            MenuButton::Play => {}
                            MenuButton::Options => {
                                self.animation_player.play_transition_to("show-settings", 1.0);
                            }
                            MenuButton::Exit => window_context.close_window(),
                        }
                    }

                    context.draw_rounded_rect(box_bounds, self.animation_player.get_value(&animation).unwrap());

                    let size = self.text_renderer.measure("default", button.as_str(), 32.0, None).unwrap();

                    context.draw_text(
                        start + Point2D::new((bounds.size.width * 0.4) / 2.0 - size.width / 2.0, 0.0),
                        "default",
                        button.as_str(),
                        32.0,
                        Color::from_u32_rgb(0xD6E8CE),
                        None,
                    );

                    start.y += 48.0;
                }

                let show_options: f32 = self.animation_player.get_value("show-settings").unwrap();

                if show_options > 0.0 {
                    let bounds = context.get_bounds();
                    let box_size = bounds.size * 0.6;
                    let box_border_size = bounds.size * 0.6 + 4.0;
                    let box_border_bounds = RRect2D::new(
                        bounds.origin + ((bounds.size - box_border_size) / 2.0).to_vector(),
                        box_border_size,
                        Thickness::all(12.0),
                    );
                    let box_bounds = RRect2D::new(bounds.origin + ((bounds.size - box_size) / 2.0).to_vector(), box_size, Thickness::all(12.0));

                    let transform = Mat4::from_translation(origin.to_raw().extend(0.0))
                        .scale(Vec3::ONE * show_options)
                        .translate(-origin.to_raw().extend(0.0));

                    context.transformed(transform, |context, _| {
                        context.draw_rounded_rect(box_border_bounds, Color::from_u32_rgb(0x8C9388));
                        context.draw_rounded_rect(box_bounds, Color::from_u32_rgb(0x323630));
                        context.clipped_bounds(
                            Rect2D::new(
                                Point2D::from_raw(transform.transform_point3(Vec3::new(box_bounds.origin.x, box_bounds.origin.y, 0.0)).truncate()),
                                Size2D::from_raw(
                                    transform
                                        .transform_point3(Vec3::new(box_bounds.size.width, box_bounds.size.height, 0.0))
                                        .truncate(),
                                ),
                            ),
                            |context, bounds| {
                                let measured = self
                                    .text_renderer
                                    .measure("default", "легендар чешек трахал...\nчленовеку секс дороже друзей...", 32.0, None)
                                    .unwrap();

                                let origin = bounds.origin + ((bounds.size - measured) / 2.0).with_height(0.0).to_vector();
                                // let center = measured / 2.0;

                                context.draw_text(
                                    origin,
                                    "default",
                                    "легендар чешек трахал...\nчленовеку секс дороже друзей...",
                                    32.0,
                                    Color::RED,
                                    None,
                                );
                            },
                        );
                    });
                }
            });

            context.tessellate_with_color(Color::RED, |mut builder| {
                self.m
                    .add_to(self.animation_player.get_value("shape-morph").unwrap(), &mut builder, false, true);

                builder.build()
            });

            self.root.measure(&mut self.text_renderer, &context, context.get_bounds());

            if let ElementChildren::Multiple(child) = self.root.children() {
                context.draw_rect(child[0].bounding_box().as_rect(), Color::RED);
            }

            self.root.draw(&mut context);

            if self.animation_player.get_value::<_, f32>("progress-opacity") > Some(0.0) {
                show_loading_screen(&self.animation_player, &mut context, &self.progress);
            }

            self.debugging
                .render_info
                .extend(&context.finish(&mut self.shape_renderer, &mut self.text_renderer, display, &mut frame, self.window_matrix));
        }

        frame.finish().expect("failed to finish draw frame");

        self.mouse.clear();
        self.keyboard.clear();
    }
}

enum MenuButton {
    Play,
    Options,
    Exit,
}

impl MenuButton {
    pub const ALL: [Self; 3] = [Self::Play, Self::Options, Self::Exit];

    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Play => "Play",
            Self::Options => "Options",
            Self::Exit => "Exit",
        }
    }
}

impl fmt::Display for MenuButton {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// fn rotate(context: &mut UiContext, origin: Point2D, angle: f32) {
// let mut normal = Mat4::IDENTITY;
//
// normal *= Mat4::from_translation(Vec3::new(origin.x, origin.y, 0.0));
// normal *= Mat4::from_rotation_z(angle);
// normal *= Mat4::from_translation(-Vec3::new(origin.x, origin.y, 0.0));
//
// context.add_transform(normal);
// }

fn show_loading_screen(animation_player: &AnimationPlayer, context: &mut RenderContext, progress: &Progress) {
    let opacity = animation_player.get_value("progress-opacity").unwrap();

    context.ui(|context, bounds| {
        context.fill(Color::from_u32_rgb(0x3C4B38).with_alpha(opacity));

        let progress_bar = Size2D::new(bounds.size.width * 0.8, 48.0);
        let stages_progress_bar = bounds.origin + (bounds.size.to_vector() / 2.0) - (progress_bar.to_vector() / 2.0);

        if let Some(name) = progress.info.as_ref().and_then(|info| info.current_stage_name.as_ref()) {
            context.draw_text(
                stages_progress_bar - Point2D::new(0.0, 40.0).to_vector(),
                "default",
                name,
                32.0,
                Color::from_u32_rgb(0xA2D398).with_alpha(opacity),
                None,
            );
        }

        context.bounds(Rect2D::new(stages_progress_bar, progress_bar), |context, _| {
            context.fill(Color::from_u32_rgb(0xA2D398).with_alpha(opacity));

            context.padding(2.0, |context, _| {
                context.fill(Color::from_u32_rgb(0x3C4B38).with_alpha(opacity));

                if progress.info.is_some() {
                    let progress: f32 = animation_player.get_value("stage-progress").unwrap();

                    context.padding(2.0, |context, bounds| {
                        context.draw_rect(
                            Rect2D::new(bounds.origin, Size2D::new(bounds.size.width * progress, bounds.size.height)),
                            Color::from_u32_rgb(0xA2D398).with_alpha(opacity),
                        );
                    });
                }
            });
        });

        context.bounds(
            Rect2D::new(stages_progress_bar + Point2D::new(0.0, progress_bar.height + 8.0), progress_bar),
            |context, _| {
                context.fill(Color::from_u32_rgb(0xA2D398).with_alpha(opacity));

                context.padding(2.0, |context, _| {
                    context.fill(Color::from_u32_rgb(0x3C4B38).with_alpha(opacity));

                    if progress.info.is_some() {
                        let progress: f32 = animation_player.get_value("stage-substage-progress").unwrap();
                        let translation: f32 = animation_player.get_value("stage-substage-translation").unwrap();

                        context.padding(2.0, |context, bounds| {
                            context.clipped(bounds, |context, bounds| {
                                context.draw_rect(
                                    Rect2D::new(
                                        bounds.origin,
                                        Size2D::new(
                                            bounds.size.width
                                                * if translation < 0.0 {
                                                    animation_player.get_value("stage-previous-progress").unwrap()
                                                } else {
                                                    progress
                                                },
                                            bounds.size.height * (1.0 + translation),
                                        ),
                                    ),
                                    Color::from_u32_rgb(0xA2D398).with_alpha(opacity),
                                );
                            });
                        });
                    }
                });
            },
        );
    });
}

#[tokio::main]
async fn main() {
    // let args = Args::parse();

    // if let Some(host) = args.host {
    //     let mut client = Client::connect(host).await.unwrap();

    //     client
    //         .send(IncomingPacket::PlayerConnected {
    //             name: args.nickname.unwrap(),
    //         })
    //         .await
    //         .unwrap();

    //     let uuid = if let Some(Ok(OutgoingPacket::UuidAssigned { uuid })) =
    // client.receive().await {         uuid
    //     } else {
    //         panic!("BRO WTF :sob::sob::sob::sob:");
    //     };

    //     client.send(IncomingPacket::GetPlayers).await.unwrap();

    //     if let Some(Ok(OutgoingPacket::PlayersList { players })) =
    // client.receive().await {         println!("{players:#?}");
    //     }
    // }

    Application::<GameLoop>::default().start().expect("failed to run app");
}
