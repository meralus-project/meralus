mod biome;
mod chunk;
mod lakes;
mod noise;
mod trees;

pub use self::{
    biome::{BiomeGenerator, BiomeNoise},
    chunk::ChunkGenerator,
    lakes::LakesGenerator,
};

pub const B0: u8 = 4;
pub const B1: u8 = 64;
pub const B2: u8 = 17;
pub const K: u8 = B0 + 1;
pub const L: u8 = B0 + 1;
pub const TERRAIN_NOISE_SIZE: usize = K as usize * B2 as usize * L as usize;
