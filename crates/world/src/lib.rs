#![allow(
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::unreadable_literal,
    clippy::missing_errors_doc
)]

mod bfs_light;
mod block;
mod chunk;
mod chunk_manager;
mod entity;
mod random;

use std::sync::LazyLock;

use meralus_shared::{DPoint2D, DPoint3D, IPoint2D, IPoint3D, USizePoint2D, USizePoint3D};
pub use serde_json::Error as JsonError;

pub use self::{
    bfs_light::{BfsLight, LightNode},
    block::{
        Axis, BlockCondition, BlockElement, BlockFace, BlockModel, BlockState, BlockStates, ConditionValue, Corner, ElementRotation, Face, Faces, Property,
        PropertyValue, TextureId, TexturePath, TextureRef,
    },
    chunk::{
        CHUNK_HEIGHT, CHUNK_HEIGHT_F32, CHUNK_HEIGHT_F64, CHUNK_HEIGHT_I32, CHUNK_HEIGHT_U16, Chunk, SUBCHUNK_COUNT, SUBCHUNK_COUNT_F32, SUBCHUNK_COUNT_I32,
        SUBCHUNK_COUNT_U16, SUBCHUNK_SIZE, SUBCHUNK_SIZE_F32, SUBCHUNK_SIZE_F64, SUBCHUNK_SIZE_I32, SUBCHUNK_SIZE_U16, SubChunk,
    },
    chunk_manager::ChunkManager,
    entity::{EntityElement, EntityElementData, EntityElementFace, EntityModel, EntityTexture},
};
use crate::random::Random;

pub fn vec_to_boxed_array<T, const S: usize>(vec: Vec<T>) -> Box<[T; S]> {
    let boxed_slice = vec.into_boxed_slice();

    let ptr = Box::into_raw(boxed_slice).cast::<[T; S]>();

    unsafe { Box::from_raw(ptr) }
}

struct Fbm<const O: usize> {
    noise_generators: Box<[Perlin; O]>,
}

impl<const O: usize> Fbm<O> {
    pub fn new(random: &mut Random) -> Self {
        Self {
            noise_generators: vec_to_boxed_array((0..O).map(|_| Perlin::new(random)).collect()),
        }
    }

    fn generate_noise(&self, offset: DPoint3D, size: IPoint3D, scale: DPoint3D) -> [f64; TERRAIN_NOISE_SIZE] {
        let mut noise_data = [0.0; TERRAIN_NOISE_SIZE];
        let mut frequency = 1.0;

        for generator in self.noise_generators.as_ref() {
            generator.generate_noise3d(&mut noise_data, offset, size, scale * frequency, frequency);

            frequency /= 2.0;
        }

        noise_data
    }

    //     public double[] a(double[] adouble, double d0, double d1, int i, int j,
    // double d2, double d3, double d4) {     return this.a(adouble, d0, d1, i,
    // j, d2, d3, d4, 0.5D); }

    fn generate_noise2d(&self, offset: DPoint2D, size: IPoint2D, mut scale: DPoint3D, frequency_step: f64) -> Vec<f64> {
        scale.x /= 1.5;
        scale.y /= 1.5;

        let mut noise_data = vec![0.0; size.x as usize * size.y as usize];

        let mut frequency = 1.0;
        let mut xy_scale = 1.0;

        for generator in self.noise_generators.as_ref() {
            generator.generate_noise2d(
                &mut noise_data,
                offset,
                size,
                DPoint3D::new(scale.x * xy_scale, scale.y * xy_scale, 0.55 / frequency),
            );

            xy_scale *= scale.z;
            frequency *= frequency_step;
        }

        noise_data
    }
}

const TERRAIN_NOISE_SIZE: usize = K as usize * B2 as usize * L as usize;
const B0: u8 = 4;
const B1: u8 = 64;
const B2: u8 = 17;
const K: u8 = B0 + 1;
const L: u8 = B0 + 1;

pub struct ChunkGenerator {
    biome_generator: BiomeGenerator,

    terrain_noise2: Fbm<16>,
    terrain_noise3: Fbm<16>,
    terrain_noise1: Fbm<8>,
    sand_and_gravel_noise_generator: Fbm<4>,
    stone_noise_generator: Fbm<4>,

    terrain_noise4: Fbm<10>,
    terrain_noise5: Fbm<16>,
}

impl ChunkGenerator {
    pub fn new(seed: i64) -> Self {
        let mut random = Random::new(seed);

        let biome_generator = BiomeGenerator::new(seed);

        let terrain_noise2 = Fbm::new(&mut random);
        let terrain_noise3 = Fbm::new(&mut random);
        let terrain_noise1 = Fbm::new(&mut random);
        let sand_and_gravel_noise_generator = Fbm::new(&mut random);
        let stone_noise_generator = Fbm::new(&mut random);
        let terrain_noise4 = Fbm::new(&mut random);
        let terrain_noise5 = Fbm::new(&mut random);

        Self {
            biome_generator,
            terrain_noise2,
            terrain_noise3,
            terrain_noise1,
            sand_and_gravel_noise_generator,
            stone_noise_generator,
            terrain_noise4,
            terrain_noise5,
        }
    }

    pub fn generate_bare_terrain<T: BlockSource>(&self, chunk: &mut Chunk, block_source: &T, biome_cache: &BiomeNoise) {
        let offset = IPoint3D::new(chunk.origin.x, 0, chunk.origin.y) * i32::from(B0);
        let terrain_noise = self.generate_terrain_noise(offset, IPoint3D::new(K.into(), B2.into(), L.into()), biome_cache);

        let air = block_source.get_block_id("air");
        let stone = block_source.get_block_id("stone");
        let water = block_source.get_block_id("water");
        let ice = block_source.get_block_id("ice");

        for i1 in 0..(B0 as usize) {
            for j1 in 0..(B0 as usize) {
                for k1 in 0..16 {
                    let d0 = 0.125;

                    let mut d1 = terrain_noise[(i1 * (L as usize) + j1) * (B2 as usize) + k1];
                    let mut d2 = terrain_noise[(i1 * (L as usize) + j1 + 1) * (B2 as usize) + k1];
                    let mut d3 = terrain_noise[((i1 + 1) * (L as usize) + j1) * (B2 as usize) + k1];
                    let mut d4 = terrain_noise[((i1 + 1) * (L as usize) + j1 + 1) * (B2 as usize) + k1];

                    let d5 = (terrain_noise[(i1 * (L as usize) + j1) * (B2 as usize) + k1 + 1] - d1) * d0;
                    let d6 = (terrain_noise[(i1 * (L as usize) + j1 + 1) * (B2 as usize) + k1 + 1] - d2) * d0;
                    let d7 = (terrain_noise[((i1 + 1) * (L as usize) + j1) * (B2 as usize) + k1 + 1] - d3) * d0;
                    let d8 = (terrain_noise[((i1 + 1) * (L as usize) + j1 + 1) * (B2 as usize) + k1 + 1] - d4) * d0;

                    for l1 in 0..8 {
                        let d9 = 0.25;
                        let mut d10 = d1;
                        let mut d11 = d2;
                        let d12 = (d3 - d1) * d9;
                        let d13 = (d4 - d2) * d9;

                        for i2 in 0..4 {
                            let mut j2 = (i2 + i1 * 4) << 11 | (j1 * 4) << 7 | (k1 * 8 + l1);
                            let d14 = 0.25;
                            let mut d15 = d10;
                            let d16 = (d11 - d10) * d14;

                            for k2 in 0..4 {
                                let temp = biome_cache.temp[(i1 * 4 + i2) * 16 + j1 * 4 + k2];
                                let block_data = if d15 > 0.0 {
                                    stone
                                } else if k1 * 8 + l1 < B1.into() {
                                    if temp < 0.5 && k1 * 8 + l1 >= const { B1 - 1 }.into() { ice } else { water }
                                } else {
                                    air
                                };

                                chunk.set_block(USizePoint3D::new(j2 >> 11, j2 & 127, (j2 >> 7) & 15), block_data);

                                j2 += 128;
                                d15 += d16;
                            }

                            d10 += d12;
                            d11 += d13;
                        }

                        d1 += d5;
                        d2 += d6;
                        d3 += d7;
                        d4 += d8;
                    }
                }
            }
        }
    }

    pub fn generate_biome_terarain<T: BlockSource>(&self, chunk: &mut Chunk, random: &mut Random, block_source: &T, biome_cache: &BiomeNoise) {
        let top_start_y: u8 = 64;
        let d0: f64 = 0.03125;
        let chunk_origin = chunk.origin.as_::<f64>() * 16.0;

        let sand_noise =
            self.sand_and_gravel_noise_generator
                .generate_noise(chunk_origin.extend(0.0), IPoint3D::new(16, 16, 1), DPoint3D::splat(d0).with_z(1.0));

        let gravel_noise = self.sand_and_gravel_noise_generator.generate_noise(
            DPoint3D::new(chunk_origin.x, 109.0134, chunk_origin.y),
            IPoint3D::new(16, 1, 16),
            DPoint3D::splat(d0).with_y(1.0),
        );

        let stone_noise = self
            .stone_noise_generator
            .generate_noise(chunk_origin.extend(0.0), IPoint3D::new(16, 16, 1), DPoint3D::splat(d0 * 2.0));

        let air = block_source.get_block_id("air");
        let stone = block_source.get_block_id("stone");
        let grass_block = block_source.get_block_id("grass_block");
        let dirt = block_source.get_block_id("dirt");
        let sand = block_source.get_block_id("sand");
        let snow = block_source.get_block_id("snow");
        let water = block_source.get_block_id("water");

        for z in 0..16 {
            for x in 0..16 {
                let biome = biome_cache.biomes[z + x * 16];
                let flag = random.next_f64().mul_add(0.2, sand_noise[z + x * 16]) > 0.0;
                let flag1 = random.next_f64().mul_add(0.2, gravel_noise[z + x * 16]) > 3.0;

                let i1 = random.next_f64().mul_add(0.25, stone_noise[z + x * 16] / 3.0 + 3.0) as i32;
                let mut j1 = -1;

                let mut top = biome.top(sand, snow, grass_block);
                let mut bottom = biome.bottom(sand, dirt);
                let mut y = 127;

                while y >= 0 {
                    let position = USizePoint3D::new(x, y as usize, z);

                    if y <= random.next_i32(5) {
                        // LegacyUtil173.setBlockData(chunkData, l1,
                        // BlockConstants.BEDROCK);
                    } else {
                        let current_block = chunk.get_block(position).filter(|&b| b != 0);

                        if current_block.is_none() {
                            j1 = -1;
                        } else if current_block == Some(stone) {
                            if j1 == -1 {
                                if i1 <= 0 {
                                    top = air;
                                    bottom = stone;
                                } else if y >= i32::from(top_start_y) - 4 && y <= i32::from(top_start_y) + 1 {
                                    top = biome.top(sand, snow, grass_block);
                                    bottom = biome.bottom(sand, dirt);

                                    if flag1 {
                                        top = air;
                                    }

                                    // if flag1 {
                                    //     bottom = 2/* BlockConstants.GRAVEL */;
                                    // }

                                    if flag {
                                        top = sand;
                                        bottom = sand;
                                    }
                                }

                                if y < top_start_y as i32 && top == air {
                                    top = water;
                                }

                                j1 = i1;

                                if y >= i32::from(top_start_y) - 1 {
                                    chunk.set_block(position, top);
                                } else {
                                    chunk.set_block(position, bottom);
                                }
                            } else if j1 > 0 {
                                j1 -= 1;

                                chunk.set_block(position, bottom);
                                // LegacyUtil173.setBlockData(chunkData, l1,
                                // b2);

                                // if (j1 == 0 && b2 == BlockConstants.SAND) {
                                //     j1 = self.random.next_i32(4);
                                //     b2 = BlockConstants.SANDSTONE;
                                // }
                            }
                        }
                    }

                    y -= 1;
                }
            }
        }
    }

    pub fn generate_unpopulated_chunk_data<T: BlockSource>(&self, chunk: &mut Chunk, block_source: &T) {
        let mut random = Random::new(i64::from(chunk.origin.x) * 341873128712 + i64::from(chunk.origin.y) * 132897987541);

        let biome_noise_cache = self.biome_generator.get_biome_noise(chunk.origin * 16, IPoint2D::splat(16));

        for z in 0..16 {
            for x in 0..16 {
                let biome = biome_noise_cache.biomes[z | (x << 4)];

                chunk.set_biome_unchecked(USizePoint2D::new(x, z), biome);
            }
        }

        self.generate_bare_terrain(chunk, block_source, &biome_noise_cache);
        self.generate_biome_terarain(chunk, &mut random, block_source, &biome_noise_cache);
        // this.caveGenerator.generate(this.world, chunkX, chunkZ, chunkData);
    }

    fn generate_terrain_noise(&self, offset: IPoint3D, size: IPoint3D, biome_cache: &BiomeNoise) -> [f64; TERRAIN_NOISE_SIZE] {
        let mut noise = [0.0; TERRAIN_NOISE_SIZE];

        let d0 = 684.412;
        let d1 = 684.412;
        let scale = DPoint3D::new(d0, d1, d0);

        let offset = offset.as_::<f64>();
        let offset2 = offset.with_y(10.0);
        let size2 = size.with_y(1);

        let terrain_noise4 = self.terrain_noise4.generate_noise(offset2, size2, DPoint3D::new(1.121, 1.0, 1.121));
        let terrain_noise5 = self.terrain_noise5.generate_noise(offset2, size2, DPoint3D::new(200.0, 1.0, 200.0));
        let terrain_noise1 = self
            .terrain_noise1
            .generate_noise(offset, size, DPoint3D::new(scale.x / 80.0, scale.y / 160.0, scale.z / 80.0));

        let terrain_noise2 = self.terrain_noise2.generate_noise(offset, size, scale);
        let terrain_noise3 = self.terrain_noise3.generate_noise(offset, size, scale);

        let mut k1 = 0;
        let mut l1 = 0;
        let i2 = 16 / size.x;

        for x in 0..size.x {
            let k2 = x * i2 + i2 / 2;

            for z in 0..size.z {
                let i3 = z * i2 + i2 / 2;
                let d2 = biome_cache.temp[(k2 * 16 + i3) as usize];
                let d3 = biome_cache.rain[(k2 * 16 + i3) as usize] * d2;
                let mut d4 = 1.0 - d3;

                d4 *= d4;
                d4 *= d4;
                d4 = 1.0 - d4;

                let mut d5 = (terrain_noise4[l1] + 256.0) / 512.0;

                d5 *= d4;

                if d5 > 1.0 {
                    d5 = 1.0;
                }

                let mut d6 = terrain_noise5[l1] / 8000.0;

                if d6 < 0.0 {
                    d6 = -d6 * 0.3;
                }

                d6 = d6.mul_add(3.0, -2.0);

                if d6 < 0.0 {
                    d6 /= 2.0;
                    if d6 < -1.0 {
                        d6 = -1.0;
                    }

                    d6 /= 1.4;
                    d6 /= 2.0;
                    d5 = 0.0;
                } else {
                    if d6 > 1.0 {
                        d6 = 1.0;
                    }

                    d6 /= 8.0;
                }

                if d5 < 0.0 {
                    d5 = 0.0;
                }

                d5 += 0.5;
                d6 = d6 * f64::from(size.y) / 16.0;

                let d7 = d6.mul_add(4.0, f64::from(size.y) / 2.0);

                l1 += 1;

                for y in 0..size.y {
                    let mut d9 = (f64::from(y) - d7) * 12.0 / d5;

                    if d9 < 0.0 {
                        d9 *= 4.0;
                    }

                    let d10 = terrain_noise2[k1] / 512.0;
                    let d11 = terrain_noise3[k1] / 512.0;
                    let d12 = f64::midpoint(terrain_noise1[k1] / 10.0, 1.0);

                    let mut d8 = if d12 < 0.0 {
                        d10
                    } else if d12 > 1.0 {
                        d11
                    } else {
                        (d11 - d10).mul_add(d12, d10)
                    };

                    d8 -= d9;

                    if y > size.y - 4 {
                        let d13 = f64::from((y - (size.y - 4)) as f32 / 3.0f32);

                        d8 = d8.mul_add(1.0 - d13, -10.0 * d13);
                    }

                    noise[k1] = d8;
                    k1 += 1;
                }
            }
        }

        noise
    }

    pub fn populate<T: BlockSource>(&self, chunk_manager: &mut ChunkManager, block_source: &T, world_seed: i64, chunk: IPoint2D) {
        let origin = chunk * SUBCHUNK_SIZE_I32;
        // let _biomebase = self.biome_generator.get_biome_noise(origin +
        // SUBCHUNK_SIZE_I32, IPoint2D::ONE).biomes[0];
        let mut random = Random::new(world_seed);

        let i1 = random.next_i64() / 2 * 2 + 1;
        let j1 = random.next_i64() / 2 * 2 + 1;

        random.set_seed(i64::from(chunk.x).wrapping_mul(i1).wrapping_add(i64::from(chunk.y).wrapping_mul(j1)) ^ world_seed);

        // let d0 = 0.25;

        if random.next_i32(4) == 0 {
            let k1 = origin.x + random.next_i32(16) + 8;
            let l1 = random.next_i32(128);
            let i2 = origin.y + random.next_i32(16) + 8;

            LakesGenerator {
                primary_block: block_source.get_block_id("water"),
            }
            .populate(chunk_manager, &mut random, IPoint3D::new(k1, l1, i2));
        }
    }
}

pub trait BlockSource {
    fn get_block_id(&self, name: &str) -> u8;
    fn blocks_light(&self, block: u8) -> bool;
    fn light_consumption(&self, block: u8) -> u8;
}

struct Perlin {
    permute_table: [i32; 512],
    x_offset: f64,
    y_offset: f64,
    z_offset: f64,
}

static F: LazyLock<f64> = LazyLock::new(|| 0.5 * (3.0f64.sqrt() - 1.0));
static G: LazyLock<f64> = LazyLock::new(|| (3.0 - 3.0f64.sqrt()) / 6.0);

#[allow(clippy::similar_names, dead_code)]
impl Perlin {
    const D: [[i32; 3]; 12] = [
        [1, 1, 0],
        [-1, 1, 0],
        [1, -1, 0],
        [-1, -1, 0],
        [1, 0, 1],
        [-1, 0, 1],
        [1, 0, -1],
        [-1, 0, -1],
        [0, 1, 1],
        [0, -1, 1],
        [0, 1, -1],
        [0, -1, -1],
    ];

    fn new(random: &mut Random) -> Self {
        let x_offset = random.next_f64() * 256.0;
        let y_offset = random.next_f64() * 256.0;
        let z_offset = random.next_f64() * 256.0;
        let mut permute_table = [0; 512];

        for (index, value) in permute_table.iter_mut().enumerate().take(256) {
            *value = index as i32;
        }

        for index in 0..256 {
            let i2 = (random.next_i32(256 - index as i32) + index as i32) as usize;

            permute_table.swap(index, i2);
            permute_table[index + 256] = permute_table[index];
        }

        Self {
            permute_table,
            x_offset,
            y_offset,
            z_offset,
        }
    }

    fn get3d(&self, x: f64, y: f64, z: f64) -> f64 {
        let mut x = x + self.x_offset;
        let mut y = y + self.y_offset;
        let mut z = z + self.z_offset;

        let mut x_i32 = x as i32;
        let mut y_i32 = y as i32;
        let mut z_i32 = z as i32;

        if x < f64::from(x_i32) {
            x_i32 -= 1;
        }

        if y < f64::from(y_i32) {
            y_i32 -= 1;
        }

        if z < f64::from(z_i32) {
            z_i32 -= 1;
        }

        let x_modulo = x_i32 & 255;
        let y_modulo = y_i32 & 255;
        let z_modulo = z_i32 & 255;

        x -= f64::from(x_i32);
        y -= f64::from(y_i32);
        z -= f64::from(z_i32);

        let final_x = x * x * x * x.mul_add(x.mul_add(6.0, -15.0), 10.0);
        let final_y = y * y * y * y.mul_add(y.mul_add(6.0, -15.0), 10.0);
        let final_z = z * z * z * z.mul_add(z.mul_add(6.0, -15.0), 10.0);

        let hash_offset = self.permute_table[x_modulo as usize] + y_modulo;
        let hash1 = self.permute_table[hash_offset as usize] + z_modulo;
        let hash2 = self.permute_table[hash_offset as usize + 1] + z_modulo;

        let hash_offset = self.permute_table[x_modulo as usize + 1] + y_modulo;
        let hash3 = self.permute_table[hash_offset as usize] + z_modulo;
        let hash4 = self.permute_table[hash_offset as usize + 1] + z_modulo;

        Self::b(
            final_z,
            Self::b(
                final_y,
                Self::b(
                    final_x,
                    Self::a4(self.permute_table[hash1 as usize], x, y, z),
                    Self::a4(self.permute_table[hash3 as usize], x - 1.0, y, z),
                ),
                Self::b(
                    final_x,
                    Self::a4(self.permute_table[hash2 as usize], x, y - 1.0, z),
                    Self::a4(self.permute_table[hash4 as usize], x - 1.0, y - 1.0, z),
                ),
            ),
            Self::b(
                final_y,
                Self::b(
                    final_x,
                    Self::a4(self.permute_table[hash1 as usize + 1], x, y, z - 1.0),
                    Self::a4(self.permute_table[hash3 as usize + 1], x - 1.0, y, z - 1.0),
                ),
                Self::b(
                    final_x,
                    Self::a4(self.permute_table[hash2 as usize + 1], x, y - 1.0, z - 1.0),
                    Self::a4(self.permute_table[hash4 as usize + 1], x - 1.0, y - 1.0, z - 1.0),
                ),
            ),
        )
    }

    fn b(x: f64, y: f64, z: f64) -> f64 {
        x.mul_add(z - y, y)
    }

    fn a3i(hash: i32, x: f64, y: f64) -> f64 {
        let hash_modulo = hash & 15;
        let side = f64::from(1 - ((hash_modulo & 8) >> 3)) * x;
        let axis = if hash_modulo < 4 {
            0.0
        } else if hash_modulo != 12 && hash_modulo != 14 {
            y
        } else {
            x
        };

        (if (hash_modulo & 1) == 0 { side } else { -side }) + (if (hash_modulo & 2) == 0 { axis } else { -axis })
    }

    fn a4(hash: i32, x: f64, y: f64, z: f64) -> f64 {
        let hash_modulo = hash & 15;
        let side = if hash_modulo < 8 { x } else { y };
        let axis = if hash_modulo < 4 {
            y
        } else if hash_modulo != 12 && hash_modulo != 14 {
            z
        } else {
            x
        };

        (if (hash_modulo & 1) == 0 { side } else { -side }) + (if (hash_modulo & 2) == 0 { axis } else { -axis })
    }

    fn get2d(&self, x: f64, y: f64) -> f64 {
        self.get3d(x, y, 0.0)
    }

    fn a_i32(d0: f64) -> i32 {
        if d0 > 0.0 { d0 as i32 } else { d0 as i32 - 1 }
    }

    fn a_f64(aint: &[i32], d0: f64, d1: f64) -> f64 {
        f64::from(aint[0]).mul_add(d0, f64::from(aint[1]) * d1)
    }

    fn generate_noise2d(&self, noise_data: &mut [f64], offset: DPoint2D, size: IPoint2D, scale: DPoint3D) {
        let mut index = 0;

        for x in 0..size.x {
            let x = (offset.x + f64::from(x)).mul_add(scale.x, self.x_offset);

            for y in 0..size.y {
                let y = (offset.y + f64::from(y)).mul_add(scale.y, self.y_offset);
                let d7 = (x + y) * *F;
                let j1 = Self::a_i32(x + d7);
                let k1 = Self::a_i32(y + d7);
                let d8 = f64::from(j1 + k1) * *G;
                let d9 = f64::from(j1) - d8;
                let d10 = f64::from(k1) - d8;
                let d11 = x - d9;
                let d12 = y - d10;
                let [b0, b1]: [u8; 2] = if d11 > d12 { [1, 0] } else { [0, 1] };

                let d13 = d11 - f64::from(b0) + *G;
                let d14 = d12 - f64::from(b1) + *G;
                let d15 = 2.0f64.mul_add(*G, d11 - 1.0);
                let d16 = 2.0f64.mul_add(*G, d12 - 1.0);
                let l1 = j1 & 255;
                let i2 = k1 & 255;
                let j2 = self.permute_table[(l1 + self.permute_table[i2 as usize]) as usize] % 12;
                let k2 = self.permute_table[(l1 + i32::from(b0) + self.permute_table[(i2 + i32::from(b1)) as usize]) as usize] % 12;
                let l2 = self.permute_table[(l1 + 1 + self.permute_table[(i2 + 1) as usize]) as usize] % 12;

                let d17 = d12.mul_add(-d12, d11.mul_add(-d11, 0.5));
                let d19 = d14.mul_add(-d14, d13.mul_add(-d13, 0.5));
                let d21 = 0.5 - d15 * d15 - d16 * d16;

                let x = if d17 < 0.0 {
                    0.0
                } else {
                    d17 * d17 * d17 * d17 * Self::a_f64(&Self::D[j2 as usize], d11, d12)
                };
                let y = if d19 < 0.0 {
                    0.0
                } else {
                    d19 * d19 * d19 * d19 * Self::a_f64(&Self::D[k2 as usize], d13, d14)
                };
                let z = if d21 < 0.0 {
                    0.0
                } else {
                    d21 * d21 * d21 * d21 * Self::a_f64(&Self::D[l2 as usize], d15, d16)
                };

                noise_data[index] += 70.0 * (x + y + z) * scale.z;

                index += 1;
            }
        }
    }

    fn generate_noise3d(&self, data: &mut [f64], offset: DPoint3D, size: IPoint3D, scale: DPoint3D, frequency: f64) {
        let inv_frequency = 1.0 / frequency;
        let mut data_offset = 0;

        if size.y == 1 {
            for x in 0..size.x {
                let mut x = (offset.x + f64::from(x)).mul_add(scale.x, self.x_offset);
                let mut x_i32 = x as i32;

                if x < f64::from(x_i32) {
                    x_i32 -= 1;
                }

                x -= f64::from(x_i32);

                let x_modulo = x_i32 & 255;
                let final_x = x * x * x * x.mul_add(x.mul_add(6.0, -15.0), 10.0);

                for z in 0..size.z {
                    let mut z = (offset.z + f64::from(z)).mul_add(scale.z, self.z_offset);
                    let mut z_i32 = z as i32;

                    if z < f64::from(z_i32) {
                        z_i32 -= 1;
                    }

                    z -= f64::from(z_i32);

                    let z_modulo = z_i32 & 255;
                    let hash1 = self.permute_table[self.permute_table[x_modulo as usize] as usize] + z_modulo;
                    let hash2 = self.permute_table[self.permute_table[x_modulo as usize + 1] as usize] + z_modulo;

                    data[data_offset as usize] += Self::b(
                        z * z * z * z.mul_add(z.mul_add(6.0, -15.0), 10.0),
                        Self::b(
                            final_x,
                            Self::a3i(self.permute_table[hash1 as usize], x, z),
                            Self::a4(self.permute_table[hash2 as usize], x - 1.0, 0.0, z),
                        ),
                        Self::b(
                            final_x,
                            Self::a4(self.permute_table[hash1 as usize + 1], x, 0.0, z - 1.0),
                            Self::a4(self.permute_table[hash2 as usize + 1], x - 1.0, 0.0, z - 1.0),
                        ),
                    ) * inv_frequency;

                    data_offset += 1;
                }
            }
        } else {
            let mut old_y = -1;
            let mut val1 = 0.0;
            let mut val2 = 0.0;
            let mut val3 = 0.0;
            let mut val4 = 0.0;

            for x in 0..size.x {
                let mut x = (offset.x + f64::from(x)).mul_add(scale.x, self.x_offset);
                let mut x_i32 = x as i32;

                if x < f64::from(x_i32) {
                    x_i32 -= 1;
                }

                x -= f64::from(x_i32);

                let x_modulo = x_i32 & 255;
                let final_x = x * x * x * x.mul_add(x.mul_add(6.0, -15.0), 10.0);

                for z in 0..size.z {
                    let mut z = (offset.z + f64::from(z)).mul_add(scale.z, self.z_offset);
                    let mut z_i32 = z as i32;

                    if z < f64::from(z_i32) {
                        z_i32 -= 1;
                    }

                    z -= f64::from(z_i32);

                    let z_modulo = z_i32 & 255;
                    let final_z = z * z * z * z.mul_add(z.mul_add(6.0, -15.0), 10.0);

                    for y in 0..size.y {
                        let is_first = y == 0;
                        let mut y = (offset.y + f64::from(y)).mul_add(scale.y, self.y_offset);
                        let mut y_i32 = y as i32;

                        if y < f64::from(y_i32) {
                            y_i32 -= 1;
                        }

                        y -= f64::from(y_i32);

                        let y_modulo = y_i32 & 255;
                        let final_y = y * y * y * y.mul_add(y.mul_add(6.0, -15.0), 10.0);

                        if is_first || y_modulo != old_y {
                            old_y = y_modulo;

                            let hash_offset = self.permute_table[x_modulo as usize] + y_modulo;
                            let hash1 = self.permute_table[hash_offset as usize] + z_modulo;
                            let hash2 = self.permute_table[hash_offset as usize + 1] + z_modulo;

                            let hash_offset = self.permute_table[x_modulo as usize + 1] + y_modulo;
                            let hash3 = self.permute_table[hash_offset as usize] + z_modulo;
                            let hash4 = self.permute_table[hash_offset as usize + 1] + z_modulo;

                            val1 = Self::b(
                                final_x,
                                Self::a4(self.permute_table[hash1 as usize], x, y, z),
                                Self::a4(self.permute_table[hash3 as usize], x - 1.0, y, z),
                            );

                            val2 = Self::b(
                                final_x,
                                Self::a4(self.permute_table[hash2 as usize], x, y - 1.0, z),
                                Self::a4(self.permute_table[hash4 as usize], x - 1.0, y - 1.0, z),
                            );

                            val3 = Self::b(
                                final_x,
                                Self::a4(self.permute_table[hash1 as usize + 1], x, y, z - 1.0),
                                Self::a4(self.permute_table[hash3 as usize + 1], x - 1.0, y, z - 1.0),
                            );

                            val4 = Self::b(
                                final_x,
                                Self::a4(self.permute_table[hash2 as usize + 1], x, y - 1.0, z - 1.0),
                                Self::a4(self.permute_table[hash4 as usize + 1], x - 1.0, y - 1.0, z - 1.0),
                            );
                        }

                        data[data_offset as usize] += Self::b(final_z, Self::b(final_y, val1, val2), Self::b(final_y, val3, val4)) * inv_frequency;
                        data_offset += 1;
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BiomeBase {
    Rainforest,
    Swampland,
    SeasonalForest,
    Forest,
    Savanna,
    Shrubland,
    Taiga,
    Desert,
    Plains,
    IceDesert,
    Tundra,
    Hell,
    Sky,
}

impl BiomeBase {
    const LOOKUP: [Self; 64 * 64] = const {
        let mut table = [const { Self::Sky }; 64 * 64];
        let mut i = 0;
        let mut k = 0;

        loop {
            table[i + k * 64] = Self::get_by_temp_rain(i as f32 / 63.0, k as f32 / 63.0);

            k += 1;

            if i >= 63 && k >= 63 {
                break;
            } else if k >= 63 {
                i += 1;
                k = 0;
            }
        }

        table
    };

    const fn get_by_temp_rain(temperature: f32, raininess: f32) -> Self {
        let rain = raininess * temperature;

        if temperature < 0.1 {
            Self::Tundra
        } else if rain < 0.2 {
            if temperature < 0.5 {
                Self::Tundra
            } else if temperature < 0.95 {
                Self::Savanna
            } else {
                Self::Desert
            }
        } else if rain > 0.5 && temperature < 0.7 {
            Self::Swampland
        } else if temperature < 0.5 {
            Self::Taiga
        } else if temperature < 0.97 {
            if rain < 0.35 { Self::Shrubland } else { Self::Forest }
        } else if rain < 0.45 {
            Self::Plains
        } else if rain < 0.9 {
            Self::SeasonalForest
        } else {
            Self::Rainforest
        }
    }

    fn a(temperature: f64, raininess: f64) -> Self {
        let i = (temperature * 63.0) as usize;
        let j = (raininess * 63.0) as usize;

        Self::LOOKUP[i + j * 64]
    }

    pub const fn top(self, sand: u8, snow: u8, grass_block: u8) -> u8 {
        match self {
            Self::Desert | Self::IceDesert => sand, // SAND
            Self::Taiga | Self::Tundra => snow,     // SAND
            _ => grass_block,                       // GRASS_BLOCK
        }
    }

    pub const fn bottom(self, sand: u8, dirt: u8) -> u8 {
        match self {
            Self::Desert | Self::IceDesert => sand, // SAND
            _ => dirt,                              // DIRT
        }
    }
}

struct BiomeGenerator {
    temp: Fbm<4>,
    rain: Fbm<4>,
    base: Fbm<2>,
}

pub struct BiomeNoise {
    pub biomes: Vec<BiomeBase>,
    pub temp: Vec<f64>,
    pub rain: Vec<f64>,
}

impl BiomeGenerator {
    pub fn new(seed: i64) -> Self {
        Self {
            temp: Fbm::new(&mut Random::new(seed * 9871)),
            rain: Fbm::new(&mut Random::new(seed * 39811)),
            base: Fbm::new(&mut Random::new(seed * 543321)),
        }
    }

    pub fn get_biome_noise(&self, origin: IPoint2D, size: IPoint2D) -> BiomeNoise {
        let mut temp = self
            .temp
            .generate_noise2d(origin.as_::<f64>(), size, DPoint3D::new(0.02500000037252903, 0.02500000037252903, 0.25), 0.5);

        let mut rain = self.rain.generate_noise2d(
            origin.as_::<f64>(),
            size,
            DPoint3D::new(0.05000000074505806, 0.05000000074505806, 0.3333333333333333),
            0.5,
        );

        let base_data = self
            .base
            .generate_noise2d(origin.as_::<f64>(), size, DPoint3D::new(0.25, 0.25, 0.5882352941176471), 0.5);

        let mut biomes = vec![BiomeBase::Sky; size.x as usize * size.y as usize];
        let mut index = 0;

        for _ in 0..size.x {
            for _ in 0..size.y {
                let d0 = base_data[index].mul_add(1.1, 0.5);
                let d1 = 0.01;
                let d2 = 1.0 - d1;
                let temperature = temp[index].mul_add(0.15, 0.7).mul_add(d2, d0 * d1);

                let d1 = 0.0020;
                let d2 = 1.0 - d1;

                let temperature = (1.0 - temperature).mul_add(-(1.0 - temperature), 1.0).clamp(0.0, 1.0);
                let raininess = rain[index].mul_add(0.15, 0.5).mul_add(d2, d0 * d1).clamp(0.0, 1.0);

                temp[index] = temperature;
                rain[index] = raininess;
                biomes[index] = BiomeBase::a(temperature, raininess);

                index += 1;
            }
        }

        BiomeNoise { biomes, temp, rain }
    }
}

struct LakesGenerator {
    primary_block: u8,
}

impl LakesGenerator {
    pub fn populate(&self, chunk_manager: &mut ChunkManager, random: &mut Random, mut center: IPoint3D) -> bool {
        center.x -= 8;
        center.z -= 8;

        while center.y > 0 && chunk_manager.get_block(center).is_none_or(|b| b == 0) {
            center.y -= 1;
        }

        center.y -= 4;

        let mut aboolean = [false; 2048];
        let l = random.next_i32(4) + 4;

        for _ in 0..l {
            let d0 = random.next_f64().mul_add(6.0, 3.0);
            let d1 = random.next_f64().mul_add(4.0, 2.0);
            let d2 = random.next_f64().mul_add(6.0, 3.0);
            let d3 = random.next_f64().mul_add(16.0 - d0 - 2.0, 1.0) + d0 / 2.0;
            let d4 = random.next_f64().mul_add(8.0 - d1 - 4.0, 2.0) + d1 / 2.0;
            let d5 = random.next_f64().mul_add(16.0 - d2 - 2.0, 1.0) + d2 / 2.0;

            for j1 in 1..15 {
                for k1 in 1..15 {
                    for l1 in 1..7 {
                        let d6 = ((j1 as f64) - d3) / (d0 / 2.0);
                        let d7 = ((l1 as f64) - d4) / (d1 / 2.0);
                        let d8 = ((k1 as f64) - d5) / (d2 / 2.0);
                        let d9 = d6 * d6 + d7 * d7 + d8 * d8;

                        if d9 < 1.0 {
                            aboolean[(j1 * 16 + k1) * 8 + l1] = true;
                        }
                    }
                }
            }
        }

        for x in 0..16 {
            for z in 0..16 {
                for y in 0..8 {
                    if !aboolean[(x * 16 + z) * 8 + y]
                        && (x < 15 && aboolean[((x + 1) * 16 + z) * 8 + y]
                            || x > 0 && aboolean[((x - 1) * 16 + z) * 8 + y]
                            || z < 15 && aboolean[(x * 16 + z + 1) * 8 + y]
                            || z > 0 && aboolean[(x * 16 + (z - 1)) * 8 + y]
                            || y < 7 && aboolean[(x * 16 + z) * 8 + y + 1]
                            || y > 0 && aboolean[(x * 16 + z) * 8 + (y - 1)])
                    {
                        let material = chunk_manager.get_block(center + IPoint3D::new(x as i32, y as i32, z as i32));

                        if y >= 4 && material == Some(self.primary_block) {
                            return false;
                        }

                        if y < 4 && material.is_none_or(|b| b == 0)
                        // || material == self.primary_block) && chunk.get_block_uncehced(centerX + i1, centerY + j2, centerZ + i2) != this.a.getMaterial())
                        {
                            return false;
                        }
                    }
                }
            }
        }

        for x in 0..16 {
            for z in 0..16 {
                for y in 0..8 {
                    if aboolean[(x * 16 + z) * 8 + y] {
                        chunk_manager.set_block(
                            center + IPoint3D::new(x as i32, y as i32, z as i32),
                            if y >= 4 { 0 } else { self.primary_block },
                        );
                    }
                }
            }
        }

        true
    }
}
