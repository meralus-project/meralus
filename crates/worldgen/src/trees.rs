use std::collections::VecDeque;

use ahash::HashSet;
use mavelin_shared::{Face, Random};
use mavelin_world::{CHUNK_HEIGHT_I32, ChunkAccess, ChunkManager, SubChunkBlockState};

struct QueuedCheck {
    pos: glam::IVec3,
    distance: i64,
}

struct LeafDistanceCalculator {
    updates: VecDeque<QueuedCheck>,
    seen: HashSet<glam::IVec3>,
}

impl LeafDistanceCalculator {
    const MAX_LEAF_DISTANCE: i64 = 7;

    pub fn new() -> Self {
        Self {
            updates: VecDeque::new(),
            seen: HashSet::default(),
        }
    }

    pub fn add_log(&mut self, pos: glam::IVec3) {
        if self.seen.insert(pos) {
            self.updates.push_front(QueuedCheck { pos, distance: 0 });
        }
    }

    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.updates.clear();
    }

    pub fn update<C: ChunkAccess>(&mut self, oak_leaves: u32, chunks: &mut C) {
        while let Some(check) = self.updates.pop_front() {
            let target_distance = check.distance + 1;

            for face in const {
                [
                    Face::Right.as_normal(),
                    Face::Left.as_normal(),
                    Face::Front.as_normal(),
                    Face::Back.as_normal(),
                    Face::Top.as_normal(),
                    Face::Bottom.as_normal(),
                ]
            } {
                let pos = check.pos + face;

                if !self.seen.insert(pos) {
                    continue;
                }

                if chunks.get_chunk(ChunkManager::<()>::to_local(pos)).is_none() {
                    continue;
                }

                let state = chunks.get_block(pos);

                if let Some(state) = state
                    && state.id == oak_leaves
                {
                    let distance = state.get_i64("distance").unwrap_or(7);

                    if distance == target_distance {
                        continue;
                    }

                    if distance < target_distance {
                        self.updates.push_back(QueuedCheck { pos, distance });

                        continue;
                    }

                    let mut state = state.clone();

                    state.set_i64("distance", target_distance);

                    chunks.set_block(pos, state);

                    if target_distance < Self::MAX_LEAF_DISTANCE {
                        self.updates.push_back(QueuedCheck {
                            pos,
                            distance: target_distance,
                        });
                    }
                }
            }
        }
    }
}

pub struct TreesGenerator {
    pub(crate) air: u32,
    pub(crate) oak_leaves: u32,
    pub(crate) oak_log: u32,
    pub(crate) grass_block: u32,
    pub(crate) dirt: u32,
    pub(crate) water: u32,
    pub(crate) ice: u32,
    pub(crate) glass: u32,
    pub(crate) snow: u32,
}

impl TreesGenerator {
    pub fn populate<C: ChunkAccess>(&self, chunks: &mut C, random: &mut Random, center: glam::IVec3) -> bool {
        let l = random.next_i32(3) + 4;
        let mut flag = true;
        let mut ldc = LeafDistanceCalculator::new();

        if center.y >= 1 && center.y + l < CHUNK_HEIGHT_I32 {
            for i1 in center.y..=center.y + 1 + l {
                let mut b0 = 1;

                if i1 == center.y {
                    b0 = 0;
                } else if i1 >= center.y + 1 + l - 2 {
                    b0 = 2;
                }

                for j1 in (center.x - b0)..=(center.x + b0) {
                    if !flag {
                        break;
                    }

                    for k1 in (center.z - b0)..=(center.z + b0) {
                        if !flag {
                            break;
                        }

                        if (0..CHUNK_HEIGHT_I32).contains(&i1) {
                            let block = chunks.get_block(glam::IVec3::new(j1, i1, k1));

                            if !block.is_none_or(|block| block.id == self.air || block.id == self.oak_leaves) {
                                flag = false;
                            }
                        } else {
                            flag = false;
                        }
                    }
                }
            }

            if flag {
                let block = chunks.get_block(center - glam::IVec3::Y);

                if block.is_some_and(|block| block.id == self.grass_block || block.id == self.dirt) && center.y < CHUNK_HEIGHT_I32 - l - 1 {
                    chunks.set_block(center - glam::IVec3::Y, SubChunkBlockState::new(self.dirt));

                    for i2 in (center.y - 3 + l)..=(center.y + l) {
                        let j1 = i2 - (center.y + l);
                        let k1 = 1 - j1 / 2;

                        for l1 in (center.x - k1)..=(center.x + k1) {
                            let j2 = l1 - center.x;

                            for k2 in (center.z - k1)..=(center.z + k1) {
                                let l2 = k2 - center.z;

                                if (j2.abs() != k1 || l2.abs() != k1 || random.next_i32(2) != 0 && j1 != 0)
                                    && !chunks
                                        .get_block(glam::IVec3::new(l1, i2, k2))
                                        .map(|block| block.id)
                                        .is_some_and(|name| name != self.air && ![self.water, self.snow, self.glass, self.ice].contains(&name))
                                {
                                    let mut state = SubChunkBlockState::new(self.oak_leaves);

                                    state.set_i64("distance", 7);

                                    chunks.set_block(glam::IVec3::new(l1, i2, k2), state);
                                }
                            }
                        }
                    }

                    for i2 in 0..l {
                        if chunks
                            .get_block(center + glam::IVec3::Y * i2)
                            .is_none_or(|block| block.id == self.air || block.id == self.oak_leaves)
                        {
                            chunks.set_block(center + glam::IVec3::Y * i2, SubChunkBlockState::new(self.oak_log));

                            ldc.add_log(center + glam::IVec3::Y * i2);
                        }
                    }

                    ldc.update(self.oak_leaves, chunks);

                    true
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        }
    }
}

#[allow(dead_code)]
pub struct BigTreeGenerator {
    random: Random,
    pos: glam::IVec3,
    e: i32,
    f: i32,
    g: f64,
    h: f64,
    i: f64,
    j: f64,
    k: f64,
    l: i32,
    m: i32,
    n: i32,
    o: Vec<Vec<i32>>,
}

// impl BigTreeGenerator {
//     const A: [u8; 6] = [2, 0, 0, 1, 2, 1];

//     pub fn new() -> Self {
//         Self {
//             random: Random::new(0),
//             pos: glam::IVec3::ZERO,
//             e: 0,
//             f: -1,
//             g: 0.618,
//             h: 1.0,
//             i: 0.381,
//             j: 1.0,
//             k: 1.0,
//             l: 1,
//             m: 12,
//             n: 4,
//             o: Vec::new(),
//         }
//     }

//     fn a2(&mut self) {
//         self.f = (self.e as f64 * self.g) as i32;

//         if self.f >= self.e {
//             self.f = self.e - 1;
//         }

//         let mut i = (1.382 + (self.k * self.e as f64 / 13.0).powi(2)) as i32;

//         if i < 1 {
//             i = 1;
//         }

//         let mut aint = vec![vec![0; i * self.e]; 4];
//         let mut j = self.pos[1] + self.e - self.n;
//         let mut k = 1;
//         let mut l = self.pos[1] + self.f;
//         let mut i1 = j - self.pos[1];

//         aint[0][0] = self.pos[0];
//         aint[0][1] = j;
//         aint[0][2] = self.pos[2];
//         aint[0][3] = l;

//         j -= 1;

//         while i1 >= 0 {
//             let mut j1 = 0;

//             let f = self.a1(i1);

//             if (f < 0.0) {
//                 j -= 1;
//                 i1 -= 1;
//             } else {
//                 let mut d0 = 0.5;

//                 while j1 < i {
//                     let mut d1 = self.j * f as f64 * (self.random.next_f32()
// as f64 + 0.328);                     let mut d2 = self.random.next_f32() as
// f64 * 2.0 * 3.14159;                     let mut k1 = (d1 * d2.sin() +
// self.pos[0] as f64 + d0).floor() as i32;                     let mut l1 = (d1
// * d2.cos() + self.pos[2] as f64 + d0).floor() as i32;                     let
// mut aint1 = [k1, j, l1];                     let mut aint2 = [k1, j + self.n,
// l1];

//                     if (self.a2(aint1, aint2) == -1) {
//                         let mut aint3 = [self.pos[0], self.pos[1],
// self.pos[2]];                         let mut d3 = (((self.pos[0] -
// aint1[0]).abs() as f64).powi(2) + ((self.pos[2] - aint1[2]).abs() as
// f64).powi(2)).sqrt();                         let mut d4 = d3 * self.i;

//                         if (aint1[1] as f64 - d4 > l as f64) {
//                             aint3[1] = l;
//                         } else {
//                             aint3[1] = (aint1[1] as f64 - d4) as i32;
//                         }

//                         if (self.a2(aint3, aint1) == -1) {
//                             aint[k][0] = k1;
//                             aint[k][1] = j;
//                             aint[k][2] = l1;
//                             aint[k][3] = aint3[1];

//                             k +=1;
//                         }
//                     }

//                     j1 += 1;
//                 }

//                 j -= 1;
//                 i -= 11;
//             }
//         }

//         self.o = vec![vec![0; k]; 4];

//         System.arraycopy(aint, 0, self.o, 0, k);
//     }

//     fn a6(&mut self, i: i32, j: i32, k: i32, f: f32, b0: u8, l: BlockData) {
//         let mut i1 = (int) ((double) f + 0.618);
//         let mut b1 = a[b0];
//         let mut b2 = a[b0 + 3];
//         let mut aint = [i, j, k];
//         let mut aint1 = [0, 0, 0];
//         let mut j1 = -i1;
//         let mut k1 = -i1;

//         for (aint1[b0] = aint[b0]; j1 <= i1; ++j1) {
//             aint1[b1] = aint[b1] + j1;
//             k1 = -i1;

//             while (k1 <= i1) {
//                 double d0 = Math.sqrt(Math.pow((double) Math.abs(j1) + 0.5,
// 2.0) + Math.pow((double) Math.abs(k1) + 0.5, 2.0));

//                 if (d0 > (double) f) {
//                     ++k1;
//                 } else {
//                     aint1[b2] = aint[b2] + k1;
//                     Material l1 = self.world.getType(aint1[0], aint1[1],
// aint1[2]);

//                     if (!BlockConstants.isAir(l1) &&
// !BlockConstants.isLeaves(l1)) {                         ++k1;
//                     } else {
//                         self.world.setBlockData(aint1[0], aint1[1], aint1[2],
// l, false);                         ++k1;
//                     }
//                 }
//             }
//         }
//     }

//     fn a1(&self, i: i32) -> f32 {
//         if (i as f64) < ((self.e as f32) * 0.3) as f64 {
//             -1.618
//         } else {
//             let f = self.e as f32 / 2.0;
//             let f1 = self.e as f32 / 2.0 - i as f32;
//             let f2;

//             if (f1 == 0.0) {
//                 f2 = f;
//             } else if (f1.abs() >= f) {
//                 f2 = 0.0;
//             } else {
//                 f2 = ((f.abs() as f64).powi(2) - (f1.abs() as
// f64).powi(2)).sqrt() as f32;             }

//             f2 * 0.5
//         }
//     }

//     fn b(&self, i: i32) -> f32 {
//         if i >= 0 && i < self.n {if i != 0 && i != self.n - 1 {3.0} else
// {2.0}} else {-1.0}     }

//     fn a3d(&mut self, i: i32, j: i32, k: i32) {
//         let mut l = j;
//         let mut i1 = j + self.n;

//         while l < i1 {
//             let f = self.b(l - j);

//             self.a6(i, l, k, f, 1, BlockConstants.OAK_LEAVES);

//             l += 1;
//         }
//     }

//     fn a4(&mut self, aint: &[i32], aint1: &[i32], i: BlockData,
// leafDistanceCalculator: &mut LeafDistanceCalculator) {         int[] aint2 =
// new int[] { 0, 0, 0};         byte b0 = 0;

//         byte b1;

//         for (b1 = 0; b0 < 3; ++b0) {
//             aint2[b0] = aint1[b0] - aint[b0];
//             if (Math.abs(aint2[b0]) > Math.abs(aint2[b1])) {
//                 b1 = b0;
//             }
//         }

//         if (aint2[b1] != 0) {
//             byte b2 = a[b1];
//             byte b3 = a[b1 + 3];
//             byte b4;

//             if (aint2[b1] > 0) {
//                 b4 = 1;
//             } else {
//                 b4 = -1;
//             }

//             double d0 = (double) aint2[b2] / (double) aint2[b1];
//             double d1 = (double) aint2[b3] / (double) aint2[b1];
//             int[] aint3 = new int[] { 0, 0, 0};
//             let mut j = 0;

//             for (let mut k = aint2[b1] + b4; j != k; j += b4) {
//                 aint3[b1] = MathHelper173.floor((double) (aint[b1] + j) +
// 0.5);                 aint3[b2] = MathHelper173.floor((double) aint[b2] +
// (double) j * d0 + 0.5);                 aint3[b3] =
// MathHelper173.floor((double) aint[b3] + (double) j * d1 + 0.5);
// self.world.setBlockData(aint3[0], aint3[1], aint3[2], i, false);
// if (leafDistanceCalculator != null) {
// leafDistanceCalculator.addLog(aint3[0], aint3[1], aint3[2]);
// }             }
//         }
//     }

//     fn b(&mut self) {
//         let mut i = 0;

//         for (let mut j = self.o.length; i < j; ++i) {
//             let mut k = self.o[i][0];
//             let mut l = self.o[i][1];
//             let mut i1 = self.o[i][2];

//             self.a3d(k, l, i1);
//         }
//     }

//     fn c(&self, i: i32) -> bool {
//         return (double) i >= (double) self.e * 0.2;
//     }

//     fn c(&mut self, leafDistanceCalculator: &mut LeafDistanceCalculator) {
//         let mut i = self.pos[0];
//         let mut j = self.pos[1];
//         let mut k = self.pos[1] + self.f;
//         let mut l = self.pos[2];
//         int[] aint = new int[] { i, j, l};
//         int[] aint1 = new int[] { i, k, l};

//         self.a4(aint, aint1, BlockConstants.OAK_LOG, leafDistanceCalculator);
//         if (self.l == 2) {
//             ++aint[0];
//             ++aint1[0];
//             self.a4(aint, aint1, BlockConstants.OAK_LOG,
// leafDistanceCalculator);             ++aint[2];
//             ++aint1[2];
//             self.a4(aint, aint1, BlockConstants.OAK_LOG,
// leafDistanceCalculator);             aint[0] += -1;
//             aint1[0] += -1;
//             self.a4(aint, aint1, BlockConstants.OAK_LOG,
// leafDistanceCalculator);         }
//     }

//     fn placeLogs(&mut self, leafDistanceCalculator: &mut
// LeafDistanceCalculator) {         let mut i = 0;
//         let mut j = self.o.length;

//         for (int[] aint = new int[] { self.pos[0], self.pos[1], self.pos[2]};
// i < j; ++i) {             int[] aint1 = self.o[i];
//             int[] aint2 = new int[] { aint1[0], aint1[1], aint1[2]};

//             aint[1] = aint1[3];
//             let mut k = aint[1] - self.pos[1];

//             if (self.c(k)) {
//                 self.a4(aint, aint2, BlockConstants.OAK_LOG,
// leafDistanceCalculator);             }
//         }
//     }

//     fn a<C: ChunkAccess>(&self, chunks: &C, aint: glam::IVec3, aint1:
// glam::IVec3) -> i32 {         let mut aint2 = glam::IVec3::ZERO;
//         let mut b0 = 0;
//         let mut b1 = 0;

//         while b0 < 3 {
//             aint2[b0] = aint1[b0] - aint[b0];

//             if aint2[b0].abs() > aint2[b1].abs() {
//                 b1 = b0;
//             }

//             b0 += 1;
//         }

//         if aint2[b1] == 0 {
//             -1
//         } else {
//             let b2 = Self::A[b1];
//             let b3 = Self::A[b1 + 3];
//             let b4;

//             if aint[b1] > 0 {
//                 b4 = 1;
//             } else {
//                 b4 = -1;
//             }

//             let d0 = aint2[b2 as usize] as f64 / aint2[b1] as f64;
//             let d1 = aint2[b3 as usize] as f64 / aint2[b1] as f64;

//             let mut aint3 = glam::IVec3::ZERO;
//             let mut i = 0;
//             let mut j = aint2[b1];

//             while i != j {
//                 aint3[b1] = aint[b1] + i;
//                 aint3[b2] = (aint[b2] as f64 + i as f64 * d0).floor();
//                 aint3[b3] = (aint[b3] as f64 + i as f64 * d1).floor();

//                 if chunks
//                     .get_block(aint3)
//                     .is_some_and(|block| block.name != self.air &&
// block.name != "oak_leaves")                 {
//                     break;
//                 }

//                 i += b4;
//             }

//             if i == j { -1 } else { i.abs() }
//         }
//     }

//     fn d<C: ChunkAccess>(&self, chunks: &C) -> bool {
//         let aint = self.pos;
//         let aint1 = self.pos + glam::IVec3::Y * (self.e - 1);

//         if chunks
//             .get_block(self.pos - glam::IVec3::Y.to_vector())
//             .is_some_and(|block| block.name == self.grass_block ||
// block.name == self.dirt)         {
//             let j = self.a(chunks, aint, aint1);

//             if j == -1 {
//                 true
//             } else if j < 6 {
//                 false
//             } else {
//                 self.e = j;

//                 true
//             }
//         } else {
//             false
//         }
//     }

//     pub fn populate<C: ChunkAccess>(&mut self, chunks: &mut C, random: &mut
// Random, center: glam::IVec3) -> bool {         let l = random.next_i64();

//         self.random.set_seed(l);
//         self.pos = center;

//         if self.e == 0 {
//             self.e = 5 + self.random.next_i32(self.m);
//         }

//         if self.d(chunks) {
//             let mut ldc = LeafDistanceCalculator::new();

//             self.a2();
//             self.b();
//             self.c(&mut ldc);
//             self.place_logs(&mut ldc);

//             ldc.update(chunks);

//             true
//         } else {
//             false
//         }
//     }
// }

pub struct ForestGenerator {
    pub(crate) air: u32,
    pub(crate) oak_leaves: u32,
    pub(crate) oak_log: u32,
    pub(crate) grass_block: u32,
    pub(crate) dirt: u32,
    pub(crate) water: u32,
    pub(crate) ice: u32,
    pub(crate) glass: u32,
    pub(crate) snow: u32,
}

impl ForestGenerator {
    pub fn populate<C: ChunkAccess>(&self, chunks: &mut C, random: &mut Random, center: glam::IVec3) -> bool {
        let l = random.next_i32(3) + 5;
        let mut flag = true;
        let mut ldc = LeafDistanceCalculator::new();

        if center.y >= 1 && center.y + l < CHUNK_HEIGHT_I32 {
            for i1 in center.y..=center.y + 1 + l {
                let mut b0 = 1;

                if i1 == center.y {
                    b0 = 0;
                } else if i1 >= center.y + 1 + l - 2 {
                    b0 = 2;
                }

                for j1 in (center.x - b0)..=(center.x + b0) {
                    if !flag {
                        break;
                    }

                    for k1 in (center.z - b0)..=(center.z + b0) {
                        if !flag {
                            break;
                        }

                        if (0..CHUNK_HEIGHT_I32).contains(&i1) {
                            let block = chunks.get_block(glam::IVec3::new(j1, i1, k1));

                            if !block.is_none_or(|block| block.id == self.air || block.id == self.oak_leaves) {
                                flag = false;
                            }
                        } else {
                            flag = false;
                        }
                    }
                }
            }

            if flag {
                let block = chunks.get_block(center - glam::IVec3::Y);

                if block.is_some_and(|block| block.id == self.grass_block || block.id == self.dirt) && center.y < CHUNK_HEIGHT_I32 - l - 1 {
                    chunks.set_block(center - glam::IVec3::Y, SubChunkBlockState::new(self.dirt));

                    for i2 in (center.y - 3 + l)..=(center.y + l) {
                        let j1 = i2 - (center.y + l);
                        let k1 = 1 - j1 / 2;

                        for l1 in (center.x - k1)..=(center.x + k1) {
                            let j2 = l1 - center.x;

                            for k2 in (center.z - k1)..=(center.z + k1) {
                                let l2 = k2 - center.z;

                                if (j2.abs() != k1 || l2.abs() != k1 || random.next_i32(2) != 0 && j1 != 0)
                                    && !chunks
                                        .get_block(glam::IVec3::new(l1, i2, k2))
                                        .map(|block| block.id)
                                        .is_some_and(|name| name != self.air && ![self.water, self.snow, self.glass, self.ice].contains(&name))
                                {
                                    let mut state = SubChunkBlockState::new(self.oak_leaves);

                                    state.set_i64("distance", 7);

                                    chunks.set_block(glam::IVec3::new(l1, i2, k2), state);
                                }
                            }
                        }
                    }

                    for i2 in 0..l {
                        if chunks
                            .get_block(center + glam::IVec3::Y * i2)
                            .is_none_or(|block| block.id == self.air || block.id == self.oak_leaves)
                        {
                            chunks.set_block(center + glam::IVec3::Y * i2, SubChunkBlockState::new(self.oak_log));

                            ldc.add_log(center + glam::IVec3::Y * i2);
                        }
                    }

                    ldc.update(self.oak_leaves, chunks);

                    true
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        }
    }
}
