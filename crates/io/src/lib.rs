mod block_model;
mod block_states;
mod entity_model;

use core::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize, de::Visitor};
pub use serde_json::Error as JsonError;

pub use self::{
    block_model::{BlockElement, BlockFace, BlockModel, ElementRotation, Faces},
    block_states::{
        BlockState, BlockStateValidationError, BlockStates, BlockStatesValidationError, NumericProperty, Property, PropertyRegistry, PropertyType,
        PropertyValue,
    },
    entity_model::{EntityElement, EntityElementData, EntityElementFace, EntityModel, EntityTexture},
};

/// Texture path in `mod_name:path/to/file` format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TexturePath(pub String, pub PathBuf);

impl fmt::Display for TexturePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.0, self.1.display())
    }
}

impl<'de> Deserialize<'de> for TexturePath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct TexturePathVisitor;

        impl Visitor<'_> for TexturePathVisitor {
            type Value = TexturePath;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("valid texture path")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let (mod_name, path) = v.split_once(':').ok_or_else(|| serde::de::Error::custom("invalid texture path format"))?;

                Ok(TexturePath(mod_name.to_string(), PathBuf::from(path)))
            }
        }

        deserializer.deserialize_str(TexturePathVisitor)
    }
}

impl Serialize for TexturePath {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let id = format!("{}:{}", self.0, self.1.display());

        serializer.serialize_str(&id)
    }
}

/// Texture id in `#your-texture-id` format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureId(pub String);

impl<'de> Deserialize<'de> for TextureId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct TextureIdVisitor;

        impl Visitor<'_> for TextureIdVisitor {
            type Value = TextureId;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("valid texture id")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let id = v.strip_prefix('#').ok_or_else(|| serde::de::Error::custom("invalid texture id format"))?;

                Ok(TextureId(id.to_string()))
            }
        }

        deserializer.deserialize_str(TextureIdVisitor)
    }
}

impl Serialize for TextureId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let id = format!("#{}", self.0);

        serializer.serialize_str(&id)
    }
}

/// Reference to a texture, which is either its id or a path to the it.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum TextureRef {
    Id(TextureId),
    Path(TexturePath),
}
