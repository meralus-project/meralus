#![allow(
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::unreadable_literal,
    clippy::missing_errors_doc
)]

mod block;
mod chunk;
mod chunk_manager;

pub use serde_json::Error as JsonError;

pub use self::{
    block::{
        Axis, BlockCondition, BlockElement, BlockFace, BlockModel, BlockState, BlockStates, ConditionValue, Corner, ElementRotation, Face, Faces, Property,
        PropertyValue, TextureId, TexturePath, TextureRef,
    },
    chunk::{
        CHUNK_HEIGHT, CHUNK_HEIGHT_F32, CHUNK_HEIGHT_F64, CHUNK_HEIGHT_I32, CHUNK_HEIGHT_U16, CHUNK_SIZE, CHUNK_SIZE_F32, CHUNK_SIZE_F64, CHUNK_SIZE_I32,
        CHUNK_SIZE_U16, Chunk, SUBCHUNK_COUNT, SUBCHUNK_COUNT_F32, SUBCHUNK_COUNT_I32, SUBCHUNK_COUNT_U16, SubChunk,
    },
    chunk_manager::ChunkManager,
};
