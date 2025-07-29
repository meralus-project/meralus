use meralus_world::Property;

use crate::Block;

pub struct AirBlock;

impl Block for AirBlock {
    fn id(&self) -> &'static str {
        "air"
    }

    fn get_properties(&self) -> Vec<Property<'_>> {
        Vec::new()
    }
}

pub struct DirtBlock;

impl Block for DirtBlock {
    fn id(&self) -> &'static str {
        "dirt"
    }

    fn get_properties(&self) -> Vec<Property<'_>> {
        Vec::new()
    }
}

pub struct GrassBlock;

impl Block for GrassBlock {
    fn id(&self) -> &'static str {
        "grass_block"
    }

    fn get_properties(&self) -> Vec<Property<'_>> {
        Vec::new()
    }
}
