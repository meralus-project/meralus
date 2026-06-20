#![allow(
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::unreadable_literal,
    clippy::missing_errors_doc
)]

mod bfs_light;
mod biome;
mod chunk;
mod chunk_manager;

use core::fmt;

pub use self::{
    bfs_light::{BfsLight, LightNode},
    biome::BiomeBase,
    chunk::{
        CHUNK_HEIGHT, CHUNK_HEIGHT_F32, CHUNK_HEIGHT_F64, CHUNK_HEIGHT_I32, CHUNK_HEIGHT_U16, Chunk, SUBCHUNK_COUNT, SUBCHUNK_COUNT_F32, SUBCHUNK_COUNT_I32,
        SUBCHUNK_COUNT_U16, SUBCHUNK_SIZE, SUBCHUNK_SIZE_F32, SUBCHUNK_SIZE_F64, SUBCHUNK_SIZE_I32, SUBCHUNK_SIZE_U16, SubChunk, SubChunkBlockState,
    },
    chunk_manager::{ChunkAccess, ChunkCache, ChunkManager, ChunkStage, LocalChunkManager},
};

pub trait BlockSource {
    fn get_block_id(&self, name: &str) -> u8;
    fn blocks_light(&self, block: &str) -> bool;
    fn light_consumption(&self, block: &str) -> u8;
}

pub fn new_boxed_array<T, const S: usize>(boxed_slice: Box<[T]>) -> Box<[T; S]> {
    unsafe { Box::from_raw(Box::into_raw(boxed_slice).cast::<[T; S]>()) }
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(untagged))]
pub enum PropertyValue {
    Number(i64),
    Float(f32),
    String(String),
    Boolean(bool),
}

impl fmt::Display for PropertyValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(value) => value.fmt(f),
            Self::Float(value) => value.fmt(f),
            Self::String(value) => f.write_str(&format!("{value:?}")),
            Self::Boolean(value) => value.fmt(f),
        }
    }
}

impl Eq for PropertyValue {}
