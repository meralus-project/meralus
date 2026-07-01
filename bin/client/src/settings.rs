use std::{collections::VecDeque, time::Duration};

use mavelin_storage::ResourceStorage;
use mavelin_world::{Chunk, ChunkAccess};

use crate::{
    render::{RenderInfo, RenderShape},
    util::vertex_ao,
};

#[derive(Debug, Clone)]
pub struct Debugging {
    pub enabled: bool,
    pub draw_calls_stat: VecDeque<usize>,
    pub draw_calls_max: usize,
    pub fps_stat: VecDeque<Duration>,
    pub fps_max: Duration,
    pub render_info: RenderInfo,
}

impl Default for Debugging {
    fn default() -> Self {
        Self {
            enabled: false,
            draw_calls_stat: VecDeque::new(),
            draw_calls_max: 0,
            fps_stat: VecDeque::new(),
            fps_max: Duration::ZERO,
            render_info: RenderInfo::default(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum LightStyle {
    Smooth,
    BlockyWithAO,
    Blocky,
}

impl LightStyle {
    #[inline]
    fn does_block_have_ao(resource_storage: &ResourceStorage, block: u32) -> bool {
        if block == 0 {
            false
        } else {
            resource_storage
                .models
                .get_unchecked(resource_storage.blocks.get_model_by_name(block))
                .ambient_occlusion
        }
    }

    #[inline]
    fn smooth_light<T: ChunkAccess>(
        chunks: &T,
        resource_storage: &ResourceStorage,
        world_position: glam::IVec3,
        light_source: glam::IVec3,
        corners: [[glam::IVec3; 3]; 4],
        have_ao: bool,
    ) -> ([f32; 4], [u8; 4]) {
        let mut aos = [1.0; 4];
        let mut lights = [0; 4];
        let init_light = chunks.get_light_level(light_source);

        for (([side1, side2, corner], ao), light) in corners.into_iter().zip(&mut aos).zip(&mut lights) {
            if have_ao {
                let side1_block: glam::IVec3 = world_position + side1;
                let side2_block: glam::IVec3 = world_position + side2;
                let corner_block: glam::IVec3 = world_position + corner;

                let (side1_block, side1_light) = chunks.get_block_with_light_level(side1_block);
                let (side2_block, side2_light) = chunks.get_block_with_light_level(side2_block);
                let (corner_block, corner_light) = chunks.get_block_with_light_level(corner_block);

                let block_light = (Chunk::block_light_from_level(init_light)
                    + Chunk::block_light_from_level(side1_light)
                    + Chunk::block_light_from_level(side2_light)
                    + Chunk::block_light_from_level(corner_light))
                    / 4;

                let sky_light = (Chunk::sky_light_from_level(init_light)
                    + Chunk::sky_light_from_level(side1_light)
                    + Chunk::sky_light_from_level(side2_light)
                    + Chunk::sky_light_from_level(corner_light))
                    / 4;

                *light = (sky_light << 4) | (block_light & 0xF);

                *ao = vertex_ao(
                    Self::does_block_have_ao(resource_storage, side1_block.map_or(0, |state| state.id)),
                    Self::does_block_have_ao(resource_storage, side2_block.map_or(0, |state| state.id)),
                    Self::does_block_have_ao(resource_storage, corner_block.map_or(0, |state| state.id)),
                );
            } else {
                *light = chunks.get_light_level(light_source);
            }
        }

        (aos, lights)
    }

    #[inline]
    fn blocky_with_ao<T: ChunkAccess>(
        chunks: &T,
        resource_storage: &ResourceStorage,
        world_position: glam::IVec3,
        light_source: glam::IVec3,
        corners: [[glam::IVec3; 3]; 4],
        have_ao: bool,
    ) -> ([f32; 4], [u8; 4]) {
        let mut aos: [f32; 4] = [1.0; 4];
        let light = chunks.get_light_level(light_source);

        for ([side1, side2, corner], ao) in corners.into_iter().zip(&mut aos) {
            if have_ao {
                *ao = vertex_ao(
                    Self::does_block_have_ao(resource_storage, chunks.get_block(world_position + side1).map_or(0, |state| state.id)),
                    Self::does_block_have_ao(resource_storage, chunks.get_block(world_position + side2).map_or(0, |state| state.id)),
                    Self::does_block_have_ao(resource_storage, chunks.get_block(world_position + corner).map_or(0, |state| state.id)),
                );
            }
        }

        (aos, [light; 4])
    }

    #[inline]
    fn blocky<T: ChunkAccess>(
        chunks: &T,
        _: &ResourceStorage,
        _: glam::IVec3,
        light_source: glam::IVec3,
        _: [[glam::IVec3; 3]; 4],
        _: bool,
    ) -> ([f32; 4], [u8; 4]) {
        let light = chunks.get_light_level(light_source);

        ([0.0; 4], [light; 4])
    }

    #[inline]
    #[allow(clippy::type_complexity)]
    pub const fn get_light_fn<T: ChunkAccess>(self) -> fn(&T, &ResourceStorage, glam::IVec3, glam::IVec3, [[glam::IVec3; 3]; 4], bool) -> ([f32; 4], [u8; 4]) {
        match self {
            Self::Smooth => Self::smooth_light::<T>,
            Self::BlockyWithAO => Self::blocky_with_ao::<T>,
            Self::Blocky => Self::blocky::<T>,
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct GraphicsSettings {
    pub light_style: LightStyle,
    pub render_shape: RenderShape,
    pub vsync: bool,
}

impl Default for GraphicsSettings {
    fn default() -> Self {
        Self {
            light_style: LightStyle::Smooth,
            render_shape: RenderShape::Circle(12),
            vsync: true,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Settings {
    pub graphics: GraphicsSettings,
    pub debugging: Debugging,
}
