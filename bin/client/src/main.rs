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
mod progress;
mod render;
mod scenes;
mod util;
mod world;

use std::{
    env::consts::{ARCH, OS},
    f32, fmt,
    path::PathBuf,
    sync::{Arc, mpsc},
    time::Duration,
};

use cpal::traits::HostTrait;
use discord_presence::models::{ActivityType, DisplayType};
use kira::{AudioManager, AudioManagerSettings, backend::cpal::CpalBackendSettings};
use meralus_engine::{Application, CursorGrabMode, KeyCode, KeyboardModifiers, MouseButton, State, WindowContext, WindowDisplay};
use meralus_physics::{Aabb, AabbSource, PhysicsContext};
use meralus_shared::{
    Angle, Color, IPoint2D, IPoint3D, Lerp, MatrixExt, Point2D, Point3D, Quat, RRect2D, Rect2D, Size2D, Thickness, Transform3D, USize2D, USizePoint3D,
    Vector2D, Vector3D,
};
use meralus_storage::{Block, ResourceStorage, TextureStorage};
use meralus_world::{BfsLight, BlockSource, Chunk, ChunkAccess, ChunkCache, ChunkManager, ChunkStage, LightNode, SubChunkBlockState};
use tracing::info;
use tracy_client::{set_thread_name, span};

use crate::{
    blocks::{
        AirBlock, BlueRoseBlock, BricksBlock, CobbleStoneBlock, DebugBlock, DirtBlock, GrassBlock, GreenGlassBlock, IceBlock, OakLeavesBlock, OakLogBlock,
        RoseBlock, SandBlock, SnowBlock, StoneBlock, StoneBricksBlock, TorchBlock, WaterBlock, WoodBlock,
    },
    camera::Camera,
    input::Input,
    player::{Item, ItemType, PlayerController},
    posteffects::{ParticleSystem, WorldScene, kawase::DualKawase},
    progress::{Progress, ProgressInfo, ProgressSender},
    scenes::{Screen, loading_overlay::LoadingOverlay},
    util::{aabb_outline, cube_outline, get_movement_direction, get_rotation_directions, vertex_ao},
    world::{EntityData, EntityManager, World, WorldType},
};

pub const TICK_RATE_MS: usize = 50;
pub const TICK_RATE: Duration = Duration::from_millis(TICK_RATE_MS as u64);
pub const TPS: usize = 1000 / TICK_RATE_MS;
pub const FIXED_FRAMERATE: Duration = Duration::from_secs(1).checked_div(60).expect("failed to calculate fixed framerate somehow");

const _TEXT_COLOR: Color = Color::from_hsl(120.0, 0.5, 0.4);
const _BG_COLOR: Color = Color::from_hsl(120.0, 0.4, 0.75);

fn get_sky_color((after_day, progress): (bool, f32), weather: f32) -> Color {
    let day_color: Color = Color::from_hsl(220.0, 0.2f32.mul_add(weather, 0.5), 0.6f32.mul_add(-weather, 0.75));
    let night_color: Color = Color::from_hsl(220.0, 0.1f32.mul_add(weather, 0.35), 0.15f32.mul_add(-weather, 0.25));

    if after_day {
        day_color.lerp(&night_color, progress)
    } else {
        night_color.lerp(&day_color, progress)
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
            item_rotation_x: 200f32.to_radians(),
            item_rotation_y: 35f32.to_radians(),
            item_rotation_z: 0.0,
        }
    }
}

enum Action {
    #[allow(dead_code)]
    RemoveBlock(IPoint2D, USizePoint3D),
    ReplaceResourceManager(ResourceStorage),
    #[cfg(feature = "addons")]
    ReplaceAddonManager(meralus_addons::AddonManager),
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

#[derive(Debug, Clone, Copy)]
enum LightStyle {
    Smooth,
    BlockyWithAO,
    Blocky,
}

impl LightStyle {
    #[inline]
    fn does_block_have_ao(resource_storage: &ResourceStorage, block: &str) -> bool {
        if block == "game:air" {
            false
        } else {
            resource_storage
                .models
                .get_unchecked(resource_storage.blocks.get_model_by_name(block))
                .ambient_occlusion
        }
    }

    fn smooth_light<T: ChunkAccess>(
        chunks: &T,
        resource_storage: &ResourceStorage,
        world_position: IPoint3D,
        light_source: IPoint3D,
        corners: [[IPoint3D; 3]; 4],
        have_ao: bool,
    ) -> ([f32; 4], [u8; 4]) {
        let mut aos = [1.0; 4];
        let mut lights = [0; 4];
        let init_light = chunks.get_light_level(light_source);

        for (([side1, side2, corner], ao), light) in corners.into_iter().zip(&mut aos).zip(&mut lights) {
            if have_ao {
                let side1_block: IPoint3D = world_position + side1;
                let side2_block: IPoint3D = world_position + side2;
                let corner_block: IPoint3D = world_position + corner;

                let (side1_block, side1_light) = chunks.get_block_with_light_level(side1_block);
                let (side2_block, side2_light) = chunks.get_block_with_light_level(side2_block);
                let (corner_block, corner_light) = chunks.get_block_with_light_level(corner_block);

                let block_light = (Chunk::block_light_from_level(init_light)
                    + Chunk::block_light_from_level(side1_light)
                    + Chunk::block_light_from_level(side2_light)
                    + Chunk::block_light_from_level(corner_light))
                    / 4;

                let sky_light = (Chunk::sky_light_from_level(init_light)
                    + Chunk::sky_light_from_level(side1_light)
                    + Chunk::sky_light_from_level(side2_light)
                    + Chunk::sky_light_from_level(corner_light))
                    / 4;

                *light = (sky_light << 4) | (block_light & 0xF);

                *ao = vertex_ao(
                    Self::does_block_have_ao(resource_storage, side1_block.map_or("game:air", |state| &state.name)),
                    Self::does_block_have_ao(resource_storage, side2_block.map_or("game:air", |state| &state.name)),
                    Self::does_block_have_ao(resource_storage, corner_block.map_or("game:air", |state| &state.name)),
                );
            } else {
                *light = chunks.get_light_level(light_source);
            }
        }

        (aos, lights)
    }

    fn blocky_with_ao<T: ChunkAccess>(
        chunks: &T,
        resource_storage: &ResourceStorage,
        world_position: IPoint3D,
        light_source: IPoint3D,
        corners: [[IPoint3D; 3]; 4],
        have_ao: bool,
    ) -> ([f32; 4], [u8; 4]) {
        let mut aos: [f32; 4] = [1.0; 4];
        let light = chunks.get_light_level(light_source);

        for ([side1, side2, corner], ao) in corners.into_iter().zip(&mut aos) {
            if have_ao {
                *ao = vertex_ao(
                    Self::does_block_have_ao(
                        resource_storage,
                        chunks.get_block(world_position + side1).map_or("game:air", |state| &state.name),
                    ),
                    Self::does_block_have_ao(
                        resource_storage,
                        chunks.get_block(world_position + side2).map_or("game:air", |state| &state.name),
                    ),
                    Self::does_block_have_ao(
                        resource_storage,
                        chunks.get_block(world_position + corner).map_or("game:air", |state| &state.name),
                    ),
                );
            }
        }

        (aos, [light; 4])
    }

    fn blocky<T: ChunkAccess>(chunks: &T, _: &ResourceStorage, _: IPoint3D, light_source: IPoint3D, _: [[IPoint3D; 3]; 4], _: bool) -> ([f32; 4], [u8; 4]) {
        let light = chunks.get_light_level(light_source);

        ([0.0; 4], [light; 4])
    }

    #[inline]
    pub const fn get_light_fn<T: ChunkAccess>(self) -> fn(&T, &ResourceStorage, IPoint3D, IPoint3D, [[IPoint3D; 3]; 4], bool) -> ([f32; 4], [u8; 4]) {
        match self {
            Self::Smooth => Self::smooth_light::<T>,
            Self::BlockyWithAO => Self::blocky_with_ao::<T>,
            Self::Blocky => Self::blocky::<T>,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct GraphicsSettings {
    light_style: LightStyle,
    render_distance: usize,
}

impl Default for GraphicsSettings {
    fn default() -> Self {
        Self {
            light_style: LightStyle::Smooth,
            render_distance: 12,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Settings {
    graphics: GraphicsSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            graphics: GraphicsSettings::default(),
        }
    }
}

struct GameLoop {
    audio_manager: AudioManager,
    input: Input,
    animation_player: AnimationPlayer,
    common_renderer: CommonRenderer,
    resource_manager: Arc<ResourceStorage>,

    particles: ParticleSystem,

    action_receiver: mpsc::Receiver<Action>,

    #[cfg(feature = "addons")]
    addons: meralus_addons::AddonManager,

    debug_interval: Interval,
    fixed_interval: Interval,

    scene: WorldScene,
    kawase: DualKawase<4>,
    texture_atlas: Texture2d,
    lightmap_atlas: Texture2d,

    context: UiContext,
    overlay: LoadingOverlay,
    current_page: Page,
    progress: Progress,

    world: Option<World>,
    settings: Settings,

    drpc: discord_presence::Client,
}

const INVENTORY_HOTBAR_SLOTS: u8 = 8;
const SLOT_SIZE: f32 = 48f32;

fn register_block<T: Block + 'static>(
    resources: &mut ResourceStorage,
    sender: &ProgressSender,
    block: T,
) -> Result<(), mpsc::SendError<progress::ProgressChange>> {
    resources.register_block("game", block);

    sender.complete_task()?;

    Ok(())
}

impl State for GameLoop {
    type Args = ();

    fn new(window: WindowContext, display: &WindowDisplay, _: Self::Args) -> Self {
        let (tx, rx) = mpsc::channel();
        let (action_sender, action_receiver) = mpsc::channel();

        let resource_manager = Arc::new(ResourceStorage::new("./resources"));

        std::thread::spawn(move || {
            set_thread_name!("Resources Loading");

            let mut resources = ResourceStorage::new("./resources");

            let sender = ProgressSender(tx);

            #[cfg(not(feature = "addons"))]
            let total_stages = 2;
            #[cfg(feature = "addons")]
            let total_stages = 3;

            sender.set_visible(true)?;
            sender.set_initial_info(ProgressInfo::new(total_stages, 0, 1, 0))?;

            sender.new_stage("Blocks loading", 20)?;

            let zone = span!("Models Loading");

            resources.load_entity_model("game", "player");

            zone.emit_text("Registered entities:game/player");

            resources.load_entity_model("game", "floating");

            zone.emit_text("Registered entities:game/player");

            register_block(&mut resources, &sender, AirBlock)?;
            zone.emit_text("Registered blocks:game/air");
            register_block(&mut resources, &sender, StoneBlock)?;
            zone.emit_text("Registered blocks:game/stone");
            register_block(&mut resources, &sender, WaterBlock)?;
            zone.emit_text("Registered blocks:game/water");
            register_block(&mut resources, &sender, DirtBlock)?;
            zone.emit_text("Registered blocks:game/dirt");
            register_block(&mut resources, &sender, GrassBlock)?;
            zone.emit_text("Registered blocks:game/grass");
            register_block(&mut resources, &sender, WoodBlock)?;
            zone.emit_text("Registered blocks:game/wood");
            register_block(&mut resources, &sender, SandBlock)?;
            zone.emit_text("Registered blocks:game/sand");
            register_block(&mut resources, &sender, OakLeavesBlock)?;
            zone.emit_text("Registered blocks:game/oak_leaves");
            register_block(&mut resources, &sender, OakLogBlock)?;
            zone.emit_text("Registered blocks:game/oak_log");
            register_block(&mut resources, &sender, IceBlock)?;
            zone.emit_text("Registered blocks:game/ice");
            register_block(&mut resources, &sender, GreenGlassBlock)?;
            zone.emit_text("Registered blocks:game/green_glass");
            register_block(&mut resources, &sender, TorchBlock)?;
            zone.emit_text("Registered blocks:game/torch");
            register_block(&mut resources, &sender, SnowBlock)?;
            zone.emit_text("Registered blocks:game/snow");
            register_block(&mut resources, &sender, RoseBlock)?;
            zone.emit_text("Registered blocks:game/rose");
            register_block(&mut resources, &sender, BlueRoseBlock)?;
            zone.emit_text("Registered blocks:game/blue_rose");
            register_block(&mut resources, &sender, CobbleStoneBlock)?;
            zone.emit_text("Registered blocks:game/cobblestone");
            register_block(&mut resources, &sender, BricksBlock)?;
            zone.emit_text("Registered blocks:game/bricks");
            register_block(&mut resources, &sender, StoneBricksBlock)?;
            zone.emit_text("Registered blocks:game/stone_bricks");
            register_block(&mut resources, &sender, DebugBlock)?;
            zone.emit_text("Registered blocks:game/debug");

            #[cfg(feature = "addons")]
            {
                sender.new_stage("Loading addons", 1);

                let mut addons = meralus_addons::AddonManager::new("./addons").unwrap();

                addons.insert_mappings(&mut resources);
                addons.execute(&mut resources);

                _ = action_sender.send(Action::ReplaceAddonManager(addons));
            }

            sender.new_stage("Mip-maps generation", 4)?;

            let zone = span!("Mip-maps generation");

            for level in 1..=4 {
                resources.generate_mipmap(level);

                sender.complete_task()?;

                zone.emit_text("Generated mip-map");
            }

            _ = action_sender.send(Action::ReplaceResourceManager(resources));

            sender.set_visible(false)
        });

        let (width, height) = display.get_framebuffer_dimensions();

        let size = window.window_size().as_vec2() / window.window_scale_factor() as f32;

        let mut common_renderer = CommonRenderer::new(display).unwrap_or_else(|e| panic!("failed to create CommonRenderer: {e}"));

        common_renderer.add_font("default", FONT);
        common_renderer.add_font("default_bold", FONT_BOLD);
        common_renderer.set_window_matrix(Transform3D::orthographic_rh_gl(0.0, size.x, size.y, 0.0, -100.0, 100.0));

        let mut animation_player = AnimationPlayer::default();

        // init_animation_player(&mut animation_player);

        let mut drpc = discord_presence::Client::new(1488208518765871164);

        drpc.start();

        // let sounds = fs::read_dir("./resources/sounds")
        //     .unwrap()
        //     .flatten()
        //     .filter_map(|sound| {
        //         let path = sound.path();

        //         if path.is_file() {
        //             StaticSoundData::from_file(path)
        //                 .ok()
        //                 .and_then(|data| Some((sound.file_name().into_string().ok()?,
        // data)))         } else {
        //             None
        //         }
        //     })
        //     .collect();

        Self {
            audio_manager: AudioManager::new(AudioManagerSettings {
                backend_settings: CpalBackendSettings {
                    device: cpal::host_from_id(cpal::HostId::Jack)
                        .ok()
                        .and_then(|host| host.default_output_device())
                        .or_else(|| cpal::default_host().default_output_device()),
                    ..CpalBackendSettings::default()
                },
                ..AudioManagerSettings::default()
            })
            .unwrap(),
            input: Input::with_binds([
                ("walk.forward", KeyCode::KeyW),
                ("walk.backward", KeyCode::KeyS),
                ("walk.left", KeyCode::KeyA),
                ("walk.right", KeyCode::KeyD),
            ]),
            animation_player,
            common_renderer,
            current_page: Page::Main,
            resource_manager,
            #[cfg(feature = "addons")]
            addons: meralus_addons::AddonManager::new("./addons").unwrap(),
            debug_interval: Interval::new(Duration::from_secs(5)),
            fixed_interval: Interval::new(FIXED_FRAMERATE),
            action_receiver,
            world: None,
            settings: Settings::default(),
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
            drpc,
            context: UiContext::new(),
            particles: ParticleSystem::new(display),
            overlay: LoadingOverlay {
                progress: TypedTransition::new(0.0, 1.0, 1000, Curve::LINEAR, RepeatMode::Once),
            },
        }
    }

    fn handle_window_resize(&mut self, facade: &WindowDisplay, size: USize2D, scale_factor: f64) {
        self.scene.resize(facade, size.to_array()).unwrap();
        self.kawase.resize(facade, size.to_array()).unwrap();

        let size = size.as_vec2() / scale_factor as f32;

        self.common_renderer
            .set_window_matrix(Transform3D::orthographic_rh_gl(0.0, size.x, size.y, 0.0, -1000.0, 1000.0));

        if let Some(world) = &mut self.world {
            world.camera.aspect_ratio = size.x / size.y;
        }
    }

    fn handle_keyboard_input(&mut self, key: KeyCode, is_pressed: bool, repeat: bool) {
        self.input.keyboard.handle_keyboard_input(key, is_pressed, repeat);
    }

    fn handle_keyboard_modifiers(&mut self, modifiers: KeyboardModifiers) {
        self.input.keyboard.modifiers = modifiers;
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
                        .is_some_and(|block| block.name == "game:air")
                    && let Some((item, _)) = world.player.inventory.take_hotbar_item(current_slot)
                {
                    let position = looking_at.position + looking_at.hit_side.as_normal();
                    let chunk = ChunkManager::<()>::to_local(position);

                    if let Some(local) = world.chunk_manager.to_chunk_local(position) {
                        let block = self.resource_manager.get_block(&item).unwrap();

                        world.chunk_manager.set_block(position, SubChunkBlockState::new(item));

                        let mut light = BfsLight::new(&mut world.chunk_manager);

                        if block.light_level() > 0 {
                            light.add_block_custom(LightNode(local, chunk), block.light_level());
                            light.calculate_block_light(self.resource_manager.as_ref());
                        } else if block.blocks_light() {
                            light.remove_sky(LightNode(local, chunk));
                            light.calculate_sky_light(self.resource_manager.as_ref());
                        }

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
            self.context.process_mouse_move(position);
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
    fn update(&mut self, context: WindowContext, display: &WindowDisplay, delta: Duration) {
        self.overlay.update(delta);

        if let Some(info) = &self.progress.info {
            self.overlay.progress.to(info.completed as f32 / info.total as f32);

            if self.overlay.progress.is_finished() {
                self.overlay.progress.reset();
            }
        }

        self.progress
            .update(&mut self.animation_player, &self.texture_atlas, &self.lightmap_atlas, &self.resource_manager);

        if let Some(world) = self.world.as_mut() {
            if world.player_controllable {
                for _ in 0..self.fixed_interval.update(delta) {
                    world.physics_step(&self.input);

                    let provider = AabbProvider {
                        chunk_manager: &world.chunk_manager,
                        entity_manager: &world.entities,
                        storage: world.resource_storage.as_ref(),
                    };

                    let context = PhysicsContext::new(provider);

                    self.particles.physics_update(&context, FIXED_FRAMERATE.as_secs_f32());

                    if let Some(entity) = world.entities.get_mut(0) {
                        entity.set_rotation(0, world.player.get_vector_for_rotation().as_vec3());
                    }
                }
            }

            for _ in 0..world.tick_interval.update(delta) {
                world.tick(true);
            }

            world.update(self.settings.graphics);
        }

        for _ in 0..self.debug_interval.update(delta) {
            let mut drpc = self.drpc.clone();
            let chunks = self.world.as_ref().map(|world| world.chunk_manager.len());

            std::thread::spawn(move || {
                drpc.set_activity(|activity| {
                    activity
                        .activity_type(ActivityType::Playing)
                        .details(chunks.map_or_else(|| String::from("In Main Menu"), |chunks| format!("In world ({chunks} loaded chunks)")))
                        .status_display(DisplayType::Details)
                })
                .expect("Failed to set activity");
            });

            if let Some(world) = self.world.as_mut() {
                world.ticks = world.tick_sum;
                world.tick_sum = 0;
            }
        }

        if self.input.keyboard.is_key_pressed_once(KeyCode::KeyL) {
            self.resource_manager.debug_save();
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

        self.animation_player.advance(delta.as_secs_f32());

        if let Some(world) = &mut self.world {
            for (_, drop) in &mut world.entities {
                if let EntityData::Item { transition, .. } = &mut drop.data {
                    transition.advance(delta.as_secs_f32());
                }
            }
        }

        if self.input.keyboard.modifiers.control_key
            && self.input.keyboard.is_key_pressed_once(KeyCode::KeyS)
            && let Some(world) = &mut self.world
        {
            info!("Saving world ({} chunks)", world.chunk_manager.len());

            world.chunk_manager.save();
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
                Action::RemoveBlock(chunk, position) => {
                    if let Some(world) = self.world.as_mut() {
                        world.destroy_block_local(chunk, position);
                    }
                }
                Action::ReplaceResourceManager(manager) => self.resource_manager = Arc::new(manager),
                #[cfg(feature = "addons")]
                Action::ReplaceAddonManager(addons) => self.addons = addons,
            }
        }

        if self.input.keyboard.is_key_pressed_once(KeyCode::KeyL) {
            self.resource_manager.debug_save();
        }

        self.context.update();

        if self.input.mouse.is_released(MouseButton::Left) {
            self.context.process_mouse_up();
        }
    }

    #[allow(clippy::too_many_lines)]
    fn render(&mut self, window_context: WindowContext, display: &WindowDisplay, delta: Duration) {
        let RenderInfo { draw_calls, vertices } = RenderInfo::default();

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
                    get_sky_color(world.clock.get_visual_progress(), 0.0),
                )
                .unwrap();

            self.common_renderer.render(&mut buffer, display, None).unwrap();

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
            self.particles.render(&mut frame, world.camera.matrix()).unwrap();
            // self.debugging.render_info.extend(&world.voxel_renderer.get_debug_info());

            // if self.debugging.draw_borders {
            //     self.common_renderer.set_matrix(world.camera.matrix());

            //     let size = world.player.player_aabb().size().as_();
            //     let lines = cube_outline(
            //         Cube3D::new(world.player.body.position - Vector3D::new(size.x *
            // 0.5, 0.0, size.z * 0.5), size),         self.common_renderer.
            // white_pixel_uv(),     );

            //     self.debugging
            //         .render_info
            //         .extend(&self.common_renderer.render_lines(&mut frame, display,
            // &lines, None).unwrap());

            //     for (_, entity) in &world.entities {
            //         let aabb = entity.body.aabb();
            //         let lines = cube_outline(Cube3D::new(aabb.min.as_(),
            // aabb.size().as_()), self.common_renderer.white_pixel_uv());

            //         self.debugging
            //             .render_info
            //             .extend(&self.common_renderer.render_lines(&mut frame, display,
            // &lines, None).unwrap());     }

            //     self.common_renderer.set_default_matrix();
            // }

            if let Some(result) = world.camera.looking_at
                && let Some(mut model) = world
                    .chunk_manager
                    .get_block(result.position)
                    .filter(|&b| b.name != "game:air")
                    .and_then(|block| {
                        self.resource_manager
                            .models
                            .get(self.resource_manager.blocks.get_model_by_name(&block.name))
                            .map(|model| model.bounding_box)
                    })
            {
                let white_pixel = self.common_renderer.white_pixel_uv();

                model.min += result.position.as_dvec3();
                model.max += result.position.as_dvec3();

                self.common_renderer.set_matrix(world.camera.matrix());
                // self.debugging.render_info.extend(
                &self
                    .common_renderer
                    .render_lines(&mut frame, display, &aabb_outline(model, white_pixel), None)
                    .unwrap();
                // );

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

            // if self.debugging.wireframe {
            //     context.ui(|context, bounds| {
            //         let height = 200.0;
            //         let y_offset = bounds.size.y - height;

            //         context.draw_rect(
            //             Rect2D::new(Point2D::new(0.0, y_offset), Size2D::new(480.0,
            // height)),             Color::from_hsl(0.0, 0.0, 0.5),
            //         );

            //         let skip_messages = world.chat_history.len().max(10) - 10;
            //         let mut y_offset = y_offset;

            //         for message in world.chat_history.iter().skip(skip_messages).take(10)
            // {             let measured = context
            //                 .measure_text("default", message, 18.0, Some(480.0 - 4.0))
            //                 .unwrap_or_else(|| panic!("failed to measure next text:
            // {message}"));

            //             context.draw_text(
            //                 Point2D::new(2.0, y_offset + 1.0),
            //                 "default",
            //                 message,
            //                 18.0,
            //                 Color::from_hsl(0.0, 0.0, 1.0),
            //                 Some(480.0 - 4.0),
            //             );

            //             y_offset += measured.y + 1.0;
            //         }
            //     });
            // }

            context.ui(|context, bounds| {
                let hotbar_width = f32::from(INVENTORY_HOTBAR_SLOTS + 1) * SLOT_SIZE;

                let origin = Point2D::new((bounds.size.x / 2.0) - (hotbar_width / 2.0), bounds.size.y - SLOT_SIZE - 8.0);

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
                let opacity: f32 = 0.0; // self.animation_player.get_value_unchecked("opacity");
                let scale: f32 = 0.0; // self.animation_player.get_value_unchecked("scale");
                let scale_vertical: f32 = 0.0; // self.animation_player.get_value_unchecked("scale-vertical");

                let screen_center = bounds.center();

                let size = Size2D::new(bounds.size.x * 0.65, bounds.size.y.mul_add(0.4, 320.0 * scale_vertical));

                let center = screen_center - (size / 2.0);

                context.draw_rect(bounds, Color::BLACK.with_alpha(opacity.min(0.25)));
                context.add_transform(Transform3D::from_scale_rotation_translation(
                    Vector3D::from_array([scale; 3]),
                    Quat::IDENTITY,
                    screen_center.extend(0.0) * (1.0 - scale),
                ));
                context.bounds(Rect2D::new(center, size), |context, _| {
                    context.fill(Color::from_hsl(130.0, 0.35, 0.25).with_alpha(opacity));

                    context.padding(2.0, |context, bounds| {
                        context.clipped(bounds, |context, bounds| {
                            let measured = context
                                .measure_text("default_bold", "Inventory", 18.0, None)
                                .unwrap_or_else(|| panic!("failed to measure next text: Inventory"));

                            context.draw_text(bounds.origin, "default_bold", "Inventory", 18.0, Color::WHITE.with_alpha(opacity), None);

                            let size = bounds.size - Size2D::new(0.0, measured.y + 4.0);
                            let origin = bounds.origin + Point2D::new(0.0, measured.y + 2.0);

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
                                            inner_origin + Point2D::new((tile_gap + tile_size.x) * x as f32, (tile_gap + tile_size.y) * y as f32),
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
                let chunk = ChunkManager::<()>::to_local(world.player.body.position.as_ivec3());

                let (hours, minutes) = {
                    let time = world.clock.time().as_secs();
                    let seconds = time % 60;
                    let minutes = (time - seconds) / 60 % 60;
                    let hours = (time - seconds - minutes * 60) / 60 / 60;

                    (hours, minutes)
                };

                let version = display.get_opengl_version_string();
                let rendered_chunks = world.voxel_renderer.rendered_chunks();
                let total_chunks = world.voxel_renderer.total_chunks();

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
Rendered chunks: {rendered_chunks} / {total_chunks}
Rendered vertices: {vertices}",
                    display.get_opengl_renderer_string(),
                    display.get_opengl_vendor_string(),
                    display.get_free_video_memory().map_or_else(|| String::from("unknown"), util::format_bytes),
                    world.player.body.position,
                    chunk.x,
                    chunk.y,
                    world.chunk_manager.get_biome(world.player.body.position.as_ivec3()),
                    1.0 / delta.as_secs_f32(),
                    delta.as_secs_f32() * 1000.0,
                    world.ticks,
                    world
                        .camera
                        .looking_at
                        .and_then(|result| world
                            .chunk_manager
                            .get_block(result.position)
                            .filter(|&b| b.name != "game:air")
                            .and_then(|state| self.resource_manager.get_block(&state.name).map(|block| format!(
                                "{} ({}){}",
                                block.id(),
                                result.hit_side,
                                if state.properties.is_empty() {
                                    String::new()
                                } else {
                                    format!(
                                        ". Properties:\n{}",
                                        state
                                            .properties
                                            .iter()
                                            .map(|(name, value)| format!("  #{name} = {value}"))
                                            .collect::<Vec<_>>()
                                            .join("\n")
                                    )
                                }
                            ))))
                        .unwrap_or_else(|| String::from("nothing")),
                    const { 200f32.to_radians() },
                    const { 35f32.to_radians() },
                    0.0,
                );

                let text_size = context
                    .measure_text("default", &text, 18.0, None)
                    .unwrap_or_else(|| panic!("failed to measure next text: {text}"));

                let overlay_width = 1.0; // self.animation_player.get_value_unchecked::<_, f32>("overlay-width");

                let text_bounds = Rect2D::new(Point2D::new(12.0, 12.0), Size2D::new((522.0 + 4.0) * overlay_width, text_size.y + 4.0));

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

            context.finish(display, &mut frame);

            let mut builder = VoxelMeshBuilder::with_capacity(world.player.inventory.get_hotbar_items().count());

            let matrix = Transform3D::from_rotation_x(Angle::from_radians(const { 200f32.to_radians() }))
                * Transform3D::from_rotation_y(Angle::from_radians(const { 35f32.to_radians() }))
                * Transform3D::from_rotation_z(Angle::from_radians(0.0));

            for (i, item) in world.player.inventory.get_hotbar_items() {
                const SIZE: f32 = SLOT_SIZE * 0.75;
                const ORIGIN: Point3D = Point3D::new(SIZE / 2.0, SIZE / 2.0, SIZE / 2.0);
                const HOTBAR_WIDTH: f32 = (INVENTORY_HOTBAR_SLOTS + 1) as f32 * SLOT_SIZE;

                let model = self
                    .resource_manager
                    .models
                    .get_unchecked(self.resource_manager.blocks.get_model_by_name(&item.id));
                let origin = Point2D::new((bounds.size.x / 2.0) - (HOTBAR_WIDTH / 2.0), bounds.size.y - SLOT_SIZE - 8.0);
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
                let origin = Point2D::new((bounds.size.x / 2.0) - (hotbar_width / 2.0), bounds.size.y - SLOT_SIZE - 8.0);

                for (column, item) in world.player.inventory.get_hotbar_items() {
                    let offset = (column + 1) as f32 * SLOT_SIZE;
                    let text = format!("x{}", item.amount);

                    let text_size = context.measure_text("default", &text, 18.0, None).unwrap();

                    context.draw_text(
                        origin.with_y(bounds.size.y - 10.0 - 18.0) + Point2D::new(offset - 3.0 - text_size.x, 0.0),
                        "default",
                        text,
                        18.0,
                        Color::WHITE,
                        None,
                    );
                }

                const CHUNK_UI_CONTAINER_SIZE: Size2D = Size2D::new(128.0, 128.0);
                const CHUNK_UI_COUNT: usize = 16;
                const CHUNK_UI_SIZE: Size2D = Size2D::new(
                    CHUNK_UI_CONTAINER_SIZE.x / CHUNK_UI_COUNT as f32,
                    CHUNK_UI_CONTAINER_SIZE.y / CHUNK_UI_COUNT as f32,
                );

                context.clipped_bounds(
                    Rect2D::new(Point2D::new(bounds.size.x - CHUNK_UI_CONTAINER_SIZE.x - 12.0, 12.0), CHUNK_UI_CONTAINER_SIZE),
                    |context, bounds| {
                        context.fill(Color::BLACK);

                        let player_chunk = ChunkManager::<()>::to_local(world.player.body.position.as_ivec3());
                        let player_offset = Point2D::new(world.player.body.position.x % 16.0, world.player.body.position.z % 16.0);
                        let origin = bounds.origin + bounds.size / 2.0;

                        for x in -1..(CHUNK_UI_COUNT + 1) as i32 {
                            let x = x - (CHUNK_UI_COUNT / 2) as i32;

                            for z in -1..(CHUNK_UI_COUNT + 1) as i32 {
                                let z = z - (CHUNK_UI_COUNT / 2) as i32;
                                let chunk = player_chunk + IPoint2D::new(x, z);

                                if let Some(stage) = world.chunk_manager.stages.get(&chunk) {
                                    let color = match stage {
                                        ChunkStage::Unloaded => continue,
                                        ChunkStage::Bare => Color::new(150, 150, 150, 255),
                                        ChunkStage::PopulationInProgress => Color::from_u32_rgb(0x73AF73),
                                        ChunkStage::Populated => Color::GREEN,
                                        ChunkStage::LightningInProgress => Color::from_u32_rgb(0xB8FF00),
                                        ChunkStage::Lighted => Color::YELLOW,
                                        ChunkStage::MeshingInProgress => Color::from_u32_rgb(0x63639C),
                                        ChunkStage::Meshed => Color::BLUE,
                                    };

                                    context.draw_rect(
                                        Rect2D::new(
                                            origin - player_offset + Vector2D::new(x as f32 * CHUNK_UI_SIZE.x, z as f32 * CHUNK_UI_SIZE.y),
                                            CHUNK_UI_SIZE,
                                        ),
                                        color,
                                    );
                                }
                            }
                        }

                        context.draw_rect(Rect2D::new(origin - Vector2D::splat(1.0), Size2D::splat(2.0)), Color::RED);
                    },
                );
            });

            context.finish(display, &mut frame);
        } else {
            let [r, g, b] = Color::from_u32_rgb(0x1D211B).to_linear();

            frame.clear_color_and_depth((r, g, b, 1.0), 1.0);

            let mut root = self.context.root(&self.common_renderer, window_context.window_size().as_vec2());

            if matches!(self.current_page, Page::Main) {
                root.center(|scope| {
                    scope.abs_pos(0.0, 24.0);
                    scope.part_of_parent_width(1.0);

                    scope.text("MERALUS", 72.0, "default", Color::from_hsl(110.0, 0.4, 0.7));
                });

                root.center(|scope| {
                    scope.fill_max_size();
                    scope.column(|scope| {
                        fn menu_button<A: ArrangeStrategy, M: MeasureStrategy>(scope: &mut UiSubcontext<'_, A, M>, name: &str) -> WidgetState {
                            scope.button(|scope| {
                                // scope.part_of_parent_width(0.75);
                                scope.set_background_color(Color::RED);

                                scope.text(name, 20.0, "default", Color::BLACK);
                            })
                        }

                        scope.set_spacing(8.0);

                        if menu_button(scope, "Play").clicked {
                            let chunk_manager = ChunkManager::new(world::ChunkFileCache {
                                root: PathBuf::from("./worlds/WRD128-0"),
                            });

                            let mut world = World::new(display, self.resource_manager.clone(), chunk_manager, WorldType::Local);

                            world.player.inventory.try_insert(Item {
                                id: "game:torch".to_owned(),
                                ty: ItemType::Block,
                                amount: 64,
                            });

                            world.player.inventory.try_insert(Item {
                                id: "game:cobblestone".to_owned(),
                                ty: ItemType::Block,
                                amount: 64,
                            });

                            world.player.inventory.try_insert(Item {
                                id: "game:bricks".to_owned(),
                                ty: ItemType::Block,
                                amount: 64,
                            });

                            world.player.inventory.try_insert(Item {
                                id: "game:green_glass_block".to_owned(),
                                ty: ItemType::Block,
                                amount: 64,
                            });

                            world.player.inventory.try_insert(Item {
                                id: "game:wood".to_owned(),
                                ty: ItemType::Block,
                                amount: 64,
                            });

                            world.player.inventory.try_insert(Item {
                                id: "game:stone_bricks".to_owned(),
                                ty: ItemType::Block,
                                amount: 64,
                            });

                            world.player.inventory.try_insert(Item {
                                id: "game:blue_rose".to_owned(),
                                ty: ItemType::Block,
                                amount: 16,
                            });

                            world.player.inventory.try_insert(Item {
                                id: "game:debug".to_owned(),
                                ty: ItemType::Block,
                                amount: 1,
                            });

                            #[cfg(feature = "addons")]
                            world.player.inventory.try_insert(Item {
                                id: self.resource_manager.get_block_id("tech_test") as usize,
                                ty: ItemType::Block,
                                amount: 64,
                            });

                            world.entities.spawn_model(Point3D::new(0.0, 128.0, 0.0), 0);
                            world.entities.spawn_model(Point3D::new(32.0, 128.0, 0.0), 1);
                            // let action_sender = self.action_sender.clone();

                            world.seed = 128;
                            // world.start_world_generation(128);

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

                            let size = window_context.window_size().as_vec2();

                            world.camera.aspect_ratio = size.x / size.y;

                            // self.event_manager.trigger(Event::WorldStart(&WorldData { world: &mut world
                            // }));
                            self.world.replace(world);
                        }

                        if menu_button(scope, "Options").clicked {
                            self.current_page = Page::Options;
                        }

                        if menu_button(scope, "Exit").clicked {
                            window_context.close_window();
                        }
                    });
                });
            }

            self.overlay.draw(&mut root);

            drop(root);

            if self.input.keyboard.is_key_pressed_once(KeyCode::KeyH) {
                println!("{:#?}", self.context);
            }

            self.context.paint_root(&mut self.common_renderer);
            self.common_renderer.render(&mut frame, display, None).unwrap();
        }

        frame.finish().expect("failed to finish draw frame");

        self.input.mouse.clear();
        self.input.keyboard.clear();
    }
}

enum Page {
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
        let mut context = RenderContext::new(display, common_renderer);

        let page = match self {
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
            let offset = Point2D::new(bounds.size.x / 2.0 - size.x / 2.0, 24.0);

            context.draw_text(
                bounds.origin + offset,
                "default",
                "Meralus",
                72.0,
                Color::from_hsl(110.0, 0.4, 0.7).with_alpha(text_opacity),
                None,
            );

            let origin = bounds.origin + offset + size;
            let size = context.measure_text("default", "hiii wrld!!", 36.0, None).unwrap();

            context.transformed(
                Transform3D::from_translation(origin.extend(0.0))
                    .scale(Vector3D::splat(text_scaling))
                    .rotate_z(-20f32.to_radians())
                    .translate(-origin.extend(0.0)),
                |context, _| {
                    context.draw_text(
                        origin - size / 2.0,
                        "default",
                        "hiii wrld!!",
                        36.0,
                        Color::from_hsl(200.0, 0.8, 0.6).with_alpha(text_opacity),
                        None,
                    );
                },
            );

            context.draw_text(
                bounds.origin + Point2D::new(8.0, bounds.size.y - 24.0),
                "default",
                format!("developer build for {OS} (arch: {ARCH}), v{}", env!("CARGO_PKG_VERSION")),
                18.0,
                Color::from_hsl(110.0, 0.6, 0.6).with_alpha(text_opacity),
                None,
            );

            let button_width = (bounds.size.x * 0.4).max(192.0);
            let mut start = bounds.origin + Point2D::new(bounds.size.x / 2.0 - button_width / 2.0, bounds.size.y / 2.0 - 68.0);

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
                        MenuButton::Play => page = Some(Page::Options),
                        MenuButton::Options => page = Some(Page::Options),
                        MenuButton::Exit => window_context.close_window(),
                    }
                }

                context.draw_rounded_rect(box_bounds, animation_player.get_value_unchecked(&animation));

                let size = context.measure_text("default", button.as_str(), 36.0, None).unwrap();

                context.draw_text(
                    start + Point2D::new((bounds.size.x * 0.4) / 2.0 - size.x / 2.0, 0.0),
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

        let progress_bar = Size2D::new(bounds.size.x * 0.8, 48.0);
        let stages_progress_bar = bounds.origin + (bounds.size / 2.0) - (progress_bar / 2.0);

        if let Some(name) = progress.info.as_ref().and_then(|info| info.current_stage_name.as_ref()) {
            context.draw_text(
                stages_progress_bar - Point2D::new(0.0, 44.0),
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
                            Rect2D::new(bounds.origin, Size2D::new(bounds.size.x * progress, bounds.size.y)),
                            Color::from_u32_rgb(0xA2D398).with_alpha(opacity),
                        );
                    });
                }
            });
        });

        context.bounds(
            Rect2D::new(stages_progress_bar + Point2D::new(0.0, progress_bar.y + 8.0), progress_bar),
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
                                            bounds.size.x
                                                * if translation < 0.0 {
                                                    animation_player.get_value_unchecked("stage-previous-progress")
                                                } else {
                                                    progress
                                                },
                                            bounds.size.y * (1.0 + translation),
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

pub struct AabbProvider<'a, C: ChunkCache> {
    pub chunk_manager: &'a ChunkManager<C>,
    pub entity_manager: &'a EntityManager,
    pub storage: &'a ResourceStorage,
}

impl<C: ChunkCache> AabbSource for AabbProvider<'_, C> {
    fn get_aabb(&self, position: Point3D) -> Option<Aabb> {
        let correct_position = position.floor();

        for (_, entity) in self.entity_manager {
            if let EntityData::Model { id, .. } = &entity.data {
                let entity_position = position.as_dvec3();

                if entity.body.aabb().contains(entity_position) {
                    for aabb in self.storage.entity_models.get_unchecked(*id).elements.iter().map(|element| element.cube) {
                        if aabb
                            .extended((entity.body.position - entity.body.size / 2.0).as_dvec3())
                            .contains(entity_position)
                        {
                            return Some(aabb);
                        }
                    }
                }
            }
        }

        if let Some(block) = self.chunk_manager.get_block(correct_position.as_ivec3())
            && self.storage.blocks.get_unchecked(self.storage.blocks.get_by_name(&block.name)).collidable()
        {
            let block_pos = position.as_dvec3();

            for aabb in self
                .storage
                .models
                .get_unchecked(self.storage.blocks.get_model_by_name(&block.name))
                .elements
                .iter()
                .map(|element| element.cube)
            {
                if aabb.contains(block_pos - correct_position.as_dvec3()) {
                    return Some(aabb);
                }
            }
        }

        None
    }

    fn get_block_aabb(&self, position: IPoint3D) -> Option<Aabb> {
        self.chunk_manager
            .get_block(position)
            .filter(|&b| b.name != "game:air" && self.storage.blocks.get_unchecked(self.storage.blocks.get_by_name(&b.name)).selectable())
            .and_then(|block| self.storage.models.get(self.storage.blocks.get_model_by_name(&block.name)))
            .map(|element| element.bounding_box)
    }
}

pub struct LimitedAabbProvider<'a, C: ChunkCache> {
    pub chunk_manager: &'a ChunkManager<C>,
    pub storage: &'a ResourceStorage,
}

impl<C: ChunkCache> AabbSource for LimitedAabbProvider<'_, C> {
    fn get_aabb(&self, position: Point3D) -> Option<Aabb> {
        let correct_position = position.floor();

        if let Some(block) = self.chunk_manager.get_block(correct_position.as_ivec3())
            && self.storage.blocks.get_unchecked(self.storage.blocks.get_by_name(&block.name)).collidable()
        {
            let block_pos = position.as_dvec3();

            for aabb in self
                .storage
                .models
                .get_unchecked(self.storage.blocks.get_model_by_name(&block.name))
                .elements
                .iter()
                .map(|element| element.cube)
            {
                if aabb.contains(block_pos - correct_position.as_dvec3()) {
                    return Some(aabb);
                }
            }
        }

        None
    }

    fn get_block_aabb(&self, position: IPoint3D) -> Option<Aabb> {
        self.chunk_manager
            .get_block(position)
            .filter(|&b| b.name != "game:air" && self.storage.blocks.get_unchecked(self.storage.blocks.get_by_name(&b.name)).selectable())
            .and_then(|block| self.storage.models.get(self.storage.blocks.get_model_by_name(&block.name)))
            .map(|element| element.bounding_box)
    }
}

#[cfg(feature = "multiplayer")]
#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() {
    tracing_subscriber::util::SubscriberInitExt::init(tracing_subscriber::layer::SubscriberExt::with(
        tracing_subscriber::registry(),
        tracing_subscriber::Layer::with_filter(
            tracing_subscriber::Layer::with_filter(tracing_subscriber::fmt::layer(), tracing_subscriber::filter::LevelFilter::INFO),
            tracing_subscriber::filter::filter_fn(|metadata| !(metadata.target() == "cranelift_jit::backend" && metadata.level() == &tracing::Level::INFO)),
        ),
    ));

    tracy_client::register_demangler!();
    tracy_client::Client::start();

    Application::<GameLoop>::new(()).start().expect("failed to run app");
}

#[cfg(not(feature = "multiplayer"))]
fn main() {
    tracing_subscriber::util::SubscriberInitExt::init(tracing_subscriber::layer::SubscriberExt::with(
        tracing_subscriber::registry(),
        tracing_subscriber::Layer::with_filter(
            tracing_subscriber::Layer::with_filter(tracing_subscriber::fmt::layer(), tracing_subscriber::filter::LevelFilter::INFO),
            tracing_subscriber::filter::filter_fn(|metadata| !(metadata.target() == "cranelift_jit::backend" && metadata.level() == &tracing::Level::INFO)),
        ),
    ));

    tracy_client::register_demangler!();
    tracy_client::Client::start();

    Application::<GameLoop>::new(()).start().expect("failed to run app");
}
