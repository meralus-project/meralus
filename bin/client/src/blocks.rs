use meralus_shared::Color;
use meralus_world::Property;

use crate::Block;

pub struct AirBlock;

impl Block for AirBlock {
    fn id(&self) -> &'static str {
        "air"
    }

    fn blocks_light(&self) -> bool {
        false
    }

    fn droppable(&self) -> bool {
        false
    }
}

pub struct StoneBlock;

impl Block for StoneBlock {
    fn id(&self) -> &'static str {
        "stone"
    }
}

pub struct WaterBlock;

impl Block for WaterBlock {
    fn id(&self) -> &'static str {
        "water"
    }

    fn tint_color(&self) -> Option<Color> {
        Some(Color::from_hsl(215.0, 1.0, 0.7))
    }

    fn blocks_light(&self) -> bool {
        false
    }

    fn consume_light_level(&self) -> u8 {
        1
    }

    fn cull_if_same(&self) -> bool {
        true
    }

    fn collidable(&self) -> bool {
        false
    }

    fn selectable(&self) -> bool {
        false
    }

    fn droppable(&self) -> bool {
        false
    }
}

pub struct DirtBlock;

impl Block for DirtBlock {
    fn id(&self) -> &'static str {
        "dirt"
    }
}

pub struct GrassBlock;

impl Block for GrassBlock {
    fn id(&self) -> &'static str {
        "grass_block"
    }

    fn tint_color(&self) -> Option<Color> {
        Some(Color::from_hsl(120.0, 0.4, 0.75))
    }
}

pub struct SandBlock;

impl Block for SandBlock {
    fn id(&self) -> &'static str {
        "sand"
    }
}

pub struct WoodBlock;

impl Block for WoodBlock {
    fn id(&self) -> &'static str {
        "wood"
    }
}

pub struct OakLeavesBlock;

impl Block for OakLeavesBlock {
    fn id(&self) -> &'static str {
        "oak_leaves"
    }

    fn tint_color(&self) -> Option<Color> {
        Some(Color::from_hsl(120.0, 0.4, 0.75))
    }

    fn consume_light_level(&self) -> u8 {
        1
    }

    fn blocks_light(&self) -> bool {
        false
    }
}

pub struct IceBlock;

impl Block for IceBlock {
    fn id(&self) -> &'static str {
        "ice"
    }

    fn blocks_light(&self) -> bool {
        false
    }

    fn consume_light_level(&self) -> u8 {
        1
    }

    fn cull_if_same(&self) -> bool {
        true
    }
}

pub struct GreenGlassBlock;

impl Block for GreenGlassBlock {
    fn id(&self) -> &'static str {
        "green_glass_block"
    }

    fn blocks_light(&self) -> bool {
        false
    }

    fn consume_light_level(&self) -> u8 {
        1
    }

    fn cull_if_same(&self) -> bool {
        true
    }
}

pub struct TorchBlock;

impl Block for TorchBlock {
    fn id(&self) -> &'static str {
        "torch"
    }

    fn blocks_light(&self) -> bool {
        false
    }

    fn light_level(&self) -> u8 {
        15
    }

    fn collidable(&self) -> bool {
        false
    }
}

pub struct SnowBlock;

impl Block for SnowBlock {
    fn id(&self) -> &'static str {
        "snow"
    }
}

#[allow(dead_code)]
pub struct TechTestBlock;

impl Block for TechTestBlock {
    fn id(&self) -> &'static str {
        "tech_test"
    }

    fn get_properties(&self) -> Vec<Property> {
        vec![]
    }
}
