#![allow(clippy::missing_errors_doc, clippy::cast_sign_loss, clippy::cast_possible_truncation)]

#[cfg(feature = "color")] mod color;
#[cfg(feature = "convert")] mod convert;
#[cfg(feature = "face")] mod face;
#[cfg(feature = "frustum")] mod frustum;
#[cfg(feature = "geometry")] mod geometry;
#[cfg(feature = "lerp")] mod lerp;
#[cfg(feature = "random")] mod random;
mod util;

#[cfg(feature = "color")] pub use color::Color;
#[cfg(feature = "convert")] pub use convert::*;
#[cfg(feature = "face")] pub use face::Face;
#[cfg(feature = "frustum")]
pub use frustum::{Frustum, FrustumCulling};
#[cfg(feature = "geometry")] pub use geometry::*;
#[cfg(feature = "lerp")] pub use lerp::Lerp;
#[cfg(feature = "random")] pub use random::Random;
pub use util::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum Axis {
    X,
    Y,
    Z,
}
