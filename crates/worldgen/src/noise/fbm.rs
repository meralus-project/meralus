use mavelin_shared::Random;
use mavelin_world::new_boxed_array;

use super::Perlin;
use crate::TERRAIN_NOISE_SIZE;

pub struct Fbm<const O: usize> {
    noise_generators: Box<[Perlin; O]>,
}

impl<const O: usize> Fbm<O> {
    pub fn new(random: &mut Random) -> Self {
        Self {
            noise_generators: new_boxed_array((0..O).map(|_| Perlin::new(random)).collect()),
        }
    }

    pub fn generate_noise_for_coords(&self, x: f64, y: f64) -> f64 {
        let mut noise = 0.0;
        let mut frequency = 1.0;

        for generator in self.noise_generators.as_ref() {
            noise += generator.get2d(x * frequency, y * frequency);

            frequency /= 2.0;
        }

        noise
    }

    pub fn generate_noise(&self, offset: glam::DVec3, size: glam::IVec3, scale: glam::DVec3) -> [f64; TERRAIN_NOISE_SIZE] {
        let mut noise_data = [0.0; TERRAIN_NOISE_SIZE];
        let mut frequency = 1.0;

        for generator in self.noise_generators.as_ref() {
            generator.generate_noise3d(&mut noise_data, offset, size, scale * frequency, frequency);

            frequency /= 2.0;
        }

        noise_data
    }

    pub fn generate_noise2d(&self, offset: glam::DVec2, size: glam::IVec2, mut scale: glam::DVec3, frequency_step: f64) -> Vec<f64> {
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
                glam::DVec3::new(scale.x * xy_scale, scale.y * xy_scale, 0.55 / frequency),
            );

            xy_scale *= scale.z;
            frequency *= frequency_step;
        }

        noise_data
    }
}
