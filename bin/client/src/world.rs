use std::{
    collections::hash_map::{Iter, IterMut},
    num::NonZero,
    sync::{
        Arc,
        mpsc::{self, TryRecvError},
    },
    thread::JoinHandle,
    time::Instant,
};

use ahash::HashMap;
use meralus_animation::{Curve, RepeatMode, Transition};
use meralus_engine::WindowDisplay;
use meralus_graphics::{CommonVertex, VoxelFace, VoxelMeshBuilder, VoxelRenderer};
#[cfg(feature = "multiplayer")]
use meralus_network::{IncomingPacket, OutgoingPacket, Uuid};
use meralus_physics::{PhysicsBody, PhysicsContext};
use meralus_shared::{Angle, Color, Cube3D, IPoint2D, IPoint3D, Point2D, Point3D, Ranged, Size3D, Transform3D, USizePoint3D, Vector3D};
use meralus_world::{
    BfsLight, CHUNK_HEIGHT, CHUNK_HEIGHT_F32, Chunk, ChunkGenerator, ChunkManager, Face, LightNode, SUBCHUNK_COUNT, SUBCHUNK_SIZE, SUBCHUNK_SIZE_F32, SubChunk,
};
use tracing::info;

use crate::{
    AabbProvider, Camera, Debugging, FIXED_FRAMERATE, INVENTORY_HOTBAR_SLOTS, Interval, Item, LimitedAabbProvider, PlayerController, ResourceStorage,
    TICK_RATE, TPS, clock::Clock, cube_outline, input::Input, player::ItemType, vertex_ao,
};

const GRASS_COLOR: Color = Color::from_hsl(120.0, 0.4, 0.75);

#[non_exhaustive]
pub enum EntityData {
    Item { item: Item, transition: Transition },
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
                transition: Transition::new(0.0, 1.0, 2000, Curve::LINEAR, RepeatMode::Infinite),
            },
        }
    }

    pub fn model(position: Point3D, id: usize, resource_storage: &ResourceStorage) -> Self {
        Self {
            body: PhysicsBody::new(position, resource_storage.entity_models.get_aabb(id as u8).unwrap().size().as_()),
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

    pub fn render_to(&self, builder: &mut VoxelMeshBuilder, chunk_manager: &ChunkManager, resource_storage: &ResourceStorage) {
        match &self.data {
            EntityData::Item { transition, item } => {
                let animation_value = transition.get::<f32>();
                let model = resource_storage.models.get_unchecked(item.id);
                let mut current_block: IPoint3D = self.body.position.floor().as_();
                let light = chunk_manager.get_light_level(current_block);
                let matrix = Transform3D::from_rotation_y(Angle::from_radians(animation_value * const { 360.0f32.to_radians() }));
                let animation_value = if animation_value > 0.5 { 1.0 - animation_value } else { animation_value };
                let position_offset = Point3D::new(0.0, const { Point3D::new(0.3, 0.3, 0.3).y / 2.0 }, 0.0);
                let block_below = loop {
                    if chunk_manager.get_block(current_block).filter(|&b| b != 0).is_some() {
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
                                    position: block_below.as_() + (Point3D::new(0.5, 0.0, 0.5) - size.with_y(0.0) / 2.0) + (Point3D::Y * 1.05),
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
                                position: self.body.position - (position_offset * animation_value).to_vector(),
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
                let half_size = model.bounding_box.size().as_::<f32>() / 2.0;

                for (i, element) in model.elements.iter().enumerate() {
                    for face_data in element.faces.as_ref() {
                        let vertices = rotations.get(i).map_or(face_data.vertices, |rotation| {
                            let mut matrix = Transform3D::IDENTITY;

                            if rotation.x > 0.0 {
                                matrix = matrix * Transform3D::from_rotation_x(Angle::from_radians(rotation.x));
                            }

                            if rotation.y > 0.0 {
                                matrix = matrix * Transform3D::from_rotation_y(Angle::from_radians(rotation.y));
                            }

                            if rotation.z > 0.0 {
                                matrix = matrix * Transform3D::from_rotation_z(Angle::from_radians(rotation.z));
                            }

                            let center = element.cube.min.as_() + element.cube.size().to_vector().as_::<f32>() / 2.0;

                            face_data.vertices.map(|v| matrix.transform_point3(v - center.to_vector()) + center)
                        });

                        builder.push(&VoxelFace {
                            position: self.body.position - half_size.to_vector(),
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

    pub chunk_manager: ChunkManager,
    pub chunk_receiver: mpsc::Receiver<Chunk>,
    pub chunk_sender: mpsc::Sender<Chunk>,
    pub chunk_mesh_receiver: mpsc::Receiver<(IPoint2D, usize)>,
    pub chunk_mesh_sender: mpsc::Sender<(IPoint2D, usize)>,
    pub voxel_renderer: VoxelRenderer,

    pub chat_history: Vec<String>,

    pub current_task: Option<JoinHandle<()>>,

    #[allow(dead_code)]
    pub ty: WorldType,
    pub resource_storage: Arc<ResourceStorage>,
    pub entities: EntityManager,

    pub seed: u32,
    pub marked: Option<IPoint3D>,
}

impl World {
    pub fn new(display: &WindowDisplay, resource_storage: Arc<ResourceStorage>, ty: WorldType) -> Self {
        let (chunk_sender, chunk_receiver) = mpsc::channel();
        let (chunk_mesh_sender, chunk_mesh_receiver) = mpsc::channel();
        let mut player = PlayerController::default();

        player.body.position = Point3D::new(2.0, 135.0, 2.0);

        Self {
            camera: Camera::new(player.camera_position()),
            ticks: 0,
            tick_sum: 0,
            current_tick: 0,
            voxel_renderer: VoxelRenderer::new(display),
            tick_interval: Interval::new(TICK_RATE),
            player,
            player_controllable: false,
            inventory_slot: Ranged::new(0, 0, INVENTORY_HOTBAR_SLOTS),
            clock: Clock::default(),
            chunk_manager: ChunkManager::new(),
            chunk_receiver,
            chunk_sender,
            chunk_mesh_receiver,
            chunk_mesh_sender,
            resource_storage: resource_storage.clone(),
            chat_history: Vec::new(),
            ty,
            entities: EntityManager::new(resource_storage),
            seed: 0,
            current_task: None,
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
            let origin = ChunkManager::to_local(position);

            #[cfg(feature = "multiplayer")]
            if let WorldType::Remote { sender, .. } = &self.ty {
                sender.send(IncomingPacket::RemoveBlock(origin, local)).unwrap();
            }

            self.destroy_block_local(origin, local);
        }
    }

    pub fn destroy_block_local(&mut self, origin: IPoint2D, local: USizePoint3D) {
        let position = Chunk::to_world_pos(origin, local);
        let [subchunk_idx, subchunk_y] = Chunk::get_subchunk_index(local.y);

        if let Some(block_id) = self.chunk_manager.get_block(position)
            && block_id != 0
            && let Some(block) = self.resource_storage.get_block(block_id.into())
        {
            if block.droppable() {
                self.entities.spawn_item(position.as_() + Point3D::new(0.35, 0.0, 0.35), Item {
                    id: block_id.into(),
                    ty: ItemType::Block,
                    amount: 1,
                });
            }

            let mut affected_chunks = self.chunk_manager.remove_block(position, self.resource_storage.as_ref());

            if let Some(chunks) = Chunk::corner(local) {
                for offset in chunks {
                    if !affected_chunks.contains(&(origin + offset)) {
                        affected_chunks.push(origin + offset);
                    }
                }
            } else if let Some(offset) = Chunk::side(local)
                && !affected_chunks.contains(&(origin + offset))
            {
                affected_chunks.push(origin + offset);
            }

            if subchunk_y == 0 && subchunk_idx > 0 {
                self.chunk_mesh_sender.send((origin, subchunk_idx - 1)).unwrap();
            } else if subchunk_y == const { SUBCHUNK_SIZE - 1 } && subchunk_idx < const { SUBCHUNK_COUNT - 1 } {
                self.chunk_mesh_sender.send((origin, subchunk_idx + 1)).unwrap();
            }

            for chunk in affected_chunks {
                if subchunk_idx < const { SUBCHUNK_COUNT - 1 } {
                    self.chunk_mesh_sender.send((chunk, subchunk_idx + 1)).unwrap();
                }

                self.chunk_mesh_sender.send((chunk, subchunk_idx)).unwrap();

                if subchunk_idx > 0 {
                    self.chunk_mesh_sender.send((chunk, subchunk_idx - 1)).unwrap();
                }
            }

            self.chunk_mesh_sender.send((origin, subchunk_idx)).unwrap();
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

    pub fn update(&mut self, white_pixel_uv: Point2D, debugging: &mut Debugging) {
        if let Ok((origin, subchunk_idx)) = self.chunk_mesh_receiver.try_recv()
            && let Some(chunk) = self.chunk_manager.get_chunk(&origin)
        {
            let subchunk = &chunk.subchunks[subchunk_idx];
            let mesh = Self::compute_subchunk_mesh(&self.chunk_manager, self.resource_storage.as_ref(), chunk, subchunk, subchunk_idx, self.marked);

            self.voxel_renderer.set_subchunk((origin, subchunk_idx), mesh);
        }

        match self.chunk_receiver.try_recv() {
            Ok(chunk) => {
                let origin = chunk.origin;

                self.chunk_manager.push(chunk);

                let instant = Instant::now();

                let mut bfs_light = BfsLight::new(&mut self.chunk_manager).apply_to_sky_light();

                bfs_light.starting_chunk = Some(origin);

                let chunk = bfs_light.chunk_manager.get_chunk_mut(&origin).unwrap();

                for z in 0..SUBCHUNK_SIZE {
                    for x in 0..SUBCHUNK_SIZE {
                        for y in (0..CHUNK_HEIGHT).rev() {
                            let pos = USizePoint3D::new(x, y, z);

                            chunk.set_sky_light(pos, 15);

                            if pos.y > 0 && chunk.check_for_local_block(pos - USizePoint3D::Y) {
                                bfs_light.addition_queue.push(LightNode(pos, origin));

                                break;
                            }
                        }
                    }
                }

                bfs_light.calculate(self.resource_storage.as_ref());

                info!(target: "client/world", ?origin, "Chunk light calculacted in {:?}", instant.elapsed());

                ChunkGenerator::new(self.seed.into()).populate(&mut self.chunk_manager, self.resource_storage.as_ref(), self.seed.into(), origin);

                for (offset, chunk) in [
                    IPoint2D::new(-1, 1),
                    IPoint2D::new(0, 1),
                    IPoint2D::new(1, 1),
                    IPoint2D::new(-1, 0),
                    IPoint2D::new(0, 0),
                    IPoint2D::new(1, 0),
                    IPoint2D::new(-1, -1),
                    IPoint2D::new(0, -1),
                    IPoint2D::new(1, -1),
                ]
                .into_iter()
                .filter_map(|offset| self.chunk_manager.get_chunk(&(origin + offset)).map(|chunk| (offset, chunk)))
                {
                    let instant = Instant::now();

                    for subchunk_idx in (0..SUBCHUNK_COUNT).rev() {
                        let subchunk = &chunk.subchunks[subchunk_idx];

                        if subchunk.blocks.iter().any(|block| block != &0) {
                            let mesh =
                                Self::compute_subchunk_mesh(&self.chunk_manager, self.resource_storage.as_ref(), chunk, subchunk, subchunk_idx, self.marked);

                            self.voxel_renderer.set_subchunk((chunk.origin, subchunk_idx), mesh);
                        } else {
                            self.voxel_renderer.set_subchunk((chunk.origin, subchunk_idx), [Vec::new(), Vec::new()]);
                        }
                    }

                    info!(target: "client/world", origin = ?chunk.origin, ?offset, "Chunk meshed in {:?}", instant.elapsed());
                }

                debugging.chunk_borders = self.chunk_borders(white_pixel_uv);
            }
            Err(TryRecvError::Empty) if self.current_task.as_ref().is_none_or(JoinHandle::is_finished) => {
                let origin = ChunkManager::to_local(self.player.body.position.as_());
                let mut chunks = (-4..4)
                    .flat_map(|x| (-4..4).map(move |z| origin + IPoint2D::new(x, z)))
                    .filter(|origin| self.chunk_manager.get_chunk(origin).is_none())
                    .collect::<Vec<_>>();

                if !chunks.is_empty() {
                    chunks.sort_unstable_by(|a, b| {
                        origin
                            .as_::<f32>()
                            .distance_squared(a.as_::<f32>())
                            .total_cmp(&origin.as_::<f32>().distance_squared(b.as_::<f32>()))
                    });

                    let sender = self.chunk_sender.clone();
                    let resource_storage = self.resource_storage.clone();
                    let seed = self.seed;

                    info!(
                        target: "client/world",
                        start = ?origin - IPoint2D::splat(16),
                        end = ?origin + IPoint2D::splat(16),
                        skipped = (32 * 32) - chunks.len(),
                        "Generating chunks"
                    );

                    self.current_task.replace(std::thread::spawn(move || {
                        let generator = ChunkGenerator::new(i64::from(seed));

                        for origin in chunks {
                            let mut chunk = Chunk::new(origin);

                            generator.generate_unpopulated_chunk_data(&mut chunk, resource_storage.as_ref());

                            sender.send(chunk).unwrap();
                        }
                    }));
                }
            }
            _ => (),
        }
    }

    pub fn chunk_borders(&self, white_pixel_uv: Point2D) -> Vec<CommonVertex> {
        self.chunk_manager.chunks().fold(Vec::new(), |mut lines, Chunk { origin, .. }| {
            let origin = origin.as_::<f32>() * SUBCHUNK_SIZE_F32;

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

    pub fn start_world_generation(&mut self, seed: u32) {
        self.seed = seed;

        let total = self.chunk_manager.len();

        let threads = std::thread::available_parallelism().map(NonZero::get).unwrap_or(1);
        let chunks_for_thread = self.chunk_manager.len() / threads;
        let chunks_for_thread = if chunks_for_thread * threads == total {
            chunks_for_thread
        } else {
            chunks_for_thread + 1
        };

        let mut iter = self.chunk_manager.chunks().map(|chunk| chunk.origin);

        for _ in 0..threads {
            let sender = self.chunk_sender.clone();
            let chunks = iter.by_ref().take(chunks_for_thread).collect::<Vec<_>>();
            let resource_storage = self.resource_storage.clone();

            std::thread::spawn(move || {
                let generator = ChunkGenerator::new(i64::from(seed));

                for origin in chunks {
                    let mut chunk = Chunk::new(origin);

                    generator.generate_unpopulated_chunk_data(&mut chunk, resource_storage.as_ref());

                    sender.send(chunk).unwrap();
                }
            });
        }
    }

    pub fn compute_subchunk_mesh_at(&self, (position, subchunk_idx): (IPoint2D, usize)) -> Option<[Vec<VoxelFace>; 2]> {
        self.chunk_manager.get_chunk(&position).map(|chunk| {
            Self::compute_subchunk_mesh(
                &self.chunk_manager,
                self.resource_storage.as_ref(),
                chunk,
                &chunk.subchunks[subchunk_idx],
                subchunk_idx,
                self.marked,
            )
        })
    }

    fn does_block_have_ao(chunk_manager: &ChunkManager, resource_storage: &ResourceStorage, position: IPoint3D) -> bool {
        chunk_manager
            .get_block(position)
            .filter(|&b| b != 0)
            .map(|block| resource_storage.models.get_unchecked(block.into()))
            .is_some_and(|block| block.ambient_occlusion)
    }

    #[allow(clippy::too_many_lines)]
    pub fn compute_subchunk_mesh(
        chunk_manager: &ChunkManager,
        resource_storage: &ResourceStorage,
        chunk: &Chunk,
        subchunk: &SubChunk,
        subchunk_idx: usize,
        #[allow(unused_variables)] marked: Option<IPoint3D>,
    ) -> [Vec<VoxelFace>; 2] {
        let mut voxels = [
            Vec::with_capacity(const { (SUBCHUNK_SIZE * SUBCHUNK_SIZE * SUBCHUNK_SIZE) / 2 }),
            Vec::with_capacity(const { (SUBCHUNK_SIZE * SUBCHUNK_SIZE * SUBCHUNK_SIZE) / 2 }),
        ];

        for (local_position, block_id) in subchunk.iter(subchunk_idx) {
            if let Some((block_id, model)) = block_id.map(|block_id| (block_id, resource_storage.models.get_unchecked(block_id.into()))) {
                let world_position = chunk.to_world(local_position);
                let neighbours = Face::NORMALS.map(|face| chunk_manager.get_block(world_position + face).filter(|&b| b != 0));

                // not visible
                if model.is_opaque
                    && neighbours
                        .iter()
                        .all(|neighbour| neighbour.is_some_and(|neighbour| resource_storage.models.get_unchecked(neighbour.into()).is_opaque))
                {
                    continue;
                }

                let (cull_if_same, tint_color) = resource_storage
                    .get_block(block_id.into())
                    .map_or((false, None), |block| (block.cull_if_same(), block.tint_color()));

                for element in &model.elements {
                    for model_face in &element.faces {
                        let culled = model_face.cull_face.as_ref().is_some_and(|(cull_face_normal, _, _, opposite_face)| {
                            neighbours[*cull_face_normal]
                                .map(|neighbour| (neighbour, resource_storage.models.get_unchecked(neighbour.into())))
                                .is_some_and(|(neighbour, model)| model.is_opaque(*opposite_face) || (cull_if_same && neighbour == block_id))
                        });

                        if !culled {
                            // let mut lights = [1.0; 4];
                            let mut aos = [1.0; 4];
                            let light_source = world_position + model_face.face_data.normal;
                            let mut lights = [0; 4];

                            for (([side1, side2, corner], ao), light) in model_face.face_data.corners.into_iter().zip(&mut aos).zip(&mut lights) {
                                let side1_block = world_position + side1;
                                let side2_block = world_position + side2;
                                let corner_block = world_position + corner;

                                let init_light = chunk_manager.get_block_light(light_source);
                                let side1_light = chunk_manager.get_block_light(side1_block);
                                let side2_light = chunk_manager.get_block_light(side2_block);
                                let corner_light = chunk_manager.get_block_light(corner_block);

                                *light = (*light & 0xF0) | ((init_light + side1_light + side2_light + corner_light) / 4);

                                let init_light = chunk_manager.get_sky_light(light_source);
                                let side1_light = chunk_manager.get_sky_light(side1_block);
                                let side2_light = chunk_manager.get_sky_light(side2_block);
                                let corner_light = chunk_manager.get_sky_light(corner_block);

                                *light = (*light & 0xF) | ((init_light + side1_light + side2_light + corner_light) / 4) << 4;

                                *ao = vertex_ao(
                                    Self::does_block_have_ao(chunk_manager, resource_storage, side1_block),
                                    Self::does_block_have_ao(chunk_manager, resource_storage, side2_block),
                                    Self::does_block_have_ao(chunk_manager, resource_storage, corner_block),
                                );
                            }

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
                                position: world_position.as_(),
                                lights,
                                uvs,
                                color: if model_face.tint { tint_color.unwrap_or(Color::WHITE) } else { Color::WHITE },
                            });
                        }
                    }
                }
            }
        }

        voxels
    }
}
