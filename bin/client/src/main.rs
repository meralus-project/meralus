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
mod physics;
mod player;
mod progress;
mod render;
mod scenes;
mod settings;
mod util;
mod world;

use std::{
    f32,
    path::PathBuf,
    sync::{Arc, mpsc},
    time::Duration,
};

use cpal::traits::HostTrait;
use horns::{MagnifyFilter, MinifyFilter, RenderBackend, RenderPass, Texture2d};
use kira::{AudioManager, AudioManagerSettings, backend::cpal::CpalBackendSettings};
use meralus_engine::{Application, CursorGrabMode, KeyCode, KeyboardModifiers, MouseButton, State, WindowContext};
use meralus_physics::PhysicsContext;
use meralus_shared::{AsValue, Color, IPoint2D, Point2D, Point3D, Rect, Size2D, Transform3D, USize2D, Vector2D};
use meralus_storage::{Block, ResourceStorage, TextureStorage};
use meralus_tween::{Animation, Tween};
use meralus_world::{BlockSource, ChunkAccess, ChunkManager, ChunkStage, SUBCHUNK_COUNT};
use tracing::info;

use crate::{
    blocks::{
        AirBlock, BlueRoseBlock, BricksBlock, CobbleStoneBlock, DebugBlock, DirtBlock, GrassBlock, GreenGlassBlock, IceBlock, OakLeavesBlock, OakLogBlock,
        RoseBlock, SandBlock, SnowBlock, StoneBlock, StoneBricksBlock, TorchBlock, WaterBlock, WoodBlock,
    },
    camera::Camera,
    input::Input,
    physics::AabbProvider,
    player::{Item, ItemType, PlayerController},
    progress::{Progress, ProgressInfo, ProgressSender},
    render::{
        chunk::{VoxelFace, VoxelMeshBuilder},
        common::CommonRenderer,
        context::{RenderContext, UiContext},
    },
    scenes::{
        Screen,
        loading_overlay::LoadingOverlay,
        main_screen::{MainScreen, MainScreenAction},
    },
    settings::Settings,
    util::{Interval, get_movement_direction, get_rotation_directions, get_sky_color},
    world::{EntityData, World, WorldType},
};

pub const TICK_RATE_MS: usize = 50;
pub const TICK_RATE: Duration = Duration::from_millis(TICK_RATE_MS as u64);
pub const TPS: usize = 1000 / TICK_RATE_MS;
pub const PHYSICS_RATE: Duration = Duration::from_secs(1).checked_div(60).expect("failed to calculate fixed framerate somehow");

enum Action {
    ReplaceResourceManager(ResourceStorage),
    #[cfg(feature = "addons")]
    ReplaceAddonManager(meralus_addons::AddonManager),
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

impl GameLoop {
    fn handle_shortcuts(&mut self, context: WindowContext, backend: &RenderBackend) {
        if self.input.keyboard.is_key_pressed_once(KeyCode::F3) {
            self.settings.debugging.enabled = !self.settings.debugging.enabled;
        }

        if self.input.keyboard.is_key_pressed_once(KeyCode::KeyL) {
            self.resource_manager.debug_save();
        }

        if let Some(world) = &mut self.world {
            if self.input.keyboard.modifiers.control_key && self.input.keyboard.is_key_pressed_once(KeyCode::KeyS) {
                info!("Saving world ({} chunks)", world.chunk_manager.len());

                world.chunk_manager.save();
            }

            if self.input.keyboard.is_key_pressed_once(KeyCode::Tab) {
                world.clock.toggle();

                if world.clock.active() {
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

            for (digit, i) in (KeyCode::Digit1 as u8..KeyCode::Digit9 as u8).zip(0..9) {
                if self.input.keyboard.is_key_pressed_once(unsafe { std::mem::transmute::<u8, KeyCode>(digit) }) {
                    world.inventory_slot.value = i;
                }
            }

            if self.input.keyboard.is_key_pressed_once(KeyCode::KeyM) {
                world.marked = world.camera.looking_at.map(|looking_at| looking_at.position);
            }
        }

        if self.input.keyboard.modifiers.control_key {
            if self.input.keyboard.is_key_pressed_once(KeyCode::KeyV) {
                backend.set_vsync(!self.settings.graphics.vsync).unwrap();

                self.settings.graphics.vsync = !self.settings.graphics.vsync;
            }

            if self.input.keyboard.is_key_pressed_once(KeyCode::KeyL) {
                self.resource_manager.debug_save();
            }
        }
    }
}

impl State for GameLoop {
    type Args = ();

    const ICON: Option<&str> = Some("./resources/icon.png");
    const NAME: &str = "Meralus";

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
    }

    fn handle_mouse_motion(&mut self, delta: Option<Vector2D>, position: Option<Point2D>) {
        if let Some(delta) = delta
            && let Some(world) = self.world.as_mut()
            && world.clock.active()
        {
            world.camera.handle_mouse(
                &PhysicsContext::new(AabbProvider {
                    chunk_manager: &world.chunk_manager,
                    entity_manager: &world.entities,
                    storage: self.resource_manager.as_ref(),
                }),
                world.player.handle_mouse(delta),
            );
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
        self.handle_shortcuts(context, backend);

        if let Some(world) = &mut self.world {
            if self.input.mouse.is_pressed_once(MouseButton::Left) {
                world.destroy_looking_at();
            } else if self.input.mouse.is_pressed(MouseButton::Right) {
                world.place_held();
            }

            world.update(backend, self.settings.graphics, &self.input, delta);

            for (_, drop) in &mut world.entities {
                if let EntityData::Item { transition, .. } = &mut drop.data {
                    transition.advance(delta);
                }
            }
        }

        self.overlay.update(delta);
        self.progress.update(&self.texture_atlas, &self.lightmap_atlas, &self.resource_manager);
        self.context.update();

        if self.input.mouse.is_released(MouseButton::Left) {
            self.context.process_mouse_up();
        }

        if let Some(info) = &self.progress.info
            && self.overlay.progress.is_finished()
        {
            self.overlay.progress.set(info.completed as f32 / info.total as f32);
        }

        if let Ok(action) = self.action_receiver.try_recv() {
            match action {
                Action::ReplaceResourceManager(manager) => self.resource_manager = Arc::new(manager),
                #[cfg(feature = "addons")]
                Action::ReplaceAddonManager(addons) => self.addons = addons,
            }
        }

        self.input.mouse.clear();
        self.input.keyboard.clear();
    }

    #[allow(clippy::too_many_lines)]
    fn render(&mut self, window_context: WindowContext, backend: &RenderBackend, delta: Duration) -> RenderPass {
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

            self.common_renderer.draw_rect(
                Point2D::ZERO,
                Size2D::new(width as f32, height as f32),
                get_sky_color(world.clock.get_visual_progress(), 0.0),
            );

            self.common_renderer.render(buffer, backend, None, window_context.window_size());

            world.chunk_renderer.set_fog_color(get_sky_color(world.clock.get_visual_progress(), 0.0));

            let rendered_subchunks = world.chunk_renderer.render(
                backend,
                buffer,
                world.camera.position,
                &world.camera.frustum,
                world.camera.matrix(),
                self.texture_atlas.with_filters(MinifyFilter::NearestMipmapLinear, MagnifyFilter::Nearest),
                self.lightmap_atlas.with_filters(MinifyFilter::NearestMipmapLinear, MagnifyFilter::Nearest),
            );

            let mut builder = VoxelMeshBuilder::with_capacity(world.entities.len());

            for (_, entity) in &world.entities {
                entity.render_to(&mut builder, &world.chunk_manager, self.resource_manager.as_ref());
            }

            builder.render(
                backend,
                &world.chunk_renderer,
                buffer,
                world.camera.world_matrix(),
                self.texture_atlas.with_filters(MinifyFilter::NearestMipmapLinear, MagnifyFilter::Nearest),
                self.lightmap_atlas.with_filters(MinifyFilter::NearestMipmapLinear, MagnifyFilter::Nearest),
            );

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
                && let Some(mut model) = world.chunk_manager.get_block(result.position).filter(|b| !b.is_air()).and_then(|block| {
                    self.resource_manager
                        .models
                        .get(self.resource_manager.blocks.get_model_by_name(block.id))
                        .map(|model| model.bounding_box)
                })
            {
                // let _white_pixel = self.common_renderer.white_pixel_uv();

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

            let position = world.player.camera_position().floor().as_ivec3();

            if let Some(block) = world.chunk_manager.get_block(position)&& block.id == self.resource_manager.get_block_id("game:water") {
                    self.common_renderer
                        .draw_rect(Point2D::ZERO, Size2D::new(width as f32, height as f32), Color::from_hsl(215.0, 1.0, 0.6).with_alpha(0.5));

                    self.common_renderer.render(buffer, backend, None, window_context.window_size());
                }

            let mut context = RenderContext::new(&mut self.common_renderer, window_context.window_size());

            let bounds = context.bounds;

            context.ui(|context, bounds| {
                let hotbar_width = f32::from(INVENTORY_HOTBAR_SLOTS + 1) * SLOT_SIZE;
                let origin = Point2D::new((bounds.size.x / 2.0) - (hotbar_width / 2.0), bounds.size.y - SLOT_SIZE - 8.0);
                let offset = f32::from(world.inventory_slot.value) * SLOT_SIZE;

                context.draw_rect(Rect::new(origin, Size2D::new(hotbar_width, SLOT_SIZE)), Color::from_u32_rgb(0x1D211B));
                context.draw_rect(
                    Rect::new(origin + Point2D::new(offset, 0.0), Size2D::new(SLOT_SIZE, SLOT_SIZE)),
                    Color::from_hsl(110.0, 0.5, 0.8),
                );

                context.draw_rect(
                    Rect::new(
                        origin + Point2D::new(2.0, 2.0) + Point2D::new(offset, 0.0),
                        Size2D::new(SLOT_SIZE - 4.0, SLOT_SIZE - 4.0),
                    ),
                    Color::from_u32_rgb(0x1D211B),
                );
            });

/*             context.ui(|context, bounds| {
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
            }); */

            {
                let (hours, minutes) = {
                    let time = world.clock.time().as_secs();
                    let seconds = time % 60;
                    let minutes = (time - seconds) / 60 % 60;
                    let hours = (time - seconds - minutes * 60) / 60 / 60;

                    (hours, minutes)
                };

                let version = backend.get_opengl_version_string();
                let renderer = backend.get_opengl_renderer_string();
                let vendor = backend.get_opengl_vendor_string();
                let free_memory = backend.get_free_video_memory().map_or_else(|| String::from("unknown"), util::format_bytes);
                let block = world
                        .camera
                        .looking_at
                        .and_then(
                            |result| world.chunk_manager.get_block(result.position).filter(|b| !b.is_air()).and_then(|state| self
                                .resource_manager
                                .blocks
                                .get(state.id)
                                .map(|block| format!(
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
                                )))
                        )
                        .unwrap_or_else(|| String::from("nothing"));

                let total_subchunks = world.chunk_manager.len() * SUBCHUNK_COUNT;

                let text = format!(
                    "OpenGL {version}
OpenGL Renderer: {renderer}
OpenGL Vendor: {vendor}
Free GPU memory: {free_memory}
Window size: {width}x{height}
Game Time: {hours:02}:{minutes:02}
Looking at {block}
Rendered subchunks: {} / {total_subchunks}",
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

            context.finish(backend, &mut frame, window_context.window_size());

            let mut builder = VoxelMeshBuilder::with_capacity(world.player.inventory.get_hotbar_items().count());

            let matrix = Transform3D::from_rotation_x(const { 200f32.to_radians() })
                * Transform3D::from_rotation_y(const { 35f32.to_radians() })
                * Transform3D::from_rotation_z(0.0);

            for (i, item) in world.player.inventory.get_hotbar_items() {
                const SIZE: f32 = SLOT_SIZE * 0.6;
                const ORIGIN: Point3D = Point3D::new(SIZE / 2.0, SIZE / 2.0, SIZE / 2.0);
                const HOTBAR_WIDTH: f32 = (INVENTORY_HOTBAR_SLOTS + 1) as f32 * SLOT_SIZE;

                let model = self
                    .resource_manager
                    .models
                    .get_unchecked(self.resource_manager.blocks.get_model_by_name(item.id));

                let origin = Point2D::new(
                    (bounds.size.x / 2.0) - (HOTBAR_WIDTH / 2.0) + ((SLOT_SIZE - SIZE) / 2.0),
                    bounds.size.y - SLOT_SIZE - 8.0 + ((SLOT_SIZE - SIZE) / 2.0),
                );

                let slot_offset = (origin + Point2D::new(i as f32 * SLOT_SIZE, 0.0)).extend(20.0);

                for element in &model.elements {
                    for model_face in &element.faces {
                        builder.push_transformed(
                            &VoxelFace {
                                position: slot_offset,
                                vertices: model_face.face_data.vertices.map(|vertex| vertex * SIZE),
                                lights: [240; 4],
                                uvs: model_face.face_data.uvs,
                                color: if model_face.tint { world::GRASS_COLOR } else { Color::WHITE }
                                    .multiply_rgb(model_face.face_data.face.get_light_level()),
                            },
                            &matrix,
                            ORIGIN,
                        );
                    }
                }
            }

            builder.render_full_bright(
                backend,
                &world.chunk_renderer,
                &mut frame,
                self.common_renderer.window_matrix(),
                self.texture_atlas.with_filters(MinifyFilter::NearestMipmapLinear, MagnifyFilter::Nearest),
                self.lightmap_atlas.with_filters(MinifyFilter::NearestMipmapLinear, MagnifyFilter::Nearest),
            );

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
                        origin.with_y(bounds.size.y - 10.0 - 21.0) + Point2D::new(offset - 3.0 - text_size.x, 0.0),
                        "default",
                        text,
                        18.0,
                        Color::from_hsl(110.0, 0.5, 0.8),
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
                                    ChunkStage::LightingInProgress => Color::from_u32_rgb(0xB8FF00),
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

                context.draw_rect(Rect::new(new_container_origin, Size2D::new(text.x + 8.0, 20.0)), Color::from_u32_rgb(0x1D211B));
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

                context.draw_rect(Rect::new(new_container_origin, Size2D::new(text.x + 8.0, 20.0)), Color::from_u32_rgb(0x1D211B));
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

            context.finish(backend, &mut frame, window_context.window_size());
        } else {
            frame.clear_color_and_depth(Color::from_u32_rgb(0x1D211B).as_value(), 1.0);

            let mut root = self.context.root(&self.common_renderer, window_context.window_size().as_vec2());

            if matches!(self.current_page, Page::Main) {
                match MainScreen.render(&mut root) {
                    Some(MainScreenAction::StartGame) => {
                        self.world.replace(apply_world_template(
                            World::new(
                                backend,
                                self.resource_manager.clone(),
                                ChunkManager::new(world::ChunkFileCache {
                                    root: PathBuf::from("./worlds/WRD128-0"),
                                }),
                                WorldType::Local,
                            ),
                            &self.resource_manager,
                            window_context.window_size().as_vec2(),
                        ));
                    }
                    Some(MainScreenAction::CloseWindow) => window_context.close_window(),
                    _ => (),
                }
            }

            self.overlay.render(&mut root);

            drop(root);

            self.context.paint_root(&mut self.common_renderer);

            _ = self.common_renderer.render(&mut frame, backend, None, window_context.window_size());
        }

        if self.settings.debugging.draw_calls_stat.len() >= 100 {
            self.settings.debugging.draw_calls_stat.pop_front();
        }

        self.settings.debugging.draw_calls_stat.push_back(info.draw_calls);
        self.settings.debugging.draw_calls_max = self.settings.debugging.draw_calls_max.max(info.draw_calls);
        self.settings.debugging.render_info = info;

        frame
    }
}

#[allow(dead_code)]
enum Page {
    Options,
    Main,
}

fn apply_world_template(mut world: World, resources: &ResourceStorage, size: Vector2D) -> World {
    world.seed = 128;
    world.entities.spawn_model(Point3D::new(0.0, 128.0, 0.0), 0);
    world.entities.spawn_model(Point3D::new(32.0, 128.0, 0.0), 1);
    world.camera.aspect_ratio = size.x / size.y;

    world.player.inventory.try_insert(&Item {
        id: resources.get_block_id("game:torch"),
        ty: ItemType::Block,
        amount: 64,
    });

    world.player.inventory.try_insert(&Item {
        id: resources.get_block_id("game:cobblestone"),
        ty: ItemType::Block,
        amount: 64,
    });

    world.player.inventory.try_insert(&Item {
        id: resources.get_block_id("game:bricks"),
        ty: ItemType::Block,
        amount: 64,
    });

    world.player.inventory.try_insert(&Item {
        id: resources.get_block_id("game:green_glass_block"),
        ty: ItemType::Block,
        amount: 64,
    });

    world.player.inventory.try_insert(&Item {
        id: resources.get_block_id("game:wood"),
        ty: ItemType::Block,
        amount: 64,
    });

    world.player.inventory.try_insert(&Item {
        id: resources.get_block_id("game:stone_bricks"),
        ty: ItemType::Block,
        amount: 64,
    });

    world.player.inventory.try_insert(&Item {
        id: resources.get_block_id("game:blue_rose"),
        ty: ItemType::Block,
        amount: 16,
    });

    world.player.inventory.try_insert(&Item {
        id: resources.get_block_id("game:debug"),
        ty: ItemType::Block,
        amount: 1,
    });

    #[cfg(feature = "addons")]
    world.player.inventory.try_insert(Item {
        id: resources.get_block_id("tech_test"),
        ty: ItemType::Block,
        amount: 64,
    });

    world
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
