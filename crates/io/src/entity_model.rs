use ahash::HashMap;
use mavelin_shared::Face;
use serde::{Deserialize, Serialize};

use crate::TextureRef;

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct EntityTexture {
    pub path: TextureRef,
    pub size: glam::Vec2,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct EntityElementFace {
    pub origin: glam::Vec2,
    pub size: glam::Vec2,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(tag = "type")]
#[serde(rename_all = "kebab-case")]
pub enum EntityElementData {
    Cube {
        start: glam::Vec3,
        end: glam::Vec3,
        pivot: Option<glam::Vec3>,
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
    #[allow(clippy::missing_errors_doc)]
    pub fn from_slice(data: &[u8]) -> serde_json::Result<Self> {
        serde_json::from_slice(data)
    }
}
