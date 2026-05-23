use bevy::prelude::*;

use crate::plugin::chunk::{VoxelChunk, CHUNK_SIZE};
use crate::plugin::voxel::Voxel;
use crate::plugin::worldgen::main::WorldGenerator;
use crate::plugin::loader::block_registry::BlockRegistry;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// CONSTANTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

// Hardcoded while only dirt and slate exist.
const DIRT_ID:  u16 = 1;
const SLATE_ID: u16 = 2;

/// World Y of the topmost solid block (sea level).
const SURFACE_Y: i32 = 0;

/// Number of dirt blocks at the top of a column, *including* the surface block.
/// With thickness = 3, the surface (y=0) plus y=-1 and y=-2 are dirt;
/// y ≤ -3 is slate.
const DIRT_THICKNESS: i32 = 3;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// FLAT GENERATOR
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Perfectly flat world:
/// - `y == 0` → dirt (placeholder for grass)
/// - `y ∈ {-1, -2}` → dirt
/// - `y ≤ -3` → slate
/// - `y ≥ 1` → air
#[derive(Clone, Debug)]
pub struct FlatGenerator {
    /// Unused by the flat generator; kept for API parity with seeded generators.
    pub seed: u64,

    // Pre-resolved blocks. Built once, copied freely.
    surface: Voxel,
    dirt:    Voxel,
    slate:   Voxel,
}

impl FlatGenerator {
    pub fn new(seed: u64, registry: &BlockRegistry) -> Self {

        fn resolve(name: String, registry: &BlockRegistry) -> Voxel {
            let id = registry.by_name(name.to_string()).unwrap_or_else(|| {
                panic!("HillsGenerator: required block '{}' not in registry", name)
            });
            Voxel::full(id.0)
        }

        Self {
            seed,
            surface: resolve("grass".to_string(), registry),
            dirt:    resolve("dirt".to_string(), registry),
            slate:   resolve("slate".to_string(), registry),
        }
    }
}

impl WorldGenerator for FlatGenerator {

    /// "generate_chunk" takes a VoxelChunk mutable reference and modifies it into
    /// a newly generated chunk. This makes it more async friendly when worldgen will
    /// be weaved into the chunk load/unload/generation system.
    fn generate_chunk(&self, chunk_pos: IVec3, out: &mut VoxelChunk) {
        let chunk_base_y = chunk_pos.y * CHUNK_SIZE as i32;
        let chunk_top_y  = chunk_base_y + CHUNK_SIZE as i32 - 1;        // inclusive

        // Lowest world-y that's still dirt (e.g. -2 when thickness = 3).
        let lowest_dirt_y = SURFACE_Y - (DIRT_THICKNESS - 1);

        // ── Fast path 1: chunk entirely above the surface -> all air ────────
        if chunk_base_y > SURFACE_Y {
            *out = VoxelChunk::empty();
            return;
        }

        // ── Fast path 2: chunk entirely below dirt band -> all slate ────────
        if chunk_top_y < lowest_dirt_y {
            *out = VoxelChunk::filled(self.slate);
            return;
        }

        // ── Slow path: chunk straddles the surface band ────────────────────
        // Start from empty so anything above the surface is already air
        // without needing per-cell branching for it.
        *out = VoxelChunk::empty();

        let surface = self.surface;
        let dirt  = self.dirt;
        let slate = self.slate;

        for ly in 0..CHUNK_SIZE {
            let world_y = chunk_base_y + ly as i32;

            let voxel = if world_y > SURFACE_Y {
                continue;                       // air, already zeroed
            } else if world_y == SURFACE_Y {
                surface
            } else if world_y >= lowest_dirt_y {
                dirt                            // surface + dirt band
            } else {
                slate                           // bedrock
            };

            for lz in 0..CHUNK_SIZE {
                for lx in 0..CHUNK_SIZE {
                    out.set(lx, ly, lz, voxel);
                }
            }
        }
    }
}

