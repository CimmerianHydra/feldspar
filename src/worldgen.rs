//! World generation: pure `(chunk coord) → chunk contents` functions.
//!
//! Sits at the simulation tier — it may name `content` (to resolve block
//! names) and `space` (to fill a `VoxelChunk`), and nothing above it.
//! Deliberately free of any spawning logic: a generator writes voxels into a
//! buffer and stops, which is what will let chunks be cooked off-thread.

pub mod flat;
pub mod hills;

pub use flat::FlatGenerator;
pub use hills::HillsGenerator;

use bevy::prelude::*;

use crate::content::block::BlockRegistry;
use crate::space::VoxelChunk;
use crate::voxel::Voxel;

pub const DEV_SEED: u64 = 0;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – THE TRAIT
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

/// Resolve a required block by name, or die loudly at startup.
///
/// A generator that silently produced air because someone renamed a block
/// would be a nightmare to diagnose three hours into a world.
pub(crate) fn resolve_block(name: &str, registry: &BlockRegistry) -> Voxel {
    let id = registry.by_name(name.to_string()).unwrap_or_else(|| {
        panic!("worldgen: required block '{name}' is not in the registry")
    });
    Voxel::full(id.0)
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – THE ACTIVE GENERATOR
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
            ActiveWorldGenerator::Flat(g)  => g.generate_chunk(chunk_pos, out),
            ActiveWorldGenerator::Hills(g) => g.generate_chunk(chunk_pos, out),
        }
    }
}

/// Installs the active generator once the block registry is populated.
///
/// Ordering against the registry population lives in [`crate::app::loading`],
/// which owns the whole load sequence.
pub fn init_active_worldgen_sys(mut commands: Commands, registry: Res<BlockRegistry>) {
    commands.insert_resource(ActiveWorldGenerator::Hills(HillsGenerator::new(
        DEV_SEED, &registry,
    )));
}
