use std::sync::LazyLock;

use mavelin_shared::Random;

pub struct Perlin {
    permute_table: [i32; 512],
    x_offset: f64,
    y_offset: f64,
    z_offset: f64,
}

static F: LazyLock<f64> = LazyLock::new(|| 0.5 * (3f64.sqrt() - 1.0));
static G: LazyLock<f64> = LazyLock::new(|| (3.0 - 3f64.sqrt()) / 6.0);

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

    pub fn new(random: &mut Random) -> Self {
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

    pub fn get3d(&self, x: f64, y: f64, z: f64) -> f64 {
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

    pub fn get2d(&self, x: f64, y: f64) -> f64 {
        self.get3d(x, y, 0.0)
    }

    fn a_i32(d0: f64) -> i32 {
        if d0 > 0.0 { d0 as i32 } else { d0 as i32 - 1 }
    }

    fn a_f64(aint: &[i32], d0: f64, d1: f64) -> f64 {
        f64::from(aint[0]).mul_add(d0, f64::from(aint[1]) * d1)
    }

    pub fn generate_noise2d(&self, noise_data: &mut [f64], offset: glam::DVec2, size: glam::IVec2, scale: glam::DVec3) {
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
                let d15 = 2f64.mul_add(*G, d11 - 1.0);
                let d16 = 2f64.mul_add(*G, d12 - 1.0);
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

                noise_data[index] = (70.0 * (x + y + z)).mul_add(scale.z, noise_data[index]);

                index += 1;
            }
        }
    }

    pub fn generate_noise3d(&self, data: &mut [f64], offset: glam::DVec3, size: glam::IVec3, scale: glam::DVec3, frequency: f64) {
        if size.y == 1 {
            self.generate_noise3d_short(data, offset, size, scale, frequency);
        } else {
            let inv_frequency = 1.0 / frequency;
            let mut data_offset = 0;
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

    fn generate_noise3d_short(&self, data: &mut [f64], offset: glam::DVec3, size: glam::IVec3, scale: glam::DVec3, frequency: f64) {
        let inv_frequency = 1.0 / frequency;
        let mut data_offset = 0;

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
    }
}
