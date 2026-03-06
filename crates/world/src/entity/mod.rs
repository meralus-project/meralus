use ahash::HashMap;
use meralus_shared::{Point2D, Point3D, Size2D};
use serde::{Deserialize, Serialize};

use crate::{Face, TextureRef};

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct EntityTexture {
    pub path: TextureRef,
    pub size: Size2D,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct EntityElementFace {
    pub from: Point2D,
    pub to: Point2D,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(tag = "type")]
#[serde(rename_all = "kebab-case")]
pub enum EntityElementData {
    Cube {
        start: Point3D,
        end: Point3D,
        faces: HashMap<Face, EntityElementFace>,
    },
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct EntityElement {
    pub name: String,
    #[serde(flatten)]
    pub data: EntityElementData,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct EntityModel {
    pub texture: EntityTexture,
    pub elements: Vec<EntityElement>,
}

impl EntityModel {
    pub fn from_slice(data: &[u8]) -> serde_json::Result<Self> {
        serde_json::from_slice(data)
    }
}
