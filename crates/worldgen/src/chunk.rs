use meralus_shared::{DPoint3D, IPoint2D, IPoint3D, Random, USizePoint2D, USizePoint3D};
use meralus_world::{BiomeBase, BlockSource, CHUNK_HEIGHT_I32, Chunk, ChunkAccess, SUBCHUNK_SIZE_I32, SubChunkBlockState};

use super::{BiomeGenerator, BiomeNoise, LakesGenerator, noise};
use crate::{
    B0, B1, B2, K, L, TERRAIN_NOISE_SIZE,
    trees::{ForestGenerator, TreesGenerator},
};

pub struct ChunkGenerator {
    biome_generator: BiomeGenerator,

    terrain_noise2: noise::Fbm<16>,
    terrain_noise3: noise::Fbm<16>,
    terrain_noise1: noise::Fbm<8>,
    sand_and_gravel_noise: noise::Fbm<4>,
    stone_noise: noise::Fbm<4>,

    terrain_noise4: noise::Fbm<10>,
    terrain_noise5: noise::Fbm<16>,

    tree_count_noise: noise::Fbm<8>,
}

impl ChunkGenerator {
    pub fn new(seed: i64) -> Self {
        let mut random = Random::new(seed);

        let biome_generator = BiomeGenerator::new(seed);

        let terrain_noise2 = noise::Fbm::new(&mut random);
        let terrain_noise3 = noise::Fbm::new(&mut random);
        let terrain_noise1 = noise::Fbm::new(&mut random);
        let sand_and_gravel_noise = noise::Fbm::new(&mut random);
        let stone_noise = noise::Fbm::new(&mut random);
        let terrain_noise4 = noise::Fbm::new(&mut random);
        let terrain_noise5 = noise::Fbm::new(&mut random);
        let tree_count_noise = noise::Fbm::new(&mut random);

        Self {
            biome_generator,
            terrain_noise2,
            terrain_noise3,
            terrain_noise1,
            sand_and_gravel_noise,
            stone_noise,
            terrain_noise4,
            terrain_noise5,
            tree_count_noise,
        }
    }

    pub fn generate_bare_terrain<T: BlockSource>(&self, chunk: &mut Chunk, block_source: &T, biome_cache: &BiomeNoise) {
        let offset = IPoint3D::new(chunk.origin.x, 0, chunk.origin.y) * i32::from(B0);
        let terrain_noise = self.generate_terrain_noise(offset, IPoint3D::new(K.into(), B2.into(), L.into()), biome_cache);

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
                                    SubChunkBlockState::new("game:stone")
                                } else if k1 * 8 + l1 < B1.into() {
                                    if temp < 0.5 && k1 * 8 + l1 >= const { B1 - 1 }.into() {
                                        SubChunkBlockState::new("game:ice")
                                    } else {
                                        SubChunkBlockState::new("game:water")
                                    }
                                } else {
                                    SubChunkBlockState::new("game:air")
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
        let chunk_origin = chunk.origin.as_dvec2() * 16.0;

        let sand_noise = self
            .sand_and_gravel_noise
            .generate_noise(chunk_origin.extend(0.0), IPoint3D::new(16, 16, 1), DPoint3D::splat(d0).with_z(1.0));

        let gravel_noise = self.sand_and_gravel_noise.generate_noise(
            DPoint3D::new(chunk_origin.x, 109.0134, chunk_origin.y),
            IPoint3D::new(16, 1, 16),
            DPoint3D::splat(d0).with_y(1.0),
        );

        let stone_noise = self
            .stone_noise
            .generate_noise(chunk_origin.extend(0.0), IPoint3D::new(16, 16, 1), DPoint3D::splat(d0 * 2.0));

        for z in 0..16 {
            for x in 0..16 {
                let biome = biome_cache.biomes[z + x * 16];
                let flag = random.next_f64().mul_add(0.2, sand_noise[z + x * 16]) > 0.0;
                let flag1 = random.next_f64().mul_add(0.2, gravel_noise[z + x * 16]) > 3.0;

                let i1 = random.next_f64().mul_add(0.25, stone_noise[z + x * 16] / 3.0 + 3.0) as i32;
                let mut j1 = -1;

                let mut top = biome.top(
                    SubChunkBlockState::new("game:sand"),
                    SubChunkBlockState::new("game:snow"),
                    SubChunkBlockState::new("game:grass_block"),
                );

                let mut bottom = biome.bottom(SubChunkBlockState::new("game:sand"), SubChunkBlockState::new("game:dirt"));

                for y in (0..128).rev() {
                    let position = USizePoint3D::new(x, y as usize, z);

                    if y <= random.next_i32(5) {
                        // LegacyUtil173.setBlockData(chunkData, l1,
                        // BlockConstants.BEDROCK);
                    } else {
                        let current_block = chunk.get_block(position).filter(|b| b.name != "game:air");

                        if current_block.is_none() {
                            j1 = -1;
                        } else if current_block.is_some_and(|state| state.name == "game:stone") {
                            if j1 == -1 {
                                if i1 <= 0 {
                                    top = SubChunkBlockState::new("game:air");
                                    bottom = SubChunkBlockState::new("game:stone");
                                } else if y >= i32::from(top_start_y) - 4 && y <= i32::from(top_start_y) + 1 {
                                    top = biome.top(
                                        SubChunkBlockState::new("game:sand"),
                                        SubChunkBlockState::new("game:snow"),
                                        SubChunkBlockState::new("game:grass_block"),
                                    );

                                    bottom = biome.bottom(SubChunkBlockState::new("game:sand"), SubChunkBlockState::new("game:dirt"));

                                    if flag1 {
                                        top = SubChunkBlockState::new("game:air");
                                    }

                                    // if flag1 {
                                    //     bottom = 2/* BlockConstants.GRAVEL */;
                                    // }

                                    if flag {
                                        top = SubChunkBlockState::new("game:sand");
                                        bottom = SubChunkBlockState::new("game:sand");
                                    }
                                }

                                if y < i32::from(top_start_y) && top.name == "game:air" {
                                    top = SubChunkBlockState::new("game:water");
                                }

                                j1 = i1;

                                if y >= i32::from(top_start_y) - 1 {
                                    chunk.set_block(position, top.clone());
                                } else {
                                    chunk.set_block(position, bottom.clone());
                                }
                            } else if j1 > 0 {
                                j1 -= 1;

                                chunk.set_block(position, bottom.clone());
                                // LegacyUtil173.setBlockData(chunkData, l1,
                                // b2);

                                // if (j1 == 0 && b2 == BlockConstants.SAND) {
                                //     j1 = self.random.next_i32(4);
                                //     b2 = BlockConstants.SANDSTONE;
                                // }
                            }
                        }
                    }
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

        let offset = offset.as_dvec3();
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
                        let d13 = f64::from((y - (size.y - 4)) as f32 / 3f32);

                        d8 = d8.mul_add(1.0 - d13, -10.0 * d13);
                    }

                    noise[k1] = d8;
                    k1 += 1;
                }
            }
        }

        noise
    }

    pub fn populate<C: ChunkAccess, T: BlockSource>(&self, chunk_manager: &mut C, block_source: &T, world_seed: i64, chunk: IPoint2D) {
        let origin = chunk * SUBCHUNK_SIZE_I32;
        let biomebase = self.biome_generator.get_biome_noise(origin + SUBCHUNK_SIZE_I32, IPoint2D::ONE).biomes[0];
        let mut random = Random::new(world_seed);

        let i1 = random.next_i64() / 2 * 2 + 1;
        let j1 = random.next_i64() / 2 * 2 + 1;

        random.set_seed(i64::from(chunk.x).wrapping_mul(i1).wrapping_add(i64::from(chunk.y).wrapping_mul(j1)) ^ world_seed);

        if random.next_i32(4) == 0 {
            let k1 = origin.x + random.next_i32(16) + 8;
            let l1 = random.next_i32(128);
            let i2 = origin.y + random.next_i32(16) + 8;

            LakesGenerator::new(block_source.get_block_id("water")).populate(chunk_manager, &mut random, IPoint3D::new(k1, l1, i2));
        }

        let d0 = 0.5;
        let k1 = ((random.next_f64().mul_add(
            4.0,
            self.tree_count_noise.generate_noise_for_coords(origin.x as f64 * d0, origin.y as f64 * d0) / 8.0,
        ) + 4.0)
            / 3.0) as i32;
        let mut tree_count = 0;

        if random.next_i32(10) == 0 {
            tree_count += 1;
        }

        tree_count += match biomebase {
            BiomeBase::SeasonalForest => k1 + 2,
            BiomeBase::Rainforest | BiomeBase::Forest | BiomeBase::Taiga => k1 + 5,
            BiomeBase::Desert | BiomeBase::Plains | BiomeBase::Tundra => -20,
            _ => 0,
        };

        for _ in 0..tree_count {
            let j2 = origin.x + random.next_i32(16) + 8;
            let k2 = origin.y + random.next_i32(16) + 8;
            let mut highest_y = CHUNK_HEIGHT_I32 - 1;

            for y in (0..CHUNK_HEIGHT_I32).rev() {
                if chunk_manager.get_block(IPoint3D::new(j2, y, k2)).is_some_and(|block| block.name != "game:air") {
                    highest_y = y;

                    break;
                }
            }

            if matches!(biomebase, BiomeBase::Forest) {
                if random.next_i32(3) == 0 {
                    ForestGenerator::populate(chunk_manager, &mut random, IPoint3D::new(j2, highest_y + 1, k2));
                } else {
                    TreesGenerator::populate(chunk_manager, &mut random, IPoint3D::new(j2, highest_y + 1, k2));
                }
            } else {
                TreesGenerator::populate(chunk_manager, &mut random, IPoint3D::new(j2, highest_y + 1, k2));
            }
        }
    }
}
