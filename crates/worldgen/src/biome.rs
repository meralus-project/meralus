use mavelin_shared::Random;
use mavelin_world::Biome;

use super::noise;

pub struct BiomeGenerator {
    temp: noise::Fbm<4>,
    rain: noise::Fbm<4>,
    base: noise::Fbm<2>,
}

pub struct BiomeNoise {
    pub biomes: Vec<Biome>,
    pub temp: Vec<f64>,
    pub rain: Vec<f64>,
}

impl BiomeGenerator {
    pub fn new(seed: i64) -> Self {
        Self {
            temp: noise::Fbm::new(&mut Random::new(seed * 9871)),
            rain: noise::Fbm::new(&mut Random::new(seed * 39811)),
            base: noise::Fbm::new(&mut Random::new(seed * 543_321)),
        }
    }

    pub fn get_biome_noise(&self, origin: glam::IVec2, size: glam::IVec2) -> BiomeNoise {
        let mut temp = self.temp.generate_noise2d(
            origin.as_dvec2(),
            size,
            glam::DVec3::new(0.025_000_000_372_529_03, 0.025_000_000_372_529_03, 0.25),
            0.5,
        );

        let mut rain = self.rain.generate_noise2d(
            origin.as_dvec2(),
            size,
            glam::DVec3::new(0.050_000_000_745_058_06, 0.050_000_000_745_058_06, 0.333_333_333_333_333_3),
            0.5,
        );

        let base_data = self
            .base
            .generate_noise2d(origin.as_dvec2(), size, glam::DVec3::new(0.25, 0.25, 0.588_235_294_117_647_1), 0.5);

        let mut biomes = vec![Biome::Sky; size.x as usize * size.y as usize];
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
                biomes[index] = Biome::new(temperature, raininess);

                index += 1;
            }
        }

        BiomeNoise { biomes, temp, rain }
    }
}
