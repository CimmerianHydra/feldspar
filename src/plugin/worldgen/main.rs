use bevy::prelude::*;

use crate::plugin::chunk::VoxelChunk;
use crate::plugin::worldgen::flat::FlatGenerator;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct WorldgenPlugin;

impl Plugin for WorldgenPlugin {
    fn build(&self, app: &mut App) {
        // Default to a flat world with seed 0.
        // Swap for `ActiveWorldGenerator::Hills(...)` once the hills generator
        // is in place, or replace at runtime when load-game UI exists.
        app
        
        .insert_resource(ActiveWorldGenerator::Flat(FlatGenerator::new(0)))

        ;
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 — Trait
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Pure `(chunk_pos) → chunk-contents` function.
///
/// Implementations must be deterministic for a given seed and `Send + Sync`,
/// so chunks can eventually be cooked off-thread on `AsyncComputeTaskPool`.
///
/// The caller supplies an already-allocated `VoxelChunk` (typically built
/// with `VoxelChunk::empty()`, but a pooled buffer works too); the generator
/// overwrites it in place.
pub trait WorldGenerator: Send + Sync {
    fn generate_chunk(&self, chunk_pos: IVec3, out: &mut VoxelChunk);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 — Active Generator (Resource)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The world generator currently in use for chunk creation.
#[derive(Resource, Clone, Debug)]
pub enum ActiveWorldGenerator {
    Flat(FlatGenerator),
    // Hills(HillsGenerator),   // TODO
}

impl WorldGenerator for ActiveWorldGenerator {
    #[inline]
    fn generate_chunk(&self, chunk_pos: IVec3, out: &mut VoxelChunk) {
        match self {
            ActiveWorldGenerator::Flat(g) => g.generate_chunk(chunk_pos, out),
        }
    }
}