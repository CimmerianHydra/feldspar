use bevy::prelude::*;
use noise::{Fbm, MultiFractal, NoiseFn, Perlin};

use crate::plugin::chunk::{VoxelChunk, CHUNK_SIZE};
use crate::plugin::voxel::Voxel;
use crate::plugin::worldgen::main::WorldGenerator;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// CONSTANTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

// Block ids (resolved via registry later).
// Change SURFACE_ID to GRASS_ID once grass exists in the registry.
const SURFACE_ID: u16 = 3; // currently dirt; will become grass
const DIRT_ID:    u16 = 1;
const SLATE_ID:   u16 = 2;

/// World Y around which the noise oscillates.
const SEA_LEVEL: i32 = 0;

/// Maximum vertical excursion from sea level (peaks ≈ +AMPLITUDE, valleys ≈ -AMPLITUDE).
const AMPLITUDE: i32 = 128;

/// Number of blocks at the top of a column that are dirt (including the surface block).
/// With thickness = 3, surface + 2 below are dirt; deeper is slate.
const DIRT_THICKNESS: i32 = 3;

// FBM parameters — see the architecture notes for tuning advice.
const OCTAVES:     usize = 4;
const FREQUENCY:   f64   = 1.0 / 256.0;
const PERSISTENCE: f64   = 0.5;
const LACUNARITY:  f64   = 2.0;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// HILLS GENERATOR
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Heightmap-based generator: rolling hills and the occasional mountain.
///
/// - Top of column: `SURFACE_ID` (grass placeholder)
/// - Next `DIRT_THICKNESS - 1` blocks down: dirt
/// - Deeper: slate
/// - Above the surface: air
#[derive(Clone, Debug)]
pub struct HillsGenerator {
    pub seed: u64,
    fbm: Fbm<Perlin>,
}

impl HillsGenerator {
    pub fn new(seed: u64) -> Self {
        let fbm = Fbm::<Perlin>::new(seed as u32)
            .set_octaves(OCTAVES)
            .set_frequency(FREQUENCY)
            .set_persistence(PERSISTENCE)
            .set_lacunarity(LACUNARITY);
        Self { seed, fbm }
    }

    /// World Y of the topmost solid block for the column at `(world_x, world_z)`.
    #[inline]
    fn surface_height(&self, world_x: i32, world_z: i32) -> i32 {
        // Fbm output is roughly in [-1, 1]; clamp defensively to avoid
        // pathological spikes from corner cases of stacked octaves.
        let n = self.fbm
            .get([world_x as f64, world_z as f64])
            .clamp(-1.0, 1.0);
        SEA_LEVEL + (n * AMPLITUDE as f64).round() as i32
    }
}

impl WorldGenerator for HillsGenerator {

    /// "generate_chunk" takes a VoxelChunk mutable reference and modifies it into
    /// a newly generated chunk. This makes it more async friendly when worldgen will
    /// be weaved into the chunk load/unload/generation system.
    fn generate_chunk(&self, chunk_pos: IVec3, out: &mut VoxelChunk) {
        let chunk_base = chunk_pos * CHUNK_SIZE as i32;
        let chunk_base_y = chunk_base.y;
        let chunk_top_y  = chunk_base_y + CHUNK_SIZE as i32 - 1;

        // Possible-range bounds on the surface across the whole world.
        let max_surface  = SEA_LEVEL + AMPLITUDE;
        let min_surface  = SEA_LEVEL - AMPLITUDE;
        let min_dirt_low = min_surface - (DIRT_THICKNESS - 1);

        // ── Fast path 1: chunk entirely above the highest possible surface ─
        if chunk_base_y > max_surface {
            *out = VoxelChunk::empty();
            return;
        }

        // ── Fast path 2: chunk entirely below the lowest possible dirt ─────
        if chunk_top_y < min_dirt_low {
            *out = VoxelChunk::filled(Voxel::full(SLATE_ID));
            return;
        }

        // ── Slow path: per-column heightmap ────────────────────────────────
        *out = VoxelChunk::empty();

        let surface_voxel = Voxel::full(SURFACE_ID);
        let dirt_voxel    = Voxel::full(DIRT_ID);
        let slate_voxel   = Voxel::full(SLATE_ID);

        for lz in 0..CHUNK_SIZE {
            for lx in 0..CHUNK_SIZE {
                let world_x = chunk_base.x + lx as i32;
                let world_z = chunk_base.z + lz as i32;

                let surface_y     = self.surface_height(world_x, world_z);
                let lowest_dirt_y = surface_y - (DIRT_THICKNESS - 1);

                for ly in 0..CHUNK_SIZE {
                    let world_y = chunk_base_y + ly as i32;

                    let voxel = if world_y > surface_y {
                        continue;                       // air, already zeroed
                    } else if world_y == surface_y {
                        surface_voxel                   // grass-to-be
                    } else if world_y >= lowest_dirt_y {
                        dirt_voxel
                    } else {
                        slate_voxel
                    };

                    out.set(lx, ly, lz, voxel);
                }
            }
        }
    }
}