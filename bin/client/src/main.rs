#![allow(
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::unreadable_literal,
    clippy::missing_panics_doc
)]

mod blocks;
mod camera;
mod clock;
mod input;
mod player;
mod posteffects;
mod progress;
mod scenes;
mod util;
mod world;

use std::{
    env::consts::{ARCH, OS},
    f32, fmt,
    sync::{Arc, mpsc},
    time::Duration,
};

use glium::{
    Surface, Texture2d,
    texture::MipmapsOption,
    uniforms::{MagnifySamplerFilter, MinifySamplerFilter},
};
use meralus_animation::{AnimationPlayer, Curve, FinishBehaviour, RepeatMode, RestartBehaviour, Transition};
use meralus_engine::{Application, CursorGrabMode, KeyCode, MouseButton, State, WindowContext, WindowDisplay};
use meralus_graphics::{CommonRenderer, CommonVertex, FONT, FONT_BOLD, RenderContext, RenderInfo, VoxelFace, VoxelMeshBuilder};
use meralus_physics::{Aabb, AabbSource, PhysicsContext};
use meralus_shared::{
    Angle, Color, Cube3D, IPoint2D, IPoint3D, Lerp, MatrixExt, Point2D, Point3D, Quat, RRect2D, Rect2D, Size2D, Thickness, Transform3D, USize2D, USizePoint3D,
    Vector2D, Vector3D,
};
use meralus_storage::{Block, ResourceStorage, TextureStorage};
use meralus_world::{BfsLight, BlockSource, Chunk, ChunkManager, LightNode, SUBCHUNK_COUNT, SUBCHUNK_SIZE};

use crate::{
    blocks::{AirBlock, DirtBlock, GrassBlock, GreenGlassBlock, IceBlock, OakLeavesBlock, SandBlock, SnowBlock, StoneBlock, TorchBlock, WaterBlock, WoodBlock},
    camera::Camera,
    input::Input,
    player::{Item, ItemType, PlayerController},
    posteffects::{WorldScene, kawase::DualKawase},
    progress::{Progress, ProgressInfo, ProgressSender},
    util::{aabb_outline, cube_outline, get_movement_direction, get_rotation_directions, vertex_ao},
    world::{EntityData, EntityManager, World, WorldType},
};

pub const TICK_RATE_MS: usize = 50;
pub const TICK_RATE: Duration = Duration::from_millis(TICK_RATE_MS as u64);
pub const TPS: usize = 1000 / TICK_RATE_MS;
pub const FIXED_FRAMERATE: Duration = Duration::from_secs(1).checked_div(60).expect("failed to calculate fixed framerate somehow");

const _TEXT_COLOR: Color = Color::from_hsl(120.0, 0.5, 0.4);
const _BG_COLOR: Color = Color::from_hsl(120.0, 0.4, 0.75);

fn get_sky_color((after_day, progress): (bool, f32)) -> Color {
    const DAY_COLOR: Color = Color::from_hsl(220.0, 0.5, 0.75);
    const NIGHT_COLOR: Color = Color::from_hsl(220.0, 0.35, 0.25);

    if after_day {
        DAY_COLOR.lerp(&NIGHT_COLOR, progress)
    } else {
        NIGHT_COLOR.lerp(&DAY_COLOR, progress)
    }
}

// #[derive(Parser, Debug)]
// #[command(version, about, long_about = None)]
// struct Args {
//     #[arg(long, requires = "net")]
//     host: Option<SocketAddrV4>,
//     #[arg(short, long, group = "net")]
//     nickname: Option<String>,
// }

const GRASS_COLOR: Color = Color::from_hsl(120.0, 0.4, 0.75);

#[allow(clippy::struct_excessive_bools)]
struct Debugging {
    time_paused: bool,
    overlay: bool,
    wireframe: bool,
    draw_borders: bool,
    inventory_open: bool,
    chunk_borders: Vec<CommonVertex>,
    render_info: RenderInfo,
    item_rotation_x: f32,
    item_rotation_y: f32,
    item_rotation_z: f32,
}

impl Default for Debugging {
    fn default() -> Self {
        Self {
            time_paused: true,
            overlay: false,
            wireframe: false,
            draw_borders: false,
            inventory_open: false,
            chunk_borders: Vec::new(),
            render_info: RenderInfo::default(),
            item_rotation_x: 200.0f32.to_radians(),
            item_rotation_y: 35.0f32.to_radians(),
            item_rotation_z: 0.0,
        }
    }
}

enum Action {
    UpdateSubChunkMesh(IPoint2D, usize),
    #[allow(dead_code)]
    RemoveBlock(IPoint2D, USizePoint3D),
    ReplaceResourceManager(ResourceStorage),
    // ReplaceCompiler(Compiler),
    // ReplaceEventManager(EventManager),
}

struct Interval {
    duration: Duration,
    accel: Duration,
}

impl Interval {
    pub const fn new(duration: Duration) -> Self {
        Self {
            duration,
            accel: Duration::ZERO,
        }
    }

    pub fn update(&mut self, delta: Duration) -> usize {
        self.accel += delta;

        let mut times = 0;

        while self.accel >= self.duration {
            self.accel -= self.duration;

            times += 1;
        }

        times
    }
}

struct GameLoop {
    input: Input,
    animation_player: AnimationPlayer,
    common_renderer: CommonRenderer,
    resource_manager: Arc<ResourceStorage>,

    debugging: Debugging,

    action_sender: mpsc::Sender<Action>,
    action_receiver: mpsc::Receiver<Action>,

    accel: Duration,
    fixed_interval: Interval,

    scene: WorldScene,
    kawase: DualKawase<4>,
    texture_atlas: Texture2d,
    lightmap_atlas: Texture2d,

    current_page: Page,
    progress: Progress,

    world: Option<World>,
}

const INVENTORY_HOTBAR_SLOTS: u8 = 8;
const SLOT_SIZE: f32 = 48f32;

fn init_animation_player(animation_player: &mut AnimationPlayer) {
    animation_player.enable();

    animation_player.add("scale", || Transition::new(0.0, 1.0, 500, Curve::EASE_IN_OUT_EXPO, RepeatMode::Once));
    animation_player.add("scale-vertical", || Transition::new(0.0, 1.0, 500, Curve::EASE_IN_OUT_EXPO, RepeatMode::Once));
    animation_player.add("opacity", || Transition::new(0.0, 1.0, 500, Curve::EASE_IN_OUT_EXPO, RepeatMode::Once));
    animation_player.add("loading-screen", || Transition::new(1.0, 0.0, 1000, Curve::LINEAR, RepeatMode::Once));
    animation_player.add("show-settings", || Transition::new(0.0, 1.0, 500, Curve::EASE_IN_OUT_EXPO, RepeatMode::Once));
    animation_player.add("overlay-width", || Transition::new(0.0, 1.0, 400, Curve::EASE_IN_OUT_EXPO, RepeatMode::Once));
    animation_player.add("text-scaling", || {
        Transition::new(0.5, 1.0, 600, Curve::EASE_IN_OUT, RepeatMode::Infinite).with_restart_behaviour(RestartBehaviour::EndValue)
    });

    animation_player.add("shape-morph", || {
        Transition::new(0.0, 1.0, 600, Curve::EASE_IN_OUT_EXPO, RepeatMode::Infinite).with_restart_behaviour(RestartBehaviour::EndValue)
    });

    animation_player.add("stage-substage-translation", || {
        Transition::new(0.0, -1.0, 400, Curve::EASE_IN_OUT_EXPO, RepeatMode::Once).with_finish_behaviour(FinishBehaviour::Reset)
    });

    animation_player.add("chunks-opacity", || Transition::new(1.0, 1.0, 400, Curve::LINEAR, RepeatMode::Once));
    animation_player.add("chunks-progress", || Transition::new(0.0, 1.0, 400, Curve::EASE_IN_OUT_EXPO, RepeatMode::Once));

    animation_player.add("progress-opacity", || Transition::new(1.0, 1.0, 400, Curve::LINEAR, RepeatMode::Once));
    animation_player.add("stage-previous-progress", || {
        Transition::new(0.0, 1.0, 400, Curve::EASE_IN_OUT_EXPO, RepeatMode::Once)
    });

    animation_player.add("stage-progress", || Transition::new(0.0, 1.0, 400, Curve::EASE_IN_OUT_EXPO, RepeatMode::Once));
    animation_player.add("stage-substage-progress", || {
        Transition::new(0.0, 1.0, 400, Curve::EASE_IN_OUT_EXPO, RepeatMode::Once)
    });
}

fn register_block<T: Block + 'static>(
    resources: &mut ResourceStorage,
    sender: &ProgressSender,
    block: T,
) -> Result<(), mpsc::SendError<progress::ProgressChange>> {
    resources.register_block("game", block);

    sender.complete_task()?;

    Ok(())
}

// #[derive(Clone, Copy)]
// #[non_exhaustive]
// enum Event<'a> {
//     Tick,
//     WorldStart(&'a WorldData<'a>),
//     Message(&'a WorldData<'a>),
// }

// #[non_exhaustive]
// enum EventHandler {
//     Tick(fn()),
//     WorldStart(fn(&WorldData)),
//     Message(fn(&WorldData)),
// }

// struct WorldData<'a> {
//     world: &'a mut World,
// }

// impl WorldData<'_> {
//     fn send_chat_message(&mut self, text: &str) {
//         self.world.send_chat_message(text);
//     }
// }

// struct EventManager {
//     handlers: Vec<EventHandler>,
// }

// impl EventManager {
//     fn on_tick(&mut self, callback: fn()) {
//         self.handlers.push(EventHandler::Tick(callback));
//     }

//     fn on_world_start(&mut self, callback: fn(&WorldData)) {
//         self.handlers.push(EventHandler::WorldStart(callback));
//     }

//     fn on_message(&mut self, callback: fn(&WorldData)) {
//         self.handlers.push(EventHandler::Message(callback));
//     }

//     fn trigger(&self, event: Event) {
//         for handler in &self.handlers {
//             match (handler, event) {
//                 (EventHandler::Tick(callback), Event::Tick) => callback(),
//                 (EventHandler::WorldStart(callback), Event::WorldStart(data))
// => callback(data),                 (EventHandler::Message(callback),
// Event::Message(data)) => callback(data),                 _ => (),
//             }
//         }
//     }
// }

impl State for GameLoop {
    type Args = ();

    fn new(window: WindowContext, display: &WindowDisplay, _: Self::Args) -> Self {
        let (tx, rx) = mpsc::channel();
        let (action_sender, action_receiver) = mpsc::channel();

        let resource_manager = Arc::new(ResourceStorage::new("./resources"));

        let action_sender_clone = action_sender.clone();

        std::thread::spawn(move || {
            let mut resources = ResourceStorage::new("./resources");

            let sender = ProgressSender(tx);

            sender.set_visible(true)?;
            sender.set_initial_info(ProgressInfo::new(3, 0, 1, 0))?;

            sender.new_stage("Blocks loading", 13)?;

            resources.load_entity_model("game", "player");
            resources.load_entity_model("game", "floating");

            register_block(&mut resources, &sender, AirBlock)?;
            register_block(&mut resources, &sender, StoneBlock)?;
            register_block(&mut resources, &sender, WaterBlock)?;
            register_block(&mut resources, &sender, DirtBlock)?;
            register_block(&mut resources, &sender, GrassBlock)?;
            register_block(&mut resources, &sender, WoodBlock)?;
            register_block(&mut resources, &sender, SandBlock)?;
            register_block(&mut resources, &sender, OakLeavesBlock)?;
            register_block(&mut resources, &sender, IceBlock)?;
            register_block(&mut resources, &sender, GreenGlassBlock)?;
            register_block(&mut resources, &sender, TorchBlock)?;
            register_block(&mut resources, &sender, SnowBlock)?;

            sender.new_stage("Mip-maps generation", 4)?;

            for level in 1..=4 {
                resources.generate_mipmap(level);
                sender.complete_task()?;
            }

            _ = action_sender_clone.send(Action::ReplaceResourceManager(resources));

            sender.set_visible(false)
        });

        let (width, height) = display.get_framebuffer_dimensions();

        let size = window.window_size().as_::<f32>() / window.window_scale_factor() as f32;

        let mut common_renderer = CommonRenderer::new(display).unwrap_or_else(|e| panic!("failed to create CommonRenderer: {e}"));

        common_renderer.add_font("default", FONT);
        common_renderer.add_font("default_bold", FONT_BOLD);
        common_renderer.set_window_matrix(Transform3D::orthographic_rh_gl(0.0, size.width, size.height, 0.0, -100.0, 100.0));

        let mut animation_player = AnimationPlayer::default();

        init_animation_player(&mut animation_player);

        Self {
            input: Input::with_binds([
                ("walk.forward", KeyCode::KeyW),
                ("walk.backward", KeyCode::KeyS),
                ("walk.left", KeyCode::KeyA),
                ("walk.right", KeyCode::KeyD),
            ]),
            animation_player,
            common_renderer,
            current_page: Page::Main,
            debugging: Debugging::default(),
            resource_manager,
            accel: Duration::ZERO,
            fixed_interval: Interval::new(FIXED_FRAMERATE),
            action_sender,
            action_receiver,
            world: None,
            progress: Progress::new(rx),
            texture_atlas: Texture2d::empty_with_mipmaps(
                display,
                MipmapsOption::EmptyMipmapsMax(4),
                TextureStorage::ATLAS_SIZE.into(),
                TextureStorage::ATLAS_SIZE.into(),
            )
            .unwrap_or_else(|e| panic!("failed to create empty texture atlas on GPU: {e}")),
            lightmap_atlas: Texture2d::empty_with_mipmaps(
                display,
                MipmapsOption::EmptyMipmapsMax(4),
                TextureStorage::ATLAS_SIZE.into(),
                TextureStorage::ATLAS_SIZE.into(),
            )
            .unwrap_or_else(|e| panic!("failed to create empty texture atlas on GPU: {e}")),
            scene: WorldScene::new(display, width, height).unwrap(),
            kawase: DualKawase::new(display, width, height).unwrap(),
        }
    }

    fn handle_window_resize(&mut self, facade: &WindowDisplay, size: USize2D, scale_factor: f64) {
        self.scene.resize(facade, size.to_array()).unwrap();
        self.kawase.resize(facade, size.to_array()).unwrap();

        let size = size.as_::<f32>() / scale_factor as f32;

        self.common_renderer
            .set_window_matrix(Transform3D::orthographic_rh_gl(0.0, size.width, size.height, 0.0, -1000.0, 1000.0));

        if let Some(world) = &mut self.world {
            world.camera.aspect_ratio = size.width / size.height;
        }
    }

    fn handle_keyboard_input(&mut self, key: KeyCode, is_pressed: bool, repeat: bool) {
        self.input.keyboard.handle_keyboard_input(key, is_pressed, repeat);
    }

    fn handle_mouse_button(&mut self, button: MouseButton, is_pressed: bool) {
        self.input.mouse.handle_mouse_button(button, is_pressed);

        if let Some(world) = &mut self.world {
            if button == MouseButton::Left && is_pressed {
                world.destroy_looking_at();
            } else if self.input.mouse.is_pressed_once(MouseButton::Right) {
                let current_slot = world.inventory_slot.value as usize;

                if let Some(looking_at) = world.camera.looking_at
                    && world
                        .chunk_manager
                        .get_block(looking_at.position + looking_at.hit_side.as_normal())
                        .is_some_and(|block| block == 0)
                    && let Some((item, _)) = world.player.inventory.take_hotbar_item(current_slot)
                {
                    let position = looking_at.position + looking_at.hit_side.as_normal();
                    let chunk = ChunkManager::to_local(position);

                    if let Some(local) = world.chunk_manager.to_chunk_local(position) {
                        let [subchunk_idx, subchunk_y] = Chunk::get_subchunk_index(local.y);

                        world.chunk_manager.set_block(position, item as u8);

                        let block = self.resource_manager.get_block(item).unwrap();

                        let mut light = BfsLight::new(&mut world.chunk_manager);

                        let mut affected_chunks = if block.light_level() > 0 {
                            light.add_custom(LightNode(local, chunk), block.light_level());
                            light.calculate_with_info(self.resource_manager.as_ref())
                        } else if block.blocks_light() {
                            let mut light = light.apply_to_sky_light();

                            light.remove(LightNode(local, chunk));
                            light.calculate_with_info(self.resource_manager.as_ref())
                        } else {
                            Vec::new()
                        };

                        if let Some(chunks) = Chunk::corner(local) {
                            for offset in chunks {
                                if !affected_chunks.contains(&(chunk + offset)) {
                                    affected_chunks.push(chunk + offset);
                                }
                            }
                        } else if let Some(offset) = Chunk::side(local)
                            && !affected_chunks.contains(&(chunk + offset))
                        {
                            affected_chunks.push(chunk + offset);
                        }

                        if subchunk_y == 0 && subchunk_idx > 0 {
                            _ = self.action_sender.send(Action::UpdateSubChunkMesh(chunk, subchunk_idx - 1));
                        } else if subchunk_y == const { SUBCHUNK_SIZE - 1 } && subchunk_idx < const { SUBCHUNK_COUNT - 1 } {
                            _ = self.action_sender.send(Action::UpdateSubChunkMesh(chunk, subchunk_idx + 1));
                        }

                        for chunk in affected_chunks {
                            if subchunk_idx < const { SUBCHUNK_COUNT - 1 } {
                                _ = self.action_sender.send(Action::UpdateSubChunkMesh(chunk, subchunk_idx + 1));
                            }

                            _ = self.action_sender.send(Action::UpdateSubChunkMesh(chunk, subchunk_idx));

                            if subchunk_idx > 0 {
                                _ = self.action_sender.send(Action::UpdateSubChunkMesh(chunk, subchunk_idx - 1));
                            }
                        }

                        _ = self.action_sender.send(Action::UpdateSubChunkMesh(chunk, subchunk_idx));

                        let provider = AabbProvider {
                            chunk_manager: &world.chunk_manager,
                            entity_manager: &world.entities,
                            storage: self.resource_manager.as_ref(),
                        };

                        let context = PhysicsContext::new(provider);

                        world.camera.update_looking_at(&context);
                    }
                }
            }
        }
    }

    fn handle_mouse_motion(&mut self, delta: Option<Vector2D>, position: Option<Point2D>) {
        if let Some(delta) = delta
            && let Some(world) = self.world.as_mut()
            && world.player_controllable
        {
            let provider = AabbProvider {
                chunk_manager: &world.chunk_manager,
                entity_manager: &world.entities,
                storage: self.resource_manager.as_ref(),
            };

            let context = PhysicsContext::new(provider);

            world.camera.handle_mouse(&context, world.player.handle_mouse(delta));
        } else if let Some(position) = position {
            self.input.mouse.handle_mouse_motion(position);
        }
    }

    fn handle_mouse_wheel(&mut self, delta: Vector2D) {
        if let Some(world) = &mut self.world {
            if delta.y > 0.0 {
                world.inventory_slot.decrease();
            } else if delta.y < 0.0 {
                world.inventory_slot.increase();
            }
        }
    }

    #[allow(clippy::too_many_lines, clippy::significant_drop_tightening)]
    fn update(&mut self, context: WindowContext, _: &WindowDisplay, delta: Duration) {
        self.accel += delta;

        self.progress
            .update(&mut self.animation_player, &self.texture_atlas, &self.lightmap_atlas, &self.resource_manager);

        if let Some(world) = self.world.as_mut() {
            if world.player_controllable {
                for _ in 0..self.fixed_interval.update(delta) {
                    world.physics_step(&self.input);

                    if let Some(entity) = world.entities.get_mut(0) {
                        entity.set_rotation(0, world.player.get_vector_for_rotation().as_());
                    }
                }
            }

            for _ in 0..world.tick_interval.update(delta) {
                world.tick(self.debugging.time_paused);
            }

            world.update(self.common_renderer.white_pixel_uv(), &mut self.debugging);
        }

        if self.accel >= const { Duration::from_secs(1) } {
            self.accel = Duration::ZERO;

            if let Some(world) = self.world.as_mut() {
                world.ticks = world.tick_sum;
                world.tick_sum = 0;
            }
        }

        if self.input.keyboard.is_key_pressed_once(KeyCode::Tab) {
            if let Some(world) = self.world.as_mut() {
                world.player_controllable = !world.player_controllable;
            }

            if self.world.as_ref().is_some_and(|world| world.player_controllable) {
                context.set_cursor_grab(CursorGrabMode::Confined);
                context.set_cursor_visible(false);
            } else {
                context.set_cursor_grab(CursorGrabMode::None);
                context.set_cursor_visible(true);
            }
        }

        if self.input.keyboard.is_key_pressed_once(KeyCode::Escape) && self.animation_player.get_value("show-settings") > Some(0f32) {
            self.animation_player.play_transition_to("show-settings", 0.0);
        }

        self.animation_player.advance(delta.as_secs_f32());

        if let Some(world) = &mut self.world {
            for (_, drop) in &mut world.entities {
                if let EntityData::Item { transition, .. } = &mut drop.data {
                    transition.advance(delta.as_secs_f32());
                }
            }
        }

        if self.input.keyboard.is_key_pressed_once(KeyCode::KeyT) {
            self.debugging.wireframe = !self.debugging.wireframe;
        }

        if self.input.keyboard.is_key_pressed_once(KeyCode::KeyV) {
            self.debugging.inventory_open = !self.debugging.inventory_open;

            if self.debugging.inventory_open {
                let scale = self.animation_player.get_mut_unchecked("scale");

                scale.set_delay(0);
                scale.to(1.0);

                let scale_vertical = self.animation_player.get_mut_unchecked("scale-vertical");

                scale_vertical.set_delay(400);
                scale_vertical.to(1.0);

                let opacity = self.animation_player.get_mut_unchecked("opacity");

                opacity.set_delay(0);
                opacity.to(1.0);
            } else {
                let scale = self.animation_player.get_mut_unchecked("scale");

                scale.set_delay(400);
                scale.to(0.0);

                let scale_vertical = self.animation_player.get_mut_unchecked("scale-vertical");

                scale_vertical.set_delay(0);
                scale_vertical.to(0.0);

                let opacity = self.animation_player.get_mut_unchecked("opacity");

                opacity.set_delay(400);
                opacity.to(0.0);
            }

            self.animation_player.play("scale");
            self.animation_player.play("opacity");
            self.animation_player.play("scale-vertical");
        }

        if self.input.keyboard.is_key_pressed_once(KeyCode::KeyX) {
            self.debugging.item_rotation_x += const { 1.0f32.to_radians() };
        }

        if self.input.keyboard.is_key_pressed_once(KeyCode::KeyY) {
            self.debugging.item_rotation_y += const { 1.0f32.to_radians() };
        }

        if self.input.keyboard.is_key_pressed_once(KeyCode::KeyZ) {
            self.debugging.item_rotation_z += const { 1.0f32.to_radians() };
        }

        if self.input.keyboard.is_key_pressed_once(KeyCode::KeyP) {
            self.debugging.time_paused = !self.debugging.time_paused;
        }

        if self.input.keyboard.is_key_pressed_once(KeyCode::KeyO) {
            self.debugging.overlay = !self.debugging.overlay;

            self.animation_player
                .play_transition_to("overlay-width", if self.debugging.overlay { 1.0 } else { 0.0 });
        }

        for (digit, i) in [
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
            KeyCode::Digit5,
            KeyCode::Digit6,
            KeyCode::Digit7,
            KeyCode::Digit8,
            KeyCode::Digit9,
        ]
        .into_iter()
        .zip(0..9)
        {
            if self.input.keyboard.is_key_pressed_once(digit)
                && let Some(world) = &mut self.world
            {
                world.inventory_slot.value = i;
            }
        }

        if self.input.keyboard.is_key_pressed_once(KeyCode::KeyM)
            && let Some(world) = &mut self.world
        {
            world.marked = world.camera.looking_at.map(|looking_at| looking_at.position);
        }

        if let Ok(action) = self.action_receiver.try_recv() {
            match action {
                Action::UpdateSubChunkMesh(origin, subchunk_idx) => {
                    if let Some(world) = self.world.as_mut()
                        && let Some(subchunk) = world.compute_subchunk_mesh_at((origin, subchunk_idx))
                    {
                        world.voxel_renderer.set_subchunk((origin, subchunk_idx), subchunk);
                    }
                }
                Action::RemoveBlock(chunk, position) => {
                    if let Some(world) = self.world.as_mut() {
                        world.destroy_block_local(chunk, position);
                    }
                }
                Action::ReplaceResourceManager(manager) => self.resource_manager = Arc::new(manager),
                // Action::ReplaceCompiler(compiler) => self.compiler = compiler,
                // Action::ReplaceEventManager(manager) => self.event_manager = manager,
            }
        }

        if self.input.keyboard.is_key_pressed_once(KeyCode::KeyB) {
            self.debugging.draw_borders = !self.debugging.draw_borders;
        }

        if self.input.keyboard.is_key_pressed_once(KeyCode::KeyL) {
            self.resource_manager.debug_save();
        }
    }

    #[allow(clippy::too_many_lines)]
    fn render(&mut self, window_context: WindowContext, display: &WindowDisplay, delta: Duration) {
        let RenderInfo { draw_calls, vertices } = self.debugging.render_info.take();

        let (width, height) = display.get_framebuffer_dimensions();
        let mut frame = display.draw();

        if let Some(world) = self.world.as_mut() {
            let mut buffer = self.scene.buffer(display);

            let progress = world.clock.get_progress();

            world
                .voxel_renderer
                .set_sun_position(if progress > 0.5 { 1.0 - progress } else { progress } * 2.0);

            buffer.clear_color_and_depth((0.0, 0.0, 0.0, 1.0), 1.0);

            self.common_renderer
                .draw_rect(
                    Point2D::ZERO,
                    Size2D::new(width as f32, height as f32),
                    get_sky_color(world.clock.get_visual_progress()),
                )
                .unwrap();

            self.debugging
                .render_info
                .extend(&self.common_renderer.render(&mut buffer, display, None).unwrap());

            world.voxel_renderer.render(
                &mut buffer,
                world.camera.position,
                &world.camera.frustum,
                world.camera.matrix(),
                self.texture_atlas
                    .sampled()
                    .minify_filter(MinifySamplerFilter::NearestMipmapLinear)
                    .magnify_filter(MagnifySamplerFilter::Nearest),
                self.lightmap_atlas
                    .sampled()
                    .minify_filter(MinifySamplerFilter::NearestMipmapLinear)
                    .magnify_filter(MagnifySamplerFilter::Nearest),
                false,
            );

            let mut builder = VoxelMeshBuilder::with_capacity(world.entities.len());

            for (_, entity) in &world.entities {
                entity.render_to(&mut builder, &world.chunk_manager, self.resource_manager.as_ref());
            }

            builder.render(
                &world.voxel_renderer,
                &mut buffer,
                false,
                world.camera.matrix(),
                self.texture_atlas
                    .sampled()
                    .minify_filter(MinifySamplerFilter::NearestMipmapLinear)
                    .magnify_filter(MagnifySamplerFilter::Nearest),
                self.lightmap_atlas
                    .sampled()
                    .minify_filter(MinifySamplerFilter::NearestMipmapLinear)
                    .magnify_filter(MagnifySamplerFilter::Nearest),
            );

            self.kawase.apply(display, &self.scene).unwrap();

            self.scene.render(&mut frame).unwrap();
            self.debugging.render_info.extend(&world.voxel_renderer.get_debug_info());

            if self.debugging.draw_borders {
                self.common_renderer.set_matrix(world.camera.matrix());

                let size = world.player.player_aabb().size().as_();
                let lines = cube_outline(
                    Cube3D::new(world.player.body.position - Vector3D::new(size.width * 0.5, 0.0, size.depth * 0.5), size),
                    self.common_renderer.white_pixel_uv(),
                );

                self.debugging
                    .render_info
                    .extend(&self.common_renderer.render_lines(&mut frame, display, &lines, None).unwrap());

                for (_, entity) in &world.entities {
                    let aabb = entity.body.aabb();
                    let lines = cube_outline(Cube3D::new(aabb.min.as_(), aabb.size().as_()), self.common_renderer.white_pixel_uv());

                    self.debugging
                        .render_info
                        .extend(&self.common_renderer.render_lines(&mut frame, display, &lines, None).unwrap());
                }

                self.common_renderer.set_default_matrix();
            }

            if let Some(result) = world.camera.looking_at
                && let Some(mut model) = world
                    .chunk_manager
                    .get_block(result.position)
                    .filter(|&b| b != 0)
                    .and_then(|block| self.resource_manager.models.get(block.into()).map(|model| model.bounding_box))
            {
                let white_pixel = self.common_renderer.white_pixel_uv();

                model.min += result.position.as_::<f64>();
                model.max += result.position.as_::<f64>();

                self.common_renderer.set_matrix(world.camera.matrix());
                self.debugging.render_info.extend(
                    &self
                        .common_renderer
                        .render_lines(&mut frame, display, &aabb_outline(model, white_pixel), None)
                        .unwrap(),
                );

                self.common_renderer.set_default_matrix();
            }

            // {
            //     let sun_position = {
            //         let angle: f32 = self.animation_player.get_value_unchecked("sun");

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
            //             Point3D::new(
            //                 0.0,
            //                 (256.0 + 64.0) * sun_position[1],
            //                 (256.0 + 64.0) * sun_position[0],
            //             ),
            //             Point3D::ZERO,
            //             Point3D::Z,
            //         )))],
            //         &mut self.debugging.draw_calls,
            //         &mut self.debugging.vertices,
            //     );
            //     self.shape_renderer.set_default_matrix();
            // }

            let mut context = RenderContext::new(display, &mut self.common_renderer);
            let bounds = context.bounds;

            if self.debugging.wireframe {
                context.ui(|context, bounds| {
                    let height = 200.0;
                    let y_offset = bounds.size.height - height;

                    context.draw_rect(
                        Rect2D::new(Point2D::new(0.0, y_offset), Size2D::new(480.0, height)),
                        Color::from_hsl(0.0, 0.0, 0.5),
                    );

                    let skip_messages = world.chat_history.len().max(10) - 10;
                    let mut y_offset = y_offset;

                    for message in world.chat_history.iter().skip(skip_messages).take(10) {
                        let measured = context
                            .measure_text("default", message, 18.0, Some(480.0 - 4.0))
                            .unwrap_or_else(|| panic!("failed to measure next text: {message}"));

                        context.draw_text(
                            Point2D::new(2.0, y_offset + 1.0),
                            "default",
                            message,
                            18.0,
                            Color::from_hsl(0.0, 0.0, 1.0),
                            Some(480.0 - 4.0),
                        );

                        y_offset += measured.height + 1.0;
                    }
                });
            }

            context.ui(|context, bounds| {
                let hotbar_width = f32::from(INVENTORY_HOTBAR_SLOTS + 1) * SLOT_SIZE;

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
                let opacity: f32 = self.animation_player.get_value_unchecked("opacity");
                let scale: f32 = self.animation_player.get_value_unchecked("scale");
                let scale_vertical: f32 = self.animation_player.get_value_unchecked("scale-vertical");

                let screen_center = bounds.center();

                let size = Size2D::new(bounds.size.width * 0.65, bounds.size.height.mul_add(0.4, 320.0 * scale_vertical));

                let center = screen_center - (size / 2.0).to_vector();

                context.draw_rect(bounds, Color::BLACK.with_alpha(opacity.min(0.25)));
                context.add_transform(Transform3D::from_scale_rotation_translation(
                    Vector3D::from_array([scale; 3]),
                    Quat::IDENTITY,
                    screen_center.to_vector().extend(0.0) * (1.0 - scale),
                ));
                context.bounds(Rect2D::new(center, size), |context, _| {
                    context.fill(Color::from_hsl(130.0, 0.35, 0.25).with_alpha(opacity));

                    context.padding(2.0, |context, bounds| {
                        context.clipped(bounds, |context, bounds| {
                            let measured = context
                                .measure_text("default_bold", "Inventory", 18.0, None)
                                .unwrap_or_else(|| panic!("failed to measure next text: Inventory"));

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

            {
                let chunk = ChunkManager::to_local(world.player.body.position.as_::<i32>());

                let (hours, minutes) = {
                    let time = world.clock.time().as_secs();
                    let seconds = time % 60;
                    let minutes = (time - seconds) / 60 % 60;
                    let hours = (time - seconds - minutes * 60) / 60 / 60;

                    (hours, minutes)
                };

                let version = display.get_opengl_version_string();
                let rendered_chunks = world.voxel_renderer.rendered_subchunks();
                let total_chunks = world.voxel_renderer.total_subchunks();

                let text = format!(
                    "OpenGL {version}
OpenGL Renderer: {}
OpenGL Vendor: {}
Free GPU memory: {}
Window size: {width}x{height}
Player position: {:?} (chunk: {} {}, biome: {:?})
Game Time: {hours:02}:{minutes:02}
FPS: {:.0} ({:.2}ms)
TPS: {}
Looking at {}
Draw calls: {draw_calls}
Rotation: {} {} {}
Rendered subchunks: {rendered_chunks} / {total_chunks}
Rendered vertices: {vertices}",
                    display.get_opengl_renderer_string(),
                    display.get_opengl_vendor_string(),
                    display.get_free_video_memory().map_or_else(|| String::from("unknown"), util::format_bytes),
                    world.player.body.position,
                    chunk.x,
                    chunk.y,
                    world.chunk_manager.get_biome(world.player.body.position.as_::<i32>()),
                    1.0 / delta.as_secs_f32(),
                    delta.as_secs_f32() * 1000.0,
                    world.ticks,
                    world
                        .camera
                        .looking_at
                        .and_then(
                            |result| world.chunk_manager.get_block(result.position).filter(|&b| b != 0).and_then(|block| self
                                .resource_manager
                                .get_block(block.into())
                                .map(|block| format!(
                                    "{} (at {}, sky light: {})",
                                    block.id(),
                                    result.hit_side,
                                    world.chunk_manager.get_sky_light(result.position + result.hit_side.as_normal())
                                )))
                        )
                        .unwrap_or_else(|| String::from("nothing")),
                    self.debugging.item_rotation_x.to_degrees(),
                    self.debugging.item_rotation_y.to_degrees(),
                    self.debugging.item_rotation_z.to_degrees(),
                );

                let text_size = context
                    .measure_text("default", &text, 18.0, None)
                    .unwrap_or_else(|| panic!("failed to measure next text: {text}"));

                let overlay_width = self.animation_player.get_value_unchecked::<_, f32>("overlay-width");

                let text_bounds = Rect2D::new(Point2D::new(12.0, 12.0), Size2D::new((522.0 + 4.0) * overlay_width, text_size.height + 4.0));

                context.bounds(text_bounds, |context, _| {
                    context.fill(Color::BLACK.with_alpha(0.25));

                    context.padding(2.0, |context, bounds| {
                        context.clipped(bounds, |context, bounds| {
                            context.draw_text(bounds.origin, "default", text, 18.0, Color::WHITE, None);
                        });
                    });
                });
            }

            // if self.animation_player.get_value::<_, f32>("chunks-opacity") > Some(0.0) {
            //     show_world_generation_screen(&self.animation_player, &mut context,
            // &world.chunks_progress, self.window_matrix); }

            self.debugging.render_info.extend(&context.finish(display, &mut frame));

            let mut builder = VoxelMeshBuilder::with_capacity(world.player.inventory.get_hotbar_items().count());

            let matrix = Transform3D::from_rotation_x(Angle::from_radians(self.debugging.item_rotation_x))
                * Transform3D::from_rotation_y(Angle::from_radians(self.debugging.item_rotation_y))
                * Transform3D::from_rotation_z(Angle::from_radians(self.debugging.item_rotation_z));

            for (i, item) in world.player.inventory.get_hotbar_items() {
                const SIZE: f32 = SLOT_SIZE * 0.75;
                const ORIGIN: Point3D = Point3D::new(SIZE / 2.0, SIZE / 2.0, SIZE / 2.0);
                const HOTBAR_WIDTH: f32 = (INVENTORY_HOTBAR_SLOTS + 1) as f32 * SLOT_SIZE;

                let model = self.resource_manager.models.get_unchecked(item.id);
                let origin = Point2D::new((bounds.size.width / 2.0) - (HOTBAR_WIDTH / 2.0), bounds.size.height - SLOT_SIZE - 8.0);
                let slot_offset = (origin + Point2D::new(4.0, 4.0) + Point2D::new(i as f32 * SLOT_SIZE, 0.0)).extend(20.0);

                for element in &model.elements {
                    for model_face in &element.faces {
                        builder.push_transformed(
                            &VoxelFace {
                                position: slot_offset,
                                vertices: model_face.face_data.vertices.map(|vertex| vertex * SIZE),
                                lights: [240; 4],
                                uvs: model_face.face_data.uvs,
                                color: if model_face.tint { GRASS_COLOR } else { Color::WHITE }.multiply_rgb(model_face.face_data.face.get_light_level()),
                            },
                            &matrix,
                            ORIGIN,
                        );
                    }
                }
            }

            builder.render_full_bright(
                &world.voxel_renderer,
                &mut frame,
                false,
                self.common_renderer.window_matrix(),
                self.texture_atlas
                    .sampled()
                    .minify_filter(MinifySamplerFilter::NearestMipmapLinear)
                    .magnify_filter(MagnifySamplerFilter::Nearest),
                self.lightmap_atlas
                    .sampled()
                    .minify_filter(MinifySamplerFilter::NearestMipmapLinear)
                    .magnify_filter(MagnifySamplerFilter::Nearest),
            );

            let mut context = RenderContext::new(display, &mut self.common_renderer);

            context.ui(|context, bounds| {
                let hotbar_width = f32::from(INVENTORY_HOTBAR_SLOTS + 1) * SLOT_SIZE;
                let origin = Point2D::new((bounds.size.width / 2.0) - (hotbar_width / 2.0), bounds.size.height - SLOT_SIZE - 8.0);

                for (column, item) in world.player.inventory.get_hotbar_items() {
                    let offset = (column + 1) as f32 * SLOT_SIZE;
                    let text = format!("x{}", item.amount);

                    let text_size = context.measure_text("default", &text, 18.0, None).unwrap();

                    context.draw_text(
                        origin.with_y(bounds.size.height - 10.0 - 18.0) + Point2D::new(offset - 3.0 - text_size.width, 0.0),
                        "default",
                        text,
                        18.0,
                        Color::WHITE,
                        None,
                    );
                }
            });

            self.debugging.render_info.extend(&context.finish(display, &mut frame));
        } else {
            match self.current_page.render(
                window_context,
                display,
                &mut frame,
                &mut self.common_renderer,
                &mut self.animation_player,
                &mut self.debugging,
                &mut self.input,
                &self.progress,
            ) {
                Some(Page::WorldCreation) => {
                    // let (world_network_sender, world_network_receiver) = mpsc::channel();
                    // let (network_sender, network_receiver) = mpsc::channel();

                    // let world = World::new(display, Some((network_sender,
                    // world_network_receiver)));
                    let mut world = World::new(display, self.resource_manager.clone(), WorldType::Local);

                    world.chunk_manager = ChunkManager::new();
                    world.player.inventory.try_insert(Item {
                        id: self.resource_manager.get_block_id("torch") as usize,
                        ty: ItemType::Block,
                        amount: 64,
                    });

                    world.player.inventory.try_insert(Item {
                        id: self.resource_manager.get_block_id("oak_leaves") as usize,
                        ty: ItemType::Block,
                        amount: 64,
                    });

                    world.player.inventory.try_insert(Item {
                        id: self.resource_manager.get_block_id("ice") as usize,
                        ty: ItemType::Block,
                        amount: 64,
                    });

                    world.player.inventory.try_insert(Item {
                        id: self.resource_manager.get_block_id("green_glass_block") as usize,
                        ty: ItemType::Block,
                        amount: 64,
                    });

                    world.player.inventory.try_insert(Item {
                        id: self.resource_manager.get_block_id("wood") as usize,
                        ty: ItemType::Block,
                        amount: 64,
                    });

                    world.player.inventory.try_insert(Item {
                        id: self.resource_manager.get_block_id("snow") as usize,
                        ty: ItemType::Block,
                        amount: 64,
                    });

                    // world.player.inventory.try_insert(Item {
                    //     id: self.resource_manager.get_block_id("tech_test") as usize,
                    //     ty: ItemType::Block,
                    //     amount: 64,
                    // });

                    world.entities.spawn_model(Point3D::new(0.0, 128.0, 0.0), 0);
                    world.entities.spawn_model(Point3D::new(32.0, 128.0, 0.0), 1);
                    // let action_sender = self.action_sender.clone();

                    world.start_world_generation(128);

                    // let Client(mut receiver, mut sender) =
                    //     Client::new(TcpStream::from_std(std::net::TcpStream::connect(self.args.
                    // host.unwrap()).unwrap()).unwrap());

                    // println!("connected");

                    // tokio::spawn(async move {
                    //     loop {
                    //         if let Some(Ok(packet)) = receiver.next().await {
                    //             match packet {
                    //                 meralus_shared::OutgoingPacket::RemoveBlock(IPoint2D,
                    // usize_Point3D) => {                     println!("received
                    // block removal");

                    //                     action_sender.send(Action::RemoveBlock(IPoint2D,
                    // usize_Point3D)).unwrap();                 }
                    //                 packet => world_network_sender.send(packet).unwrap(),
                    //             }
                    //         }
                    //     }
                    // });

                    // let nickname = self.args.nickname.clone().unwrap();

                    // tokio::spawn(async move {
                    //     sender.send(IncomingPacket::PlayerConnected(nickname)).await.unwrap();

                    //     println!("sent nickname");

                    //     loop {
                    //         if let Ok(packet) = network_receiver.recv() {
                    //             sender.send(packet).await.unwrap();
                    //         }
                    //     }
                    // });

                    let size = window_context.window_size().as_::<f32>();

                    world.camera.aspect_ratio = size.width / size.height;

                    // self.event_manager.trigger(Event::WorldStart(&WorldData { world: &mut world
                    // }));
                    self.world.replace(world);
                }
                Some(page) => self.current_page = page,
                None => {}
            }

            self.common_renderer.render(&mut frame, display, None).unwrap();
        }

        frame.finish().expect("failed to finish draw frame");

        self.input.mouse.clear();
        self.input.keyboard.clear();
    }
}

enum Page {
    WorldCreation,
    Options,
    Main,
}

impl Page {
    fn render(
        &self,
        window_context: WindowContext,
        display: &WindowDisplay,
        frame: &mut glium::Frame,
        common_renderer: &mut CommonRenderer,
        animation_player: &mut AnimationPlayer,
        debugging: &mut Debugging,
        input: &mut Input,
        progress: &Progress,
    ) -> Option<Self> {
        let [r, g, b] = Color::from_u32_rgb(0x1D211B).to_linear();

        frame.clear_color_and_depth((r, g, b, 1.0), 1.0);

        let mut context = RenderContext::new(display, common_renderer);

        let page = match self {
            Self::WorldCreation => WorldCreationPage::render(window_context, &mut context, animation_player, input),
            Self::Options => OptionsPage::render(window_context, &mut context, animation_player, input),
            Self::Main => MainPage::render(window_context, &mut context, animation_player, input),
        };

        if animation_player.get_value::<_, f32>("progress-opacity") > Some(0.0) {
            show_loading_screen(animation_player, &mut context, progress);
        }

        debugging.render_info.extend(&context.finish(display, frame));

        page
    }
}

struct WorldCreationPage;

impl WorldCreationPage {
    const fn render(_window_context: WindowContext, _context: &mut RenderContext, _animation_player: &mut AnimationPlayer, _input: &Input) -> Option<Page> {
        None
    }
}

struct OptionsPage;

impl OptionsPage {
    fn render(_: WindowContext, _: &mut RenderContext, _: &mut AnimationPlayer, _: &Input) -> Option<Page> {
        None
    }
}

struct MainPage;

impl MainPage {
    fn render(window_context: WindowContext, context: &mut RenderContext, animation_player: &mut AnimationPlayer, input: &mut Input) -> Option<Page> {
        let mut page = None;

        context.ui(|context, bounds| {
            let text_scaling: f32 = animation_player.get_value_unchecked("text-scaling");
            let text_opacity = 1.0 - animation_player.get_value::<_, f32>("progress-opacity").unwrap_or(0.0);

            let size = context
                .measure_text("default", "Meralus", 72.0, None)
                .unwrap_or_else(|| panic!("failed to measure next text: Meralus"));
            let offset = Point2D::new(bounds.size.width / 2.0 - size.width / 2.0, 24.0);

            context.draw_text(
                bounds.origin + offset,
                "default",
                "Meralus",
                72.0,
                Color::from_hsl(110.0, 0.4, 0.7).with_alpha(text_opacity),
                None,
            );

            let origin = bounds.origin + offset + size.to_vector();
            let size = context.measure_text("default", "hiii wrld!!", 36.0, None).unwrap();

            context.transformed(
                Transform3D::from_translation(origin.to_vector().extend(0.0))
                    .scale(Vector3D::splat(text_scaling))
                    .rotate_z(-20f32.to_radians())
                    .translate(-origin.to_vector().extend(0.0)),
                |context, _| {
                    context.draw_text(
                        origin - size.to_vector() / 2.0,
                        "default",
                        "hiii wrld!!",
                        36.0,
                        Color::from_hsl(200.0, 0.8, 0.6).with_alpha(text_opacity),
                        None,
                    );
                },
            );

            context.draw_text(
                bounds.origin + Point2D::new(8.0, bounds.size.height - 24.0),
                "default",
                format!("developer build for {OS} (arch: {ARCH}), v{}", env!("CARGO_PKG_VERSION")),
                18.0,
                Color::from_hsl(110.0, 0.6, 0.6).with_alpha(text_opacity),
                None,
            );

            let button_width = (bounds.size.width * 0.4).max(192.0);
            let mut start = bounds.origin + Point2D::new(bounds.size.width / 2.0 - button_width / 2.0, bounds.size.height / 2.0 - 68.0);

            for (i, button) in MenuButton::ALL.into_iter().enumerate() {
                let animation = format!("menu-button-{i}");

                if !animation_player.contains(&animation) {
                    animation_player.add(&animation, || {
                        Transition::new(
                            Color::from_u32_rgb(0x3C4B38),
                            Color::from_u32_rgb(0x3C4B38),
                            200,
                            Curve::LINEAR,
                            RepeatMode::Once,
                        )
                    });
                }

                let box_bounds = RRect2D::new(start, Size2D::new(button_width, 40.0), Thickness::all(8.0));

                if box_bounds.contains(input.mouse.position) {
                    if input.mouse.entered.insert(i) {
                        animation_player.get_mut(&animation).unwrap().to(Color::from_u32_rgb(0x5E7558));
                        animation_player.play(&animation);
                    }
                } else if input.mouse.entered.remove(&i) {
                    animation_player.get_mut(&animation).unwrap().to(Color::from_u32_rgb(0x3C4B38));
                    animation_player.play(&animation);
                }

                if input.mouse.is_pressed_once(MouseButton::Left) && box_bounds.contains(input.mouse.position) {
                    match button {
                        MenuButton::Play => page = Some(Page::WorldCreation),
                        MenuButton::Options => page = Some(Page::Options),
                        MenuButton::Exit => window_context.close_window(),
                    }
                }

                context.draw_rounded_rect(box_bounds, animation_player.get_value_unchecked(&animation));

                let size = context.measure_text("default", button.as_str(), 36.0, None).unwrap();

                context.draw_text(
                    start + Point2D::new((bounds.size.width * 0.4) / 2.0 - size.width / 2.0, 0.0),
                    "default",
                    button.as_str(),
                    36.0,
                    Color::from_u32_rgb(0xD6E8CE),
                    None,
                );

                start.y += 48.0;
            }
        });

        // {
        //     let progress: f32 = animation_player.get_value_unchecked("text-scaling");
        //     let origin = Point2D::new(48.0, 48.0);
        //     let width = 64.0;
        //     let height = 48.0;
        //     let pixel_size = 4.0;
        //     let real_width = width * progress * pixel_size;
        //     let real_width = real_width - (real_width % pixel_size);

        //     for w in 0..49u16 {
        //         context.draw_rect(
        //             Rect2D::new(origin + Point2D::new(0.0, f32::from(w) *
        // pixel_size), Size2D::new(width * pixel_size, 1.0)),
        // Color::BLUE,         );
        //     }

        //     for h in 0..65u16 {
        //         context.draw_rect(
        //             Rect2D::new(origin + Point2D::new(f32::from(h) * pixel_size,
        // 0.0), Size2D::new(1.0, height * pixel_size)),
        // Color::BLUE,         );
        //     }

        //     // LEFT
        //     context.draw_rect(Rect2D::new(origin, Size2D::new(pixel_size, height *
        // pixel_size)), Color::RED);     // TOP
        //     context.draw_rect(Rect2D::new(origin, Size2D::new(real_width,
        // pixel_size)), Color::RED);     // RIGHT
        //     context.draw_rect(
        //         Rect2D::new(origin + Point2D::new(real_width, 0.0),
        // Size2D::new(pixel_size, (height + 1.0) * pixel_size)),
        //         Color::RED,
        //     );
        //     // BOTTOM
        //     context.draw_rect(
        //         Rect2D::new(origin + Point2D::new(0.0, height * pixel_size),
        // Size2D::new(real_width, pixel_size)),         Color::RED,
        //     );
        // }

        {
            let p: f32 = animation_player.get_value_unchecked("text-scaling");
            let xm = 32.0;
            let ym = 32.0;
            let pixel_size = 1.0;
            let r = 32.0 * p * pixel_size;
            let mut r = r - (r % pixel_size);

            let mut x = -r;
            let mut y = 0.0;
            let mut err = 2.0 - 2.0 * r;
            let pixel_size = 4.0;

            loop {
                context.draw_rect(
                    Rect2D::new(Point2D::new((xm - x) * pixel_size, (ym + y) * pixel_size), Size2D::splat(pixel_size)),
                    Color::RED,
                );
                context.draw_rect(
                    Rect2D::new(Point2D::new((xm - y) * pixel_size, (ym - x) * pixel_size), Size2D::splat(pixel_size)),
                    Color::RED,
                );
                context.draw_rect(
                    Rect2D::new(Point2D::new((xm + x) * pixel_size, (ym - y) * pixel_size), Size2D::splat(pixel_size)),
                    Color::RED,
                );
                context.draw_rect(
                    Rect2D::new(Point2D::new((xm + y) * pixel_size, (ym + x) * pixel_size), Size2D::splat(pixel_size)),
                    Color::RED,
                );

                r = err;

                if r <= y {
                    y += 1.0;
                    err += y * 2.0 + 1.0;
                }

                if r > x || err > y {
                    x += 1.0;
                    err += x * 2.0 + 1.0;
                }

                if x >= 0.0 {
                    break;
                }
            }
        }

        page
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

fn show_loading_screen(animation_player: &AnimationPlayer, context: &mut RenderContext, progress: &Progress) {
    let opacity = animation_player.get_value_unchecked("progress-opacity");

    context.ui(|context, bounds| {
        context.fill(Color::from_u32_rgb(0x3C4B38).with_alpha(opacity));

        let progress_bar = Size2D::new(bounds.size.width * 0.8, 48.0);
        let stages_progress_bar = bounds.origin + (bounds.size.to_vector() / 2.0) - (progress_bar.to_vector() / 2.0);

        if let Some(name) = progress.info.as_ref().and_then(|info| info.current_stage_name.as_ref()) {
            context.draw_text(
                stages_progress_bar - Point2D::new(0.0, 44.0).to_vector(),
                "default",
                name,
                36.0,
                Color::from_u32_rgb(0xA2D398).with_alpha(opacity),
                None,
            );
        }

        context.bounds(Rect2D::new(stages_progress_bar, progress_bar), |context, _| {
            context.fill(Color::from_u32_rgb(0xA2D398).with_alpha(opacity));

            context.padding(2.0, |context, _| {
                context.fill(Color::from_u32_rgb(0x3C4B38).with_alpha(opacity));

                if progress.info.is_some() {
                    let progress: f32 = animation_player.get_value_unchecked("stage-progress");

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
                        let progress: f32 = animation_player.get_value_unchecked("stage-substage-progress");
                        let translation: f32 = animation_player.get_value_unchecked("stage-substage-translation");

                        context.padding(2.0, |context, bounds| {
                            context.clipped(bounds, |context, bounds| {
                                context.draw_rect(
                                    Rect2D::new(
                                        bounds.origin,
                                        Size2D::new(
                                            bounds.size.width
                                                * if translation < 0.0 {
                                                    animation_player.get_value_unchecked("stage-previous-progress")
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

pub struct AabbProvider<'a> {
    pub chunk_manager: &'a ChunkManager,
    pub entity_manager: &'a EntityManager,
    pub storage: &'a ResourceStorage,
}

impl AabbSource for AabbProvider<'_> {
    fn get_aabb(&self, position: Point3D) -> Option<Aabb> {
        let correct_position = position.floor();

        for (_, entity) in self.entity_manager {
            if let EntityData::Model { id, .. } = &entity.data {
                let entity_position = position.as_::<f64>();

                if entity.body.aabb().contains(entity_position) {
                    for aabb in self.storage.entity_models.get_unchecked(*id).elements.iter().map(|element| element.cube) {
                        if aabb
                            .extended((entity.body.position - entity.body.size.to_vector() / 2.0).as_::<f64>())
                            .contains(entity_position)
                        {
                            return Some(aabb);
                        }
                    }
                }
            }
        }

        if let Some(block) = self.chunk_manager.get_block(correct_position.as_::<i32>())
            && self.storage.blocks.get_unchecked(block.into()).collidable()
        {
            let block_pos = position.as_::<f64>();

            for aabb in self.storage.models.get_unchecked(block.into()).elements.iter().map(|element| element.cube) {
                if aabb.contains(block_pos - correct_position.to_vector().as_::<f64>()) {
                    return Some(aabb);
                }
            }
        }

        None
    }

    fn get_block_aabb(&self, position: IPoint3D) -> Option<Aabb> {
        self.chunk_manager
            .get_block(position)
            .filter(|&b| b != 0 && self.storage.blocks.get_unchecked(b.into()).selectable())
            .and_then(|block| self.storage.models.get(block.into()))
            .map(|element| element.bounding_box)
    }
}

pub struct LimitedAabbProvider<'a> {
    pub chunk_manager: &'a ChunkManager,
    pub storage: &'a ResourceStorage,
}

impl AabbSource for LimitedAabbProvider<'_> {
    fn get_aabb(&self, position: Point3D) -> Option<Aabb> {
        let correct_position = position.floor();

        if let Some(block) = self.chunk_manager.get_block(correct_position.as_::<i32>())
            && self.storage.blocks.get_unchecked(block.into()).collidable()
        {
            let block_pos = position.as_::<f64>();

            for aabb in self.storage.models.get_unchecked(block.into()).elements.iter().map(|element| element.cube) {
                if aabb.contains(block_pos - correct_position.to_vector().as_::<f64>()) {
                    return Some(aabb);
                }
            }
        }

        None
    }

    fn get_block_aabb(&self, position: IPoint3D) -> Option<Aabb> {
        self.chunk_manager
            .get_block(position)
            .filter(|&b| b != 0 && self.storage.blocks.get_unchecked(b.into()).selectable())
            .and_then(|block| self.storage.models.get(block.into()))
            .map(|element| element.bounding_box)
    }
}

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() {
    tracing_subscriber::fmt().init();

    // let args = Args::parse();

    Application::<GameLoop>::new(()).start().expect("failed to run app");
}
