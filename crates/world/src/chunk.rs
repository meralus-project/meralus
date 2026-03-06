use std::io::{self, Read};

use meralus_shared::{IPoint2D, IPoint3D, USizePoint2D, USizePoint3D};

use crate::{BiomeBase, Face, vec_to_boxed_array};

pub const SUBCHUNK_XZ_MAX: usize = SUBCHUNK_SIZE - 1;

pub const SUBCHUNK_SIZE: usize = 16;
pub const SUBCHUNK_SIZE_U16: u16 = 16;
pub const SUBCHUNK_SIZE_I32: i32 = 16;
pub const SUBCHUNK_SIZE_F32: f32 = 16.0;
pub const SUBCHUNK_SIZE_F64: f64 = 16.0;

pub const SUBCHUNK_COUNT: usize = 16;
pub const SUBCHUNK_COUNT_I32: i32 = 16;
pub const SUBCHUNK_COUNT_U16: u16 = 16;
pub const SUBCHUNK_COUNT_F32: f32 = 16.0;
pub const SUBCHUNK_COUNT_F64: f64 = 16.0;

pub const CHUNK_HEIGHT: usize = SUBCHUNK_SIZE * SUBCHUNK_COUNT;
pub const CHUNK_HEIGHT_I32: i32 = SUBCHUNK_SIZE_I32 * SUBCHUNK_COUNT_I32;
pub const CHUNK_HEIGHT_U16: u16 = SUBCHUNK_SIZE_U16 * SUBCHUNK_COUNT_U16;
pub const CHUNK_HEIGHT_F32: f32 = SUBCHUNK_SIZE_F32 * SUBCHUNK_COUNT_F32;
pub const CHUNK_HEIGHT_F64: f64 = SUBCHUNK_SIZE_F64 * SUBCHUNK_COUNT_F64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Cube whose size is specified by [`CHUNK_SIZE`] constant.
pub struct SubChunk {
    /// 3D array of block IDs.
    pub blocks: [u8; SUBCHUNK_SIZE * SUBCHUNK_SIZE * SUBCHUNK_SIZE],
    /// 3D array of block light level values.
    pub light_levels: [u8; SUBCHUNK_SIZE * SUBCHUNK_SIZE * SUBCHUNK_SIZE],
}

impl SubChunk {
    pub const EMPTY: Self = Self {
        blocks: [0; SUBCHUNK_SIZE * SUBCHUNK_SIZE * SUBCHUNK_SIZE],
        light_levels: [0; SUBCHUNK_SIZE * SUBCHUNK_SIZE * SUBCHUNK_SIZE],
    };

    #[inline]
    pub const fn index_of(position: USizePoint3D) -> usize {
        position.y * const { SUBCHUNK_SIZE * SUBCHUNK_SIZE } + position.z * SUBCHUNK_SIZE + position.x
    }

    #[inline]
    pub fn filled_full_height(value: u8) -> Box<[Self; SUBCHUNK_COUNT]> {
        vec_to_boxed_array(vec![
            Self {
                blocks: [value; SUBCHUNK_SIZE * SUBCHUNK_SIZE * SUBCHUNK_SIZE],
                light_levels: [0; SUBCHUNK_SIZE * SUBCHUNK_SIZE * SUBCHUNK_SIZE],
            };
            SUBCHUNK_COUNT
        ])
    }

    #[inline]
    pub fn empty_full_height() -> Box<[Self; SUBCHUNK_COUNT]> {
        vec_to_boxed_array(vec![Self::EMPTY; SUBCHUNK_COUNT])
    }

    #[must_use]
    #[inline]
    pub fn get_block_unchecked(&self, position: USizePoint3D) -> Option<u8> {
        let &block_id = unsafe { self.blocks.get_unchecked(Self::index_of(position)) };

        if block_id == 0 { None } else { Some(block_id) }
    }

    #[inline]
    pub const fn iter(&self, subchunk_idx: usize) -> SubChunkIter<'_> {
        SubChunkIter::new(self, subchunk_idx)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Part of the world consisting of subchunks, number of which is specified by
/// [`SUBCHUNK_COUNT`] constant.
pub struct Chunk {
    /// Chunk location on a 2D grid
    pub origin: IPoint2D,
    /// 2D array of block IDs.
    pub biomes: [BiomeBase; SUBCHUNK_SIZE * SUBCHUNK_SIZE],
    /// Array of chunk vertical sections
    pub subchunks: Box<[SubChunk; SUBCHUNK_COUNT]>,
}

impl Chunk {
    pub fn empty() -> Self {
        Self {
            origin: IPoint2D::ZERO,
            biomes: [BiomeBase::Sky; SUBCHUNK_SIZE * SUBCHUNK_SIZE],
            subchunks: SubChunk::empty_full_height(),
        }
    }

    pub fn filled(value: u8) -> Self {
        Self {
            origin: IPoint2D::ZERO,
            biomes: [BiomeBase::Sky; SUBCHUNK_SIZE * SUBCHUNK_SIZE],
            subchunks: SubChunk::filled_full_height(value),
        }
    }

    pub const fn index_of_biome(position: USizePoint2D) -> usize {
        position.y * SUBCHUNK_SIZE + position.x
    }

    pub fn deserialize<T: AsRef<[u8]>>(data: T) -> io::Result<Self> {
        let mut chunk = Self::empty();

        let mut data = data.as_ref();

        chunk.origin = {
            let mut x = [0; 4];
            let mut z = [0; 4];

            data.read_exact(&mut x)?;
            data.read_exact(&mut z)?;

            let x = i32::from_be_bytes(x);
            let z = i32::from_be_bytes(z);

            IPoint2D::new(x, z)
        };

        for y in 0..CHUNK_HEIGHT {
            for z in 0..SUBCHUNK_SIZE {
                for x in 0..SUBCHUNK_SIZE {
                    let mut buf = [0; 2];

                    data.read_exact(&mut buf)?;

                    let [subchunk, y] = Self::get_subchunk_index(y);

                    chunk.subchunks[subchunk].blocks[SubChunk::index_of(USizePoint3D::new(x, y, z))] = buf[0];
                    chunk.subchunks[subchunk].light_levels[SubChunk::index_of(USizePoint3D::new(x, y, z))] = buf[1];
                }
            }
        }

        Ok(chunk)
    }

    pub const fn corner(position: USizePoint3D) -> Option<[IPoint2D; 3]> {
        match (position.x, position.z) {
            (0, 0) => Some([IPoint2D::NEG_X, IPoint2D::NEG_Y, IPoint2D::NEG_ONE]),
            (0, SUBCHUNK_XZ_MAX) => Some([IPoint2D::NEG_X, IPoint2D::Y, IPoint2D::new(-1, 1)]),
            (SUBCHUNK_XZ_MAX, 0) => Some([IPoint2D::X, IPoint2D::NEG_Y, IPoint2D::new(1, -1)]),
            (SUBCHUNK_XZ_MAX, SUBCHUNK_XZ_MAX) => Some([IPoint2D::X, IPoint2D::Y, IPoint2D::ONE]),
            _ => None,
        }
    }

    pub const fn side(position: USizePoint3D) -> Option<IPoint2D> {
        if position.x == 0 {
            Some(IPoint2D::NEG_X)
        } else if position.x == SUBCHUNK_XZ_MAX {
            Some(IPoint2D::X)
        } else if position.z == 0 {
            Some(IPoint2D::NEG_Y)
        } else if position.z == SUBCHUNK_XZ_MAX {
            Some(IPoint2D::Y)
        } else {
            None
        }
    }

    #[must_use]
    pub fn into_serialized(self) -> Vec<u8> {
        let mut data = Vec::new();

        data.extend_from_slice(&self.origin.x.to_be_bytes());
        data.extend_from_slice(&self.origin.y.to_be_bytes());

        for y in 0..CHUNK_HEIGHT {
            for z in 0..SUBCHUNK_SIZE {
                for x in 0..SUBCHUNK_SIZE {
                    let [subchunk, y] = Self::get_subchunk_index(y);

                    data.push(self.subchunks[subchunk].blocks[SubChunk::index_of(USizePoint3D::new(x, y, z))]);
                    data.push(self.subchunks[subchunk].light_levels[SubChunk::index_of(USizePoint3D::new(x, y, z))]);
                }
            }
        }

        data
    }

    #[must_use]
    pub fn serialize(&self) -> Vec<u8> {
        let mut data = Vec::new();

        data.extend_from_slice(&self.origin.x.to_be_bytes());
        data.extend_from_slice(&self.origin.y.to_be_bytes());

        for y in 0..CHUNK_HEIGHT {
            for z in 0..SUBCHUNK_SIZE {
                for x in 0..SUBCHUNK_SIZE {
                    let [subchunk, y] = Self::get_subchunk_index(y);

                    data.push(self.subchunks[subchunk].blocks[SubChunk::index_of(USizePoint3D::new(x, y, z))]);
                    data.push(self.subchunks[subchunk].light_levels[SubChunk::index_of(USizePoint3D::new(x, y, z))]);
                }
            }
        }

        data
    }

    #[inline]
    pub const fn to_local(position: IPoint3D) -> USizePoint3D {
        USizePoint3D::new(
            position.x.rem_euclid(SUBCHUNK_SIZE_I32) as usize,
            position.y as usize,
            position.z.rem_euclid(SUBCHUNK_SIZE_I32) as usize,
        )
    }

    #[inline]
    pub const fn get_subchunk_index(y: usize) -> [usize; 2] {
        [y >> 4, y.rem_euclid(SUBCHUNK_SIZE)]
    }

    #[inline]
    pub const fn to_world_pos(origin: IPoint2D, position: USizePoint3D) -> IPoint3D {
        let IPoint2D { x, y } = origin;

        IPoint3D::new(
            (x * SUBCHUNK_SIZE_I32) + position.x as i32,
            position.y as i32,
            (y * SUBCHUNK_SIZE_I32) + position.z as i32,
        )
    }

    #[inline]
    pub const fn to_world(&self, position: USizePoint3D) -> IPoint3D {
        Self::to_world_pos(self.origin, position)
    }

    #[inline]
    pub const fn contains_local_position(&self, position: USizePoint3D) -> bool {
        position.x < SUBCHUNK_SIZE && position.y < (SUBCHUNK_SIZE * SUBCHUNK_COUNT) && position.z < SUBCHUNK_SIZE
    }

    #[inline]
    pub fn contains_position(&self, position: IPoint3D) -> bool {
        self.origin.x == (position.x >> 4) && self.origin.y == (position.z >> 4) && (0..SUBCHUNK_COUNT_I32).contains(&(position.y >> 4))
    }

    pub fn set_block(&mut self, position: USizePoint3D, block: u8) {
        if self.contains_local_position(position) {
            self.set_block_unchecked(position, block);
        }
    }

    #[inline]
    pub fn set_block_unchecked(&mut self, position: USizePoint3D, block: u8) {
        let [subchunk, y] = Self::get_subchunk_index(position.y);

        unsafe {
            *self
                .subchunks
                .get_unchecked_mut(subchunk)
                .blocks
                .get_unchecked_mut(SubChunk::index_of(position.with_y(y))) = block;
        }
    }

    pub fn get_block(&self, position: USizePoint3D) -> Option<u8> {
        if self.contains_local_position(position) {
            Some(self.get_block_unchecked(position))
        } else {
            None
        }
    }

    #[must_use]
    #[inline]
    pub fn get_block_unchecked(&self, position: USizePoint3D) -> u8 {
        let [subchunk, y] = Self::get_subchunk_index(position.y);

        unsafe {
            *self
                .subchunks
                .get_unchecked(subchunk)
                .blocks
                .get_unchecked(SubChunk::index_of(USizePoint3D { y, ..position }))
        }
    }

    #[inline]
    pub fn set_biome_unchecked(&mut self, position: USizePoint2D, biome: BiomeBase) {
        unsafe {
            *self.biomes.get_unchecked_mut(Self::index_of_biome(position)) = biome;
        }
    }

    #[must_use]
    #[inline]
    pub fn get_biome_unchecked(&self, position: USizePoint2D) -> BiomeBase {
        unsafe { *self.biomes.get_unchecked(Self::index_of_biome(position)) }
    }

    #[inline]
    pub fn get_subchunk(&self, y: f32) -> Option<&SubChunk> {
        self.subchunks.get((y.floor() as i32 >> 4) as usize)
    }

    #[inline]
    pub fn get_subchunk_mut(&mut self, y: f32) -> Option<&mut SubChunk> {
        self.subchunks.get_mut((y.floor() as i32 >> 4) as usize)
    }

    #[inline]
    pub fn get_light_level_unchecked(&self, position: USizePoint3D) -> u8 {
        let [subchunk, y] = Self::get_subchunk_index(position.y);

        unsafe {
            *self
                .subchunks
                .get_unchecked(subchunk)
                .light_levels
                .get_unchecked(SubChunk::index_of(USizePoint3D { y, ..position }))
        }
    }

    #[inline]
    pub const fn get_light_level(&self, position: USizePoint3D) -> u8 {
        let [subchunk, y] = Self::get_subchunk_index(position.y);

        self.subchunks[subchunk].light_levels[SubChunk::index_of(USizePoint3D { y, ..position })]
    }

    #[inline]
    pub const fn get_light_level_mut(&mut self, position: USizePoint3D) -> &mut u8 {
        let [subchunk, y] = Self::get_subchunk_index(position.y);

        &mut self.subchunks[subchunk].light_levels[SubChunk::index_of(USizePoint3D { y, ..position })]
    }

    #[inline]
    pub const fn check_for_local_block(&self, local_position: USizePoint3D) -> bool {
        let [subchunk, y] = Self::get_subchunk_index(local_position.y);

        self.subchunks[subchunk].blocks[SubChunk::index_of(USizePoint3D { y, ..local_position })] != 0
    }

    #[inline]
    pub fn check_for_block(&self, position: IPoint3D) -> bool {
        self.contains_position(position) && self.check_for_local_block(Self::to_local(position))
    }

    pub const fn get_light(&self, position: USizePoint3D, is_sky_light: bool) -> u8 {
        if is_sky_light {
            self.get_sky_light(position)
        } else {
            self.get_block_light(position)
        }
    }

    pub const fn set_light(&mut self, position: USizePoint3D, is_sky_light: bool, value: u8) {
        if is_sky_light {
            self.set_sky_light(position, value);
        } else {
            self.set_block_light(position, value);
        }
    }

    #[inline]
    pub const fn get_sky_light(&self, position: USizePoint3D) -> u8 {
        (self.get_light_level(position) >> 4) & 0xF
    }

    #[inline]
    pub fn get_sky_light_unchecked(&self, position: USizePoint3D) -> u8 {
        (self.get_light_level_unchecked(position) >> 4) & 0xF
    }

    #[inline]
    pub const fn set_sky_light(&mut self, position: USizePoint3D, value: u8) {
        let level = self.get_light_level_mut(position);

        *level = (*level & 0xF) | (value << 4);
    }

    #[inline]
    pub const fn get_block_light(&self, position: USizePoint3D) -> u8 {
        self.get_light_level(position) & 0xF
    }

    #[inline]
    pub fn get_block_light_unchecked(&self, position: USizePoint3D) -> u8 {
        self.get_light_level_unchecked(position) & 0xF
    }

    #[inline]
    pub const fn set_block_light(&mut self, position: USizePoint3D, value: u8) {
        let level = self.get_light_level_mut(position);

        *level = (*level & 0xF0) | value;
    }

    #[inline]
    pub fn new(origin: IPoint2D) -> Self {
        Self { origin, ..Self::empty() }
    }

    #[inline]
    pub const fn iter(&self) -> ChunkIter<'_> {
        ChunkIter::new(self)
    }

    #[inline]
    pub const fn face_iter(&self, face: Face) -> ChunkFaceIter<'_> {
        ChunkFaceIter::new(self, face)
    }
}

impl<'a> IntoIterator for &'a Chunk {
    type IntoIter = ChunkIter<'a>;
    type Item = (USizePoint3D, u8);

    fn into_iter(self) -> Self::IntoIter {
        ChunkIter::new(self)
    }
}

pub struct ChunkFaceIter<'a> {
    chunk: &'a Chunk,
    min: USizePoint3D,
    max: USizePoint3D,
    y: usize,
    z: usize,
    x: usize,
}

impl<'a> ChunkFaceIter<'a> {
    pub const fn new(chunk: &'a Chunk, face: Face) -> Self {
        let min = match face {
            Face::Right => USizePoint3D::new(SUBCHUNK_XZ_MAX, 0, 0),
            Face::Top => USizePoint3D::new(0, CHUNK_HEIGHT - 1, 0),
            Face::Front => USizePoint3D::new(0, 0, SUBCHUNK_XZ_MAX),
            Face::Left | Face::Back | Face::Bottom => USizePoint3D::ZERO,
        };

        let max = match face {
            Face::Left => USizePoint3D::new(1, CHUNK_HEIGHT, SUBCHUNK_SIZE),
            Face::Bottom => USizePoint3D::new(SUBCHUNK_SIZE, 1, SUBCHUNK_SIZE),
            Face::Back => USizePoint3D::new(SUBCHUNK_SIZE, CHUNK_HEIGHT, 1),
            Face::Right | Face::Top | Face::Front => USizePoint3D::new(SUBCHUNK_SIZE, CHUNK_HEIGHT, SUBCHUNK_SIZE),
        };

        Self {
            chunk,
            min,
            max,
            y: 0,
            z: 0,
            x: 0,
        }
    }
}

impl Iterator for ChunkFaceIter<'_> {
    type Item = (USizePoint3D, u8);

    fn next(&mut self) -> Option<Self::Item> {
        if self.y == self.max.y {
            None
        } else if self.z == self.max.z - 1 && self.x == self.max.x - 1 {
            let position = USizePoint3D::new(self.x, self.y, self.z);

            self.y += 1;
            self.z = self.min.z;
            self.x = self.min.x;

            Some((position, self.chunk.get_block_unchecked(position)))
        } else if self.x == self.max.x - 1 {
            let position = USizePoint3D::new(self.x, self.y, self.z);

            self.z += 1;
            self.x = self.min.x;

            Some((position, self.chunk.get_block_unchecked(position)))
        } else {
            let position = USizePoint3D::new(self.x, self.y, self.z);

            self.x += 1;

            Some((position, self.chunk.get_block_unchecked(position)))
        }
    }
}

pub struct ChunkIter<'a> {
    chunk: &'a Chunk,
    y: usize,
    z: usize,
    x: usize,
}

impl<'a> ChunkIter<'a> {
    pub const fn new(chunk: &'a Chunk) -> Self {
        Self { chunk, y: 0, z: 0, x: 0 }
    }
}

impl Iterator for ChunkIter<'_> {
    type Item = (USizePoint3D, u8);

    fn next(&mut self) -> Option<Self::Item> {
        if self.y == CHUNK_HEIGHT {
            None
        } else if self.z == SUBCHUNK_XZ_MAX && self.x == SUBCHUNK_XZ_MAX {
            let position = USizePoint3D::new(self.x, self.y, SUBCHUNK_XZ_MAX);

            self.y += 1;
            self.z = 0;
            self.x = 0;

            Some((position, self.chunk.get_block_unchecked(position)))
        } else if self.x == SUBCHUNK_XZ_MAX {
            let position = USizePoint3D::new(SUBCHUNK_XZ_MAX, self.y, self.z);

            self.z += 1;
            self.x = 0;

            Some((position, self.chunk.get_block_unchecked(position)))
        } else {
            let position = USizePoint3D::new(self.x, self.y, self.z);

            self.x += 1;

            Some((position, self.chunk.get_block_unchecked(position)))
        }
    }
}

pub struct SubChunkIter<'a> {
    subchunk: &'a SubChunk,
    y_offset: USizePoint3D,
    y: usize,
    z: usize,
    x: usize,
}

impl<'a> SubChunkIter<'a> {
    pub const fn new(subchunk: &'a SubChunk, subchunk_idx: usize) -> Self {
        Self {
            subchunk,
            y_offset: USizePoint3D::new(0, subchunk_idx * SUBCHUNK_SIZE, 0),
            y: 0,
            z: 0,
            x: 0,
        }
    }
}

impl Iterator for SubChunkIter<'_> {
    type Item = (USizePoint3D, Option<u8>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.y == SUBCHUNK_SIZE {
            None
        } else if self.z == SUBCHUNK_XZ_MAX && self.x == SUBCHUNK_XZ_MAX {
            let position = USizePoint3D::new(self.x, self.y, SUBCHUNK_XZ_MAX);

            self.y += 1;
            self.z = 0;
            self.x = 0;

            Some((self.y_offset + position, self.subchunk.get_block_unchecked(position)))
        } else if self.x == SUBCHUNK_XZ_MAX {
            let position = USizePoint3D::new(SUBCHUNK_XZ_MAX, self.y, self.z);

            self.z += 1;
            self.x = 0;

            Some((self.y_offset + position, self.subchunk.get_block_unchecked(position)))
        } else {
            let position = USizePoint3D::new(self.x, self.y, self.z);

            self.x += 1;

            Some((self.y_offset + position, self.subchunk.get_block_unchecked(position)))
        }
    }
}

#[cfg(test)]
mod tests {
    use meralus_shared::{IPoint2D, USizePoint3D};

    use crate::{CHUNK_HEIGHT, Chunk, Face, SUBCHUNK_SIZE};

    #[test]
    fn test_chunk_face_iter() {
        let chunk = Chunk::filled(1);

        let mut iter = chunk.face_iter(Face::Back);

        for y in 0..CHUNK_HEIGHT {
            for z in 0..1 {
                for x in 0..SUBCHUNK_SIZE {
                    assert_eq!(iter.next(), Some((USizePoint3D::new(x, y, z), 1)));
                }
            }
        }

        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_chunk_iter() {
        let chunk = Chunk::filled(1);

        let mut iter = chunk.iter();

        for y in 0..CHUNK_HEIGHT {
            for z in 0..SUBCHUNK_SIZE {
                for x in 0..SUBCHUNK_SIZE {
                    assert_eq!(iter.next(), Some((USizePoint3D::new(x, y, z), 1)));
                }
            }
        }

        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_chunk_serialization() {
        let chunk = Chunk::new(IPoint2D::new(0, 0));

        let serialized = chunk.serialize();

        println!("{}", serialized.len());

        let deserialized = Chunk::deserialize(&serialized).unwrap();

        assert_eq!(chunk.origin, deserialized.origin);
        assert_eq!(chunk.subchunks, deserialized.subchunks);
    }
}
