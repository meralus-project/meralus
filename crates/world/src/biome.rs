use crate::chunk::SubChunkBlockState;

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

        while i < 64 {
            let mut k = 0;

            while k < 64 {
                table[i + k * 64] = Self::get_by_temp_rain(i as f32 / 63.0, k as f32 / 63.0);

                k += 1;
            }

            i += 1;
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
        } else {
            if rain > 0.5 && temperature < 0.7 {
                Self::Swampland
            } else if temperature < 0.5 {
                Self::Taiga
            } else if temperature < 0.97 {
                if rain < 0.35 { Self::Shrubland } else { Self::Forest }
            } else {
                if rain < 0.45 {
                    Self::Plains
                } else if rain < 0.9 {
                    Self::SeasonalForest
                } else {
                    Self::Rainforest
                }
            }
        }
    }

    pub fn new(temperature: f64, raininess: f64) -> Self {
        let i = (temperature * 63.0) as usize;
        let j = (raininess * 63.0) as usize;

        Self::LOOKUP[i + j * 64]
    }

    pub fn top(self, sand: SubChunkBlockState, snow: SubChunkBlockState, grass_block: SubChunkBlockState) -> SubChunkBlockState {
        match self {
            Self::Desert | Self::IceDesert => sand, // SAND
            Self::Taiga | Self::Tundra => snow,     // SAND
            _ => grass_block,                       // GRASS_BLOCK
        }
    }

    pub fn bottom(self, sand: SubChunkBlockState, dirt: SubChunkBlockState) -> SubChunkBlockState {
        match self {
            Self::Desert | Self::IceDesert => sand, // SAND
            _ => dirt,                              // DIRT
        }
    }
}
