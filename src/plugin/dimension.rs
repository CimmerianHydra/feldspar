// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – DIMENSIONS  (dimension.rs)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Lightweight identifier for a logical dimension / world.
///
/// Stored in the `VoxelWorld` lookup key and on every `StaticChunk`.
/// Add your own constants or derive from an enum as needed.

use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Component, Reflect)]
pub struct DimensionID(pub u8);

impl DimensionID {
    pub const OVERWORLD:    Self = Self(0);
    pub const UNDERWORLD:   Self = Self(1);
    pub const LUA:          Self = Self(2);
    pub const MARS:         Self = Self(3);
}