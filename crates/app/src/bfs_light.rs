use glam::{IVec2, U16Vec3};
use meralus_world::{ChunkManager, Face};

use crate::BakedBlockModelLoader;

pub struct LightNode(pub U16Vec3, pub IVec2);

impl LightNode {
    pub const fn get_position(&self) -> U16Vec3 {
        self.0
    }
}

pub struct BfsLight {
    queue: Vec<LightNode>,
}

impl BfsLight {
    pub const fn new() -> Self {
        Self { queue: Vec::new() }
    }

    pub fn push(&mut self, node: LightNode) {
        self.queue.push(node);
    }

    pub fn calculate(&mut self, chunk_manager: &mut ChunkManager, blocks: &BakedBlockModelLoader, is_sky_light: bool) {
        while let Some(node) = self.queue.pop() {
            if let Some(chunk) = chunk_manager.get_chunk_mut(&node.1) {
                let local_position = node.get_position();
                let world_position = chunk.to_world(local_position);

                let light_level = chunk.get_light(local_position, is_sky_light);

                for face in Face::ALL {
                    let neighbour_pos = world_position + face.as_normal();
                    let neighbour_position = neighbour_pos.as_vec3();

                    if let Some(chunk) = chunk_manager.get_chunk_mut(&ChunkManager::to_local(neighbour_position)) {
                        let local_position = chunk.to_local(neighbour_position);

                        if !chunk.contains_local_position(local_position) {
                            continue;
                        }

                        if chunk
                            .get_block_unchecked(local_position)
                            .is_none_or(|block| !blocks.get(block.into()).unwrap().is_opaque())
                            && chunk.get_light(local_position, is_sky_light) + 2 <= light_level
                        {
                            chunk.set_light(
                                local_position,
                                is_sky_light,
                                if is_sky_light && face == Face::Bottom && light_level == 15 {
                                    light_level
                                } else {
                                    light_level - 1
                                },
                            );

                            self.queue.push(LightNode(local_position, chunk.origin));
                        }
                    }
                }
            }
        }
    }
}

impl Default for BfsLight {
    fn default() -> Self {
        Self::new()
    }
}
