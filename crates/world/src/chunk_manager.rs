use std::{
    ops::{Index, IndexMut},
    sync::Arc,
};

use ahash::HashMap;
use meralus_shared::{IPoint2D, IPoint3D, USizePoint2D, USizePoint3D};

use crate::{BfsLight, BiomeBase, BlockSource, Chunk, LightNode, SUBCHUNK_COUNT_I32, SUBCHUNK_SIZE, SUBCHUNK_SIZE_I32};

pub trait ChunkCache {
    fn all(&self) -> impl Iterator<Item = Chunk>;
    fn get(&self, origin: IPoint2D) -> Option<Chunk>;
    fn insert(&mut self, origin: IPoint2D, chunk: &Chunk);
}

impl ChunkCache for () {
    fn all(&self) -> impl Iterator<Item = Chunk> {
        std::iter::empty()
    }

    fn get(&self, _: IPoint2D) -> Option<Chunk> {
        None
    }

    fn insert(&mut self, _: IPoint2D, _: &Chunk) {}
}

pub trait ChunkAccess {
    fn get_chunk(&self, origin: IPoint2D) -> Option<&Arc<Chunk>>;
    fn get_chunk_mut(&mut self, origin: IPoint2D) -> Option<&mut Chunk>;
    fn get_block(&self, position: IPoint3D) -> Option<u8>;
    fn set_block(&mut self, position: IPoint3D, block: u8);
    fn get_block_light(&self, position: IPoint3D) -> u8;
    fn get_sky_light(&self, position: IPoint3D) -> u8;
    fn get_light_level(&self, position: IPoint3D) -> u8;
    fn get_chunk_by_block(&self, position: IPoint3D) -> Option<&Arc<Chunk>>;
    fn get_chunk_by_block_mut(&mut self, position: IPoint3D) -> Option<&mut Chunk>;
    fn get_local_light(&self, origin: IPoint2D, position: USizePoint3D, is_sky_light: bool) -> u8;
    fn set_local_light(&mut self, origin: IPoint2D, position: USizePoint3D, is_sky_light: bool, light: u8);
}

pub struct LocalChunkManager {
    chunk: Arc<Chunk>,
    neighbours: [Arc<Chunk>; 8],
}

impl LocalChunkManager {
    pub fn new<C: ChunkCache>(chunk: Arc<Chunk>, chunk_manager: &ChunkManager<C>) -> Self {
        let origin = chunk.origin;

        Self {
            chunk,
            neighbours: [
                IPoint2D::new(-1, 0),
                IPoint2D::new(1, 0),
                IPoint2D::new(0, 1),
                IPoint2D::new(0, -1),
                IPoint2D::new(-1, 1),
                IPoint2D::new(1, 1),
                IPoint2D::new(-1, -1),
                IPoint2D::new(1, -1),
            ]
            .map(|offset| {
                chunk_manager
                    .get_chunk(origin + offset)
                    .cloned()
                    .unwrap_or_else(|| Arc::new(Chunk::empty().with_origin(origin + offset)))
            }),
        }
    }

    pub fn into_inner(self) -> (Arc<Chunk>, [Arc<Chunk>; 8]) {
        (self.chunk, self.neighbours)
    }
}

impl ChunkAccess for LocalChunkManager {
    fn get_chunk(&self, origin: IPoint2D) -> Option<&Arc<Chunk>> {
        let delta = origin - self.chunk.origin;

        if delta.x == 0 && delta.y == 0 {
            return Some(&self.chunk);
        }

        if delta.x.abs() > 1 || delta.y.abs() > 1 {
            return None;
        }

        let idx = ((delta.x + 1) + (delta.y + 1) * 3) as usize;
        let mapping = [6, 3, 7, 0, 99, 1, 4, 2, 5]; // 99 is a placeholder for center

        Some(&self.neighbours[mapping[idx]])
    }

    fn get_chunk_mut(&mut self, origin: IPoint2D) -> Option<&mut Chunk> {
        let delta = origin - self.chunk.origin;

        if delta.x == 0 && delta.y == 0 {
            return Some(Arc::make_mut(&mut self.chunk));
        }

        if delta.x.abs() > 1 || delta.y.abs() > 1 {
            return None;
        }

        let idx = ((delta.x + 1) + (delta.y + 1) * 3) as usize;
        let mapping = [6, 3, 4, 0, 99, 1, 7, 2, 5]; // 99 is a placeholder for center

        Some(Arc::make_mut(&mut self.neighbours[mapping[idx]]))
    }

    fn get_block(&self, position: IPoint3D) -> Option<u8> {
        if position.y >= 0 && position.y < const { SUBCHUNK_SIZE_I32 * SUBCHUNK_COUNT_I32 } {
            let chunk = self.get_chunk(ChunkManager::<()>::to_local(position))?;

            Some(chunk.get_block_unchecked(Chunk::to_local(position)))
        } else {
            None
        }
    }

    fn set_block(&mut self, position: IPoint3D, block: u8) {
        if position.y >= 0
            && position.y < const { SUBCHUNK_SIZE_I32 * SUBCHUNK_COUNT_I32 }
            && let Some(chunk) = self.get_chunk_mut(ChunkManager::<()>::to_local(position))
        {
            chunk.set_block_unchecked(Chunk::to_local(position), block);
        }
    }

    fn get_block_light(&self, position: IPoint3D) -> u8 {
        self.get_chunk(ChunkManager::<()>::to_local(position)).map_or(15, |chunk| {
            if position.y >= 0 && position.y < const { SUBCHUNK_SIZE_I32 * SUBCHUNK_COUNT_I32 } {
                chunk.get_block_light_unchecked(Chunk::to_local(position))
            } else {
                15
            }
        })
    }

    fn get_sky_light(&self, position: IPoint3D) -> u8 {
        self.get_chunk(ChunkManager::<()>::to_local(position)).map_or(15, |chunk| {
            let local_position = Chunk::to_local(position);

            if position.y >= 0 && position.y < const { SUBCHUNK_SIZE_I32 * SUBCHUNK_COUNT_I32 } {
                chunk.get_sky_light_unchecked(local_position)
            } else {
                15
            }
        })
    }

    fn get_light_level(&self, position: IPoint3D) -> u8 {
        self.get_chunk(ChunkManager::<()>::to_local(position)).map_or(240, |chunk| {
            if position.y >= 0 && position.y < const { SUBCHUNK_SIZE_I32 * SUBCHUNK_COUNT_I32 } {
                chunk.get_light_level_unchecked(Chunk::to_local(position))
            } else {
                240
            }
        })
    }

    fn get_local_light(&self, origin: IPoint2D, position: USizePoint3D, is_sky_light: bool) -> u8 {
        self.get_chunk(origin).map_or(0, |chunk| chunk.get_light(position, is_sky_light))
    }

    fn set_local_light(&mut self, origin: IPoint2D, position: USizePoint3D, is_sky_light: bool, light_level: u8) {
        if let Some(chunk) = self.get_chunk_mut(origin) {
            chunk.set_light(position, is_sky_light, light_level);
        }
    }

    fn get_chunk_by_block(&self, position: IPoint3D) -> Option<&Arc<Chunk>> {
        self.get_chunk(ChunkManager::<()>::to_local(position))
    }

    fn get_chunk_by_block_mut(&mut self, position: IPoint3D) -> Option<&mut Chunk> {
        self.get_chunk_mut(ChunkManager::<()>::to_local(position))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChunkStage {
    Unloaded,
    Bare,
    PopulationInProgress,
    Populated,
    LightningInProgress,
    Lighted,
    MeshingInProgress,
    Meshed,
}

#[derive(Debug, Clone)]
pub struct ChunkManager<C: ChunkCache> {
    cache: C,
    pub chunks: HashMap<IPoint2D, Arc<Chunk>>,
    pub stages: HashMap<IPoint2D, ChunkStage>,
}

impl<C: ChunkCache> ChunkManager<C> {
    pub fn new(cache: C) -> Self {
        let chunks: HashMap<IPoint2D, _> = cache.all().map(|chunk| (chunk.origin, Arc::new(chunk))).collect();
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

    pub fn set_stage(&mut self, origin: IPoint2D, stage: ChunkStage) {
        self.stages.insert(origin, stage);
    }

    // pub fn stages(&self) -> impl Iterator<Item = (IPoint2D, ChunkStage)> {
    //   self.stages.iter().map(|(&key, &stage)| (key, stage))
    // }

    // pub fn stages_mut(&mut self) -> impl Iterator<Item = (IPoint2D, &mut
    // ChunkStage)> {   self.stages.iter_mut().map(|(&key, stage)| (key, stage))
    // }
    pub fn neighbours_of(&self, origin: IPoint2D) -> impl Iterator<Item = IPoint2D> {
        [
            IPoint2D::new(-1, 0),  // left
            IPoint2D::new(1, 0),   // right
            IPoint2D::new(0, 1),   // top
            IPoint2D::new(0, -1),  // bottom
            IPoint2D::new(-1, 1),  // top left
            IPoint2D::new(1, 1),   // top right
            IPoint2D::new(-1, -1), // bottom left
            IPoint2D::new(1, -1),  // bottom right
        ]
        .into_iter()
        .map(move |offset| origin + offset.to_vector())
    }

    pub fn neighbours_at_least(&self, origin: IPoint2D, stage: ChunkStage) -> bool {
        self.neighbours_of(origin)
            .all(|inner_origin| self.stages.get(&inner_origin).is_some_and(|&chunk_stage| chunk_stage >= stage))
    }

    pub fn update_neighbors(&mut self, center_origin: IPoint2D, modified_neighbors: [Option<Chunk>; 8], stage: ChunkStage) {
        let offsets = [
            IPoint2D::new(-1, 0),  // left
            IPoint2D::new(1, 0),   // right
            IPoint2D::new(0, 1),   // top
            IPoint2D::new(0, -1),  // bottom
            IPoint2D::new(-1, 1),  // top left
            IPoint2D::new(1, 1),   // top right
            IPoint2D::new(-1, -1), // bottom left
            IPoint2D::new(1, -1),  // bottom right
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

    pub fn local_of(&self, origin: IPoint2D) -> Option<LocalChunkManager> {
        self.chunks.get(&origin).cloned().map(|chunk| LocalChunkManager::new(chunk, self))
    }

    pub fn from_range<T: Iterator<Item = i32> + Clone>(mut cache: C, x: T, z: &T) -> Self {
        let chunks: HashMap<IPoint2D, _> = x
            .flat_map(|x| z.clone().map(move |z| IPoint2D::new(x, z)))
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
                    let position = USizePoint3D::new(x, 255, z);

                    if chunk.get_block_unchecked(position) == 0
                    //_or(|block| !resource_manager.read().models.get(block.into()).unwrap().is_opaque)
                    {
                        chunk.set_sky_light(position, 15);
                        queue.push(LightNode(position, chunk.origin));
                    }
                }
            }
        }

        let mut bfs_light = BfsLight::new(self).apply_to_sky_light();

        bfs_light.addition_queue = queue.into();
        bfs_light.calculate(block_source);
    }

    pub fn surface_size(&self) -> IPoint3D {
        let mut min = IPoint2D::ZERO;
        let mut max = IPoint2D::ZERO;

        for chunk in self.chunks.keys() {
            min = min.min(*chunk);
            max = max.max(*chunk);
        }

        let size = (max - min) * 16;

        IPoint3D::new(size.x + SUBCHUNK_SIZE_I32, SUBCHUNK_SIZE_I32 * SUBCHUNK_COUNT_I32, size.y + SUBCHUNK_SIZE_I32)
    }

    pub fn bounds(&self) -> (IPoint2D, IPoint2D) {
        let mut min = IPoint2D::ZERO;
        let mut max = IPoint2D::ZERO;

        for chunk in self.chunks.keys() {
            min = min.min(*chunk);
            max = max.max(*chunk);
        }

        (min * SUBCHUNK_SIZE_I32, max * SUBCHUNK_SIZE_I32)
    }

    pub const fn to_local(position: IPoint3D) -> IPoint2D {
        IPoint2D::new(position.x >> 4, position.z >> 4)
    }

    pub fn to_chunk_local(&self, position: IPoint3D) -> Option<USizePoint3D> {
        self.get_chunk(Self::to_local(position)).map(|_| Chunk::to_local(position))
    }

    pub fn get_biome(&self, position: IPoint3D) -> Option<BiomeBase> {
        if position.y >= 0 && position.y < const { SUBCHUNK_SIZE_I32 * SUBCHUNK_COUNT_I32 } {
            let chunk = self.get_chunk(Self::to_local(position))?;
            let local = Chunk::to_local(position);

            Some(chunk.get_biome_unchecked(USizePoint2D::new(local.x, local.z)))
        } else {
            None
        }
    }

    pub fn remove_block<T: BlockSource>(&mut self, position: IPoint3D, block_source: &T) -> Vec<IPoint2D> {
        let chunk_position = Self::to_local(position);
        let mut affected_chunks = Vec::with_capacity(8);

        if let Some(chunk) = self.get_chunk_mut(chunk_position) {
            let local = Chunk::to_local(position);

            chunk.set_block(local, 0);

            let mut bfs_light = BfsLight::new(self);

            bfs_light.remove(LightNode(local, chunk_position));

            for chunk in bfs_light.calculate_with_info(block_source) {
                if !affected_chunks.contains(&chunk) {
                    affected_chunks.push(chunk);
                }
            }

            let mut bfs_light = bfs_light.apply_to_sky_light();

            bfs_light.remove(LightNode(local, chunk_position));

            for chunk in bfs_light.calculate_with_info(block_source) {
                if !affected_chunks.contains(&chunk) {
                    affected_chunks.push(chunk);
                }
            }

            let up = local + USizePoint3D::Y;

            if up.y < 256 && bfs_light.chunk_manager[chunk_position].get_sky_light(up) == 15 {
                let mut y = local.y;

                loop {
                    if bfs_light.chunk_manager[chunk_position].get_block(local.with_y(y)).is_some_and(|b| b != 0) {
                        break;
                    }

                    bfs_light.add_custom(LightNode(local.with_y(y), chunk_position), 15);

                    if y == 0 {
                        break;
                    }

                    y -= 1;
                }
            }

            for chunk in bfs_light.calculate_with_info(block_source) {
                if !affected_chunks.contains(&chunk) {
                    affected_chunks.push(chunk);
                }
            }
        }

        affected_chunks
    }

    pub fn set_block_light(&mut self, position: IPoint3D, light_level: u8) {
        if let Some(chunk) = self.get_chunk_mut(Self::to_local(position)) {
            chunk.set_block_light(Chunk::to_local(position), light_level);
        }
    }

    pub fn set_sky_light(&mut self, position: IPoint3D, light_level: u8) {
        if let Some(chunk) = self.get_chunk_mut(Self::to_local(position)) {
            chunk.set_sky_light(Chunk::to_local(position), light_level);
        }
    }

    pub fn set_light(&mut self, position: IPoint3D, is_sky_light: bool, light_level: u8) {
        if let Some(chunk) = self.get_chunk_mut(Self::to_local(position)) {
            chunk.set_light(Chunk::to_local(position), is_sky_light, light_level);
        }
    }

    pub fn contains_block(&self, position: IPoint3D) -> bool {
        self.get_chunk(Self::to_local(position)).is_some_and(|chunk| chunk.check_for_block(position))
    }

    pub fn contains_chunk(&self, origin: &IPoint2D) -> bool {
        self.chunks.contains_key(origin)
    }

    pub fn get_light(&self, position: IPoint3D, is_sky_light: bool) -> u8 {
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
        self.chunks.values_mut().map(|chunk| Arc::make_mut(chunk))
    }

    pub fn save(&mut self) {
        for (origin, chunk) in &self.chunks {
            self.cache.insert(*origin, chunk);
        }
    }
}

impl<C: ChunkCache> ChunkAccess for ChunkManager<C> {
    fn get_chunk(&self, position: IPoint2D) -> Option<&Arc<Chunk>> {
        self.chunks.get(&position)
    }

    fn get_chunk_mut(&mut self, position: IPoint2D) -> Option<&mut Chunk> {
        self.chunks.get_mut(&position).map(|chunk| Arc::make_mut(chunk))
    }

    fn get_block(&self, position: IPoint3D) -> Option<u8> {
        if position.y >= 0 && position.y < const { SUBCHUNK_SIZE_I32 * SUBCHUNK_COUNT_I32 } {
            let chunk = self.get_chunk(Self::to_local(position))?;

            Some(chunk.get_block_unchecked(Chunk::to_local(position)))
        } else {
            None
        }
    }

    fn set_block(&mut self, position: IPoint3D, block: u8) {
        if let Some(chunk) = self.get_chunk_mut(Self::to_local(position)) {
            chunk.set_block(Chunk::to_local(position), block);
        }
    }

    fn get_block_light(&self, position: IPoint3D) -> u8 {
        self.get_chunk(Self::to_local(position)).map_or(15, |chunk| {
            if position.y >= 0 && position.y < const { SUBCHUNK_SIZE_I32 * SUBCHUNK_COUNT_I32 } {
                chunk.get_block_light_unchecked(Chunk::to_local(position))
            } else {
                15
            }
        })
    }

    fn get_sky_light(&self, position: IPoint3D) -> u8 {
        self.get_chunk(Self::to_local(position)).map_or(15, |chunk| {
            let local_position = Chunk::to_local(position);

            if position.y >= 0 && position.y < const { SUBCHUNK_SIZE_I32 * SUBCHUNK_COUNT_I32 } {
                chunk.get_sky_light_unchecked(local_position)
            } else {
                15
            }
        })
    }

    fn get_light_level(&self, position: IPoint3D) -> u8 {
        self.get_chunk(Self::to_local(position)).map_or(240, |chunk| {
            if position.y >= 0 && position.y < const { SUBCHUNK_SIZE_I32 * SUBCHUNK_COUNT_I32 } {
                chunk.get_light_level_unchecked(Chunk::to_local(position))
            } else {
                240
            }
        })
    }

    fn get_local_light(&self, origin: IPoint2D, position: USizePoint3D, is_sky_light: bool) -> u8 {
        self.get_chunk(origin).map_or(0, |chunk| chunk.get_light(position, is_sky_light))
    }

    fn set_local_light(&mut self, origin: IPoint2D, position: USizePoint3D, is_sky_light: bool, light_level: u8) {
        if let Some(chunk) = self.get_chunk_mut(origin) {
            chunk.set_light(position, is_sky_light, light_level);
        }
    }

    fn get_chunk_by_block(&self, position: IPoint3D) -> Option<&Arc<Chunk>> {
        self.chunks.get(&Self::to_local(position))
    }

    fn get_chunk_by_block_mut(&mut self, position: IPoint3D) -> Option<&mut Chunk> {
        self.chunks.get_mut(&Self::to_local(position)).map(|chunk| Arc::make_mut(chunk))
    }
}

impl Default for ChunkManager<()> {
    fn default() -> Self {
        Self::new(())
    }
}

impl<C: ChunkCache> Index<IPoint2D> for ChunkManager<C> {
    type Output = Chunk;

    fn index(&self, index: IPoint2D) -> &Self::Output {
        &self.chunks[&index]
    }
}

impl<C: ChunkCache> IndexMut<IPoint2D> for ChunkManager<C> {
    fn index_mut(&mut self, index: IPoint2D) -> &mut Self::Output {
        Arc::make_mut(self.chunks.get_mut(&index).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use meralus_shared::IPoint2D;

    use crate::{Chunk, ChunkAccess, ChunkManager, ChunkStage};

    #[test]
    fn test_local_chunk_manager() {
        let inner = || {
            let mut chunk_manager = ChunkManager::default();

            chunk_manager.push(Chunk::filled(0).with_origin(IPoint2D::ZERO), ChunkStage::Bare);
            chunk_manager.push(Chunk::filled(1).with_origin(IPoint2D::NEG_X), ChunkStage::Bare);
            chunk_manager.push(Chunk::filled(2).with_origin(IPoint2D::X), ChunkStage::Bare);
            chunk_manager.push(Chunk::filled(3).with_origin(IPoint2D::Y), ChunkStage::Bare);
            chunk_manager.push(Chunk::filled(4).with_origin(IPoint2D::NEG_Y), ChunkStage::Bare);
            chunk_manager.push(Chunk::filled(5).with_origin(IPoint2D::NEG_ONE), ChunkStage::Bare);
            chunk_manager.push(Chunk::filled(6).with_origin(IPoint2D::ONE), ChunkStage::Bare);

            let chunks = [
                Chunk::filled(0).with_origin(IPoint2D::ZERO),    // center
                Chunk::filled(1).with_origin(IPoint2D::NEG_X),   // left
                Chunk::filled(2).with_origin(IPoint2D::X),       // right
                Chunk::filled(3).with_origin(IPoint2D::Y),       // top
                Chunk::filled(4).with_origin(IPoint2D::NEG_Y),   // bottom
                Chunk::filled(5).with_origin(IPoint2D::NEG_ONE), // left_bottom
                Chunk::filled(6).with_origin(IPoint2D::ONE),     // right_top
            ];

            let chunk_manager = chunk_manager.local_of(IPoint2D::ZERO)?;

            for chunk in chunks {
                assert!(chunk_manager.get_chunk(chunk.origin).is_some_and(|inside| &**inside == &chunk));
            }

            Some(())
        };

        match inner() {
            Some(()) => (),
            None => panic!("test failed"),
        }
    }
}
