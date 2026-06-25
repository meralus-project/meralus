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
    collections::VecDeque,
    f32,
    path::PathBuf,
    sync::{Arc, mpsc},
    time::Duration,
};

use cpal::traits::HostTrait;
use horns::{MagnifyFilter, MinifyFilter, RenderBackend, Texture2d};
use kira::{AudioManager, AudioManagerSettings, backend::cpal::CpalBackendSettings};
use meralus_engine::{Application, CursorGrabMode, KeyCode, KeyboardModifiers, MouseButton, State, WindowContext};
use meralus_physics::{Aabb, AabbSource, PhysicsContext};
use meralus_shared::{AsValue, Color, Face, IPoint2D, IPoint3D, Lerp, Point2D, Point3D, Quat, Rect, Size2D, Transform3D, USize2D, Vector2D, Vector3D};
use meralus_storage::{Block, ResourceStorage, TextureStorage};
use meralus_tween::{Animation, Tween};
use meralus_world::{BfsLight, Chunk, ChunkAccess, ChunkCache, ChunkManager, ChunkStage, LightNode, SUBCHUNK_COUNT, SubChunkBlockState};
use tracing::info;

use crate::{
    blocks::{
        AirBlock, BlueRoseBlock, BricksBlock, CobbleStoneBlock, DebugBlock, DirtBlock, GrassBlock, GreenGlassBlock, IceBlock, OakLeavesBlock, OakLogBlock,
        RoseBlock, SandBlock, SnowBlock, StoneBlock, StoneBricksBlock, TorchBlock, WaterBlock, WoodBlock,
    },
    camera::Camera,
    input::Input,
    player::{Item, ItemType, PlayerController},
    progress::{Progress, ProgressInfo, ProgressSender},
    render::{
        chunk::{VoxelFace, VoxelMeshBuilder},
        common::CommonRenderer,
        context::{ArrangeStrategy, Arrangement, MeasureStrategy, RenderContext, RenderInfo, UiContext, UiSubcontext, WidgetState},
    },
    scenes::{Screen, loading_overlay::LoadingOverlay},
    util::{cube_outline, get_movement_direction, get_rotation_directions, vertex_ao},
    world::{EntityData, EntityManager, World, WorldType},
};

pub const TICK_RATE_MS: usize = 50;
pub const TICK_RATE: Duration = Duration::from_millis(TICK_RATE_MS as u64);
pub const TPS: usize = 1000 / TICK_RATE_MS;
pub const FIXED_FRAMERATE: Duration = Duration::from_secs(1).checked_div(60).expect("failed to calculate fixed framerate somehow");

const _TEXT_COLOR: Color = Color::from_hsl(120.0, 0.5, 0.4);
const _BG_COLOR: Color = Color::from_hsl(120.0, 0.4, 0.75);

pub(crate) fn get_sky_color((after_day, progress): (bool, f32), weather: f32) -> Color {
    let day_color: Color = Color::from_hsl(220.0, 0.2f32.mul_add(weather, 0.5), 0.6f32.mul_add(-weather, 0.75));
    let night_color: Color = Color::from_hsl(220.0, 0.1f32.mul_add(weather, 0.35), 0.15f32.mul_add(-weather, 0.25));

    if after_day {
        day_color.lerp(&night_color, progress)
    } else {
        night_color.lerp(&day_color, progress)
    }
}

const GRASS_COLOR: Color = Color::from_hsl(120.0, 0.4, 0.75);

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone)]
struct Debugging {
    enabled: bool,
    draw_calls_stat: VecDeque<usize>,
    draw_calls_max: usize,
    fps_stat: VecDeque<Duration>,
    fps_max: Duration,
    render_info: RenderInfo,
}

impl Default for Debugging {
    fn default() -> Self {
        Self {
            enabled: false,
            draw_calls_stat: VecDeque::new(),
            draw_calls_max: 0,
            fps_stat: VecDeque::new(),
            fps_max: Duration::ZERO,
            render_info: RenderInfo::default(),
        }
    }
}

enum Action {
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
#[allow(dead_code)]
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

    #[inline]
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

    #[inline]
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

    #[inline]
    fn blocky<T: ChunkAccess>(chunks: &T, _: &ResourceStorage, _: IPoint3D, light_source: IPoint3D, _: [[IPoint3D; 3]; 4], _: bool) -> ([f32; 4], [u8; 4]) {
        let light = chunks.get_light_level(light_source);

        ([0.0; 4], [light; 4])
    }

    #[inline]
    #[allow(clippy::type_complexity)]
    pub const fn get_light_fn<T: ChunkAccess>(self) -> fn(&T, &ResourceStorage, IPoint3D, IPoint3D, [[IPoint3D; 3]; 4], bool) -> ([f32; 4], [u8; 4]) {
        match self {
            Self::Smooth => Self::smooth_light::<T>,
            Self::BlockyWithAO => Self::blocky_with_ao::<T>,
            Self::Blocky => Self::blocky::<T>,
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
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

#[derive(Debug, Clone, Default)]
struct Settings {
    graphics: GraphicsSettings,
    debugging: Debugging,
}

struct GameLoop {
    #[allow(dead_code)]
    audio_manager: AudioManager,
    input: Input,
    common_renderer: CommonRenderer,
    resource_manager: Arc<ResourceStorage>,

    // particles: ParticleSystem,
    action_receiver: mpsc::Receiver<Action>,

    #[cfg(feature = "addons")]
    addons: meralus_addons::AddonManager,

    debug_interval: Interval,
    fixed_interval: Interval,

    // scene: WorldScene,
    // kawase: DualKawase<4>,
    texture_atlas: Texture2d,
    lightmap_atlas: Texture2d,

    context: UiContext,
    overlay: LoadingOverlay,
    current_page: Page,
    progress: Progress,

    world: Option<World>,
    settings: Settings,
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

    fn new(window: WindowContext, backend: &RenderBackend, (): Self::Args) -> Self {
        let (tx, rx) = mpsc::channel();
        let (action_sender, action_receiver) = mpsc::channel();

        let resource_manager = Arc::new(ResourceStorage::new("./resources"));

        std::thread::spawn(move || {
            let mut resources = ResourceStorage::new("./resources");

            let sender = ProgressSender(tx);

            #[cfg(not(feature = "addons"))]
            let total_stages = 2;
            #[cfg(feature = "addons")]
            let total_stages = 3;

            sender.set_visible(true)?;
            sender.set_initial_info(ProgressInfo::new(total_stages, 0, 1, 0))?;

            sender.new_stage("Blocks loading", 20)?;

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
            register_block(&mut resources, &sender, OakLogBlock)?;
            register_block(&mut resources, &sender, IceBlock)?;
            register_block(&mut resources, &sender, GreenGlassBlock)?;
            register_block(&mut resources, &sender, TorchBlock)?;
            register_block(&mut resources, &sender, SnowBlock)?;
            register_block(&mut resources, &sender, RoseBlock)?;
            register_block(&mut resources, &sender, BlueRoseBlock)?;
            register_block(&mut resources, &sender, CobbleStoneBlock)?;
            register_block(&mut resources, &sender, BricksBlock)?;
            register_block(&mut resources, &sender, StoneBricksBlock)?;
            register_block(&mut resources, &sender, DebugBlock)?;
            #[cfg(feature = "addons")]
            {
                sender.new_stage("Loading addons", 1);

                let mut addons = meralus_addons::AddonManager::new("./addons").unwrap();

                addons.insert_mappings(&mut resources);
                addons.execute(&mut resources);

                _ = action_sender.send(Action::ReplaceAddonManager(addons));
            }

            sender.new_stage("Mip-maps generation", 4)?;

            for level in 1..=4 {
                resources.generate_mipmap(level);

                sender.complete_task()?;
            }

            _ = action_sender.send(Action::ReplaceResourceManager(resources));

            sender.set_visible(false)
        });

        let size = window.window_size().as_vec2();

        let mut common_renderer = CommonRenderer::new(backend).unwrap_or_else(|e| panic!("failed to create CommonRenderer: {e}"));

        common_renderer.add_font("default", include_bytes!("../../../resources/fonts/Monocraft.ttf"));
        common_renderer.add_font("default_bold", include_bytes!("../../../resources/fonts/Monocraft-Bold.ttf"));
        common_renderer.set_window_matrix(Transform3D::orthographic_rh_gl(0.0, size.x, size.y, 0.0, -100.0, 100.0));

        // let mut animation_player = AnimationPlayer::default();

        // init_animation_player(&mut animation_player);

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
            // animation_player,
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
            texture_atlas: backend
                .create_empty_texture2d_with_mipmaps(TextureStorage::ATLAS_SIZE.into(), TextureStorage::ATLAS_SIZE.into(), 4)
                .unwrap_or_else(|e| panic!("failed to create empty texture atlas on GPU: {e}")),
            lightmap_atlas: backend
                .create_empty_texture2d_with_mipmaps(TextureStorage::ATLAS_SIZE.into(), TextureStorage::ATLAS_SIZE.into(), 4)
                .unwrap_or_else(|e| panic!("failed to create empty texture atlas on GPU: {e}")),
            // scene: WorldScene::new(backend, width, height).unwrap(),
            // kawase: DualKawase::new(backend, width, height).unwrap(),
            context: UiContext::new(),
            // particles: ParticleSystem::new(backend),
            overlay: LoadingOverlay {
                progress: Tween::new(0.0, 1.0, 200),
            },
        }
    }

    fn handle_window_resize(&mut self, _facade: &RenderBackend, size: USize2D, _scale_factor: f64) {
        // self.scene.resize(facade, size.to_array()).unwrap();
        // self.kawase.resize(facade, size.to_array()).unwrap();

        let size = size.as_vec2();

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

                        if let Some(chunk) = world.chunk_manager.get_chunk_mut(chunk) {
                            chunk.dirty = true;
                        }

                        for normal in Face::NORMALS {
                            let chunk_position = ChunkManager::<()>::to_local(position + normal);

                            if chunk_position != chunk
                                && let Some(chunk) = world.chunk_manager.get_chunk_mut(chunk_position)
                            {
                                chunk.dirty = true;
                            }
                        }

                        for normal in [IPoint3D::NEG_ONE, IPoint3D::NEG_ONE.with_x(1), IPoint3D::ONE.with_x(-1), IPoint3D::ONE] {
                            let chunk_position = ChunkManager::<()>::to_local(position + normal);

                            if chunk_position != chunk
                                && let Some(chunk) = world.chunk_manager.get_chunk_mut(chunk_position)
                            {
                                chunk.dirty = true;
                            }
                        }

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
    fn update(&mut self, context: WindowContext, backend: &RenderBackend, delta: Duration) {
        self.overlay.update(delta);

        if let Some(info) = &self.progress.info
            && self.overlay.progress.is_finished()
        {
            self.overlay.progress.set(info.completed as f32 / info.total as f32);
        }

        self.progress.update(&self.texture_atlas, &self.lightmap_atlas, &self.resource_manager);

        if let Some(world) = self.world.as_mut() {
            if world.player_controllable {
                for _ in 0..self.fixed_interval.update(delta) {
                    world.physics_step(&self.input);

                    if let Some(entity) = world.entities.get_mut(0) {
                        entity.set_rotation(0, world.player.get_vector_for_rotation().as_vec3());
                    }
                }
            }

            for _ in 0..world.tick_interval.update(delta) {
                world.tick(true);
            }

            world.update(backend, self.settings.graphics);
        }

        for _ in 0..self.debug_interval.update(delta) {
            if let Some(world) = self.world.as_mut() {
                world.ticks = world.tick_sum;
                world.tick_sum = 0;
            }
        }

        if self.input.keyboard.is_key_pressed_once(KeyCode::KeyL) {
            self.resource_manager.debug_save();
        }

        if self.input.keyboard.is_key_pressed_once(KeyCode::F3) {
            self.settings.debugging.enabled = !self.settings.debugging.enabled;
        }

        if self.input.keyboard.is_key_pressed_once(KeyCode::Tab) {
            if let Some(world) = self.world.as_mut() {
                world.player_controllable = !world.player_controllable;
            }

            if self.world.as_ref().is_some_and(|world| world.player_controllable) {
                #[cfg(not(target_os = "macos"))]
                context.set_cursor_grab(CursorGrabMode::Confined);
                #[cfg(target_os = "macos")]
                context.set_cursor_grab(CursorGrabMode::Locked);
                context.set_cursor_visible(false);
            } else {
                context.set_cursor_grab(CursorGrabMode::None);
                context.set_cursor_visible(true);
            }
        }

        if let Some(world) = &mut self.world {
            for (_, drop) in &mut world.entities {
                if let EntityData::Item { transition, .. } = &mut drop.data {
                    transition.advance(delta);
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
    fn render(&mut self, window_context: WindowContext, backend: &RenderBackend, delta: Duration) {
        if self.settings.debugging.fps_stat.len() >= 100 {
            self.settings.debugging.fps_stat.pop_front();
        }

        self.settings.debugging.fps_stat.push_back(delta);
        self.settings.debugging.fps_max = self.settings.debugging.fps_max.max(delta);

        let info = self.settings.debugging.render_info.take();

        let (width, height) = window_context.window_size().into();
        let mut frame = backend.begin_pass();

        if let Some(world) = self.world.as_mut() {
            let buffer = &mut frame; // self.scene.buffer(backend);

            let progress = world.clock.get_progress();

            world
                .chunk_renderer
                .set_sun_position(if progress > 0.5 { 1.0 - progress } else { progress } * 2.0);

            buffer.clear_color_and_depth(Color::BLACK.to_linear_rgba(), 1.0);

            self.common_renderer
                .draw_rect(
                    Point2D::ZERO,
                    Size2D::new(width as f32, height as f32),
                    get_sky_color(world.clock.get_visual_progress(), 0.0),
                )
                .unwrap();

            self.settings
                .debugging
                .render_info
                .extend(&self.common_renderer.render(buffer, backend, None, window_context.window_size()));

            let rendered_subchunks = world.chunk_renderer.render(
                buffer,
                world.camera.position,
                &world.camera.frustum,
                world.camera.matrix(),
                self.texture_atlas.with_filters(MinifyFilter::NearestMipmapLinear, MagnifyFilter::Nearest),
                self.lightmap_atlas.with_filters(MinifyFilter::NearestMipmapLinear, MagnifyFilter::Nearest),
            );

            self.settings.debugging.render_info.extend(&rendered_subchunks);

            let mut builder = VoxelMeshBuilder::with_capacity(world.entities.len());

            for (_, entity) in &world.entities {
                entity.render_to(&mut builder, &world.chunk_manager, self.resource_manager.as_ref());
            }

            self.settings.debugging.render_info.extend(&builder.render(
                backend,
                &world.chunk_renderer,
                buffer,
                world.camera.matrix(),
                self.texture_atlas.with_filters(MinifyFilter::NearestMipmapLinear, MagnifyFilter::Nearest),
                self.lightmap_atlas.with_filters(MinifyFilter::NearestMipmapLinear, MagnifyFilter::Nearest),
            ));

            // self.kawase.apply(backend, &self.scene).unwrap();

            // self.scene.render(&mut frame).unwrap();
            // self.particles.render(&mut frame, world.camera.matrix()).unwrap();
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
                let _white_pixel = self.common_renderer.white_pixel_uv();

                model.min += result.position.as_dvec3();
                model.max += result.position.as_dvec3();

                self.common_renderer.set_matrix(world.camera.matrix());
                // self.debugging.render_info.extend(
                // &self
                //     .common_renderer
                //     .render_lines(&mut frame, backend, &aabb_outline(model, white_pixel),
                // None)     .unwrap();
                // );

                self.common_renderer.set_default_matrix();
            }

            let mut context = RenderContext::new(&mut self.common_renderer, window_context.window_size());
            let bounds = context.bounds;

            // if self.debugging.wireframe {
            //     context.ui(|context, bounds| {
            //         let height = 200.0;
            //         let y_offset = bounds.size.y - height;

            //         context.draw_rect(
            //             Rect::new(Point2D::new(0.0, y_offset), Size2D::new(480.0,
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

                context.draw_rect(Rect::new(origin, Size2D::new(hotbar_width, SLOT_SIZE)), Color::from_hsl(0.0, 0.0, 0.5));
                context.draw_rect(
                    Rect::new(origin + Point2D::new(offset, 0.0), Size2D::new(SLOT_SIZE, SLOT_SIZE)),
                    Color::from_hsl(0.0, 0.0, 0.8),
                );

                context.draw_rect(
                    Rect::new(
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
                context.bounds(Rect::new(center, size), |context, _| {
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

                            context.draw_rect(Rect::new(origin, size), Color::from_hsl(130.0, 0.5, 0.75).with_alpha(opacity));

                            for x in 0..tile_count {
                                for y in 0..tile_count {
                                    context.draw_rect(
                                        Rect::new(
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
                let (hours, minutes) = {
                    let time = world.clock.time().as_secs();
                    let seconds = time % 60;
                    let minutes = (time - seconds) / 60 % 60;
                    let hours = (time - seconds - minutes * 60) / 60 / 60;

                    (hours, minutes)
                };

                let version = backend.get_opengl_version_string();
                let total_subchunks = world.chunk_manager.len() * SUBCHUNK_COUNT;

                let text = format!(
                    "OpenGL {version}
OpenGL Renderer: {}
OpenGL Vendor: {}
Free GPU memory: {}
Window size: {width}x{height}
Game Time: {hours:02}:{minutes:02}
Looking at {}
Rendered subchunks: {} / {total_subchunks}",
                    backend.get_opengl_renderer_string(),
                    backend.get_opengl_vendor_string(),
                    backend.get_free_video_memory().map_or_else(|| String::from("unknown"), util::format_bytes),
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
                    rendered_subchunks.draw_calls
                );

                let text_size = context
                    .measure_text("default", &text, 18.0, None)
                    .unwrap_or_else(|| panic!("failed to measure next text: {text}"));

                let overlay_width = 1.0; // self.animation_player.get_value_unchecked::<_, f32>("overlay-width");

                let text_bounds = Rect::new(Point2D::new(12.0, 12.0), Size2D::new((522.0 + 4.0) * overlay_width, text_size.y + 4.0));

                context.bounds(text_bounds, |context, _| {
                    context.fill(Color::BLACK.with_alpha(0.25));

                    context.padding(2.0, |context, bounds| {
                        context.clipped(bounds, |context, bounds| {
                            context.draw_text(bounds.origin, "default", text, 18.0, Color::WHITE, None);
                        });
                    });
                });
            }

            self.settings
                .debugging
                .render_info
                .extend(&context.finish(backend, &mut frame, window_context.window_size()));

            let mut builder = VoxelMeshBuilder::with_capacity(world.player.inventory.get_hotbar_items().count());

            let matrix = Transform3D::from_rotation_x(const { 200f32.to_radians() })
                * Transform3D::from_rotation_y(const { 35f32.to_radians() })
                * Transform3D::from_rotation_z(0.0);

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

            self.settings.debugging.render_info.extend(&builder.render_full_bright(
                backend,
                &world.chunk_renderer,
                &mut frame,
                self.common_renderer.window_matrix(),
                self.texture_atlas.with_filters(MinifyFilter::NearestMipmapLinear, MagnifyFilter::Nearest),
                self.lightmap_atlas.with_filters(MinifyFilter::NearestMipmapLinear, MagnifyFilter::Nearest),
            ));

            let mut context = RenderContext::new(&mut self.common_renderer, window_context.window_size());

            context.ui(|context, bounds| {
                const CHUNK_UI_CONTAINER_SIZE: Size2D = Size2D::new(128.0, 128.0);
                const CHUNK_UI_COUNT: usize = 16;
                const CHUNK_UI_SIZE: Size2D = Size2D::new(
                    CHUNK_UI_CONTAINER_SIZE.x / CHUNK_UI_COUNT as f32,
                    CHUNK_UI_CONTAINER_SIZE.y / CHUNK_UI_COUNT as f32,
                );

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

                let container_origin = Point2D::new(bounds.size.x - CHUNK_UI_CONTAINER_SIZE.x - 12.0, 12.0);

                context.clipped_bounds(Rect::new(container_origin, CHUNK_UI_CONTAINER_SIZE), |context, bounds| {
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
                                    Rect::new(
                                        origin - player_offset + Vector2D::new(x as f32 * CHUNK_UI_SIZE.x, z as f32 * CHUNK_UI_SIZE.y),
                                        CHUNK_UI_SIZE,
                                    ),
                                    color,
                                );
                            }
                        }
                    }

                    context.draw_rect(Rect::new(origin - Vector2D::splat(1.0), Size2D::splat(2.0)), Color::RED);
                });

                let text = context
                    .measure_text(
                        "default",
                        format!(
                            "{} {} {}",
                            world.player.body.position.x as i32, world.player.body.position.y as i32, world.player.body.position.z as i32
                        ),
                        9.0,
                        None,
                    )
                    .unwrap_or_default();
                let new_container_origin =
                    container_origin + CHUNK_UI_CONTAINER_SIZE.with_x((CHUNK_UI_CONTAINER_SIZE.x - text.x) / 2.0) + Point2D::new(0.0, 2.0);

                context.draw_rect(Rect::new(new_container_origin, Size2D::new(text.x + 4.0, 20.0)), Color::from_u32_rgb(0x1D211B));
                context.draw_text(
                    new_container_origin + Point2D::splat(4.0),
                    "default",
                    format!(
                        "{} {} {}",
                        world.player.body.position.x as i32, world.player.body.position.y as i32, world.player.body.position.z as i32
                    ),
                    9.0,
                    Color::from_hsl(110.0, 0.5, 0.8),
                    None,
                );

                let text = context
                    .measure_text(
                        "default",
                        format!("{:?}", world.chunk_manager.get_biome(world.player.body.position.as_ivec3())),
                        9.0,
                        None,
                    )
                    .unwrap_or_default();
                let new_container_origin =
                    container_origin + CHUNK_UI_CONTAINER_SIZE.with_x((CHUNK_UI_CONTAINER_SIZE.x - text.x) / 2.0) + Point2D::new(0.0, 24.0);

                context.draw_rect(Rect::new(new_container_origin, Size2D::new(text.x + 4.0, 20.0)), Color::from_u32_rgb(0x1D211B));
                context.draw_text(
                    new_container_origin + Point2D::splat(4.0),
                    "default",
                    format!("{:?}", world.chunk_manager.get_biome(world.player.body.position.as_ivec3())),
                    9.0,
                    Color::from_hsl(110.0, 0.5, 0.8),
                    None,
                );
            });

            if self.settings.debugging.enabled {
                context.ui(|context, bounds| {
                    const SPACING: f32 = 1.0;
                    const SIZE: Size2D = Size2D::new(100.0 * (2.0 + SPACING), 96.0);
                    const CONTAINER_SIZE: Size2D = Size2D::new(SIZE.x - SPACING, SIZE.y);
                    const ELEMENT_WIDTH: f32 = (SIZE.x - 100.0 * SPACING) / 100.0;

                    context.draw_rect(
                        Rect::new(
                            bounds.origin + bounds.size.with_x(4.0) - CONTAINER_SIZE.with_x(0.0) - Point2D::new(0.0, 4.0),
                            CONTAINER_SIZE,
                        ),
                        Color::from_u32_rgb(0x1D211B),
                    );

                    let mut x = 0.0;

                    for stat in &self.settings.debugging.fps_stat {
                        let size = Size2D::new(
                            ELEMENT_WIDTH,
                            CONTAINER_SIZE.y * (stat.as_secs_f32() / self.settings.debugging.fps_max.as_secs_f32()),
                        );

                        context.draw_rect(
                            Rect::new(bounds.origin + bounds.size.with_x(4.0 + x) - size.with_x(0.0) - Point2D::new(0.0, 4.0), size),
                            Color::from_hsl(110.0, 0.4, 0.7),
                        );

                        x += ELEMENT_WIDTH + SPACING;
                    }

                    let text = context
                        .measure_text(
                            "default",
                            format!("fps: {:.0} ({:.2}ms)", 1.0 / delta.as_secs_f32(), delta.as_secs_f32() * 1000.0),
                            9.0,
                            None,
                        )
                        .unwrap_or_default();

                    context.draw_rect(
                        Rect::new(
                            bounds.origin + bounds.size.with_x(8.0) - CONTAINER_SIZE.with_x(0.0) - Point2D::new(0.0, 4.0),
                            text,
                        ),
                        Color::from_u32_rgb(0x1D211B),
                    );

                    context.draw_text(
                        bounds.origin + bounds.size.with_x(8.0) - CONTAINER_SIZE.with_x(0.0) - Point2D::new(0.0, 4.0),
                        "default",
                        format!("fps: {:.0} ({:.2}ms)", 1.0 / delta.as_secs_f32(), delta.as_secs_f32() * 1000.0),
                        9.0,
                        Color::from_hsl(110.0, 0.5, 0.8),
                        None,
                    );
                });

                context.ui(|context, bounds| {
                    const SPACING: f32 = 1.0;
                    const SIZE: Size2D = Size2D::new(100.0 * (2.0 + SPACING), 96.0);
                    const CONTAINER_SIZE: Size2D = Size2D::new(SIZE.x - SPACING, SIZE.y);
                    const ELEMENT_WIDTH: f32 = (SIZE.x - 100.0 * SPACING) / 100.0;

                    let container_origin = bounds.origin + bounds.size - CONTAINER_SIZE - Point2D::splat(4.0);

                    context.draw_rect(Rect::new(container_origin, CONTAINER_SIZE), Color::from_u32_rgb(0x1D211B));

                    let mut x = 0.0;

                    for &stat in &self.settings.debugging.draw_calls_stat {
                        let size = Size2D::new(ELEMENT_WIDTH, CONTAINER_SIZE.y * (stat as f32 / self.settings.debugging.draw_calls_max as f32));

                        context.draw_rect(
                            Rect::new(container_origin + CONTAINER_SIZE.with_x(0.0) - size.with_x(-x), size),
                            Color::from_hsl(110.0, 0.4, 0.7),
                        );

                        x += ELEMENT_WIDTH + SPACING;
                    }

                    let text = context
                        .measure_text("default", format!("draw calls: {}\nvertices: {}", info.draw_calls, info.vertices), 9.0, None)
                        .unwrap_or_default();

                    context.draw_rect(Rect::new(container_origin + Point2D::splat(4.0), text), Color::from_u32_rgb(0x1D211B));
                    context.draw_text(
                        container_origin + Point2D::splat(4.0),
                        "default",
                        format!("draw calls: {}\nvertices: {}", info.draw_calls, info.vertices),
                        9.0,
                        Color::from_hsl(110.0, 0.5, 0.8),
                        None,
                    );
                });
            }

            self.settings
                .debugging
                .render_info
                .extend(&context.finish(backend, &mut frame, window_context.window_size()));

            if self.settings.debugging.draw_calls_stat.len() >= 100 {
                self.settings.debugging.draw_calls_stat.pop_front();
            }

            self.settings
                .debugging
                .draw_calls_stat
                .push_back(self.settings.debugging.render_info.draw_calls);

            self.settings.debugging.draw_calls_max = self.settings.debugging.draw_calls_max.max(self.settings.debugging.render_info.draw_calls);
        } else {
            frame.clear_color_and_depth(Color::from_u32_rgb(0x1D211B).as_value(), 1.0);

            let mut root = self.context.root(&self.common_renderer, window_context.window_size().as_vec2());

            if matches!(self.current_page, Page::Main) {
                root.center(|scope| {
                    scope.abs_pos(0.0, 24.0);
                    scope.part_of_parent_width(1.0);

                    scope.column(|scope| {
                        scope.set_h_arrangement(Arrangement::End);

                        scope.text("MERALUS", 72.0, "default", Color::from_hsl(110.0, 0.4, 0.7));
                        scope.text("deltarune today!", 18.0, "default", Color::from_hsl(110.0, 0.3, 0.6));
                    });
                });

                root.center(|scope| {
                    scope.fill_max_size();
                    scope.column(|scope| {
                        fn menu_button<A: ArrangeStrategy, M: MeasureStrategy>(scope: &mut UiSubcontext<'_, A, M>, name: &str) -> WidgetState {
                            scope.button(|scope| {
                                // scope.part_of_parent_width(0.75);
                                scope.set_background_color(Color::from_hsl(110.0, 0.4, 0.7));

                                scope.column(|scope| {
                                    scope.row(|scope| {
                                        scope.add_space(const { Point2D::new(12.0, 0.0) });
                                        scope.text(name, 18.0, "default", Color::from_hsl(110.0, 0.25, 0.1));
                                        scope.add_space(const { Point2D::new(12.0, 0.0) });
                                    });

                                    scope.add_space(const { Point2D::new(0.0, 6.0) });
                                });
                            })
                        }

                        scope.set_h_arrangement(Arrangement::Center);
                        scope.set_spacing(8.0);

                        if menu_button(scope, "Play").clicked {
                            let chunk_manager = ChunkManager::new(world::ChunkFileCache {
                                root: PathBuf::from("./worlds/WRD128-0"),
                            });

                            let mut world = World::new(backend, self.resource_manager.clone(), chunk_manager, WorldType::Local);

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
                            world.seed = 128;

                            let size = window_context.window_size().as_vec2();

                            world.camera.aspect_ratio = size.x / size.y;

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
            _ = self.common_renderer.render(&mut frame, backend, None, window_context.window_size());
        }

        self.input.mouse.clear();
        self.input.keyboard.clear();

        frame.finish(backend);
    }
}

enum Page {
    Options,
    Main,
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

    Application::<GameLoop>::new(()).start().expect("failed to run app");
}
