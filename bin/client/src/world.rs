use std::{
    cell::RefCell,
    collections::hash_map::{Iter, IterMut},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use ahash::{HashMap, HashSet};
use mavelin_engine::{MouseButton, WindowContext};
#[cfg(feature = "multiplayer")]
use mavelin_network::{IncomingPacket, OutgoingPacket, Uuid};
use mavelin_physics::{Aabb, PhysicsBody, PhysicsContext};
use mavelin_shared::{
    Color, Face, IPoint2D, IPoint3D, Point2D, Point3D, Ranged, Rect, Size2D, Size3D, Transform3D, USize2D, USizePoint2D, USizePoint3D, Vector2D, Vector3D,
};
use mavelin_tween::{Animation, RepeatMode, Tween};
use mavelin_world::{
    BfsLight, Biome, BlockSource, CHUNK_HEIGHT, Chunk, ChunkAccess, ChunkCache, ChunkManager, ChunkStage, LightNode, LocalChunkManager, SUBCHUNK_COUNT,
    SUBCHUNK_SIZE, SubChunkBlockState,
};
use mavelin_worldgen::ChunkGenerator;
use tracing::info;

use crate::{
    Camera, Interval, Item, PHYSICS_RATE, Player, ResourceStorage, TICK_RATE,
    clock::Clock,
    input::Input,
    physics::{AabbProvider, LimitedAabbProvider},
    player::ItemType,
    render::{
        RenderInfo,
        chunk::{ChunkRenderer, TranslucentSubchunk, VoxelFace, VoxelMeshBuilder},
        common::CommonRenderer,
    },
    settings::{Debugging, GraphicsSettings, Settings},
};

pub const GRASS_COLOR: Color = Color::from_hsl(120.0, 0.525, 0.525);

#[non_exhaustive]
pub enum EntityData {
    Item { item: Item, transition: Tween<f32> },
    Model { id: usize, rotations: Vec<(Vector3D, Vector3D)> },
}

pub struct Entity {
    pub body: PhysicsBody,
    pub data: EntityData,
}

impl Entity {
    pub fn item(position: Point3D, item: Item) -> Self {
        Self {
            body: PhysicsBody::new(position, Size3D::new(0.3, 0.3, 0.3)),
            data: EntityData::Item {
                item,
                transition: Tween::new(0.0, 1.0, 2000).with_repeat_mode(RepeatMode::Infinite),
            },
        }
    }

    pub fn model(position: Point3D, id: usize, resource_storage: &ResourceStorage) -> Self {
        Self {
            body: PhysicsBody::new(position, resource_storage.entity_models.get_aabb(id as u8).unwrap().size().as_vec3()),
            data: EntityData::Model { id, rotations: Vec::new() },
        }
    }

    pub fn set_rotation(&mut self, i: usize, vector: Vector3D) {
        if let EntityData::Model { rotations, .. } = &mut self.data {
            if rotations.len() <= i {
                rotations.resize(i + 1, (Vector3D::ZERO, Vector3D::ZERO));
            }

            rotations[i].0 = vector;
        }
    }

    pub fn set_translation(&mut self, i: usize, vector: Vector3D) {
        if let EntityData::Model { rotations, .. } = &mut self.data {
            if rotations.len() <= i {
                rotations.resize(i + 1, (Vector3D::ZERO, Vector3D::ZERO));
            }

            rotations[i].1 = vector;
        }
    }

    pub fn render_to<C: ChunkCache>(&self, builder: &mut VoxelMeshBuilder, chunk_manager: &ChunkManager<C>, resource_storage: &ResourceStorage) {
        match &self.data {
            EntityData::Item { transition, item } => {
                let animation_value = transition.get_copy();
                let model = resource_storage.models.get_unchecked(resource_storage.blocks.get_model_by_name(item.id));
                let mut current_block = self.body.position.floor().as_ivec3();
                let light = chunk_manager.get_light_level(current_block);
                let matrix = Transform3D::from_rotation_y(animation_value * const { 360f32.to_radians() });
                let animation_value = if animation_value > 0.5 { 1.0 - animation_value } else { animation_value };
                let position_offset = Point3D::new(0.0, const { Point3D::new(0.3, 0.3, 0.3).y / 2.0 }, 0.0);
                let block_below = loop {
                    if chunk_manager.get_block(current_block).is_some_and(|b| !b.is_air()) {
                        break Some(current_block);
                    } else if current_block.y <= 0 {
                        break None;
                    }

                    current_block.y -= 1;
                };

                for element in &model.elements {
                    for model_face in &element.faces {
                        if model_face.face_data.face == Face::Top
                            && let Some(block_below) = block_below
                        {
                            let size = Point3D::new(0.3 * 1.5, 0.3 * 1.0, 0.3 * 1.5);
                            let origin = size / 2.0;

                            builder.push_transformed(
                                &VoxelFace {
                                    position: block_below.as_vec3() + (Point3D::new(0.5, 0.0, 0.5) - size.with_y(0.0) / 2.0) + (Point3D::Y * 1.05),
                                    vertices: model_face
                                        .face_data
                                        .vertices
                                        .map(|vertex| Point3D::new(vertex.x * size.x, 0.0, vertex.z * size.z)),
                                    lights: [light; 4],
                                    uvs: model_face.face_data.uvs,
                                    color: Color::BLACK,
                                },
                                &matrix,
                                origin,
                            );
                        }

                        let origin = Point3D::new(0.3, 0.3, 0.3) / 2.0;

                        builder.push_transformed(
                            &VoxelFace {
                                position: self.body.position - (position_offset * animation_value),
                                vertices: model_face
                                    .face_data
                                    .vertices
                                    .map(|vertex| Point3D::new(vertex.x * 0.3, vertex.y * 0.3, vertex.z * 0.3)),
                                lights: [light; 4],
                                uvs: model_face.face_data.uvs,
                                color: if model_face.tint { GRASS_COLOR } else { Color::WHITE },
                            },
                            &matrix,
                            origin,
                        );
                    }
                }
            }
            EntityData::Model { id, rotations } => {
                let model = resource_storage.entity_models.get_unchecked(*id);
                let half_size = model.bounding_box.size().as_vec3() / 2.0;

                for (i, element) in model.elements.iter().enumerate() {
                    for face_data in element.faces.as_ref() {
                        let vertices = rotations.get(i).map_or(face_data.vertices, |rotation| {
                            let mut matrix = Transform3D::IDENTITY;

                            matrix *= Transform3D::from_rotation_x(rotation.0.x);
                            matrix *= Transform3D::from_rotation_y(rotation.0.y);
                            matrix *= Transform3D::from_rotation_z(rotation.0.z);

                            face_data.vertices.map(|v| matrix.transform_point3(v - element.pivot) + element.pivot)
                        });

                        builder.push(&VoxelFace {
                            position: self.body.position - half_size,
                            vertices,
                            lights: [240; 4],
                            uvs: face_data.uvs,
                            color: Color::WHITE,
                        });
                    }
                }
            }
        }
    }

    // pub fn aabb(&self) -> Aabb {
    //     Aabb::new(self.position.as_dvec3(), (self.position +
    // Self::SIZE).as_dvec3()) }
}

pub enum WorldType {
    Local,
    #[cfg(feature = "multiplayer")]
    Remote {
        sender: mpsc::Sender<IncomingPacket>,
        receiver: mpsc::Receiver<OutgoingPacket>,
        player_uuid: Option<Uuid>,
    },
}

pub struct EntityManager {
    resource_storage: Arc<ResourceStorage>,
    next_entity_id: usize,
    entities: HashMap<usize, Entity>,
}

impl EntityManager {
    pub fn new(resource_storage: Arc<ResourceStorage>) -> Self {
        Self {
            resource_storage,
            next_entity_id: 0,
            entities: HashMap::default(),
        }
    }

    pub fn spawn(&mut self, entity: Entity) -> usize {
        let id = self.next_entity_id;

        self.entities.insert(id, entity);
        self.next_entity_id += 1;

        id
    }

    pub fn spawn_item(&mut self, position: Point3D, item: Item) -> usize {
        self.spawn(Entity::item(position, item))
    }

    pub fn spawn_model(&mut self, position: Point3D, model_id: usize) -> usize {
        self.spawn(Entity::model(position, model_id, self.resource_storage.as_ref()))
    }

    pub fn get_mut(&mut self, entity_id: usize) -> Option<&mut Entity> {
        self.entities.get_mut(&entity_id)
    }

    pub fn remove(&mut self, entity_id: usize) -> Option<Entity> {
        self.entities.remove(&entity_id)
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }
}

impl<'a> IntoIterator for &'a EntityManager {
    type IntoIter = Iter<'a, usize, Entity>;
    type Item = (&'a usize, &'a Entity);

    fn into_iter(self) -> Self::IntoIter {
        self.entities.iter()
    }
}

impl<'a> IntoIterator for &'a mut EntityManager {
    type IntoIter = IterMut<'a, usize, Entity>;
    type Item = (&'a usize, &'a mut Entity);

    fn into_iter(self) -> Self::IntoIter {
        self.entities.iter_mut()
    }
}

pub struct ChunkFileCache {
    pub root: PathBuf,
}

impl ChunkCache for ChunkFileCache {
    fn all(&self) -> impl Iterator<Item = Chunk> {
        // self.root.read_dir().into_iter().flatten().filter_map(|e| {
        //     let e = e.ok()?;
        //     let path = e.path();

        //     if path.is_file() && path.extension().is_some_and(|ext| ext == "cdt") {
        //         let data = std::fs::read(path).ok()?;

        //         Chunk::deserialize(data).ok()
        //     } else {
        //         None
        //     }
        // })
        std::iter::empty()
    }

    fn get(&self, origin: IPoint2D) -> Option<Chunk> {
        let _path = self.root.join(format!("{}x{}.cdt", origin.x, origin.y));

        // if path.is_file() {
        //     let data = std::fs::read(path).ok()?;

        //     None
        // } else {
        None
        // }
    }

    fn insert(&mut self, _origin: IPoint2D, _chunk: &Chunk) {
        // let path = self.root.join(format!("{}x{}.cdt", origin.x, origin.y));
        // let data = chunk.serialize();

        // _ = std::fs::write(path, data);
    }
}

#[allow(dead_code)]
pub enum Weather {
    Clear,
    Rain,
    Thunder,
}

#[allow(clippy::large_enum_variant)]
enum JobResult {
    /// Bare terrain
    Generation {
        chunk: Chunk,
    },
    /// Populated terrain with lakes, trees, etc.
    Population {
        chunk: Arc<Chunk>,
        neighbours: [Arc<Chunk>; 8],
    },
    /// Populated terrain with lights.
    Lighting {
        chunk: Arc<Chunk>,
        neighbours: [Arc<Chunk>; 8],
    },
    Meshing {
        origin: IPoint2D,
        mesh: Box<[[Vec<VoxelFace>; 2]]>,
    },
}

pub struct JobManager {
    sender: crossbeam_channel::Sender<JobResult>,
    receiver: crossbeam_channel::Receiver<JobResult>,
    jobs: HashSet<IPoint2D>,
}

impl JobManager {
    fn new() -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();

        Self {
            sender,
            receiver,
            jobs: HashSet::default(),
        }
    }

    fn spawn_generation_job(&self, seed: u32, origin: IPoint2D, resource_storage: Arc<ResourceStorage>) {
        let sender = self.sender.clone();

        rayon::spawn(move || {
            let instant = Instant::now();
            let generator = ChunkGenerator::new(i64::from(seed));
            let mut chunk = Chunk::new(origin);

            generator.generate_unpopulated_chunk_data(&mut chunk, resource_storage.as_ref());

            _ = sender.send(JobResult::Generation { chunk });

            info!(target: "client/world", origin = ?origin, "Chunk generated in {:?}", instant.elapsed());
        });
    }

    fn spawn_population_job(&self, origin: IPoint2D, seed: u32, mut chunk_manager: LocalChunkManager, resource_storage: Arc<ResourceStorage>) {
        let sender = self.sender.clone();

        rayon::spawn(move || {
            let instant = Instant::now();
            let seed = i64::from(seed);

            ChunkGenerator::new(seed).populate(&mut chunk_manager, resource_storage.as_ref(), seed, origin);

            let (chunk, neighbours) = chunk_manager.into_inner();

            _ = sender.send(JobResult::Population { chunk, neighbours });

            info!(target: "client/world", origin = ?origin, "Chunk populated in {:?}", instant.elapsed());
        });
    }

    fn spawn_lighting_job(&self, origin: IPoint2D, mut chunk_manager: LocalChunkManager, resource_storage: Arc<ResourceStorage>) {
        let sender = self.sender.clone();

        rayon::spawn(move || {
            let instant = Instant::now();
            let mut bfs_light = BfsLight::new(&mut chunk_manager);

            for offset in [
                IPoint2D::NEG_ONE,
                IPoint2D::NEG_ONE.with_y(1),
                IPoint2D::NEG_X,
                IPoint2D::NEG_Y,
                IPoint2D::ZERO,
                IPoint2D::X,
                IPoint2D::Y,
                IPoint2D::ONE.with_y(-1),
                IPoint2D::ONE,
            ] {
                let origin = origin + offset;

                if let Some(chunk) = bfs_light.chunk_manager.get_chunk_mut(origin) {
                    for z in 0..SUBCHUNK_SIZE {
                        for x in 0..SUBCHUNK_SIZE {
                            for y in (0..CHUNK_HEIGHT).rev() {
                                let pos = USizePoint3D::new(x, y, z);

                                chunk.set_sky_light(pos, 15);

                                if pos.y > 0 && chunk.get_block(pos - USizePoint3D::Y).is_some_and(|b| !b.is_air()) {
                                    bfs_light.sky_addition_queue.push_back((LightNode(pos, origin), 15));

                                    break;
                                }
                            }
                        }
                    }
                }
            }

            bfs_light.calculate_sky_light(resource_storage.as_ref());

            info!(target: "client/world", origin = ?origin, "Chunk lighted in {:?}", instant.elapsed());

            let (chunk, neighbours) = chunk_manager.into_inner();

            _ = sender.send(JobResult::Lighting { chunk, neighbours });
        });
    }

    fn spawn_meshing_job(&self, origin: IPoint2D, chunk_manager: LocalChunkManager, resource_storage: Arc<ResourceStorage>, settings: GraphicsSettings) {
        let sender = self.sender.clone();

        rayon::spawn(move || {
            let instant = Instant::now();
            let snapshot = WorldSnapshot::new(&chunk_manager, resource_storage, settings);
            let mesh = (0..SUBCHUNK_COUNT)
                .rev()
                .map(|subchunk_idx| {
                    if snapshot.chunk_manager.get_chunk(origin).unwrap().subchunks[subchunk_idx]
                        .palette
                        .iter()
                        .any(|block| !block.is_air())
                    {
                        snapshot.compute_subchunk_mesh(origin, subchunk_idx)
                    } else {
                        [Vec::new(), Vec::new()]
                    }
                })
                .collect();

            _ = sender.send(JobResult::Meshing { origin, mesh });

            info!(target: "client/world", origin = ?origin, "Chunk meshed in {:?}", instant.elapsed());
        });
    }
}

pub struct WorldColors {
    pub biome: Biome,
    pub sky: Tween<Color>,
    pub fog: Tween<Color>,
}

pub struct World {
    pub clock: Clock,
    pub tick_interval: Interval,
    pub physics_interval: Interval,

    pub camera: Camera,
    pub player: Player,
    pub inventory_slot: Ranged<u8>,

    pub chunk_manager: ChunkManager<ChunkFileCache>,
    pub job_manager: JobManager,
    pub chunk_renderer: ChunkRenderer,

    #[allow(dead_code)]
    pub current_weather: Weather,

    pub chat_history: Vec<String>,

    #[allow(dead_code)]
    pub ty: WorldType,
    pub resource_storage: Arc<ResourceStorage>,
    pub entities: EntityManager,

    pub seed: u32,
    pub marked: Option<IPoint3D>,

    pub colors: WorldColors,
}

impl World {
    pub fn new(
        context: &WindowContext,
        texture: &wgpu::Texture,
        lightmap: &wgpu::Texture,
        resource_storage: Arc<ResourceStorage>,
        chunk_manager: ChunkManager<ChunkFileCache>,
        ty: WorldType,
    ) -> Self {
        let mut player = Player::default();

        player.body.position = Point3D::new(2.0, 135.0, 2.0);

        let sky = resource_storage.color_config.base_sky_color;
        let fog = resource_storage.color_config.base_fog_color.unwrap_or(sky);

        let colors = WorldColors {
            biome: Biome::Sky,
            sky: Tween::new(sky, sky, 1000),
            fog: Tween::new(fog, fog, 1000),
        };

        Self {
            camera: Camera::new(player.camera_position()),
            chunk_renderer: ChunkRenderer::new(context, texture, lightmap),
            tick_interval: Interval::new(TICK_RATE),
            physics_interval: Interval::new(PHYSICS_RATE),
            player,
            inventory_slot: Ranged::new(0, 0, 8),
            clock: Clock::default(),
            chunk_manager,
            colors,
            job_manager: JobManager::new(),
            resource_storage: resource_storage.clone(),
            chat_history: Vec::new(),
            current_weather: Weather::Clear,
            ty,
            entities: EntityManager::new(resource_storage),
            seed: 0,
            marked: None,
        }
    }

    #[allow(dead_code)]
    pub fn send_chat_message<T: Into<String>>(&mut self, message: T) {
        self.chat_history.push(message.into());
    }

    fn destroy_block2(&mut self, position: IPoint3D) {
        let local = self.chunk_manager.to_chunk_local(position);

        if let Some(local) = local {
            let origin = ChunkManager::<()>::to_local(position);

            #[cfg(feature = "multiplayer")]
            if let WorldType::Remote { sender, .. } = &self.ty {
                sender.send(IncomingPacket::RemoveBlock(origin, local)).unwrap();
            }

            self.destroy_block_local(origin, local);
        }
    }

    pub fn destroy_block_local(&mut self, origin: IPoint2D, local: USizePoint3D) {
        let position = Chunk::to_world_pos(origin, local);

        if let Some(state) = self.chunk_manager.get_block(position)
            && !state.is_air()
            && let Some(block) = self.resource_storage.blocks.get(state.id)
        {
            if block.droppable() {
                self.entities.spawn_item(position.as_vec3() + Point3D::new(0.35, 0.0, 0.35), Item {
                    id: state.id,
                    ty: ItemType::Block,
                    amount: 1,
                });
            }

            self.chunk_manager.remove_block(position, self.resource_storage.as_ref());
        }
    }

    pub fn destroy_looking_at(&mut self) {
        if let Some(looking_at) = self.camera.looking_at {
            self.destroy_block2(looking_at.position);

            let provider = AabbProvider {
                chunk_manager: &self.chunk_manager,
                entity_manager: &self.entities,
                storage: self.resource_storage.as_ref(),
            };

            let context = PhysicsContext::new(provider);

            self.camera.update_looking_at(&context);
        }
    }

    pub fn place(&mut self, position: IPoint3D, id: u32) {
        let chunk = ChunkManager::<()>::to_local(position);

        info!("placing block in {chunk}");

        if let Some(local) = self.chunk_manager.to_chunk_local(position) {
            let block = self.resource_storage.blocks.get(id).unwrap();

            if let Some(chunk) = self.chunk_manager.get_chunk_mut(chunk) {
                chunk.set_block(local, SubChunkBlockState::new(id));
                chunk.dirty = true;

                info!("placed block at {position}");
            }

            for normal in Face::NORMALS {
                let chunk_position = ChunkManager::<()>::to_local(position + normal);

                if chunk_position != chunk
                    && let Some(chunk) = self.chunk_manager.get_chunk_mut(chunk_position)
                {
                    chunk.dirty = true;

                    info!("marked neighbour {chunk_position} dirty");
                }
            }

            for normal in [IPoint3D::NEG_ONE, IPoint3D::NEG_ONE.with_x(1), IPoint3D::ONE.with_x(-1), IPoint3D::ONE] {
                let chunk_position = ChunkManager::<()>::to_local(position + normal);

                if chunk_position != chunk
                    && let Some(chunk) = self.chunk_manager.get_chunk_mut(chunk_position)
                {
                    chunk.dirty = true;

                    info!("marked neighbour {chunk_position} dirty");
                }
            }

            info!("calculating light");

            let mut light = BfsLight::new(&mut self.chunk_manager);

            if block.light_level() > 0 {
                info!("calculating light for torch");

                light.add_block_custom(LightNode(local, chunk), block.light_level());
                light.calculate_block_light(self.resource_storage.as_ref());

                info!("calculated light for torch");
            } else if block.blocks_light() {
                info!("calculating light for block");

                light.remove_sky(LightNode(local, chunk));
                light.calculate_sky_light(self.resource_storage.as_ref());

                info!("calculated light for block");
            }
        }
    }

    pub fn place_held(&mut self) {
        let Some(result) = self.camera.looking_at else {
            return;
        };

        let position = result.position + result.hit_side.as_normal();

        if self.chunk_manager.get_block(position).is_none_or(|block| !block.is_air()) || Aabb::cube(position.as_dvec3()).intersects(&self.player.aabb()) {
            return;
        }

        let Some((id, _)) = self.player.inventory.take_hotbar_item(self.inventory_slot.value as usize) else {
            return;
        };

        self.place(position, id);
        self.camera.update_looking_at(&PhysicsContext::new(AabbProvider {
            chunk_manager: &self.chunk_manager,
            entity_manager: &self.entities,
            storage: self.resource_storage.as_ref(),
        }));
    }

    pub fn physics_step(&mut self, input: &Input) {
        let provider = AabbProvider {
            chunk_manager: &self.chunk_manager,
            entity_manager: &self.entities,
            storage: self.resource_storage.as_ref(),
        };

        let context = PhysicsContext::new(provider);
        let grounded = self.player.body.is_on_ground;

        context.physics_step(&mut self.player.body, PHYSICS_RATE.as_secs_f32());

        self.player.physics_step(input, &mut self.camera, PHYSICS_RATE.as_secs_f32());
        self.player.body.config.friction = if self.player.body.config.gravity_scale <= 1e-7 {
            8.0
        } else if !grounded {
            2.0
        } else {
            10.0
        };

        self.camera.set_position(&context, self.player.camera_position());

        let player_aabb = self.player.aabb();
        let mut remove_entities: Vec<usize> = Vec::new();

        let context = PhysicsContext::new(LimitedAabbProvider {
            chunk_manager: &self.chunk_manager,
            storage: self.resource_storage.as_ref(),
        });

        for (id, entity) in &mut self.entities {
            context.physics_step(&mut entity.body, PHYSICS_RATE.as_secs_f32());

            if matches!(entity.data, EntityData::Item { .. }) {
                let entity_aabb = entity.body.aabb();

                if entity_aabb.intersects(&player_aabb) {
                    remove_entities.push(*id);
                }
            }
        }

        for entity_id in remove_entities {
            if let Some(entity) = self.entities.remove(entity_id)
                && let EntityData::Item { item, .. } = entity.data
            {
                self.player.inventory.try_insert(&item);
            }
        }
    }

    pub const fn tick(&mut self) {
        self.clock.tick();

        #[cfg(feature = "multiplayer")]
        if let WorldType::Remote { sender, receiver, player_uuid } = &mut self.ty {
            if let Some(uuid) = *player_uuid {
                sender
                    .send(IncomingPacket::PlayerMoved {
                        uuid,
                        position: self.player.body.position,
                    })
                    .unwrap();
            }

            match receiver.try_recv() {
                Ok(OutgoingPacket::ChunkData { data }) => {
                    let chunk = Chunk::deserialize(data).unwrap();

                    info!(target: "client/network", origin = ?chunk.origin, "Received chunk");

                    if ChunkManager::to_local(self.player.body.position.as_()) == chunk.origin {
                        self.player_controllable = true;
                    }

                    self.chunk_sender.send(chunk).unwrap();
                }
                Ok(OutgoingPacket::PlayerConnected { uuid, name }) => info!(target: "client/network", "{name} ({uuid}) connected!"),
                Ok(OutgoingPacket::UuidAssigned { uuid }) => {
                    for x in -2..2 {
                        for z in -2..2 {
                            sender.send(IncomingPacket::RequestChunk(IPoint2D::new(x, z))).unwrap();
                        }
                    }

                    player_uuid.replace(uuid);
                }
                _ => {}
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    pub fn update(&mut self, context: &WindowContext, settings: GraphicsSettings, input: &Input, delta: Duration) {
        self.colors.sky.advance(delta);
        self.colors.fog.advance(delta);

        self.player.handle_keyboard(input);

        if self.clock.active() {
            for _ in 0..self.physics_interval.update(delta) {
                self.physics_step(input);
            }

            for _ in 0..self.tick_interval.update(delta) {
                self.tick();
            }
        }

        if input.mouse.is_pressed_once(MouseButton::Left) {
            self.destroy_looking_at();
        } else if input.mouse.is_pressed(MouseButton::Right) {
            self.place_held();
        }

        if let Some(biome) = self.chunk_manager.get_biome(self.player.body.position.as_ivec3())
            && self.colors.biome != biome
        {
            self.colors.biome = biome;

            let base_sky = self.resource_storage.color_config.base_sky_color;
            let base_fog = self.resource_storage.color_config.base_fog_color.unwrap_or(base_sky);
            let (sky, fog) = self.resource_storage.color_config.biomes.get(&biome).map_or((base_sky, base_fog), |biome| {
                (biome.sky_color.unwrap_or(base_sky), biome.fog_color.unwrap_or(base_fog))
            });

            self.colors.sky.set(sky);
            self.colors.fog.set(fog);
        }

        for result in self.job_manager.receiver.try_iter() {
            match result {
                JobResult::Generation { chunk } => {
                    self.job_manager.jobs.remove(&chunk.origin);
                    self.chunk_manager.push(chunk, ChunkStage::Bare);
                }
                JobResult::Population { chunk, neighbours } => {
                    self.job_manager.jobs.remove(&chunk.origin);
                    self.chunk_manager.replace(chunk, ChunkStage::Populated);

                    for chunk in neighbours {
                        self.job_manager.jobs.remove(&chunk.origin);

                        self.chunk_manager.replace(chunk, ChunkStage::Populated);
                    }
                }
                JobResult::Lighting { chunk, neighbours } => {
                    self.job_manager.jobs.remove(&chunk.origin);
                    self.chunk_manager.replace(chunk, ChunkStage::Lighted);

                    for chunk in neighbours {
                        let origin = chunk.origin;

                        self.job_manager.jobs.remove(&origin);
                        self.chunk_manager.replace(chunk, ChunkStage::Lighted);

                        if self.chunk_manager.stages.get(&origin).is_some_and(|stage| stage >= &ChunkStage::Meshed) {
                            let chunk = unsafe { self.chunk_manager.get_chunk_mut(origin).unwrap_unchecked() };

                            chunk.dirty = true;
                        }
                    }
                }
                JobResult::Meshing { origin, mesh } => {
                    self.job_manager.jobs.remove(&origin);

                    self.chunk_manager.set_stage(origin, ChunkStage::Meshed);

                    for (subchunk_idx, mesh) in mesh.into_iter().rev().enumerate() {
                        let (solid, translucent) = mesh.into();
                        let solid = VoxelMeshBuilder::build_from_slice(context.device, &solid);
                        let translucent = TranslucentSubchunk::new(context.device, translucent, self.player.camera_position(), origin);

                        self.chunk_renderer.set_subchunk((origin, subchunk_idx), solid, translucent);
                    }
                }
            }
        }

        thread_local! {
            static STAGE_QUEUE: RefCell<Vec<(IPoint2D, ChunkStage)>> = const { RefCell::new(Vec::new()) };
        }

        let player_origin = ChunkManager::<()>::to_local(self.player.body.position.as_ivec3());

        for origin in self.chunk_renderer.filter_by_shape(player_origin, settings.render_shape) {
            self.chunk_manager.stages.insert(origin, ChunkStage::Lighted);
        }

        STAGE_QUEUE.with_borrow_mut(|queue| {
            if !self.job_manager.jobs.is_empty() {
                return;
            }

            for origin in settings.render_shape.enlarge(1).iter_from_center(player_origin) {
                match self.chunk_manager.stages.get(&origin) {
                    None | Some(ChunkStage::Unloaded) => {
                        self.job_manager.jobs.insert(origin);
                        self.job_manager.spawn_generation_job(self.seed, origin, self.resource_storage.clone());

                        queue.push((origin, ChunkStage::GenerationInProgress));
                    }
                    Some(ChunkStage::Bare)
                        if self.chunk_manager.neighbours_at_least(origin, ChunkStage::Bare)
                            && !self.job_manager.jobs.contains(&origin)
                            && self.chunk_manager.neighbours_of(origin).all(|origin| !self.job_manager.jobs.contains(&origin)) =>
                    {
                        let chunk_manager = self.chunk_manager.local_of(origin).unwrap();

                        self.job_manager.jobs.insert(origin);

                        for chunk in self.chunk_manager.neighbours_of(origin) {
                            if self.chunk_manager.stages.get(&chunk).is_some_and(|&stage| stage == ChunkStage::Bare) {
                                self.job_manager.jobs.insert(chunk);
                            }
                        }

                        self.job_manager
                            .spawn_population_job(origin, self.seed, chunk_manager, self.resource_storage.clone());

                        queue.push((origin, ChunkStage::PopulationInProgress));
                    }
                    Some(ChunkStage::Populated)
                        if self.chunk_manager.neighbours_at_least(origin, ChunkStage::Populated)
                            && !self.job_manager.jobs.contains(&origin)
                            && self.chunk_manager.neighbours_of(origin).all(|origin| !self.job_manager.jobs.contains(&origin)) =>
                    {
                        let chunk_manager = self.chunk_manager.local_of(origin).unwrap();

                        self.job_manager.jobs.insert(origin);

                        for chunk in self.chunk_manager.neighbours_of(origin) {
                            if self.chunk_manager.stages.get(&chunk).is_some_and(|&stage| stage <= ChunkStage::Populated) {
                                self.job_manager.jobs.insert(chunk);
                            }
                        }

                        self.job_manager.spawn_lighting_job(origin, chunk_manager, self.resource_storage.clone());

                        queue.push((origin, ChunkStage::LightingInProgress));
                    }
                    Some(ChunkStage::Lighted)
                        if settings.render_shape.test(player_origin, origin)
                            && self.chunk_manager.neighbours_at_least(origin, ChunkStage::Lighted)
                            && !self.job_manager.jobs.contains(&origin)
                            && self.chunk_manager.neighbours_of(origin).all(|origin| !self.job_manager.jobs.contains(&origin)) =>
                    {
                        let chunk_manager = self.chunk_manager.local_of(origin).unwrap();

                        self.job_manager.jobs.insert(origin);
                        self.job_manager
                            .spawn_meshing_job(origin, chunk_manager, self.resource_storage.clone(), settings);

                        queue.push((origin, ChunkStage::MeshingInProgress));
                    }
                    Some(ChunkStage::Meshed)
                        if settings.render_shape.test(player_origin, origin)
                            && self.chunk_manager.neighbours_at_least(origin, ChunkStage::Lighted)
                            && self.chunk_manager.get_chunk(origin).is_some_and(|chunk| chunk.dirty)
                            && !self.job_manager.jobs.contains(&origin) =>
                    {
                        let chunk_manager = self.chunk_manager.local_of(origin).unwrap();

                        self.job_manager.jobs.insert(origin);
                        self.job_manager
                            .spawn_meshing_job(origin, chunk_manager, self.resource_storage.clone(), settings);

                        queue.push((origin, ChunkStage::MeshingInProgress));
                    }
                    _ => (),
                }
            }

            for (origin, stage) in queue.drain(..) {
                self.chunk_manager.set_stage(origin, stage);

                if matches!(stage, ChunkStage::MeshingInProgress)
                    && let Some(chunk) = self.chunk_manager.get_chunk_mut(origin)
                {
                    chunk.dirty = false;
                }
            }
        });
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub fn render(
        &mut self,
        context: &WindowContext,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        common_renderer: &mut CommonRenderer,
        surface_size: USize2D,
        settings: &Settings,
        info: RenderInfo,
        delta: Duration,
    ) {
        // self.scene.buffer(backend);

        let progress = self.clock.get_progress();

        self.chunk_renderer
            .set_sun_position(if progress > 0.5 { 1.0 - progress } else { progress } * 2.0);

        // pass.clear_color_and_depth(Color::BLACK.to_linear_rgba(), 1.0);

        let sky_color = *self.colors.sky.get() /* get_sky_color(self.clock.get_visual_progress(), 0.0) */;
        let fog_color = *self.colors.fog.get() /* get_sky_color(self.clock.get_visual_progress(), 0.0) */;

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Main Menu Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear({
                        let [r, g, b, a]: [f32; 4] = sky_color.to_linear_rgba();
                        let [r, g, b, a] = [f64::from(r), f64::from(g), f64::from(b), f64::from(a)];

                        wgpu::Color { r, g, b, a }
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &context.depth_texture.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });

        let pass = &mut pass;

        self.chunk_renderer.set_fog_color(fog_color);

        let rendered_subchunks = self
            .chunk_renderer
            .render(context.queue, pass, self.camera.position, &self.camera.frustum, self.camera.matrix());

        let mut builder = VoxelMeshBuilder::with_capacity(self.entities.len());

        for (_, entity) in &self.entities {
            entity.render_to(&mut builder, &self.chunk_manager, self.resource_storage.as_ref());
        }

        builder.render(context, pass, &mut self.chunk_renderer, self.camera.world_matrix());

        // self.kawase.apply(backend, &self.scene).unwrap();

        // self.scene.render(&mut frame).unwrap();
        // self.particles.render(&mut frame, self.camera.matrix()).unwrap();

        if self.is_underwater(self.player.camera_position()) {
            common_renderer.draw_rect(Point2D::ZERO, surface_size.as_vec2(), Color::from_hsl(215.0, 1.0, 0.6).with_alpha(0.5));
        }

        self.render_hotbar(context, common_renderer, surface_size);

        if settings.debugging.enabled {
            self.render_debug_text(common_renderer, context, settings.graphics, rendered_subchunks.draw_calls, surface_size);
            self.render_chunk_map(context.queue, common_renderer, surface_size);

            Self::render_fps_stat(context.queue, common_renderer, &settings.debugging, delta, surface_size);
            Self::render_draw_calls_stat(context.queue, common_renderer, &settings.debugging, info, surface_size);
        } else {
            common_renderer.draw_text(
                context.queue,
                Point2D::new(8.0, 4.0),
                "default",
                "Press F3 to view debug information.",
                Color::WHITE,
                18.0,
                None,
            );
        }

        common_renderer.render(pass, context);

        let mut builder = VoxelMeshBuilder::with_capacity(self.player.inventory.get_hotbar_items().count());

        let matrix = Transform3D::from_rotation_x(const { 200f32.to_radians() })
            * Transform3D::from_rotation_y(const { 35f32.to_radians() })
            * Transform3D::from_rotation_z(0.0);

        for (i, item) in self.player.inventory.get_hotbar_items() {
            const INVENTORY_HOTBAR_SLOTS: u8 = 8;
            const SLOT_SIZE: f32 = 48f32;
            const SIZE: f32 = SLOT_SIZE * 0.6;
            const ORIGIN: Point3D = Point3D::new(SIZE / 2.0, SIZE / 2.0, SIZE / 2.0);
            const HOTBAR_WIDTH: f32 = (INVENTORY_HOTBAR_SLOTS + 1) as f32 * SLOT_SIZE;

            let model = self
                .resource_storage
                .models
                .get_unchecked(self.resource_storage.blocks.get_model_by_name(item.id));

            let size = surface_size.as_vec2();
            let origin = Point2D::new(
                (size.x / 2.0) - (HOTBAR_WIDTH / 2.0) + ((SLOT_SIZE - SIZE) / 2.0),
                size.y - SLOT_SIZE - 8.0 + ((SLOT_SIZE - SIZE) / 2.0),
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
                            color: if model_face.tint { GRASS_COLOR } else { Color::WHITE }.multiply_rgb(model_face.face_data.face.get_light_level()),
                        },
                        &matrix,
                        ORIGIN,
                    );
                }
            }
        }

        builder.render_full_bright(context, pass, &mut self.chunk_renderer, common_renderer.window_matrix());
    }

    #[inline]
    fn is_underwater(&self, position: Point3D) -> bool {
        self.chunk_manager
            .get_block(position.floor().as_ivec3())
            .is_some_and(|block| block.id == self.resource_storage.get_block_id("game:water"))
    }

    fn render_chunk_map(&self, queue: &wgpu::Queue, context: &mut CommonRenderer, surface_size: USize2D) {
        const CHUNK_UI_CONTAINER_SIZE: Size2D = Size2D::new(128.0, 128.0);
        const CHUNK_UI_COUNT: usize = 16;
        const SPACING: f32 = 1.0;
        const CHUNK_UI_SIZE: Size2D = Size2D::new(
            (CHUNK_UI_CONTAINER_SIZE.x - SPACING * (CHUNK_UI_COUNT - 1) as f32) / CHUNK_UI_COUNT as f32,
            (CHUNK_UI_CONTAINER_SIZE.y - SPACING * (CHUNK_UI_COUNT - 1) as f32) / CHUNK_UI_COUNT as f32,
        );

        let container_origin = Point2D::new(surface_size.as_vec2().x - CHUNK_UI_CONTAINER_SIZE.x - 12.0, 12.0);
        let bounds = Rect::new(container_origin, CHUNK_UI_CONTAINER_SIZE);

        context.draw_rect(bounds.origin, bounds.size, Color::BLACK);

        let player_chunk = ChunkManager::<()>::to_local(self.player.body.position.as_ivec3());
        let origin = bounds.origin + CHUNK_UI_COUNT as f32 * CHUNK_UI_SIZE / 2.0;

        for x in 0..CHUNK_UI_COUNT as i32 {
            let x = x - (CHUNK_UI_COUNT / 2) as i32;

            for z in 0..CHUNK_UI_COUNT as i32 {
                let z = z - (CHUNK_UI_COUNT / 2) as i32;
                let xz = IPoint2D::new(x, z);
                let chunk = player_chunk + xz;

                if let Some(stage) = self.chunk_manager.stages.get(&chunk) {
                    let color = match stage {
                        ChunkStage::Unloaded => continue,
                        ChunkStage::GenerationInProgress => Color::new(100, 100, 100, 255),
                        ChunkStage::Bare => Color::new(150, 150, 150, 255),
                        ChunkStage::PopulationInProgress => Color::from_u32_rgb(0x73AF73),
                        ChunkStage::Populated => Color::GREEN,
                        ChunkStage::LightingInProgress => Color::from_u32_rgb(0xB8FF00),
                        ChunkStage::Lighted => Color::YELLOW,
                        ChunkStage::MeshingInProgress => Color::from_u32_rgb(0x63639C),
                        ChunkStage::Meshed => Color::BLUE,
                    };

                    context.draw_rect(origin - xz.as_vec2() * (CHUNK_UI_SIZE + 1.0), CHUNK_UI_SIZE, color);
                }
            }
        }

        context.draw_rect(container_origin + bounds.size / 2.0 - Vector2D::splat(1.0), Size2D::splat(2.0), Color::RED);

        let text = context
            .measure(
                "default",
                format!(
                    "{} {} {}",
                    self.player.body.position.x as i32, self.player.body.position.y as i32, self.player.body.position.z as i32
                ),
                9.0,
                None,
            )
            .unwrap_or_default();
        let new_container_origin = container_origin + CHUNK_UI_CONTAINER_SIZE.with_x((CHUNK_UI_CONTAINER_SIZE.x - text.x) / 2.0) + Point2D::new(0.0, 2.0);

        context.draw_rect(new_container_origin, Size2D::new(text.x + 8.0, 20.0), Color::from_u32_rgb(0x1D211B));
        context.draw_text(
            queue,
            new_container_origin + Point2D::splat(4.0),
            "default",
            format!(
                "{} {} {}",
                self.player.body.position.x as i32, self.player.body.position.y as i32, self.player.body.position.z as i32
            ),
            Color::from_hsl(110.0, 0.5, 0.8),
            9.0,
            None,
        );

        let text = context
            .measure(
                "default",
                format!("{:?}", self.chunk_manager.get_biome(self.player.body.position.as_ivec3())),
                9.0,
                None,
            )
            .unwrap_or_default();
        let new_container_origin = container_origin + CHUNK_UI_CONTAINER_SIZE.with_x((CHUNK_UI_CONTAINER_SIZE.x - text.x) / 2.0) + Point2D::new(0.0, 24.0);

        context.draw_rect(new_container_origin, Size2D::new(text.x + 8.0, 20.0), Color::from_u32_rgb(0x1D211B));
        context.draw_text(
            queue,
            new_container_origin + Point2D::splat(4.0),
            "default",
            format!("{:?}", self.chunk_manager.get_biome(self.player.body.position.as_ivec3())),
            Color::from_hsl(110.0, 0.5, 0.8),
            9.0,
            None,
        );
    }

    fn render_debug_text(
        &self,
        context: &mut CommonRenderer,
        backend: &WindowContext,
        GraphicsSettings { render_shape, vsync, .. }: GraphicsSettings,
        rendered_subchunks: usize,
        USize2D { x, y }: USize2D,
    ) {
        let (hours, minutes) = {
            let time = self.clock.time().as_secs();
            let seconds = time % 60;
            let minutes = (time - seconds) / 60 % 60;
            let hours = (time - seconds - minutes * 60) / 60 / 60;

            (hours, minutes)
        };

        let info = backend.adapter.get_info();
        let renderer = match info.backend {
            wgpu::Backend::Noop => "Noop",
            wgpu::Backend::Vulkan => "Vulkan",
            wgpu::Backend::Metal => "Metal",
            wgpu::Backend::Dx12 => "DirectX 12",
            wgpu::Backend::Gl => "OpenGL",
            wgpu::Backend::BrowserWebGpu => "WebGPU",
        };

        let gpu = info.name;
        let version = info.driver_info;
        let block = self
            .camera
            .looking_at
            .and_then(|result| Some((result, self.chunk_manager.get_block(result.position)?)))
            .filter(|(_, state)| !state.is_air())
            .and_then(|(result, state)| Some((result, state, self.resource_storage.blocks.get(state.id)?)))
            .map_or_else(
                || String::from("nothing"),
                |(result, state, block)| {
                    format!(
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
                    )
                },
            );

        let total_chunks = self.chunk_manager.len();
        let total_subchunks = total_chunks * SUBCHUNK_COUNT;

        let text = format!(
            "Render Backend: {renderer}
GPU: {gpu}
Version: {version}
Surface size: {x}x{y}
Game Time: {hours:02}:{minutes:02}
Looking at {block}
VSync: {vsync}
Render Shape: {render_shape}
Rendered subchunks: {rendered_subchunks} / {total_subchunks} ({total_chunks} total chunks)",
        );

        // let text_size = context.measure("default", &text, 18.0,
        // None).unwrap_or_default(); let text_bounds =
        // Rect::new(Point2D::new(12.0, 12.0), Size2D::new((522.0 +
        // 4.0) * overlay_width, text_size.y + 4.0));

        context.draw_text(backend.queue, Point2D::new(8.0, 4.0), "default", text, Color::WHITE, 18.0, None);
    }

    fn render_hotbar(&self, backend: &WindowContext, context: &mut CommonRenderer, surface_size: USize2D) {
        const INVENTORY_HOTBAR_SLOTS: u8 = 8;
        const SLOT_SIZE: f32 = 48f32;

        let bounds = Rect {
            origin: Point2D::ZERO,
            size: surface_size.as_vec2(),
        };

        let hotbar_width = f32::from(INVENTORY_HOTBAR_SLOTS + 1) * SLOT_SIZE;
        let origin = Point2D::new((bounds.size.x / 2.0) - (hotbar_width / 2.0), bounds.size.y - SLOT_SIZE - 8.0);
        let offset = f32::from(self.inventory_slot.value) * SLOT_SIZE;

        context.draw_rect(origin, Size2D::new(hotbar_width, SLOT_SIZE), Color::from_u32_rgb(0x1D211B));
        context.draw_rect(
            origin + Point2D::new(offset, 0.0),
            Size2D::new(SLOT_SIZE, SLOT_SIZE),
            Color::from_hsl(110.0, 0.5, 0.8),
        );

        context.draw_rect(
            origin + Point2D::new(2.0, 2.0) + Point2D::new(offset, 0.0),
            Size2D::new(SLOT_SIZE - 4.0, SLOT_SIZE - 4.0),
            Color::from_u32_rgb(0x1D211B),
        );

        let hotbar_width = f32::from(INVENTORY_HOTBAR_SLOTS + 1) * SLOT_SIZE;
        let origin = Point2D::new((bounds.size.x / 2.0) - (hotbar_width / 2.0), bounds.size.y - SLOT_SIZE - 8.0);

        for (column, item) in self.player.inventory.get_hotbar_items() {
            let offset = (column + 1) as f32 * SLOT_SIZE;
            let text = format!("x{}", item.amount);

            let text_size = context.measure("default", &text, 18.0, None).unwrap();

            context.draw_text(
                backend.queue,
                origin.with_y(bounds.size.y - 10.0 - 21.0) + Point2D::new(offset - 3.0 - text_size.x, 0.0),
                "default",
                text,
                Color::from_hsl(110.0, 0.5, 0.8),
                18.0,
                None,
            );
        }
    }

    fn render_draw_calls_stat(queue: &wgpu::Queue, context: &mut CommonRenderer, debugging: &Debugging, info: RenderInfo, surface_size: USize2D) {
        const SPACING: f32 = 1.0;
        const SIZE: Size2D = Size2D::new(100.0 * (2.0 + SPACING), 96.0);
        const CONTAINER_SIZE: Size2D = Size2D::new(SIZE.x - SPACING, SIZE.y);
        const ELEMENT_WIDTH: f32 = (SIZE.x - 100.0 * SPACING) / 100.0;

        let bounds = Rect {
            origin: Point2D::ZERO,
            size: surface_size.as_vec2(),
        };

        let container_origin = bounds.origin + bounds.size - CONTAINER_SIZE - Point2D::splat(4.0);

        context.draw_rect(container_origin, CONTAINER_SIZE, Color::from_u32_rgb(0x1D211B));

        let mut x = 0.0;

        for &stat in &debugging.draw_calls_stat {
            let size = Size2D::new(ELEMENT_WIDTH, CONTAINER_SIZE.y * (stat as f32 / debugging.draw_calls_max as f32));

            context.draw_rect(
                container_origin + CONTAINER_SIZE.with_x(0.0) - size.with_x(-x),
                size,
                Color::from_hsl(110.0, 0.4, 0.7),
            );

            x += ELEMENT_WIDTH + SPACING;
        }

        let text = context
            .measure("default", format!("draw calls: {}\nvertices: {}", info.draw_calls, info.vertices), 9.0, None)
            .unwrap_or_default();

        context.draw_rect(container_origin + Point2D::splat(4.0), text, Color::from_u32_rgb(0x1D211B));
        context.draw_text(
            queue,
            container_origin + Point2D::splat(4.0),
            "default",
            format!("draw calls: {}\nvertices: {}", info.draw_calls, info.vertices),
            Color::from_hsl(110.0, 0.5, 0.8),
            9.0,
            None,
        );
    }

    fn render_fps_stat(queue: &wgpu::Queue, context: &mut CommonRenderer, debugging: &Debugging, delta: Duration, surface_size: USize2D) {
        const SPACING: f32 = 1.0;
        const SIZE: Size2D = Size2D::new(100.0 * (2.0 + SPACING), 96.0);
        const CONTAINER_SIZE: Size2D = Size2D::new(SIZE.x - SPACING, SIZE.y);
        const ELEMENT_WIDTH: f32 = (SIZE.x - 100.0 * SPACING) / 100.0;

        let bounds = Rect {
            origin: Point2D::ZERO,
            size: surface_size.as_vec2(),
        };

        context.draw_rect(
            bounds.origin + bounds.size.with_x(4.0) - CONTAINER_SIZE.with_x(0.0) - Point2D::new(0.0, 4.0),
            CONTAINER_SIZE,
            Color::from_u32_rgb(0x1D211B),
        );

        let mut x = 0.0;

        for stat in &debugging.fps_stat {
            let size = Size2D::new(ELEMENT_WIDTH, CONTAINER_SIZE.y * (stat.as_secs_f32() / debugging.fps_max.as_secs_f32()));

            context.draw_rect(
                bounds.origin + bounds.size.with_x(4.0 + x) - size.with_x(0.0) - Point2D::new(0.0, 4.0),
                size,
                Color::from_hsl(110.0, 0.4, 0.7),
            );

            x += ELEMENT_WIDTH + SPACING;
        }

        let text = context
            .measure(
                "default",
                format!("fps: {:.0} ({:.2}ms)", 1.0 / delta.as_secs_f32(), delta.as_secs_f32() * 1000.0),
                9.0,
                None,
            )
            .unwrap_or_default();

        context.draw_rect(
            bounds.origin + bounds.size.with_x(8.0) - CONTAINER_SIZE.with_x(0.0) - Point2D::new(0.0, 4.0),
            text,
            Color::from_u32_rgb(0x1D211B),
        );

        context.draw_text(
            queue,
            bounds.origin + bounds.size.with_x(8.0) - CONTAINER_SIZE.with_x(0.0) - Point2D::new(0.0, 4.0),
            "default",
            format!("fps: {:.0} ({:.2}ms)", 1.0 / delta.as_secs_f32(), delta.as_secs_f32() * 1000.0),
            Color::from_hsl(110.0, 0.5, 0.8),
            9.0,
            None,
        );
    }

    // #[allow(dead_code)]
    // pub fn chunk_borders(&self, white_pixel_uv: Point2D) -> Vec<CommonVertex> {
    //     self.chunk_manager.chunks().fold(Vec::new(), |mut lines, chunk| {
    //         let origin = chunk.origin.as_vec2() * SUBCHUNK_SIZE_F32;

    //         lines.extend(cube_outline(
    //             Cube3D::new(
    //                 Point3D::new(origin.x, 0.0, origin.y),
    //                 Size3D::new(SUBCHUNK_SIZE_F32, CHUNK_HEIGHT_F32,
    // SUBCHUNK_SIZE_F32),             ),
    //             white_pixel_uv,
    //         ));

    //         lines
    //     })
    // }
}

#[allow(clippy::type_complexity)]
struct WorldSnapshot<'a, C: ChunkAccess> {
    chunk_manager: &'a C,
    resource_storage: Arc<ResourceStorage>,
    calc_light_fn: fn(&C, &ResourceStorage, IPoint3D, IPoint3D, [[IPoint3D; 3]; 4], bool) -> ([f32; 4], [u8; 4]),
}

impl<'a, C: ChunkAccess> WorldSnapshot<'a, C> {
    const fn new(chunk_manager: &'a C, resource_storage: Arc<ResourceStorage>, settings: GraphicsSettings) -> Self {
        Self {
            chunk_manager,
            resource_storage,
            calc_light_fn: settings.light_style.get_light_fn::<C>(),
        }
    }

    pub fn compute_subchunk_mesh(&self, origin: IPoint2D, subchunk_idx: usize) -> [Vec<VoxelFace>; 2] {
        use std::cell::RefCell;

        thread_local! {
            static MESH_BUFFERS: RefCell<[Vec<VoxelFace>; 2]> = RefCell::new([
                Vec::with_capacity(const { SUBCHUNK_SIZE * SUBCHUNK_SIZE * SUBCHUNK_SIZE }),
                Vec::with_capacity(const { SUBCHUNK_SIZE * SUBCHUNK_SIZE * SUBCHUNK_SIZE }),
            ]);
        }

        MESH_BUFFERS.with_borrow_mut(|voxels| {
            let chunk = self.chunk_manager.get_chunk(origin).unwrap();
            let subchunk = &chunk.subchunks[subchunk_idx];

            for (local_position, block_id) in subchunk.iter(subchunk_idx) {
                let Some(state) = block_id else { continue };
                let model = self
                    .resource_storage
                    .models
                    .get_unchecked(self.resource_storage.blocks.get_model_by_name(state.id));

                let world_position = chunk.to_world(local_position);
                let biome = chunk.get_biome_unchecked(USizePoint2D::new(local_position.x, local_position.z));
                let (cull_if_same, tint_color): (bool, Option<Color>) = self
                    .resource_storage
                    .blocks
                    .get(state.id)
                    .map(|block| (block.cull_if_same(), block.tint_color(&self.resource_storage.color_config, biome)))
                    .unwrap_or_default();

                let neighbours = Face::NORMALS.map(|face| {
                    self.chunk_manager.get_block(world_position + face).filter(|b| !b.is_air()).map(|neighbour| {
                        (
                            cull_if_same && neighbour.id == state.id,
                            self.resource_storage
                                .models
                                .get_unchecked(self.resource_storage.blocks.get_model_by_name(neighbour.id)),
                        )
                    })
                });

                for element in &model.elements {
                    for model_face in &element.faces {
                        let culled = model_face.cull_face.as_ref().is_some_and(|&(cull_face_normal, _, _, opposite_face)| {
                            neighbours[cull_face_normal].is_some_and(|(culled, model)| model.is_opaque(opposite_face) || culled)
                        });

                        if !culled {
                            let (aos, mut lights) = (self.calc_light_fn)(
                                self.chunk_manager,
                                self.resource_storage.as_ref(),
                                world_position,
                                world_position + model_face.face_data.normal,
                                model_face.face_data.corners,
                                model.ambient_occlusion,
                            );

                            let mut vertices = model_face.face_data.vertices;
                            let mut uvs = model_face.face_data.uvs;

                            if aos[0] + aos[3] > aos[1] + aos[2] {
                                vertices.swap(0, 2); // ABCD => CBAD
                                vertices.swap(0, 1); // CBAD => BCAD
                                vertices.swap(1, 3); // BCAD => BDAC

                                uvs.swap(0, 2); // ABCD => CBAD
                                uvs.swap(0, 1); // CBAD => BCAD
                                uvs.swap(1, 3); // BCAD => BDAC

                                lights.swap(0, 2); // ABCD => CBAD
                                lights.swap(0, 1); // CBAD => BCAD
                                lights.swap(1, 3); // BCAD => BDAC
                            }

                            if model_face.is_opaque { &mut voxels[0] } else { &mut voxels[1] }.push(VoxelFace {
                                vertices,
                                position: local_position.as_vec3(),
                                lights,
                                uvs,
                                color: (if model_face.tint { tint_color.unwrap_or(Color::WHITE) } else { Color::WHITE })
                                    .multiply_rgb(model_face.face_data.face.get_light_level()),
                            });
                        }
                    }
                }
            }

            [voxels[0].drain(..).collect(), voxels[1].drain(..).collect()]
        })
    }
}
