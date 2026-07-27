//! Turning voxels into triangles.
//!
//! - `element` — the box/quad IR every model is authored in
//! - `shapes`  — the canonical primitives, one orientation each
//! - `model`   — the arena of baked models, plus derived occlusion
//! - `bake`    — one pass at load that fills every block's `ModelTable`
//! - `mesher`  — the per-chunk mesh build, driven by `Changed<VoxelChunk>`

pub mod bake;
pub mod element;
pub mod mesher;
pub mod model;
pub mod shapes;

pub use element::{BakedQuad, CullTag, Element, ElementBox, FaceSpec};
pub use model::{BakedModel, ModelArena, ModelFlags};

use bevy::prelude::*;

use crate::BakeSet;

pub struct ChunkMeshPlugin;

impl Plugin for ChunkMeshPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                mesher::add_material_to_chunk_sys,
                mesher::update_dirty_mesh_sys,
            )
                .chain()
                .in_set(BakeSet::Mesh),
        );
    }
}
