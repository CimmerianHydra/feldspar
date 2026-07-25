use bevy::prelude::*;

use crate::plugin::loader::block_registry::BlockRegistry;
use crate::plugin::space::main::VoxelChunk;
use crate::plugin::loader::block_assets::populate_block_registry_sys;
use crate::plugin::state::GameState;
use crate::plugin::worldgen::{flat::FlatGenerator, hills::HillsGenerator};


// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━


pub const DEV_SEED: u64 = 0;

pub struct WorldgenPlugin;

impl Plugin for WorldgenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnExit(GameState::AssetLoading),
            init_active_worldgen_sys.after(populate_block_registry_sys),
        );
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

fn init_active_worldgen_sys(
    mut commands: Commands,
    registry:     Res<BlockRegistry>,
) {
    let generator = ActiveWorldGenerator::Hills(HillsGenerator::new(DEV_SEED, &registry));
    commands.insert_resource(generator);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 — Active Generator (Resource)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The world generator currently in use for chunk creation.
#[derive(Resource, Clone, Debug)]
pub enum ActiveWorldGenerator {
    Flat(FlatGenerator),
    Hills(HillsGenerator),
}

impl WorldGenerator for ActiveWorldGenerator {
    #[inline]
    fn generate_chunk(&self, chunk_pos: IVec3, out: &mut VoxelChunk) {
        match self {
            ActiveWorldGenerator::Flat(g) => g.generate_chunk(chunk_pos, out),
            ActiveWorldGenerator::Hills(g) => g.generate_chunk(chunk_pos, out),  // new
        }
    }
}
