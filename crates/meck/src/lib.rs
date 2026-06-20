use std::{borrow::Borrow, collections::HashMap, hash::Hash};

use glam::{UVec2, Vec2};
use image::{GenericImage, Rgba, RgbaImage};

#[allow(clippy::cast_possible_truncation)]
const fn alpha_blend(mut one: u32, mut two: u32) -> (u8, u8, u8, u8) {
    let mut i = (one.cast_signed() & -16_777_216).cast_unsigned() >> 24 & 255;
    let mut j = (two.cast_signed() & -16_777_216).cast_unsigned() >> 24 & 255;
    let mut k = u32::midpoint(i, j);

    if i == 0 && j == 0 {
        i = 1;
        j = 1;
    } else {
        if i == 0 {
            one = two;
            k /= 2;
        }

        if j == 0 {
            two = one;
            k /= 2;
        }
    }

    let l = (one >> 16 & 255) * i;
    let i1 = (one >> 8 & 255) * i;
    let j1 = (one & 255) * i;
    let k1 = (two >> 16 & 255) * j;
    let l1 = (two >> 8 & 255) * j;
    let i2 = (two & 255) * j;
    let j2 = (l + k1) / (i + j);
    let k2 = (i1 + l1) / (i + j);
    let l2 = (j1 + i2) / (i + j);

    (j2 as u8, k2 as u8, l2 as u8, k as u8)
}

const fn blend_colors(a: u32, b: u32, c: u32, d: u32) -> (u8, u8, u8, u8) {
    alpha_blend(pack_rgba(alpha_blend(a, b)), pack_rgba(alpha_blend(c, d)))
}

const fn pack_rgba((r, g, b, a): (u8, u8, u8, u8)) -> u32 {
    (a as u32) << 24 | (r as u32) << 16 | (g as u32) << 8 | b as u32
}

pub struct TextureAtlas<K: Hash + Eq> {
    texture_map: HashMap<K, (UVec2, UVec2, u8)>,
    next_texture_offset: UVec2,
    spacing: u32,
    mipmaps: Vec<RgbaImage>,
}

impl<K: Hash + Eq> TextureAtlas<K> {
    pub fn new(size: u32) -> Self {
        Self {
            texture_map: HashMap::new(),
            next_texture_offset: UVec2::ZERO,
            spacing: 0,
            mipmaps: vec![RgbaImage::new(size, size)],
        }
    }

    #[must_use]
    pub fn with_mipmaps(mut self, mipmaps: u32) -> Self {
        let (width, height) = self.main_texture().dimensions();

        self.mipmaps.extend((1..=mipmaps).map(|level| RgbaImage::new(width >> level, height >> level)));

        self
    }

    #[must_use]
    pub const fn with_spacing(mut self, spacing: u32) -> Self {
        self.spacing = spacing;

        self
    }

    pub fn mipmaps(&self) -> &[RgbaImage] {
        &self.mipmaps
    }

    pub fn main_texture(&self) -> &RgbaImage {
        &self.mipmaps[0]
    }

    pub fn main_level_mut(&mut self) -> &mut RgbaImage {
        &mut self.mipmaps[0]
    }

    pub fn size(&self) -> Vec2 {
        UVec2::from(self.main_texture().dimensions()).as_vec2()
    }

    pub fn get_texture_rect<Q: ?Sized + Hash + Eq>(&self, key: &Q) -> Option<(UVec2, UVec2, u8)>
    where
        K: Borrow<Q>,
    {
        self.texture_map.get(key).copied()
    }

    pub fn get_texture_uv<Q: ?Sized + Hash + Eq>(&self, key: &Q) -> Option<(Vec2, Vec2, u8)>
    where
        K: Borrow<Q>,
    {
        let size = self.size();

        self.get_texture_rect(key)
            .map(|(origin, texture_size, alpha)| (origin.as_vec2() / size, texture_size.as_vec2() / size, alpha))
    }

    pub fn textures(&self) -> usize {
        self.texture_map.len()
    }

    pub fn generate_mipmaps(&mut self, level: usize) {
        for i in 1..=level {
            self.generate_mipmap(i);
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn generate_mipmap(&mut self, level: usize) {
        if (1..self.mipmaps.len()).contains(&level) {
            let pixels = &self.mipmaps[level - 1];
            let size = self.main_texture().width() as usize >> level;

            let mut data = RgbaImage::new(size as u32, size as u32);

            for i1 in 0..(size as u32) {
                for j1 in 0..(size as u32) {
                    let [i2, j2] = [i1 * 2, j1 * 2];

                    let color: [u8; 4] = blend_colors(
                        pack_rgba(pixels[(i2, j2)].0.into()),
                        pack_rgba(pixels[(i2 + 1, j2)].0.into()),
                        pack_rgba(pixels[(i2, j2 + 1)].0.into()),
                        pack_rgba(pixels[(i2 + 1, j2 + 1)].0.into()),
                    )
                    .into();

                    data.put_pixel(i1, j1, Rgba(color));
                }
            }

            self.mipmaps[level] = data;
        }
    }

    pub fn contains_texture<Q: ?Sized + Hash + Eq>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
    {
        self.texture_map.contains_key(key)
    }

    pub fn step_next(&mut self, size: UVec2) {
        self.next_texture_offset = self.next_texture_offset.with_x(self.next_texture_offset.x + size.x + self.spacing);
    }

    /// # Errors
    ///
    /// Returns an error if the image is too large to be copied at the given
    /// position.
    pub fn special_append(&mut self, key: K, image: &RgbaImage) -> UVec2 {
        if let Some((_, size, _)) = self.get_texture_rect(&key) {
            return size;
        }

        let alpha = image.pixels().map(|pixel| pixel.0[3]).min().unwrap_or(0);
        let size = UVec2::from(image.dimensions());
        let offset = (self.next_texture_offset, size);

        let main_image = self.main_level_mut();
        let mut sub_image = main_image.sub_image(offset.0.x, 0, offset.1.x, offset.1.y);

        for k in 0..image.height() {
            for i in 0..image.width() {
                sub_image.put_pixel(i, k, image[(i, image.height() - 1 - k)]);
            }
        }

        self.texture_map.insert(key, (offset.0, offset.1, alpha));
        self.step_next(size);

        size
    }

    /// # Errors
    ///
    /// Returns an error if the image is too large to be copied at the given
    /// position.
    pub fn append(&mut self, key: K, image: &RgbaImage) -> (Vec2, Vec2, u8) {
        if let Some(rect) = self.get_texture_uv(&key) {
            return rect;
        }

        let alpha = image.pixels().map(|pixel| pixel.0[3]).min().unwrap_or(0);
        let offset = (self.next_texture_offset, UVec2::from(image.dimensions()));

        let main_image = self.main_level_mut();
        let mut sub_image = main_image.sub_image(offset.0.x, 0, offset.1.x, offset.1.y);

        for k in 0..image.height() {
            for i in 0..image.width() {
                sub_image.put_pixel(i, k, image[(i, image.height() - 1 - k)]);
            }
        }

        self.texture_map.insert(key, (offset.0, offset.1, alpha));
        self.step_next(image.dimensions().into());

        let size = self.size();

        (offset.0.as_vec2() / size, offset.1.as_vec2() / size, alpha)
    }
}
