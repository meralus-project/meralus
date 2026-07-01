use mavelin_physics::{Aabb, AabbSource};
use mavelin_storage::ResourceStorage;
use mavelin_world::{ChunkAccess, ChunkCache, ChunkManager};

use crate::world::{EntityData, EntityManager};

pub struct AabbProvider<'a, C: ChunkCache> {
    pub chunk_manager: &'a ChunkManager<C>,
    pub entity_manager: &'a EntityManager,
    pub storage: &'a ResourceStorage,
}

impl<C: ChunkCache> AabbSource for AabbProvider<'_, C> {
    fn get_aabb(&self, position: glam::Vec3) -> Option<Aabb> {
        let correct_position = position.floor();

        for (_, entity) in self.entity_manager {
            if let EntityData::Model { id, .. } = &entity.data {
                let entity_position = position.as_dvec3();

                if entity.body.aabb().contains(entity_position) {
                    for aabb in self.storage.entity_models.get_unchecked(*id).elements.iter().map(|element| element.cube) {
                        if aabb
                            .extended((entity.body.position - entity.body.size / 2.0).as_dvec3())
                            .contains(entity_position)
                        {
                            return Some(aabb);
                        }
                    }
                }
            }
        }

        if let Some(block) = self.chunk_manager.get_block(correct_position.as_ivec3())
            && self.storage.blocks.get_unchecked(block.id).collidable()
        {
            let block_pos = position.as_dvec3();

            for aabb in self
                .storage
                .models
                .get_unchecked(self.storage.blocks.get_model_by_name(block.id))
                .elements
                .iter()
                .map(|element| element.cube)
            {
                if aabb.contains(block_pos - correct_position.as_dvec3()) {
                    return Some(aabb);
                }
            }
        }

        None
    }

    fn get_block_aabb(&self, position: glam::IVec3) -> Option<Aabb> {
        self.chunk_manager
            .get_block(position)
            .filter(|&b| !b.is_air() && self.storage.blocks.get_unchecked(b.id).selectable())
            .and_then(|block| self.storage.models.get(self.storage.blocks.get_model_by_name(block.id)))
            .map(|element| element.bounding_box)
    }
}

pub struct LimitedAabbProvider<'a, C: ChunkCache> {
    pub chunk_manager: &'a ChunkManager<C>,
    pub storage: &'a ResourceStorage,
}

impl<C: ChunkCache> AabbSource for LimitedAabbProvider<'_, C> {
    fn get_aabb(&self, position: glam::Vec3) -> Option<Aabb> {
        let correct_position = position.floor();

        if let Some(block) = self.chunk_manager.get_block(correct_position.as_ivec3())
            && self.storage.blocks.get_unchecked(block.id).collidable()
        {
            let block_pos = position.as_dvec3();

            for aabb in self
                .storage
                .models
                .get_unchecked(self.storage.blocks.get_model_by_name(block.id))
                .elements
                .iter()
                .map(|element| element.cube)
            {
                if aabb.contains(block_pos - correct_position.as_dvec3()) {
                    return Some(aabb);
                }
            }
        }

        None
    }

    fn get_block_aabb(&self, position: glam::IVec3) -> Option<Aabb> {
        self.chunk_manager
            .get_block(position)
            .filter(|&b| !b.is_air() && self.storage.blocks.get_unchecked(b.id).selectable())
            .and_then(|block| self.storage.models.get(self.storage.blocks.get_model_by_name(block.id)))
            .map(|element| element.bounding_box)
    }
}
