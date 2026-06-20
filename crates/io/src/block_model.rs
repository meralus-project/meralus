use std::{fmt, path::PathBuf};

use ahash::HashMap;
use meralus_shared::{Axis, Face, Point2D, Point3D};
use serde::{
    Deserialize, Serialize,
    de::{Error, Visitor},
};

use crate::{TexturePath, TextureRef};

/// Represents a single face (or side) of a [`BlockElement`].
#[derive(Debug, Default, Serialize)]
pub struct BlockFace {
    /// Texture used by this face.
    ///
    /// At this moment it should always be a reference to an id from
    /// [`BlockModel::textures`] (in `#your-texture-id` format).
    pub texture: String,
    /// Optional UV coordinates in the range `0.0..1.0` on both axes (where
    /// `0.0, 0.0` is bottom-left and `1.0, 1.0` is top-right).
    pub uv: Option<[Point2D; 2]>,
    /// Specifies whether to apply color of current biome to the texture.
    pub tint: bool,
    /// Face, if there is a block on which this face will be "skipped"
    /// when creating a chunk mesh.
    pub cull_face: Option<Face>,
}

impl<'de> Deserialize<'de> for BlockFace {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct BlockFaceVisitor;

        impl<'de> Visitor<'de> for BlockFaceVisitor {
            type Value = BlockFace;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("expected valid block face")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(BlockFace {
                    texture: v.to_string(),
                    ..Default::default()
                })
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut value = BlockFace::default();
                let mut texture = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "texture" => {
                            texture.replace(map.next_value()?);
                        }
                        "tint" => value.tint = map.next_value()?,
                        "uv" => value.uv = Some(map.next_value()?),
                        "cull_face" => value.cull_face = Some(map.next_value()?),
                        field => Err(Error::unknown_field(field, &["texture", "uv", "cull_face"]))?,
                    }
                }

                value.texture = texture.ok_or_else(|| Error::missing_field("texture"))?;

                Ok(value)
            }
        }

        deserializer.deserialize_any(BlockFaceVisitor)
    }
}

/// A complete block model made up of [`elements`].
///
/// [`elements`]: BlockModel::elements
#[derive(Debug, Deserialize, Serialize)]
pub struct BlockModel {
    /// Optional path to another model whose properties will be merged with
    /// those of current model.
    pub parent: Option<TexturePath>,
    /// Object of form key-value where key is the texture id and value is the
    /// path to the texture
    #[serde(default)]
    pub textures: HashMap<String, TextureRef>,
    /// Specifies whether this model will create ambient occlusion.
    ///
    /// `true` by default, but it is recommended to set `false` for models that
    /// are not a full cuboid (e.g. levers).
    pub ambient_occlusion: Option<bool>,
    /// List of [`BlockElement`] describing the individual parts of the model.
    #[serde(default)]
    pub elements: Vec<BlockElement>,
}

impl BlockModel {
    pub fn from_slice(data: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(data)
    }

    pub fn is_transparent(&self) -> bool {
        self.textures.is_empty() && self.elements.is_empty()
    }

    pub fn is_opaque(&self) -> bool {
        self.elements
            .iter()
            .any(|element| element.start == Point3D::ZERO && element.end == Point3D::ONE)
    }
}

/// Cuboid from the [`start`] point to the [`end`] point.
///
/// [`start`]: BlockElement::start
/// [`end`]: BlockElement::end
#[derive(Debug, Deserialize, Serialize)]
pub struct BlockElement {
    /// Start point in the range `0.0..1.0` for all axes.
    pub start: Point3D,
    /// End point in the range `0.0..1.0` for all axes.
    pub end: Point3D,
    /// Faces with certain UV coordinates, texture and some other parameters.
    #[serde(flatten)]
    pub faces: Faces,
    /// Optional element rotation.
    #[serde(default)]
    pub rotation: Option<ElementRotation>,
}

/// Represents rotation about either axis.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub struct ElementRotation {
    /// Point around which rotation will be performed.
    pub origin: Point3D,
    /// Axis of rotation.
    pub axis: Axis,
    /// Angle of rotation (in degrees).
    pub angle: f32,
}

/// Faces with certain UV coordinates, texture and some other parameters.
#[derive(Debug, Deserialize, Serialize)]
pub enum Faces {
    /// Variant representing the **same** properties for **each** of faces.
    #[serde(rename = "all")]
    All(BlockFace),
    /// Variant representing unique properties for each of faces.
    ///
    /// It is not necessary to describe all faces.
    #[serde(rename = "faces")]
    Unique(HashMap<Face, BlockFace>),
}

#[cfg(test)]
mod tests {
    use crate::BlockModel;

    #[test]
    fn test_block_model_parsing() {
        let data = include_bytes!("../../../resources/models/grass_block.json");

        assert!(serde_json::from_slice::<BlockModel>(data).is_ok());
    }
}
