use meralus_shared::{IPoint3D, Random};
use meralus_world::{ChunkAccess, SubChunkBlockState};

pub struct LakesGenerator;

impl LakesGenerator {
    pub fn populate<C: ChunkAccess>(&self, chunk_manager: &mut C, random: &mut Random, mut center: IPoint3D) -> bool {
        center.x -= 8;
        center.z -= 8;

        while center.y > 0 && chunk_manager.get_block(center).is_none_or(|b| b.name == "game:air") {
            center.y -= 1;
        }

        center.y -= 4;

        let mut water_noise = [false; 2048];
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
                            water_noise[(j1 * 16 + k1) * 8 + l1] = true;
                        }
                    }
                }
            }
        }

        for x in 0..16 {
            for z in 0..16 {
                for y in 0..8 {
                    if !water_noise[(x * 16 + z) * 8 + y]
                        && (x < 15 && water_noise[((x + 1) * 16 + z) * 8 + y]
                            || x > 0 && water_noise[((x - 1) * 16 + z) * 8 + y]
                            || z < 15 && water_noise[(x * 16 + z + 1) * 8 + y]
                            || z > 0 && water_noise[(x * 16 + (z - 1)) * 8 + y]
                            || y < 7 && water_noise[(x * 16 + z) * 8 + y + 1]
                            || y > 0 && water_noise[(x * 16 + z) * 8 + (y - 1)])
                    {
                        let material = chunk_manager.get_block(center + IPoint3D::new(x as i32, y as i32, z as i32));

                        if y >= 4 && material.is_some_and(|material| material.name == "game:water") {
                            return false;
                        }

                        if y < 4 && material.is_none_or(|b| b.name == "game:air")
                        // || material == self.primary_block) && chunk.get_block_uncehced(center.x + i1, center.y + j2, center.z + i2) != this.a.getMaterial())
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
                    if water_noise[(x * 16 + z) * 8 + y] {
                        chunk_manager.set_block(
                            center + IPoint3D::new(x as i32, y as i32, z as i32),
                            if y >= 4 {
                                SubChunkBlockState::new("game:air")
                            } else {
                                SubChunkBlockState::new("game:water")
                            },
                        );
                    }
                }
            }
        }

        true
    }
}
