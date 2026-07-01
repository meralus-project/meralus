use std::{
    ops::{Index, IndexMut},
    sync::Arc,
};

use ahash::HashMap;
use mavelin_shared::Face;

use crate::{BfsLight, Biome, BlockSource, Chunk, LightNode, SUBCHUNK_COUNT_I32, SUBCHUNK_SIZE, SUBCHUNK_SIZE_I32, chunk::SubChunkBlockState};

pub trait ChunkCache {
    fn all(&self) -> impl Iterator<Item = Chunk>;
    fn get(&self, origin: glam::IVec2) -> Option<Chunk>;
    fn insert(&mut self, origin: glam::IVec2, chunk: &Chunk);
}

impl ChunkCache for () {
    fn all(&self) -> impl Iterator<Item = Chunk> {
        std::iter::empty()
    }

    fn get(&self, _: glam::IVec2) -> Option<Chunk> {
        None
    }

    fn insert(&mut self, _: glam::IVec2, _: &Chunk) {}
}

pub trait ChunkAccess {
    fn get_chunk(&self, origin: glam::IVec2) -> Option<&Arc<Chunk>>;
    fn get_chunk_mut(&mut self, origin: glam::IVec2) -> Option<&mut Chunk>;
    fn get_block(&self, position: glam::IVec3) -> Option<&SubChunkBlockState>;
    fn set_block(&mut self, position: glam::IVec3, block: SubChunkBlockState);
    fn get_block_light(&self, position: glam::IVec3) -> u8;
    fn get_sky_light(&self, position: glam::IVec3) -> u8;
    fn get_light_level(&self, position: glam::IVec3) -> u8;
    fn get_block_with_light_level(&self, position: glam::IVec3) -> (Option<&SubChunkBlockState>, u8);
    fn get_chunk_by_block(&self, position: glam::IVec3) -> Option<&Arc<Chunk>>;
    fn get_chunk_by_block_mut(&mut self, position: glam::IVec3) -> Option<&mut Chunk>;
    fn get_local_block_light(&self, origin: glam::IVec2, position: glam::USizeVec3) -> u8;
    fn set_local_block_light(&mut self, origin: glam::IVec2, position: glam::USizeVec3, light: u8);
    fn get_local_sky_light(&self, origin: glam::IVec2, position: glam::USizeVec3) -> u8;
    fn set_local_sky_light(&mut self, origin: glam::IVec2, position: glam::USizeVec3, light: u8);
    fn get_local_light(&self, origin: glam::IVec2, position: glam::USizeVec3, is_sky_light: bool) -> u8;
    fn set_local_light(&mut self, origin: glam::IVec2, position: glam::USizeVec3, is_sky_light: bool, light: u8);
}

pub struct LocalChunkManager {
    origin: glam::IVec2,
    chunks: [Arc<Chunk>; 9],
}

impl LocalChunkManager {
    pub fn new<C: ChunkCache>(chunk: Arc<Chunk>, chunk_manager: &ChunkManager<C>) -> Self {
        let origin = chunk.origin;

        let get_chunk = |offset| {
            chunk_manager
                .get_chunk(origin + offset)
                .cloned()
                .unwrap_or_else(|| Arc::new(Chunk::empty().with_origin(origin + offset)))
        };

        Self {
            origin,
            chunks: [
                get_chunk(glam::IVec2::new(-1, -1)),
                get_chunk(glam::IVec2::new(0, -1)),
                get_chunk(glam::IVec2::new(1, -1)),
                get_chunk(glam::IVec2::new(-1, 0)),
                chunk,
                get_chunk(glam::IVec2::new(1, 0)),
                get_chunk(glam::IVec2::new(-1, 1)),
                get_chunk(glam::IVec2::new(0, 1)),
                get_chunk(glam::IVec2::new(1, 1)),
            ],
        }
    }

    pub fn into_inner(self) -> (Arc<Chunk>, [Arc<Chunk>; 8]) {
        let [
            neg_x_neg_y,
            zer_x_neg_y,
            pos_x_neg_y,
            neg_x_zer_y,
            center,
            pos_x_zer_y,
            neg_x_pos_y,
            zer_x_pos_y,
            pos_x_pos_y,
        ] = self.chunks;

        (center, [
            neg_x_neg_y,
            zer_x_neg_y,
            pos_x_neg_y,
            neg_x_zer_y,
            pos_x_zer_y,
            neg_x_pos_y,
            zer_x_pos_y,
            pos_x_pos_y,
        ])
    }
}

impl ChunkAccess for LocalChunkManager {
    #[inline]
    fn get_chunk(&self, origin: glam::IVec2) -> Option<&Arc<Chunk>> {
        self.chunks
            .get((origin.x - self.origin.x + 1) as usize + (origin.y - self.origin.y + 1) as usize * 3)
    }

    #[inline]
    fn get_chunk_mut(&mut self, origin: glam::IVec2) -> Option<&mut Chunk> {
        self.chunks
            .get_mut((origin.x - self.origin.x + 1) as usize + (origin.y - self.origin.y + 1) as usize * 3)
            .map(Arc::make_mut)
    }

    #[inline]
    fn get_block(&self, position: glam::IVec3) -> Option<&SubChunkBlockState> {
        if position.y >= 0 && position.y < const { SUBCHUNK_SIZE_I32 * SUBCHUNK_COUNT_I32 } {
            let chunk = self.get_chunk(ChunkManager::<()>::to_local(position))?;

            Some(chunk.get_block_unchecked(Chunk::to_local(position)))
        } else {
            None
        }
    }

    #[inline]
    fn set_block(&mut self, position: glam::IVec3, block: SubChunkBlockState) {
        if position.y >= 0
            && position.y < const { SUBCHUNK_SIZE_I32 * SUBCHUNK_COUNT_I32 }
            && let Some(chunk) = self.get_chunk_mut(ChunkManager::<()>::to_local(position))
        {
            chunk.set_block_unchecked(Chunk::to_local(position), block);
        }
    }

    #[inline]
    fn get_block_light(&self, position: glam::IVec3) -> u8 {
        self.get_chunk(ChunkManager::<()>::to_local(position)).map_or(15, |chunk| {
            if position.y >= 0 && position.y < const { SUBCHUNK_SIZE_I32 * SUBCHUNK_COUNT_I32 } {
                chunk.get_block_light_unchecked(Chunk::to_local(position))
            } else {
                15
            }
        })
    }

    #[inline]
    fn get_sky_light(&self, position: glam::IVec3) -> u8 {
        self.get_chunk(ChunkManager::<()>::to_local(position)).map_or(15, |chunk| {
            let local_position = Chunk::to_local(position);

            if position.y >= 0 && position.y < const { SUBCHUNK_SIZE_I32 * SUBCHUNK_COUNT_I32 } {
                chunk.get_sky_light_unchecked(local_position)
            } else {
                15
            }
        })
    }

    #[inline]
    fn get_light_level(&self, position: glam::IVec3) -> u8 {
        self.get_chunk(ChunkManager::<()>::to_local(position)).map_or(240, |chunk| {
            if position.y >= 0 && position.y < const { SUBCHUNK_SIZE_I32 * SUBCHUNK_COUNT_I32 } {
                chunk.get_light_level_unchecked(Chunk::to_local(position))
            } else {
                240
            }
        })
    }

    #[inline]
    fn get_block_with_light_level(&self, position: glam::IVec3) -> (Option<&SubChunkBlockState>, u8) {
        if position.y >= 0
            && position.y < const { SUBCHUNK_SIZE_I32 * SUBCHUNK_COUNT_I32 }
            && let Some(chunk) = self.get_chunk(ChunkManager::<()>::to_local(position))
        {
            let block = chunk.get_block_unchecked(Chunk::to_local(position));
            let light_level = chunk.get_light_level_unchecked(Chunk::to_local(position));

            (Some(block), light_level)
        } else {
            (None, 240)
        }
    }

    #[inline]
    fn get_local_block_light(&self, origin: glam::IVec2, position: glam::USizeVec3) -> u8 {
        self.get_chunk(origin).map_or(0, |chunk| chunk.get_block_light(position))
    }

    #[inline]
    fn set_local_block_light(&mut self, origin: glam::IVec2, position: glam::USizeVec3, light_level: u8) {
        if let Some(chunk) = self.get_chunk_mut(origin) {
            chunk.set_block_light(position, light_level);
        }
    }

    #[inline]
    fn get_local_sky_light(&self, origin: glam::IVec2, position: glam::USizeVec3) -> u8 {
        self.get_chunk(origin).map_or(0, |chunk| chunk.get_sky_light(position))
    }

    #[inline]
    fn set_local_sky_light(&mut self, origin: glam::IVec2, position: glam::USizeVec3, light_level: u8) {
        if let Some(chunk) = self.get_chunk_mut(origin) {
            chunk.set_sky_light(position, light_level);
        }
    }

    #[inline]
    fn get_local_light(&self, origin: glam::IVec2, position: glam::USizeVec3, is_sky_light: bool) -> u8 {
        self.get_chunk(origin).map_or(0, |chunk| chunk.get_light(position, is_sky_light))
    }

    #[inline]
    fn set_local_light(&mut self, origin: glam::IVec2, position: glam::USizeVec3, is_sky_light: bool, light_level: u8) {
        if let Some(chunk) = self.get_chunk_mut(origin) {
            chunk.set_light(position, is_sky_light, light_level);
        }
    }

    #[inline]
    fn get_chunk_by_block(&self, position: glam::IVec3) -> Option<&Arc<Chunk>> {
        self.get_chunk(ChunkManager::<()>::to_local(position))
    }

    #[inline]
    fn get_chunk_by_block_mut(&mut self, position: glam::IVec3) -> Option<&mut Chunk> {
        self.get_chunk_mut(ChunkManager::<()>::to_local(position))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChunkStage {
    Unloaded,
    GenerationInProgress,
    Bare,
    PopulationInProgress,
    Populated,
    LightingInProgress,
    Lighted,
    MeshingInProgress,
    Meshed,
}

#[derive(Debug, Clone)]
pub struct ChunkManager<C: ChunkCache> {
    cache: C,
    pub chunks: HashMap<glam::IVec2, Arc<Chunk>>,
    pub stages: HashMap<glam::IVec2, ChunkStage>,
}

impl<C: ChunkCache> ChunkManager<C> {
    pub fn new(cache: C) -> Self {
        let chunks: HashMap<glam::IVec2, _> = cache.all().map(|chunk| (chunk.origin, Arc::new(chunk))).collect();
        let stages = chunks.keys().map(|&origin| (origin, ChunkStage::Lighted)).collect();

        Self { cache, chunks, stages }
    }

    pub fn push(&mut self, chunk: Chunk, stage: ChunkStage) {
        self.stages.insert(chunk.origin, stage);
        self.chunks.insert(chunk.origin, Arc::new(chunk));
    }

    pub fn replace(&mut self, chunk: Arc<Chunk>, stage: ChunkStage) {
        self.stages.insert(chunk.origin, stage);
        self.chunks.insert(chunk.origin, chunk);
    }

    pub fn set_stage(&mut self, origin: glam::IVec2, stage: ChunkStage) {
        self.stages.insert(origin, stage);
    }

    // pub fn stages(&self) -> impl Iterator<Item = (glam::IVec2, ChunkStage)> {
    //   self.stages.iter().map(|(&key, &stage)| (key, stage))
    // }

    // pub fn stages_mut(&mut self) -> impl Iterator<Item = (glam::IVec2, &mut
    // ChunkStage)> {   self.stages.iter_mut().map(|(&key, stage)| (key, stage))
    // }
    pub fn neighbours_of(&self, origin: glam::IVec2) -> impl Iterator<Item = glam::IVec2> {
        [
            glam::IVec2::NEG_X,                // left
            glam::IVec2::X,                    // right
            glam::IVec2::Y,                    // top
            glam::IVec2::NEG_Y,                // bottom
            const { glam::IVec2::new(-1, 1) }, // top left
            glam::IVec2::ONE,                  // top right
            glam::IVec2::NEG_ONE,              // bottom left
            const { glam::IVec2::new(1, -1) }, // bottom right
        ]
        .into_iter()
        .map(move |offset| origin + offset)
    }

    pub fn neighbours_at_least(&self, origin: glam::IVec2, stage: ChunkStage) -> bool {
        self.neighbours_of(origin)
            .all(|inner_origin| self.stages.get(&inner_origin).is_some_and(|&chunk_stage| chunk_stage >= stage))
    }

    pub fn update_neighbors(&mut self, center_origin: glam::IVec2, modified_neighbors: [Option<Chunk>; 8], stage: ChunkStage) {
        let offsets = [
            glam::IVec2::new(-1, 0),  // left
            glam::IVec2::new(1, 0),   // right
            glam::IVec2::new(0, 1),   // top
            glam::IVec2::new(0, -1),  // bottom
            glam::IVec2::new(-1, 1),  // top left
            glam::IVec2::new(1, 1),   // top right
            glam::IVec2::new(-1, -1), // bottom left
            glam::IVec2::new(1, -1),  // bottom right
        ];

        for (offset, neighbor_opt) in offsets.into_iter().zip(modified_neighbors) {
            if neighbor_opt.is_some() {
                let neighbor_origin = center_origin + offset;

                // if let Some(existing_chunk) = self.chunks.get_mut(&neighbor_origin) {
                //   for i in 0..SUBCHUNK_COUNT {
                //     if existing_chunk.subchunks[i] != modified_neighbor.subchunks[i] {
                //       existing_chunk.subchunks[i] = modified_neighbor.subchunks[i];
                //     }
                //   }
                // }

                self.stages.insert(neighbor_origin, stage);
            }
        }
    }

    pub fn local_of(&self, origin: glam::IVec2) -> Option<LocalChunkManager> {
        self.chunks.get(&origin).cloned().map(|chunk| LocalChunkManager::new(chunk, self))
    }

    pub fn from_range<T: Iterator<Item = i32> + Clone>(mut cache: C, x: T, z: &T) -> Self {
        let chunks: HashMap<glam::IVec2, _> = x
            .flat_map(|x| z.clone().map(move |z| glam::IVec2::new(x, z)))
            .map(|origin| {
                let chunk = cache.get(origin).unwrap_or_else(|| {
                    let chunk = Chunk::new(origin);

                    cache.insert(origin, &chunk);

                    chunk
                });

                (origin, Arc::new(chunk))
            })
            .collect();

        let stages = chunks.keys().map(|&origin| (origin, ChunkStage::Unloaded)).collect();

        Self { cache, chunks, stages }
    }

    // pub fn from_chunks<T: IntoIterator<Item = Chunk>>(cache: C, chunks: T) ->
    // Self {   Self {
    //     cache,
    //     chunks: chunks.into_iter().map(|chunk| (chunk.origin, chunk)).collect(),
    //   }
    // }

    pub fn generate_sky_lights<T: BlockSource>(&mut self, block_source: &T) {
        let mut queue = Vec::new();

        for chunk in self.chunks_mut() {
            for z in 0..SUBCHUNK_SIZE {
                for x in 0..SUBCHUNK_SIZE {
                    let position = glam::USizeVec3::new(x, 255, z);

                    if chunk.get_block_unchecked(position).is_air()
                    //_or(|block| !resource_manager.read().models.get(block.into()).unwrap().is_opaque)
                    {
                        chunk.set_sky_light(position, 15);
                        queue.push((LightNode(position, chunk.origin), 15));
                    }
                }
            }
        }

        let mut bfs_light = BfsLight::new(self);

        bfs_light.sky_addition_queue = queue.into();
        bfs_light.calculate_sky_light(block_source);
    }

    pub fn surface_size(&self) -> glam::IVec3 {
        let mut min = glam::IVec2::ZERO;
        let mut max = glam::IVec2::ZERO;

        for chunk in self.chunks.keys() {
            min = min.min(*chunk);
            max = max.max(*chunk);
        }

        let size = (max - min) * 16;

        glam::IVec3::new(size.x + SUBCHUNK_SIZE_I32, SUBCHUNK_SIZE_I32 * SUBCHUNK_COUNT_I32, size.y + SUBCHUNK_SIZE_I32)
    }

    pub fn bounds(&self) -> (glam::IVec2, glam::IVec2) {
        let mut min = glam::IVec2::ZERO;
        let mut max = glam::IVec2::ZERO;

        for chunk in self.chunks.keys() {
            min = min.min(*chunk);
            max = max.max(*chunk);
        }

        (min * SUBCHUNK_SIZE_I32, max * SUBCHUNK_SIZE_I32)
    }

    pub const fn to_local(position: glam::IVec3) -> glam::IVec2 {
        glam::IVec2::new(position.x >> 4, position.z >> 4)
    }

    pub fn to_chunk_local(&self, position: glam::IVec3) -> Option<glam::USizeVec3> {
        self.get_chunk(Self::to_local(position)).map(|_| Chunk::to_local(position))
    }

    pub fn get_biome(&self, position: glam::IVec3) -> Option<Biome> {
        if position.y >= 0 && position.y < const { SUBCHUNK_SIZE_I32 * SUBCHUNK_COUNT_I32 } {
            let chunk = self.get_chunk(Self::to_local(position))?;
            let local = Chunk::to_local(position);

            Some(chunk.get_biome_unchecked(glam::USizeVec2::new(local.x, local.z)))
        } else {
            None
        }
    }

    pub fn remove_block<T: BlockSource>(&mut self, position: glam::IVec3, block_source: &T) {
        let chunk_position = Self::to_local(position);

        if let Some(chunk) = self.get_chunk_mut(chunk_position) {
            let local = Chunk::to_local(position);

            chunk.set_block(local, SubChunkBlockState::air());
            chunk.dirty = true;

            for normal in Face::NORMALS {
                let chunk = Self::to_local(position + normal);

                if chunk != chunk_position
                    && let Some(chunk) = self.get_chunk_mut(chunk)
                {
                    chunk.dirty = true;
                }
            }

            for normal in [
                glam::IVec3::NEG_ONE,
                glam::IVec3::NEG_ONE.with_x(1),
                glam::IVec3::ONE.with_x(-1),
                glam::IVec3::ONE,
            ] {
                let chunk = Self::to_local(position + normal);

                if chunk != chunk_position
                    && let Some(chunk) = self.get_chunk_mut(chunk)
                {
                    chunk.dirty = true;
                }
            }

            let mut bfs_light = BfsLight::new(self);

            bfs_light.remove_block(LightNode(local, chunk_position));
            bfs_light.calculate_block_light(block_source);
            bfs_light.remove_sky(LightNode(local, chunk_position));
            bfs_light.calculate_sky_light(block_source);

            let up = local + glam::USizeVec3::Y;

            if up.y < 256 && bfs_light.chunk_manager[chunk_position].get_sky_light(up) == 15 {
                let mut y = local.y;

                loop {
                    if bfs_light.chunk_manager[chunk_position].get_block(local.with_y(y)).is_some_and(|b| !b.is_air()) {
                        break;
                    }

                    bfs_light.add_sky_custom(LightNode(local.with_y(y), chunk_position), 15);

                    if y == 0 {
                        break;
                    }

                    y -= 1;
                }
            }

            bfs_light.calculate_sky_light(block_source);
        }
    }

    pub fn set_block_light(&mut self, position: glam::IVec3, light_level: u8) {
        if let Some(chunk) = self.get_chunk_mut(Self::to_local(position)) {
            chunk.set_block_light(Chunk::to_local(position), light_level);
        }
    }

    pub fn set_sky_light(&mut self, position: glam::IVec3, light_level: u8) {
        if let Some(chunk) = self.get_chunk_mut(Self::to_local(position)) {
            chunk.set_sky_light(Chunk::to_local(position), light_level);
        }
    }

    pub fn set_light(&mut self, position: glam::IVec3, is_sky_light: bool, light_level: u8) {
        if let Some(chunk) = self.get_chunk_mut(Self::to_local(position)) {
            chunk.set_light(Chunk::to_local(position), is_sky_light, light_level);
        }
    }

    pub fn contains_block(&self, position: glam::IVec3) -> bool {
        self.get_chunk(Self::to_local(position)).is_some_and(|chunk| chunk.check_for_block(position))
    }

    pub fn contains_chunk(&self, origin: &glam::IVec2) -> bool {
        self.chunks.contains_key(origin)
    }

    pub fn get_light(&self, position: glam::IVec3, is_sky_light: bool) -> u8 {
        self.get_chunk(Self::to_local(position)).map_or(15, |chunk| {
            let local_position = Chunk::to_local(position);

            if chunk.contains_local_position(local_position) {
                chunk.get_light(local_position, is_sky_light)
            } else {
                15
            }
        })
    }

    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    // #[must_use]
    // pub fn take(&mut self) -> Self {
    //     let mut chunks = HashMap::with_capacity(self.chunks.capacity());

    //     for chunk in self.chunks_mut() {
    //         chunks.insert(chunk.origin, std::mem::replace(chunk,
    // Chunk::new(chunk.origin)));     }

    //     Self { chunks }
    // }

    pub fn take_chunks(self) -> impl Iterator<Item = Arc<Chunk>> {
        self.chunks.into_values()
    }

    pub fn chunks(&self) -> impl Iterator<Item = &Arc<Chunk>> {
        self.chunks.values()
    }

    pub fn chunks_mut(&mut self) -> impl Iterator<Item = &mut Chunk> {
        self.chunks.values_mut().map(Arc::make_mut)
    }

    pub fn save(&mut self) {
        for (origin, chunk) in &self.chunks {
            self.cache.insert(*origin, chunk);
        }
    }
}

impl<C: ChunkCache> ChunkAccess for ChunkManager<C> {
    fn get_chunk(&self, position: glam::IVec2) -> Option<&Arc<Chunk>> {
        self.chunks.get(&position)
    }

    fn get_chunk_mut(&mut self, position: glam::IVec2) -> Option<&mut Chunk> {
        self.chunks.get_mut(&position).map(Arc::make_mut)
    }

    fn get_block(&self, position: glam::IVec3) -> Option<&SubChunkBlockState> {
        if position.y >= 0 && position.y < const { SUBCHUNK_SIZE_I32 * SUBCHUNK_COUNT_I32 } {
            let chunk = self.get_chunk(Self::to_local(position))?;

            Some(chunk.get_block_unchecked(Chunk::to_local(position)))
        } else {
            None
        }
    }

    fn set_block(&mut self, position: glam::IVec3, block: SubChunkBlockState) {
        if let Some(chunk) = self.get_chunk_mut(Self::to_local(position)) {
            chunk.set_block(Chunk::to_local(position), block);
        }
    }

    fn get_block_light(&self, position: glam::IVec3) -> u8 {
        self.get_chunk(Self::to_local(position)).map_or(15, |chunk| {
            if position.y >= 0 && position.y < const { SUBCHUNK_SIZE_I32 * SUBCHUNK_COUNT_I32 } {
                chunk.get_block_light_unchecked(Chunk::to_local(position))
            } else {
                15
            }
        })
    }

    fn get_sky_light(&self, position: glam::IVec3) -> u8 {
        self.get_chunk(Self::to_local(position)).map_or(15, |chunk| {
            let local_position = Chunk::to_local(position);

            if position.y >= 0 && position.y < const { SUBCHUNK_SIZE_I32 * SUBCHUNK_COUNT_I32 } {
                chunk.get_sky_light_unchecked(local_position)
            } else {
                15
            }
        })
    }

    fn get_light_level(&self, position: glam::IVec3) -> u8 {
        self.get_chunk(Self::to_local(position)).map_or(240, |chunk| {
            if position.y >= 0 && position.y < const { SUBCHUNK_SIZE_I32 * SUBCHUNK_COUNT_I32 } {
                chunk.get_light_level_unchecked(Chunk::to_local(position))
            } else {
                240
            }
        })
    }

    fn get_block_with_light_level(&self, position: glam::IVec3) -> (Option<&SubChunkBlockState>, u8) {
        if position.y >= 0
            && position.y < const { SUBCHUNK_SIZE_I32 * SUBCHUNK_COUNT_I32 }
            && let Some(chunk) = self.get_chunk(ChunkManager::<()>::to_local(position))
        {
            let block = chunk.get_block_unchecked(Chunk::to_local(position));
            let light_level = chunk.get_light_level_unchecked(Chunk::to_local(position));

            (Some(block), light_level)
        } else {
            (None, 240)
        }
    }

    fn get_local_block_light(&self, origin: glam::IVec2, position: glam::USizeVec3) -> u8 {
        self.get_chunk(origin).map_or(0, |chunk| chunk.get_block_light(position))
    }

    fn set_local_block_light(&mut self, origin: glam::IVec2, position: glam::USizeVec3, light_level: u8) {
        if let Some(chunk) = self.get_chunk_mut(origin) {
            chunk.set_block_light(position, light_level);
        }
    }

    fn get_local_sky_light(&self, origin: glam::IVec2, position: glam::USizeVec3) -> u8 {
        self.get_chunk(origin).map_or(0, |chunk| chunk.get_sky_light(position))
    }

    fn set_local_sky_light(&mut self, origin: glam::IVec2, position: glam::USizeVec3, light_level: u8) {
        if let Some(chunk) = self.get_chunk_mut(origin) {
            chunk.set_sky_light(position, light_level);
        }
    }

    fn get_local_light(&self, origin: glam::IVec2, position: glam::USizeVec3, is_sky_light: bool) -> u8 {
        self.get_chunk(origin).map_or(0, |chunk| chunk.get_light(position, is_sky_light))
    }

    fn set_local_light(&mut self, origin: glam::IVec2, position: glam::USizeVec3, is_sky_light: bool, light_level: u8) {
        if let Some(chunk) = self.get_chunk_mut(origin) {
            chunk.set_light(position, is_sky_light, light_level);
        }
    }

    fn get_chunk_by_block(&self, position: glam::IVec3) -> Option<&Arc<Chunk>> {
        self.chunks.get(&Self::to_local(position))
    }

    fn get_chunk_by_block_mut(&mut self, position: glam::IVec3) -> Option<&mut Chunk> {
        self.chunks.get_mut(&Self::to_local(position)).map(Arc::make_mut)
    }
}

impl Default for ChunkManager<()> {
    fn default() -> Self {
        Self::new(())
    }
}

impl<C: ChunkCache> Index<glam::IVec2> for ChunkManager<C> {
    type Output = Chunk;

    fn index(&self, index: glam::IVec2) -> &Self::Output {
        &self.chunks[&index]
    }
}

impl<C: ChunkCache> IndexMut<glam::IVec2> for ChunkManager<C> {
    fn index_mut(&mut self, index: glam::IVec2) -> &mut Self::Output {
        Arc::make_mut(self.chunks.get_mut(&index).unwrap())
    }
}
