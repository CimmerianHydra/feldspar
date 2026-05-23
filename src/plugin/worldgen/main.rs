use bevy::prelude::*;

use crate::plugin::block_registry::BlockRegistry;
use crate::plugin::chunk::VoxelChunk;
use crate::plugin::loader::texture_assets::VoxelMaterialHandle;
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
            OnExit(GameState::Loading),
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

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// DEV FUNCTIONS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

use crate::plugin::chunk::{StaticChunk, NeedsRemeshing};
use crate::plugin::dimension::DimensionID;

/// How many chunks out from origin to pre-spawn on each axis.
/// Total chunk count = (2*R + 1)^3 — with R=8 that's 4913 chunks.
/// Drop this to 2 or 3 if startup feels heavy while testing.
const DEV_CHUNK_RADIUS: i32 = 4;
const DEV_CHUNK_HEIGHT: i32 = 4;

pub fn setup_dev_chunks(
    mut commands:                   Commands,
    terrain_material_handle_res:    Res<VoxelMaterialHandle>,
    worldgen:                       Res<ActiveWorldGenerator>,
) {
    // ── chunk generation + spawn ──────────────────────────────────────────
    let dim_id = DimensionID::OVERWORLD;

    for cx in -DEV_CHUNK_RADIUS..=DEV_CHUNK_RADIUS {
        for cy in -DEV_CHUNK_HEIGHT..=DEV_CHUNK_HEIGHT {
            for cz in -DEV_CHUNK_RADIUS..=DEV_CHUNK_RADIUS {
                let chunk_pos = IVec3::new(cx, cy, cz);

                let mut chunk_data = VoxelChunk::empty();
                worldgen.generate_chunk(chunk_pos, &mut chunk_data);

                bevy::log::debug!("Generating static chunk at position ({}, {}, {})", cx, cy, cz);

                commands.spawn((
                    StaticChunk { dimension: dim_id, position: chunk_pos },
                    chunk_data.clone(),
                    MeshMaterial3d(terrain_material_handle_res.0.clone()),
                    NeedsRemeshing,
                ));
            }
        }
    }
}