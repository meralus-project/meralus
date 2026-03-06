use std::ops::{Index, IndexMut};

use ahash::{HashMap, HashMapExt};
use meralus_shared::{IPoint2D, IPoint3D, USizePoint2D, USizePoint3D};

use crate::{BfsLight, BiomeBase, BlockSource, Chunk, LightNode, SUBCHUNK_COUNT_I32, SUBCHUNK_SIZE, SUBCHUNK_SIZE_I32};

#[derive(Debug, Clone)]
pub struct ChunkManager {
    chunks: HashMap<IPoint2D, Chunk>,
}

impl ChunkManager {
    pub fn new() -> Self {
        Self { chunks: HashMap::new() }
    }

    pub fn push(&mut self, chunk: Chunk) {
        self.chunks.insert(chunk.origin, chunk);
    }

    pub fn from_range<T: Iterator<Item = i32> + Clone>(x: T, z: &T) -> Self {
        Self {
            chunks: x
                .flat_map(|x| {
                    z.clone().map(move |z| {
                        let origin = IPoint2D::new(x, z);

                        (origin, Chunk::new(origin))
                    })
                })
                .collect(),
        }
    }

    pub fn from_chunks<T: IntoIterator<Item = Chunk>>(chunks: T) -> Self {
        Self {
            chunks: chunks.into_iter().map(|chunk| (chunk.origin, chunk)).collect(),
        }
    }

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

        bfs_light.addition_queue = queue;
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
        self.get_chunk(&Self::to_local(position)).map(|_| Chunk::to_local(position))
    }

    pub fn get_chunk_by_block(&self, position: IPoint3D) -> Option<&Chunk> {
        self.chunks.get(&Self::to_local(position))
    }

    pub fn get_chunk_by_block_mut(&mut self, position: IPoint3D) -> Option<&mut Chunk> {
        self.chunks.get_mut(&Self::to_local(position))
    }

    pub fn get_chunk(&self, position: &IPoint2D) -> Option<&Chunk> {
        self.chunks.get(position)
    }

    pub fn get_chunk_mut(&mut self, position: &IPoint2D) -> Option<&mut Chunk> {
        self.chunks.get_mut(position)
    }

    pub fn get_biome(&self, position: IPoint3D) -> Option<BiomeBase> {
        if position.y >= 0 && position.y < const { SUBCHUNK_SIZE_I32 * SUBCHUNK_COUNT_I32 } {
            let chunk = self.get_chunk(&Self::to_local(position))?;
            let local = Chunk::to_local(position);

            Some(chunk.get_biome_unchecked(USizePoint2D::new(local.x, local.z)))
        } else {
            None
        }
    }

    pub fn get_block(&self, position: IPoint3D) -> Option<u8> {
        if position.y >= 0 && position.y < const { SUBCHUNK_SIZE_I32 * SUBCHUNK_COUNT_I32 } {
            let chunk = self.get_chunk(&Self::to_local(position))?;

            Some(chunk.get_block_unchecked(Chunk::to_local(position)))
        } else {
            None
        }
    }

    pub fn set_block(&mut self, position: IPoint3D, block: u8) {
        if let Some(chunk) = self.get_chunk_mut(&Self::to_local(position)) {
            chunk.set_block(Chunk::to_local(position), block);
        }
    }

    pub fn remove_block<T: BlockSource>(&mut self, position: IPoint3D, block_source: &T) -> Vec<IPoint2D> {
        let chunk_position = Self::to_local(position);
        let mut affected_chunks = Vec::with_capacity(8);

        if let Some(chunk) = self.get_chunk_mut(&chunk_position) {
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
                    if bfs_light.chunk_manager[chunk_position].get_block(local.with_y(y)).filter(|&b| b != 0).is_some() {
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
        if let Some(chunk) = self.get_chunk_mut(&Self::to_local(position)) {
            chunk.set_block_light(Chunk::to_local(position), light_level);
        }
    }

    pub fn set_sky_light(&mut self, position: IPoint3D, light_level: u8) {
        if let Some(chunk) = self.get_chunk_mut(&Self::to_local(position)) {
            chunk.set_sky_light(Chunk::to_local(position), light_level);
        }
    }

    pub fn set_light(&mut self, position: IPoint3D, is_sky_light: bool, light_level: u8) {
        if let Some(chunk) = self.get_chunk_mut(&Self::to_local(position)) {
            chunk.set_light(Chunk::to_local(position), is_sky_light, light_level);
        }
    }

    pub fn contains_block(&self, position: IPoint3D) -> bool {
        self.get_chunk(&Self::to_local(position)).is_some_and(|chunk| chunk.check_for_block(position))
    }

    pub fn contains_chunk(&self, origin: &IPoint2D) -> bool {
        self.chunks.contains_key(origin)
    }

    pub fn get_block_light(&self, position: IPoint3D) -> u8 {
        self.get_chunk(&Self::to_local(position)).map_or(15, |chunk| {
            if position.y >= 0 && position.y < const { SUBCHUNK_SIZE_I32 * SUBCHUNK_COUNT_I32 } {
                chunk.get_block_light_unchecked(Chunk::to_local(position))
            } else {
                15
            }
        })
    }

    pub fn get_sky_light(&self, position: IPoint3D) -> u8 {
        self.get_chunk(&Self::to_local(position)).map_or(15, |chunk| {
            let local_position = Chunk::to_local(position);

            if position.y >= 0 && position.y < const { SUBCHUNK_SIZE_I32 * SUBCHUNK_COUNT_I32 } {
                chunk.get_sky_light_unchecked(local_position)
            } else {
                15
            }
        })
    }

    pub fn get_light_level(&self, position: IPoint3D) -> u8 {
        self.get_chunk(&Self::to_local(position)).map_or(240, |chunk| {
            if position.y >= 0 && position.y < const { SUBCHUNK_SIZE_I32 * SUBCHUNK_COUNT_I32 } {
                chunk.get_light_level_unchecked(Chunk::to_local(position))
            } else {
                240
            }
        })
    }

    pub fn get_light(&self, position: IPoint3D, is_sky_light: bool) -> u8 {
        self.get_chunk(&Self::to_local(position)).map_or(15, |chunk| {
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

    #[must_use]
    pub fn take(&mut self) -> Self {
        let mut chunks = HashMap::with_capacity(self.chunks.capacity());

        for chunk in self.chunks_mut() {
            chunks.insert(chunk.origin, std::mem::replace(chunk, Chunk::new(chunk.origin)));
        }

        Self { chunks }
    }

    pub fn take_chunks(self) -> impl Iterator<Item = Chunk> {
        self.chunks.into_values()
    }

    pub fn chunks(&self) -> impl Iterator<Item = &Chunk> {
        self.chunks.values()
    }

    pub fn chunks_mut(&mut self) -> impl Iterator<Item = &mut Chunk> {
        self.chunks.values_mut()
    }
}

impl Default for ChunkManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Index<IPoint2D> for ChunkManager {
    type Output = Chunk;

    fn index(&self, index: IPoint2D) -> &Self::Output {
        &self.chunks[&index]
    }
}

impl IndexMut<IPoint2D> for ChunkManager {
    fn index_mut(&mut self, index: IPoint2D) -> &mut Self::Output {
        self.chunks.get_mut(&index).unwrap()
    }
}
