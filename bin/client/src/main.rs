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
use horns::{MagnifyFilter, MinifyFilter, RenderBackend, Texture2d};
use kira::{AudioManager, AudioManagerSettings, backend::cpal::CpalBackendSettings};
use meralus_engine::{Application, CursorGrabMode, KeyCode, KeyboardModifiers, MouseButton, State, WindowContext};
use meralus_physics::PhysicsContext;
use meralus_shared::{AsValue, Color, Point2D, Point3D, Transform3D, USize2D, Vector2D};
use meralus_storage::{Block, ResourceStorage, TextureStorage};
use meralus_tween::{Animation, Tween};
use meralus_world::{BlockSource, ChunkManager};
use tracing::info;

use crate::{
    blocks::{
        AirBlock, BlueRoseBlock, BricksBlock, CobbleStoneBlock, DebugBlock, DirtBlock, GrassBlock, GreenGlassBlock, IceBlock, OakLeavesBlock, OakLogBlock,
        RoseBlock, SandBlock, SnowBlock, StoneBlock, StoneBricksBlock, TorchBlock, WaterBlock, WoodBlock,
    },
    camera::Camera,
    input::Input,
    physics::AabbProvider,
    player::{Item, ItemType, Player},
    progress::{Progress, ProgressInfo, ProgressSender},
    render::{common::CommonRenderer, context::UiContext},
    scenes::{
        Screen,
        loading_overlay::LoadingOverlay,
        main_screen::{MainScreen, MainScreenAction},
    },
    settings::Settings,
    util::{Interval, get_movement_direction, get_rotation_directions},
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

            for (digit, i) in (KeyCode::Digit1 as u8..=KeyCode::Digit9 as u8).zip(0..9) {
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
    fn render(&mut self, window_context: WindowContext, backend: &RenderBackend, delta: Duration) {
        if self.settings.debugging.fps_stat.len() >= 100 {
            self.settings.debugging.fps_stat.pop_front();
        }

        self.settings.debugging.fps_stat.push_back(delta);
        self.settings.debugging.fps_max = self.settings.debugging.fps_max.max(delta);

        let info = self.settings.debugging.render_info.take();

        let (width, height) = window_context.window_size().into();
        let mut pass = backend.begin_pass();

        if let Some(world) = self.world.as_mut() {
            world.render(
                backend,
                &mut pass,
                &mut self.common_renderer,
                self.texture_atlas.with_filters(MinifyFilter::NearestMipmapLinear, MagnifyFilter::Nearest),
                self.lightmap_atlas.with_filters(MinifyFilter::NearestMipmapLinear, MagnifyFilter::Nearest),
                USize2D::new(width, height),
                &self.settings,
                info,
                delta,
            );
        } else {
            pass.clear_color_and_depth(Color::from_u32_rgb(0x1D211B).as_value(), 1.0);

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

            _ = self.common_renderer.render(&mut pass, backend, None, window_context.window_size());
        }

        if self.settings.debugging.draw_calls_stat.len() >= 100 {
            self.settings.debugging.draw_calls_stat.pop_front();
        }

        window_context.pre_present_notify();

        let info = pass.finish(backend);

        self.settings.debugging.draw_calls_stat.push_back(info.draw_calls);
        self.settings.debugging.draw_calls_max = self.settings.debugging.draw_calls_max.max(info.draw_calls);
        self.settings.debugging.render_info = info;
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
