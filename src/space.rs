//! # Layer 3 — space
//!
//! The authoritative "what is where". Chunks, the spaces that own them,
//! addressing, and the single write path every block change funnels through.
//!
//! May import: [`crate::voxel`].
//!
//! Notably it does **not** import the renderer or the physics builder. There
//! is no `NeedsRemeshing` here: a write touches `VoxelChunk` through
//! `DerefMut`, and the consumers query `Changed<VoxelChunk>` for themselves.
//! The world model does not know it is being drawn.

pub mod access;
pub mod address;
pub mod chunk;
pub mod entities;
pub mod frame;
pub mod write;

pub use access::{VoxelWorld, VoxelWorldMut};
pub use address::{BlockPos, VoxelAddress};
pub use chunk::{ChunkMap, ChunkSlot, VoxelChunk};
pub use entities::ChunkBlockEntities;
pub use frame::{
    build_moving_grid, spawn_dimension, Dimension, DimensionID, DimensionRegistry, MovingGrid,
};
pub use write::{BlockEvent, VoxelWriteRequest};

use bevy::prelude::*;

pub struct SpacePlugin;

impl Plugin for SpacePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DimensionRegistry>()
            .add_observer(write::voxel_write_obs);

        // Spawned directly against the World, not from a startup system, so
        // the overworld exists before any system can try to put a chunk in it.
        spawn_dimension(app.world_mut(), DimensionID::OVERWORLD, "Overworld");
    }
}
