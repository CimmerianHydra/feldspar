//! # Layer 6 — presentation
//!
//! Everything that turns simulation state into pixels: the voxel material,
//! the chunk mesher and the model arena behind it, block icons, the sky, and
//! the block highlight.
//!
//! May import: [`crate::voxel`], [`crate::space`], [`crate::content`],
//! [`crate::sim`].
//!
//! The rule that matters is the direction: presentation observes simulation
//! and never the reverse. Nothing here is named by any lower layer, and
//! removing this plugin leaves a game that still runs, just invisibly.

pub mod highlight;
pub mod icons;
pub mod material;
pub mod mesh;
pub mod sky;

pub use icons::{icon_voxel, BlockIcons};
pub use material::{VoxelMaterial, VoxelMaterialExtension, VoxelMaterialHandle};
pub use mesh::{BakedQuad, CullTag, Element, ElementBox, FaceSpec, ModelArena};

use bevy::prelude::*;

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            material::VoxelMaterialPlugin,
            mesh::ChunkMeshPlugin,
            icons::BlockIconPlugin,
            sky::SkyPlugin,
            highlight::BlockHighlightPlugin,
        ));
    }
}
