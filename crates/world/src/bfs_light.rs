use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use meralus_shared::{Face, IPoint2D, USizePoint3D};

use crate::{BlockSource, CHUNK_HEIGHT, Chunk, ChunkAccess, SubChunk};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LightNode(pub USizePoint3D, pub IPoint2D);

pub struct BfsLight<'a, C: ChunkAccess> {
    pub chunk_manager: &'a mut C,
    pub block_addition_queue: VecDeque<LightNode>,
    pub block_removing_queue: Vec<(LightNode, u8)>,
    pub sky_addition_queue: VecDeque<LightNode>,
    pub sky_removing_queue: Vec<(LightNode, u8)>,
}

impl<'a, C: ChunkAccess> BfsLight<'a, C> {
    #[must_use]
    pub const fn new(chunk_manager: &'a mut C) -> Self {
        Self {
            chunk_manager,
            block_addition_queue: VecDeque::new(),
            block_removing_queue: Vec::new(),
            sky_addition_queue: VecDeque::new(),
            sky_removing_queue: Vec::new(),
        }
    }

    pub fn add_block(&mut self, node: LightNode) {
        self.block_addition_queue.push_back(node);
    }

    pub fn add_block_custom(&mut self, node: LightNode, light_level: u8) {
        self.chunk_manager.set_local_block_light(node.1, node.0, light_level);
        self.block_addition_queue.push_back(node);
    }

    pub fn remove_block(&mut self, node: LightNode) {
        let light_level = self.chunk_manager.get_local_block_light(node.1, node.0);

        self.block_removing_queue.push((node, light_level));
        self.chunk_manager.set_local_block_light(node.1, node.0, 0);
    }

    pub fn add_sky(&mut self, node: LightNode) {
        self.sky_addition_queue.push_back(node);
    }

    pub fn add_sky_custom(&mut self, node: LightNode, light_level: u8) {
        self.chunk_manager.set_local_sky_light(node.1, node.0, light_level);
        self.sky_addition_queue.push_back(node);
    }

    pub fn remove_sky(&mut self, node: LightNode) {
        let light_level = self.chunk_manager.get_local_sky_light(node.1, node.0);

        self.sky_removing_queue.push((node, light_level));
        self.chunk_manager.set_local_sky_light(node.1, node.0, if node.0.y == 255 { 15 } else { 0 });
    }

    pub fn calculate_block_light<T: BlockSource>(&mut self, block_source: &T) {
        while let Some((node, node_light_level)) = self.block_removing_queue.pop() {
            let world_position = Chunk::to_world_pos(node.1, node.0);

            for face in [Face::Left, Face::Right, Face::Back, Face::Front, Face::Bottom, Face::Top] {
                let neighbour_pos = world_position + face.as_normal();

                if let Some(chunk) = self.chunk_manager.get_chunk_by_block_mut(neighbour_pos) {
                    let local_position = Chunk::to_local(neighbour_pos);

                    if local_position.y < CHUNK_HEIGHT {
                        let neighbour_light_level = chunk.get_block_light(local_position);

                        if neighbour_light_level != 0 && neighbour_light_level < node_light_level {
                            chunk.dirty = true;
                            chunk.set_block_light(local_position, 0);

                            self.block_removing_queue.push((LightNode(local_position, chunk.origin), neighbour_light_level));
                        } else if neighbour_light_level >= node_light_level {
                            self.block_addition_queue.push_back(LightNode(local_position, chunk.origin));
                        }
                    }
                }
            }
        }

        while let Some(node) = self.block_addition_queue.pop_front() {
            let world_position = Chunk::to_world_pos(node.1, node.0);
            let light_level = self.chunk_manager.get_local_block_light(node.1, node.0);

            for face in [Face::Left, Face::Right, Face::Back, Face::Front, Face::Bottom, Face::Top] {
                let (chunk, position) = Chunk::to_origin_and_local(world_position + face.as_normal());

                if let Some(chunk) = self.chunk_manager.get_chunk_mut(chunk) {
                    if position.y >= CHUNK_HEIGHT {
                        continue;
                    }

                    let block = chunk.get_block_unchecked(position);

                    if !block_source.blocks_light(&block.name) && chunk.get_block_light(position) + 2 <= light_level {
                        let light_consumed = block_source.light_consumption(&block.name);

                        chunk.dirty = true;
                        chunk.set_block_light(position, light_level - light_consumed);

                        self.block_addition_queue.push_back(LightNode(position, chunk.origin));
                    }
                }
            }
        }
    }

    pub fn calculate_sky_light<T: BlockSource>(&mut self, block_source: &T) {
        while let Some((node, node_light_level)) = self.sky_removing_queue.pop() {
            let world_position = Chunk::to_world_pos(node.1, node.0);

            for face in [Face::Left, Face::Right, Face::Back, Face::Front, Face::Bottom, Face::Top] {
                let neighbour_pos = world_position + face.as_normal();

                if let Some(chunk) = self.chunk_manager.get_chunk_by_block_mut(neighbour_pos) {
                    let local_position = Chunk::to_local(neighbour_pos);

                    if local_position.y < CHUNK_HEIGHT {
                        let neighbour_light_level = chunk.get_sky_light(local_position);

                        if (face == Face::Bottom && node_light_level == 15) || (neighbour_light_level != 0 && neighbour_light_level < node_light_level) {
                            chunk.dirty = true;
                            chunk.set_sky_light(local_position, 0);

                            self.sky_removing_queue.push((LightNode(local_position, chunk.origin), neighbour_light_level));
                        } else if neighbour_light_level >= node_light_level {
                            self.sky_addition_queue.push_back(LightNode(local_position, chunk.origin));
                        }
                    }
                }
            }
        }

        let mut get_block_unchecked = Duration::ZERO;
        let mut get_block_unchecked_t = 0;
        let mut get_local_sky_light = Duration::ZERO;
        let mut get_local_sky_light_t = 0;
        let mut get_sky_light = Duration::ZERO;
        let mut get_sky_light_t = 0;
        let mut set_sky_light = Duration::ZERO;
        let mut set_sky_light_t = 0;

        while let Some(node) = self.sky_addition_queue.pop_front() {
            let world_position = Chunk::to_world_pos(node.1, node.0);
            let get_local_sky_light_i = Instant::now();
            let light_level = self.chunk_manager.get_local_sky_light(node.1, node.0);

            get_local_sky_light += get_local_sky_light_i.elapsed();
            get_local_sky_light_t += 1;

            for face in [Face::Left, Face::Right, Face::Back, Face::Front, Face::Bottom, Face::Top] {
                let (chunk, position) = Chunk::to_origin_and_local(world_position + face.as_normal());

                if let Some(chunk) = self.chunk_manager.get_chunk_mut(chunk) {
                    if position.y >= CHUNK_HEIGHT {
                        continue;
                    }

                    let get_block_unchecked_i = Instant::now();
                    let [subchunk, y] = Chunk::get_subchunk_index(position.y);
                    let index = SubChunk::index_of(position.with_y(y));
                    let block = chunk.get_block_by_idx_unchecked(subchunk, index);

                    get_block_unchecked += get_block_unchecked_i.elapsed();
                    get_block_unchecked_t += 1;

                    let skip_decrease = face == Face::Bottom && light_level == 15;

                    if !block_source.blocks_light(&block.name) && {
                        let get_sky_light_i = Instant::now();
                        let value = chunk.get_sky_light_by_idx(subchunk, index);

                        get_sky_light += get_sky_light_i.elapsed();
                        get_sky_light_t += 1;

                        value
                    } + 2
                        <= light_level
                    {
                        let light_consumed = block_source.light_consumption(&block.name);

                        chunk.dirty = true;

                        let set_sky_light_i = Instant::now();

                        chunk.set_sky_light(position, light_level - light_consumed - u8::from(!skip_decrease));

                        set_sky_light += set_sky_light_i.elapsed();
                        set_sky_light_t += 1;

                        self.sky_addition_queue.push_back(LightNode(position, chunk.origin));
                    }
                }
            }
        }

        let sum = get_block_unchecked + get_local_sky_light + get_sky_light + set_sky_light;

        tracing::info!(
            "get_block_unchecked avg takes ~{:?}, {}%",
            get_block_unchecked / get_block_unchecked_t,
            (get_block_unchecked.as_secs_f32() / sum.as_secs_f32()) * 100.0
        );
        tracing::info!(
            "get_local_sky_light avg takes ~{:?}, {}%",
            get_local_sky_light / get_local_sky_light_t,
            (get_local_sky_light.as_secs_f32() / sum.as_secs_f32()) * 100.0
        );
        tracing::info!(
            "get_sky_light avg takes ~{:?}, {}%",
            get_sky_light / get_sky_light_t,
            (get_sky_light.as_secs_f32() / sum.as_secs_f32()) * 100.0
        );
        tracing::info!(
            "set_sky_light avg takes ~{:?}, {}%",
            set_sky_light / set_sky_light_t,
            (set_sky_light.as_secs_f32() / sum.as_secs_f32()) * 100.0
        );
    }
}

#[cfg(test)]
mod tests {
    use meralus_shared::{IPoint2D, IPoint3D, USizePoint3D};

    use crate::{BfsLight, BlockSource, ChunkAccess, ChunkManager};

    struct TestBlockSource {
        ids: Vec<&'static str>,
    }

    // impl BlockSource for TestBlockSource {
    //     fn get_block_id(&self, name: &str) -> u8 {
    //         self.ids.iter().position(|id| *id == name).unwrap_or_default() as
    // u8     }

    //     fn blocks_light(&self, _: u8) -> bool {
    //         true
    //     }

    //     fn light_consumption(&self, _: u8) -> u8 {
    //         0
    //     }
    // }

    // #[test]
    // fn test_sunlight() {
    //     let mut chunk_manager = ChunkManager::from_range((), 0..1, &(0..1));
    //     let source = TestBlockSource {
    //         ids: vec!["air", "stone", "dirt", "grass_block", "sand"],
    //     };

    //     let chunk = chunk_manager.get_chunk_mut(IPoint2D::ZERO).unwrap();

    //     for y in 0..240 {
    //         for z in 0..16 {
    //             for x in 0..16 {
    //                 chunk.set_block_unchecked(USizePoint3D::new(x, y, z), 1);
    //             }
    //         }
    //     }

    //     let mut light = BfsLight::new(&mut chunk_manager);

    //     for x in 0..16 {
    //         for z in 0..16 {
    //             light.chunk_manager.set_sky_light(IPoint3D::new(x as i32,
    // 255, z as i32), 15);
    // light.add_sky_custom(crate::LightNode(USizePoint3D::new(x, 255, z),
    // IPoint2D::ZERO), 15);         }
    //     }

    //     light.calculate_block_light(&source);
    // }
}
