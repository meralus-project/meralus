use meralus_shared::{IPoint2D, USizePoint3D};

use crate::{BlockSource, CHUNK_HEIGHT, Chunk, ChunkManager, Face};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LightNode(pub USizePoint3D, pub IPoint2D);

impl LightNode {
    pub const fn get_position(&self) -> USizePoint3D {
        self.0
    }
}

pub struct BfsLight<'a> {
    pub chunk_manager: &'a mut ChunkManager,
    pub addition_queue: Vec<LightNode>,
    pub removing_queue: Vec<(LightNode, u8)>,
    pub is_sky_light: bool,
    pub starting_chunk: Option<IPoint2D>,
}

impl<'a> BfsLight<'a> {
    #[must_use]
    pub const fn new(chunk_manager: &'a mut ChunkManager) -> Self {
        Self {
            chunk_manager,
            addition_queue: Vec::new(),
            removing_queue: Vec::new(),
            is_sky_light: false,
            starting_chunk: None,
        }
    }

    #[must_use]
    pub const fn apply_to_sky_light(mut self) -> Self {
        self.is_sky_light = true;

        self
    }

    pub fn add(&mut self, node: LightNode) {
        self.starting_chunk = Some(node.1);
        self.addition_queue.push(node);
    }

    pub fn add_custom(&mut self, node: LightNode, light_level: u8) {
        self.starting_chunk = Some(node.1);
        self.chunk_manager[node.1].set_light(node.0, self.is_sky_light, light_level);
        self.addition_queue.push(node);
    }

    pub fn remove(&mut self, node: LightNode) {
        self.starting_chunk = Some(node.1);

        let light_level = self.chunk_manager[node.1].get_light(node.0, self.is_sky_light);

        self.removing_queue.push((node, light_level));
        self.chunk_manager[node.1].set_light(node.0, self.is_sky_light, if self.is_sky_light && node.0.y == 255 { 15 } else { 0 });
    }

    pub fn calculate<T: BlockSource>(&mut self, block_source: &T) {
        self.calculate_with_info(block_source);
    }

    pub fn calculate_with_info<T: BlockSource>(&mut self, block_source: &T) -> Vec<IPoint2D> {
        let mut other_chunks_affected = Vec::with_capacity(8);

        while let Some((node, node_light_level)) = self.removing_queue.pop() {
            if self.starting_chunk != Some(node.1) && !other_chunks_affected.contains(&node.1) {
                other_chunks_affected.push(node.1);
            }

            let world_position = Chunk::to_world_pos(node.1, node.0);

            for face in [Face::Left, Face::Right, Face::Back, Face::Front, Face::Bottom, Face::Top] {
                let neighbour_pos = world_position + face.as_normal();

                if let Some(chunk) = self.chunk_manager.get_chunk_by_block_mut(neighbour_pos) {
                    let local_position = Chunk::to_local(neighbour_pos);

                    if local_position.y < CHUNK_HEIGHT {
                        let neighbour_light_level = chunk.get_light(local_position, self.is_sky_light);

                        if (self.is_sky_light && face == Face::Bottom && node_light_level == 15)
                            || (neighbour_light_level != 0 && neighbour_light_level < node_light_level)
                        {
                            chunk.set_light(local_position, self.is_sky_light, 0);

                            self.removing_queue.push((LightNode(local_position, chunk.origin), neighbour_light_level));
                        } else if neighbour_light_level >= node_light_level {
                            self.addition_queue.push(LightNode(local_position, chunk.origin));
                        }
                    }
                }
            }
        }

        while let Some(node) = self.addition_queue.pop() {
            if self.starting_chunk != Some(node.1) && !other_chunks_affected.contains(&node.1) {
                other_chunks_affected.push(node.1);
            }

            let local_position = node.get_position();
            let world_position = Chunk::to_world_pos(node.1, local_position);
            let node_light_level = self.chunk_manager[node.1].get_light(local_position, self.is_sky_light);

            for face in [Face::Left, Face::Right, Face::Back, Face::Front, Face::Bottom, Face::Top] {
                let neighbour_pos = world_position + face.as_normal();

                if let Some(chunk) = self.chunk_manager.get_chunk_by_block_mut(neighbour_pos) {
                    let local_position = Chunk::to_local(neighbour_pos);

                    if local_position.y >= CHUNK_HEIGHT {
                        continue;
                    }

                    let light_level = chunk.get_light(local_position, self.is_sky_light);
                    let block = chunk.get_block_unchecked(local_position);
                    let skip_decrease = self.is_sky_light && face == Face::Bottom && node_light_level == 15;

                    if !block_source.blocks_light(block) && light_level + 2 - u8::from(skip_decrease) <= node_light_level {
                        let mut light = node_light_level - u8::from(!skip_decrease);

                        if light > 0 {
                            light -= block_source.light_consumption(block);
                        }

                        chunk.set_light(local_position, self.is_sky_light, light);

                        self.addition_queue.push(LightNode(local_position, chunk.origin));
                    }
                }
            }
        }

        other_chunks_affected
    }
}

#[cfg(test)]
mod tests {
    use meralus_shared::{IPoint2D, IPoint3D, USizePoint3D};

    use crate::{BfsLight, BlockSource, ChunkManager};

    struct TestBlockSource {
        ids: Vec<&'static str>,
    }

    impl BlockSource for TestBlockSource {
        fn get_block_id(&self, name: &str) -> u8 {
            self.ids.iter().position(|id| *id == name).unwrap_or_default() as u8
        }

        fn blocks_light(&self, _: u8) -> bool {
            true
        }

        fn light_consumption(&self, _: u8) -> u8 {
            0
        }
    }

    #[test]
    fn test_sunlight() {
        let mut chunk_manager = ChunkManager::from_range(0..1, &(0..1));
        let source = TestBlockSource {
            ids: vec!["air", "stone", "dirt", "grass_block", "sand"],
        };

        let chunk = chunk_manager.get_chunk_mut(&IPoint2D::ZERO).unwrap();

        for y in 0..240 {
            for z in 0..16 {
                for x in 0..16 {
                    chunk.set_block_unchecked(USizePoint3D::new(x, y, z), 1);
                }
            }
        }

        let mut light = BfsLight::new(&mut chunk_manager).apply_to_sky_light();

        for x in 0..16 {
            for z in 0..16 {
                light.chunk_manager.set_sky_light(IPoint3D::new(x as i32, 255, z as i32), 15);
                light.add_custom(crate::LightNode(USizePoint3D::new(x, 255, z), IPoint2D::ZERO), 15);
            }
        }

        light.calculate(&source);
    }
}
