use std::{
    collections::hash_map::{Iter, IterMut},
    path::PathBuf,
    sync::Arc,
    time::Instant,
};

use ahash::{HashMap, HashSet};
use horns::RenderBackend;
#[cfg(feature = "multiplayer")]
use meralus_network::{IncomingPacket, OutgoingPacket, Uuid};
use meralus_physics::{PhysicsBody, PhysicsContext};
use meralus_shared::{Color, Cube3D, Face, IPoint2D, IPoint3D, Point2D, Point3D, Ranged, Size3D, Transform3D, USizePoint3D, Vector3D};
use meralus_storage::Block;
use meralus_tween::{RepeatMode, Tween};
use meralus_world::{
    BfsLight, CHUNK_HEIGHT, CHUNK_HEIGHT_F32, Chunk, ChunkAccess, ChunkCache, ChunkManager, ChunkStage, LightNode, LocalChunkManager, SUBCHUNK_COUNT,
    SUBCHUNK_SIZE, SUBCHUNK_SIZE_F32,
};
use meralus_worldgen::ChunkGenerator;
use tracing::info;

use crate::{
    AabbProvider, Camera, FIXED_FRAMERATE, GraphicsSettings, INVENTORY_HOTBAR_SLOTS, Interval, Item, LimitedAabbProvider, PlayerController, ResourceStorage,
    TICK_RATE, TPS,
    clock::Clock,
    cube_outline,
    input::Input,
    player::ItemType,
    render::{
        chunk::{ChunkRenderer, VoxelFace, VoxelMeshBuilder},
        common::CommonVertex,
    },
};

const GRASS_COLOR: Color = Color::from_hsl(120.0, 0.4, 0.75);

#[non_exhaustive]
pub enum EntityData {
    Item { item: Item, transition: Tween<f32> },
    Model { id: usize, rotations: Vec<Vector3D> },
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
                rotations.resize(i + 1, Vector3D::ZERO);
            }

            rotations[i] = vector;
        }
    }

    pub fn render_to<C: ChunkCache>(&self, builder: &mut VoxelMeshBuilder, chunk_manager: &ChunkManager<C>, resource_storage: &ResourceStorage) {
        match &self.data {
            EntityData::Item { transition, item } => {
                let animation_value = transition.get_copy();
                let model = resource_storage.models.get_unchecked(resource_storage.blocks.get_model_by_name(&item.id));
                let mut current_block = self.body.position.floor().as_ivec3();
                let light = chunk_manager.get_light_level(current_block);
                let matrix = Transform3D::from_rotation_y(animation_value * const { 360f32.to_radians() });
                let animation_value = if animation_value > 0.5 { 1.0 - animation_value } else { animation_value };
                let position_offset = Point3D::new(0.0, const { Point3D::new(0.3, 0.3, 0.3).y / 2.0 }, 0.0);
                let block_below = loop {
                    if chunk_manager.get_block(current_block).is_some_and(|b| b.name != "game:air") {
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

                            if rotation.x > 0.0 {
                                matrix = matrix * Transform3D::from_rotation_x(rotation.x);
                            }

                            if rotation.y > 0.0 {
                                matrix = matrix * Transform3D::from_rotation_y(rotation.y);
                            }

                            if rotation.z > 0.0 {
                                matrix = matrix * Transform3D::from_rotation_z(rotation.z);
                            }

                            let center = element.cube.min.as_vec3() + element.cube.size().as_vec3() / 2.0;

                            face_data.vertices.map(|v| matrix.transform_point3(v - center) + center)
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

pub enum Weather {
    Clear,
    Rain,
    Thunder,
}

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
    Lightning {
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

    fn spawn_lightning_job(&self, origin: IPoint2D, mut chunk_manager: LocalChunkManager, resource_storage: Arc<ResourceStorage>) {
        let sender = self.sender.clone();

        rayon::spawn(move || {
            let instant = Instant::now();
            let mut bfs_light = BfsLight::new(&mut chunk_manager);

            let chunk = bfs_light.chunk_manager.get_chunk_mut(origin).unwrap();

            for z in 0..SUBCHUNK_SIZE {
                for x in 0..SUBCHUNK_SIZE {
                    for y in (0..CHUNK_HEIGHT).rev() {
                        let pos = USizePoint3D::new(x, y, z);

                        chunk.set_sky_light(pos, 15);

                        if pos.y > 0 && chunk.get_block(pos - USizePoint3D::Y).is_some() {
                            bfs_light.sky_addition_queue.push_back(LightNode(pos, origin));

                            break;
                        }
                    }
                }
            }

            bfs_light.calculate_sky_light(resource_storage.as_ref());

            let (chunk, neighbours) = chunk_manager.into_inner();

            _ = sender.send(JobResult::Lightning { chunk, neighbours });

            info!(target: "client/world", origin = ?origin, "Chunk lighted in {:?}", instant.elapsed());
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
                        .any(|block| block.name != "game:air")
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

pub struct World {
    pub clock: Clock,
    pub tick_interval: Interval,
    pub ticks: usize,
    pub tick_sum: usize,
    pub current_tick: usize,

    pub camera: Camera,
    pub player: PlayerController,
    pub player_controllable: bool,
    pub inventory_slot: Ranged<u8>,

    pub chunk_manager: ChunkManager<ChunkFileCache>,
    pub job_manager: JobManager,
    pub chunk_renderer: ChunkRenderer,

    pub current_weather: Weather,

    pub chat_history: Vec<String>,

    #[allow(dead_code)]
    pub ty: WorldType,
    pub resource_storage: Arc<ResourceStorage>,
    pub entities: EntityManager,

    pub seed: u32,
    pub marked: Option<IPoint3D>,
}

impl World {
    pub fn new(backend: &RenderBackend, resource_storage: Arc<ResourceStorage>, chunk_manager: ChunkManager<ChunkFileCache>, ty: WorldType) -> Self {
        let mut player = PlayerController::default();

        player.body.position = Point3D::new(2.0, 135.0, 2.0);

        Self {
            camera: Camera::new(player.camera_position()),
            ticks: 0,
            tick_sum: 0,
            current_tick: 0,
            chunk_renderer: ChunkRenderer::new(backend),
            tick_interval: Interval::new(TICK_RATE),
            player,
            player_controllable: false,
            inventory_slot: Ranged::new(0, 0, INVENTORY_HOTBAR_SLOTS),
            clock: Clock::default(),
            chunk_manager,
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
            && state.name != "game:air"
            && let Some(block) = self.resource_storage.get_block(&state.name)
        {
            if block.droppable() {
                self.entities.spawn_item(position.as_vec3() + Point3D::new(0.35, 0.0, 0.35), Item {
                    id: state.name.clone(),
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

    pub fn physics_step(&mut self, input: &Input) {
        let provider = AabbProvider {
            chunk_manager: &self.chunk_manager,
            entity_manager: &self.entities,
            storage: self.resource_storage.as_ref(),
        };

        let context = PhysicsContext::new(provider);
        let grounded = self.player.body.is_on_ground;

        context.physics_step(&mut self.player.body, FIXED_FRAMERATE.as_secs_f32());

        self.player.physics_step(input, &mut self.camera, FIXED_FRAMERATE.as_secs_f32());
        self.player.body.config.friction = if self.player.body.config.gravity_scale <= 1e-7 {
            8.0
        } else if !grounded {
            2.0
        } else {
            10.0
        };

        self.camera.set_position(&context, self.player.camera_position());

        let player_aabb = self.player.player_aabb();
        let mut remove_entities: Vec<usize> = Vec::new();

        let provider = LimitedAabbProvider {
            chunk_manager: &self.chunk_manager,
            storage: self.resource_storage.as_ref(),
        };

        let context = PhysicsContext::new(provider);

        for (id, entity) in &mut self.entities {
            context.physics_step(&mut entity.body, FIXED_FRAMERATE.as_secs_f32());

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
                self.player.inventory.try_insert(item);
            }
        }
    }

    pub const fn tick(&mut self, time_paused: bool) {
        self.tick_sum += 1;
        self.current_tick = self.tick_sum % TPS;

        if !time_paused {
            self.clock.tick();
        }

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

    pub fn update(&mut self, backend: &RenderBackend, settings: GraphicsSettings) {
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
                JobResult::Lightning { chunk, neighbours } => {
                    self.job_manager.jobs.remove(&chunk.origin);
                    self.chunk_manager.replace(chunk, ChunkStage::Lighted);

                    for chunk in neighbours {
                        self.job_manager.jobs.remove(&chunk.origin);

                        if self.chunk_manager.stages.get(&chunk.origin).is_some_and(|stage| stage >= &ChunkStage::Meshed) {
                            let chunk = unsafe { self.chunk_manager.get_chunk_mut(chunk.origin).unwrap_unchecked() };

                            chunk.dirty = true;
                        }
                    }
                }
                JobResult::Meshing { origin, mesh } => {
                    self.job_manager.jobs.remove(&origin);

                    self.chunk_manager.set_stage(origin, ChunkStage::Meshed);

                    for (subchunk_idx, mesh) in mesh.into_iter().rev().enumerate() {
                        let solid = VoxelMeshBuilder::build_from_slice(backend, &self.chunk_renderer.shader, &mesh[0]);
                        let translucent = VoxelMeshBuilder::build_from_slice(backend, &self.chunk_renderer.shader, &mesh[1]);

                        self.chunk_renderer.set_subchunk((origin, subchunk_idx), solid, translucent);
                    }
                }
            }
        }

        let mut queue = Vec::new();
        let origin = ChunkManager::<()>::to_local(self.player.body.position.as_ivec3());
        let mut chunks = (-10..10)
            .flat_map(|x| (-10..10).map(move |z| origin + IPoint2D::new(x, z)))
            .filter(|origin| self.chunk_manager.stages.contains_key(origin))
            .collect::<Vec<_>>();

        chunks.sort_unstable_by(|a, b| {
            origin
                .as_vec2()
                .distance_squared(a.as_vec2())
                .total_cmp(&origin.as_vec2().distance_squared(b.as_vec2()))
        });

        for origin in chunks {
            match unsafe { self.chunk_manager.stages.get(&origin).unwrap_unchecked() } {
                ChunkStage::Bare
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
                ChunkStage::Populated
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

                    self.job_manager.spawn_lightning_job(origin, chunk_manager, self.resource_storage.clone());

                    queue.push((origin, ChunkStage::LightningInProgress));
                }
                ChunkStage::Lighted
                    if self.chunk_manager.neighbours_at_least(origin, ChunkStage::Lighted)
                        && !self.job_manager.jobs.contains(&origin)
                        && self.chunk_manager.neighbours_of(origin).all(|origin| !self.job_manager.jobs.contains(&origin)) =>
                {
                    let chunk_manager = self.chunk_manager.local_of(origin).unwrap();

                    self.job_manager.jobs.insert(origin);
                    self.job_manager
                        .spawn_meshing_job(origin, chunk_manager, self.resource_storage.clone(), settings);

                    queue.push((origin, ChunkStage::MeshingInProgress));
                }
                ChunkStage::Meshed
                    if self.chunk_manager.neighbours_at_least(origin, ChunkStage::Lighted)
                        && self.chunk_manager.get_chunk(origin).is_some_and(|chunk| chunk.dirty)
                        && !self.job_manager.jobs.contains(&origin) =>
                {
                    let chunk_manager = self.chunk_manager.local_of(origin).unwrap();

                    self.job_manager.jobs.insert(origin);
                    self.job_manager
                        .spawn_meshing_job(origin, chunk_manager, self.resource_storage.clone(), settings);

                    queue.push((origin, ChunkStage::MeshingInProgress));
                }
                _ => {}
            }
        }

        for (origin, stage) in queue {
            self.chunk_manager.set_stage(origin, stage);

            if matches!(stage, ChunkStage::MeshingInProgress)
                && let Some(chunk) = self.chunk_manager.get_chunk_mut(origin)
            {
                chunk.dirty = false;
            }
        }

        if self.job_manager.jobs.is_empty() {
            let origin = ChunkManager::<()>::to_local(self.player.body.position.as_ivec3());
            let mut chunks = (-10..10)
                .flat_map(|x| (-10..10).map(move |z| origin + IPoint2D::new(x, z)))
                .filter(|&origin| self.chunk_manager.get_chunk(origin).is_none())
                .collect::<Vec<_>>();

            if !chunks.is_empty() {
                chunks.sort_unstable_by(|a, b| {
                    origin
                        .as_vec2()
                        .distance_squared(a.as_vec2())
                        .total_cmp(&origin.as_vec2().distance_squared(b.as_vec2()))
                });

                info!(
                    target: "client/world",
                    start = ?origin - IPoint2D::splat(16),
                    end = ?origin + IPoint2D::splat(16),
                    skipped = (32 * 32) - chunks.len(),
                    "Generating chunks"
                );

                for origin in chunks {
                    self.job_manager.spawn_generation_job(self.seed, origin, self.resource_storage.clone());
                    self.job_manager.jobs.insert(origin);
                }
            }
        }
    }

    pub fn chunk_borders(&self, white_pixel_uv: Point2D) -> Vec<CommonVertex> {
        self.chunk_manager.chunks().fold(Vec::new(), |mut lines, chunk| {
            let origin = chunk.origin.as_vec2() * SUBCHUNK_SIZE_F32;

            lines.extend(cube_outline(
                Cube3D::new(
                    Point3D::new(origin.x, 0.0, origin.y),
                    Size3D::new(SUBCHUNK_SIZE_F32, CHUNK_HEIGHT_F32, SUBCHUNK_SIZE_F32),
                ),
                white_pixel_uv,
            ));

            lines
        })
    }
}

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

    #[allow(clippy::too_many_lines)]
    pub fn compute_subchunk_mesh(&self, origin: IPoint2D, subchunk_idx: usize) -> [Vec<VoxelFace>; 2] {
        use std::cell::RefCell;

        thread_local! {
            static MESH_BUFFERS: RefCell<[Vec<VoxelFace>; 2]> = RefCell::new([
                Vec::with_capacity(const { (SUBCHUNK_SIZE * SUBCHUNK_SIZE * SUBCHUNK_SIZE) / 2 }),
                Vec::with_capacity(const { (SUBCHUNK_SIZE * SUBCHUNK_SIZE * SUBCHUNK_SIZE) / 2 }),
            ]);
        }

        let mut voxels = MESH_BUFFERS.take();

        voxels[0].clear();
        voxels[1].clear();

        let chunk = self.chunk_manager.get_chunk(origin).unwrap();
        let subchunk = &chunk.subchunks[subchunk_idx];

        for (local_position, block_id) in subchunk.iter(subchunk_idx) {
            if let Some((state, model)) = block_id.map(|state| {
                (
                    state,
                    self.resource_storage
                        .models
                        .get_unchecked(self.resource_storage.blocks.get_model_by_name(&state.name)),
                )
            }) {
                let world_position = chunk.to_world(local_position);
                let neighbours = Face::NORMALS.map(|face| self.chunk_manager.get_block(world_position + face).filter(|b| b.name != "game:air"));

                let (cull_if_same, tint_color): (bool, Option<Color>) = self
                    .resource_storage
                    .get_block(&state.name)
                    .map_or((false, None), |block: &dyn Block| (block.cull_if_same(), block.tint_color()));

                for element in &model.elements {
                    for model_face in &element.faces {
                        let culled = model_face.cull_face.as_ref().is_some_and(|(cull_face_normal, _, _, opposite_face)| {
                            neighbours[*cull_face_normal]
                                .map(|neighbour| {
                                    (
                                        neighbour,
                                        self.resource_storage
                                            .models
                                            .get_unchecked(self.resource_storage.blocks.get_model_by_name(&neighbour.name)),
                                    )
                                })
                                .is_some_and(|(neighbour, model)| model.is_opaque(*opposite_face) || (cull_if_same && neighbour.name == state.name))
                        });

                        if !culled {
                            // let mut lights = [1.0; 4];
                            let light_source: IPoint3D = world_position + model_face.face_data.normal;

                            let (aos, mut lights) = (self.calc_light_fn)(
                                self.chunk_manager,
                                self.resource_storage.as_ref(),
                                world_position,
                                light_source,
                                model_face.face_data.corners,
                                model.ambient_occlusion,
                            );

                            let mut vertices: [Point3D; 4] = model_face.face_data.vertices;
                            let mut uvs: [Point2D; 4] = model_face.face_data.uvs;

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
                                position: world_position.as_vec3(),
                                lights,
                                uvs,
                                color: (if model_face.tint { tint_color.unwrap_or(Color::WHITE) } else { Color::WHITE })
                                    .multiply_rgb(model_face.face_data.face.get_light_level()),
                            });
                        }
                    }
                }
            }
        }

        MESH_BUFFERS.replace(voxels.clone());

        voxels
    }
}
