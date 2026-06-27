use std::{iter::repeat_n, marker::PhantomData};

use ahash::HashMap;
use meralus_shared::{Face, IPoint2D, IPoint3D, USizePoint2D, USizePoint3D};

use crate::{BiomeBase, PropertyValue, new_boxed_array};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubChunkBlockState {
    pub id: u32,
    pub properties: HashMap<String, PropertyValue>,
}

impl SubChunkBlockState {
    #[inline]
    pub fn new(id: u32) -> Self {
        Self {
            id,
            properties: HashMap::default(),
        }
    }

    #[inline]
    pub fn air() -> Self {
        Self::new(0)
    }

    #[inline]
    pub const fn is_air(&self) -> bool {
        self.id == 0
    }

    #[inline]
    pub fn set_i64(&mut self, property: &str, value: i64) {
        self.properties.insert(property.to_string(), PropertyValue::Number(value));
    }

    #[inline]
    pub fn set_f32(&mut self, property: &str, value: f32) {
        self.properties.insert(property.to_string(), PropertyValue::Float(value));
    }

    #[inline]
    pub fn set_bool(&mut self, property: &str, value: bool) {
        self.properties.insert(property.to_string(), PropertyValue::Boolean(value));
    }

    #[inline]
    pub fn get_i64(&self, property: &str) -> Option<i64> {
        self.properties
            .get(property)
            .and_then(|value| if let &PropertyValue::Number(value) = value { Some(value) } else { None })
    }

    #[inline]
    pub fn get_f32(&self, property: &str) -> Option<f32> {
        self.properties
            .get(property)
            .and_then(|value| if let &PropertyValue::Float(value) = value { Some(value) } else { None })
    }

    #[inline]
    pub fn get_bool(&self, property: &str) -> Option<bool> {
        self.properties
            .get(property)
            .and_then(|value| if let &PropertyValue::Boolean(value) = value { Some(value) } else { None })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackedArray<const N: usize> {
    data: Vec<u64>,
    /// Bits per Entry
    bpe: u32,
    /// Entries per Element
    epe: u32,
    mask: u64,
    _phantom: PhantomData<[u8; N]>,
}

#[allow(clippy::inline_always)]
impl<const N: usize> PackedArray<N> {
    pub fn new(palette_size: usize) -> Self {
        let bpe = Self::ceil_log2(palette_size as u64);
        let epe = u64::BITS / bpe;
        let mask = (1u64 << u64::from(bpe)) - 1;

        Self {
            data: vec![0u64; N.div_ceil(epe as usize)],
            bpe,
            epe,
            mask,
            _phantom: PhantomData,
        }
    }

    #[inline(always)]
    const fn ceil_log2(x: u64) -> u32 {
        u64::BITS - (x.saturating_sub(1)).leading_zeros()
    }

    #[inline]
    const fn index_of(&self, i: usize) -> (usize, u64) {
        (i / self.epe as usize, ((i % self.epe as usize) * self.bpe as usize) as u64)
    }

    #[inline]
    pub const fn get(&self, i: usize) -> usize {
        let (index, bit) = self.index_of(i);

        ((self.data.as_slice()[index] >> bit) & self.mask) as usize
    }

    #[inline]
    pub const fn set(&mut self, i: usize, value: usize) {
        let (index, bit) = self.index_of(i);

        self.data.as_mut_slice()[index] &= !(self.mask << bit);
        self.data.as_mut_slice()[index] |= (value as u64) << bit;
    }

    pub fn grow(&mut self, palette_size: usize) {
        let new_bits = Self::ceil_log2(palette_size as u64);

        if new_bits == self.bpe {
            return;
        }

        let epe = u64::BITS / new_bits;
        let mut new_data = vec![0u64; N.div_ceil(epe as usize)];

        for i in 0..N {
            new_data[i / epe as usize] |= (self.get(i) as u64) << ((i % epe as usize) * new_bits as usize);
        }

        self.data = new_data;
        self.bpe = new_bits;
        self.epe = epe;
        self.mask = (1u64 << new_bits) - 1;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteData<const N: usize> {
    Single,
    Linear(PackedArray<N>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Cube whose size is specified by [`CHUNK_SIZE`] constant.
pub struct SubChunk {
    /// Palette of block states.
    pub palette: Vec<SubChunkBlockState>,
    /// Array of palette indices.
    pub data: PaletteData<{ SUBCHUNK_SIZE * SUBCHUNK_SIZE * SUBCHUNK_SIZE }>,
    /// Array of block light level values.
    pub light_levels: [u8; SUBCHUNK_SIZE * SUBCHUNK_SIZE * SUBCHUNK_SIZE],
}

impl SubChunk {
    #[inline]
    pub fn empty() -> Self {
        Self {
            palette: vec![SubChunkBlockState::air()],
            data: PaletteData::Single,
            light_levels: [0; SUBCHUNK_SIZE * SUBCHUNK_SIZE * SUBCHUNK_SIZE],
        }
    }

    #[inline]
    pub const fn index_of(position: USizePoint3D) -> usize {
        position.y * const { SUBCHUNK_SIZE * SUBCHUNK_SIZE } + position.z * SUBCHUNK_SIZE + position.x
    }

    #[inline]
    pub fn empty_full_height() -> Box<[Self; SUBCHUNK_COUNT]> {
        new_boxed_array(repeat_n(Self::empty(), SUBCHUNK_COUNT).collect())
    }

    #[inline]
    pub fn index_of_state(&self, id: u32) -> usize {
        self.palette.iter().position(|palette_block| palette_block.id == id).unwrap_or(0)
    }

    #[inline]
    pub fn try_insert(&mut self, block: SubChunkBlockState) -> usize {
        if let Some(index) = self.palette.iter().position(|palette_block| palette_block == &block) {
            index
        } else {
            let index = self.palette.len();

            self.palette.push(block);

            if self.palette.len() > 1 {
                match &mut self.data {
                    PaletteData::Single => self.data = PaletteData::Linear(PackedArray::new(self.palette.len())),
                    PaletteData::Linear(packed_array) => packed_array.grow(self.palette.len()),
                }
            }

            index
        }
    }

    #[must_use]
    #[inline]
    pub fn get_block_unchecked(&self, position: USizePoint3D) -> &SubChunkBlockState {
        self.get_block_by_idx_unchecked(Self::index_of(position))
    }

    #[must_use]
    #[inline]
    pub fn get_block_by_idx_unchecked(&self, index: usize) -> &SubChunkBlockState {
        unsafe { self.palette.get_unchecked(self.get_index_unchecked(index)) }
    }

    #[must_use]
    #[inline]
    pub const fn get_index_unchecked(&self, index: usize) -> usize {
        match &self.data {
            PaletteData::Single => 0,
            PaletteData::Linear(data) => data.get(index),
        }
    }

    #[inline]
    pub const fn set_index_unchecked(&mut self, index: usize, val: usize) {
        match &mut self.data {
            PaletteData::Single => (),
            PaletteData::Linear(data) => data.set(index, val),
        }
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
    pub dirty: bool,
}

impl Chunk {
    #[inline]
    pub fn empty() -> Self {
        Self {
            origin: IPoint2D::ZERO,
            biomes: [BiomeBase::Sky; SUBCHUNK_SIZE * SUBCHUNK_SIZE],
            subchunks: SubChunk::empty_full_height(),
            dirty: true,
        }
    }

    #[must_use]
    #[inline]
    pub const fn with_origin(mut self, origin: IPoint2D) -> Self {
        self.origin = origin;

        self
    }

    #[inline]
    pub const fn index_of_biome(position: USizePoint2D) -> usize {
        position.y * SUBCHUNK_SIZE + position.x
    }

    // pub fn deserialize<T: AsRef<[u8]>>(data: T) -> io::Result<Self> {
    //     let mut chunk = Self::empty();

    //     let mut data = data.as_ref();

    //     chunk.origin = {
    //         let mut x = [0; 4];
    //         let mut z = [0; 4];

    //         data.read_exact(&mut x)?;
    //         data.read_exact(&mut z)?;

    //         let x = i32::from_be_bytes(x);
    //         let z = i32::from_be_bytes(z);

    //         IPoint2D::new(x, z)
    //     };

    //     for y in 0..CHUNK_HEIGHT {
    //         for z in 0..SUBCHUNK_SIZE {
    //             for x in 0..SUBCHUNK_SIZE {
    //                 let mut buf = [0; 2];

    //                 data.read_exact(&mut buf)?;

    //                 let [subchunk, y] = Self::get_subchunk_index(y);

    // chunk.subchunks[subchunk].blocks[SubChunk::index_of(USizePoint3D::new(x, y,
    // z))] = buf[0];
    // chunk.subchunks[subchunk].
    // light_levels[SubChunk::index_of(USizePoint3D::new(x, y, z))] = buf[1];
    //             }
    //         }
    //     }

    //     Ok(chunk)
    // }

    #[inline]
    pub const fn corner(position: USizePoint3D) -> Option<[IPoint2D; 3]> {
        match (position.x, position.z) {
            (0, 0) => Some([IPoint2D::NEG_X, IPoint2D::NEG_Y, IPoint2D::NEG_ONE]),
            (0, SUBCHUNK_XZ_MAX) => Some([IPoint2D::NEG_X, IPoint2D::Y, IPoint2D::new(-1, 1)]),
            (SUBCHUNK_XZ_MAX, 0) => Some([IPoint2D::X, IPoint2D::NEG_Y, IPoint2D::new(1, -1)]),
            (SUBCHUNK_XZ_MAX, SUBCHUNK_XZ_MAX) => Some([IPoint2D::X, IPoint2D::Y, IPoint2D::ONE]),
            _ => None,
        }
    }

    #[inline]
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

    // #[must_use]
    // pub fn into_serialized(self) -> Vec<u8> {
    //     let mut data = Vec::new();

    //     data.extend_from_slice(&self.origin.x.to_be_bytes());
    //     data.extend_from_slice(&self.origin.y.to_be_bytes());

    //     for y in 0..CHUNK_HEIGHT {
    //         for z in 0..SUBCHUNK_SIZE {
    //             for x in 0..SUBCHUNK_SIZE {
    //                 let [subchunk, y] = Self::get_subchunk_index(y);

    // data.push(self.subchunks[subchunk].
    // blocks[SubChunk::index_of(USizePoint3D::new(x, y, z))]);
    // data.push(self.subchunks[subchunk].
    // light_levels[SubChunk::index_of(USizePoint3D::new(x, y, z))]);
    //             }
    //         }
    //     }

    //     data
    // }

    // #[must_use]
    // pub fn serialize(&self) -> Vec<u8> {
    //     let mut data = Vec::new();

    //     data.extend_from_slice(&self.origin.x.to_be_bytes());
    //     data.extend_from_slice(&self.origin.y.to_be_bytes());

    //     for y in 0..CHUNK_HEIGHT {
    //         for z in 0..SUBCHUNK_SIZE {
    //             for x in 0..SUBCHUNK_SIZE {
    //                 let [subchunk, y] = Self::get_subchunk_index(y);

    // data.push(self.subchunks[subchunk].
    // blocks[SubChunk::index_of(USizePoint3D::new(x, y, z))]);
    // data.push(self.subchunks[subchunk].
    // light_levels[SubChunk::index_of(USizePoint3D::new(x, y, z))]);
    //             }
    //         }
    //     }

    //     data
    // }

    #[inline]
    pub const fn to_origin_and_local(position: IPoint3D) -> (IPoint2D, USizePoint3D) {
        let local_x = position.x.rem_euclid(SUBCHUNK_SIZE_I32);
        let local_z = position.z.rem_euclid(SUBCHUNK_SIZE_I32);
        let origin = IPoint2D::new((position.x - local_x) / SUBCHUNK_SIZE_I32, (position.z - local_z) / SUBCHUNK_SIZE_I32);
        let local = USizePoint3D::new(local_x as usize, position.y as usize, local_z as usize);

        (origin, local)
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

    #[inline]
    pub fn set_block(&mut self, position: USizePoint3D, block: SubChunkBlockState) {
        if self.contains_local_position(position) {
            self.set_block_unchecked(position, block);
        }
    }

    #[inline]
    pub fn set_block_unchecked(&mut self, position: USizePoint3D, block: SubChunkBlockState) {
        let [subchunk, y] = Self::get_subchunk_index(position.y);

        unsafe {
            let subchunk = self.subchunks.get_unchecked_mut(subchunk);
            let index = subchunk.try_insert(block);

            subchunk.set_index_unchecked(SubChunk::index_of(position.with_y(y)), index);
        }
    }

    #[inline]
    pub fn get_block(&self, position: USizePoint3D) -> Option<&SubChunkBlockState> {
        if self.contains_local_position(position) {
            Some(self.get_block_unchecked(position))
        } else {
            None
        }
    }

    #[must_use]
    #[inline]
    pub fn get_block_unchecked(&self, position: USizePoint3D) -> &SubChunkBlockState {
        let [subchunk, y] = Self::get_subchunk_index(position.y);

        unsafe { self.subchunks.get_unchecked(subchunk).get_block_unchecked(USizePoint3D { y, ..position }) }
    }

    #[must_use]
    #[inline]
    pub fn get_block_by_idx_unchecked(&self, subchunk: usize, index: usize) -> &SubChunkBlockState {
        unsafe { self.subchunks.get_unchecked(subchunk).get_block_by_idx_unchecked(index) }
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
    pub fn get_light_level_by_idx(&self, subchunk: usize, index: usize) -> u8 {
        unsafe { *self.subchunks.get_unchecked(subchunk).light_levels.get_unchecked(index) }
    }

    #[inline]
    pub fn get_light_level_by_idx_mut(&mut self, subchunk: usize, index: usize) -> &mut u8 {
        unsafe { self.subchunks.get_unchecked_mut(subchunk).light_levels.get_unchecked_mut(index) }
    }

    #[inline]
    pub const fn get_light_level_mut(&mut self, position: USizePoint3D) -> &mut u8 {
        let [subchunk, y] = Self::get_subchunk_index(position.y);

        &mut self.subchunks[subchunk].light_levels[SubChunk::index_of(USizePoint3D { y, ..position })]
    }

    #[inline]
    pub fn check_for_local_block(&self, local_position: USizePoint3D) -> bool {
        let [subchunk, y] = Self::get_subchunk_index(local_position.y);

        !self.subchunks[subchunk]
            .get_block_by_idx_unchecked(SubChunk::index_of(USizePoint3D { y, ..local_position }))
            .is_air()
    }

    #[inline]
    pub fn check_for_block(&self, position: IPoint3D) -> bool {
        self.contains_position(position) && self.check_for_local_block(Self::to_local(position))
    }

    #[inline]
    pub const fn get_light(&self, position: USizePoint3D, is_sky_light: bool) -> u8 {
        if is_sky_light {
            self.get_sky_light(position)
        } else {
            self.get_block_light(position)
        }
    }

    #[inline]
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
    pub fn get_sky_light_by_idx(&self, subchunk: usize, index: usize) -> u8 {
        (self.get_light_level_by_idx(subchunk, index) >> 4) & 0xF
    }

    #[inline]
    pub fn get_sky_light_unchecked(&self, position: USizePoint3D) -> u8 {
        (self.get_light_level_unchecked(position) >> 4) & 0xF
    }

    #[inline]
    pub const fn sky_light_from_level(level: u8) -> u8 {
        (level >> 4) & 0xF
    }

    #[inline]
    pub const fn block_light_from_level(level: u8) -> u8 {
        level & 0xF
    }

    #[inline]
    pub fn set_sky_light_by_idx(&mut self, subchunk: usize, index: usize, value: u8) {
        let level = self.get_light_level_by_idx_mut(subchunk, index);

        *level = (*level & 0xF) | (value << 4);
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
    pub fn set_block_light_by_idx(&mut self, subchunk: usize, index: usize, value: u8) {
        let level = self.get_light_level_by_idx_mut(subchunk, index);

        *level = (*level & 0xF0) | value;
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
    type Item = (USizePoint3D, &'a SubChunkBlockState);

    #[inline]
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
    #[inline]
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

impl<'a> Iterator for ChunkFaceIter<'a> {
    type Item = (USizePoint3D, &'a SubChunkBlockState);

    #[inline]
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
    #[inline]
    pub const fn new(chunk: &'a Chunk) -> Self {
        Self { chunk, y: 0, z: 0, x: 0 }
    }
}

impl<'a> Iterator for ChunkIter<'a> {
    type Item = (USizePoint3D, &'a SubChunkBlockState);

    #[inline]
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
    world_y_offset: usize,
    current_index: usize,
}

impl<'a> SubChunkIter<'a> {
    #[inline]
    pub const fn new(subchunk: &'a SubChunk, subchunk_idx: usize) -> Self {
        Self {
            subchunk,
            world_y_offset: subchunk_idx * SUBCHUNK_SIZE,
            current_index: 0,
        }
    }
}

impl<'a> Iterator for SubChunkIter<'a> {
    type Item = (USizePoint3D, Option<&'a SubChunkBlockState>);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.current_index >= const { SUBCHUNK_SIZE * SUBCHUNK_SIZE * SUBCHUNK_SIZE } {
            return None;
        }

        let idx = self.current_index;

        self.current_index += 1;

        let block_state = self.subchunk.get_block_by_idx_unchecked(idx);

        let local_x = idx & 0xF;
        let local_z = (idx >> 4) & 0xF;
        let local_y = idx >> 8;

        let chunk_local_position = USizePoint3D::new(local_x, self.world_y_offset + local_y, local_z);

        Some((chunk_local_position, if block_state.is_air() { None } else { Some(block_state) }))
    }
}
