use bevy::prelude::*;

use crate::plugin::chunk::{VoxelChunk, CHUNK_SIZE};
use crate::plugin::voxel::Voxel;
use crate::plugin::worldgen::main::WorldGenerator;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// CONSTANTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

// Hardcoded while only dirt and slate exist.
// TODO: resolve through the BlockRegistry once stable-by-name lookup is in.
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
}

impl FlatGenerator {
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }
}

impl WorldGenerator for FlatGenerator {
    fn generate_chunk(&self, chunk_pos: IVec3, out: &mut VoxelChunk) {
        let chunk_base_y = chunk_pos.y * CHUNK_SIZE as i32;
        let chunk_top_y  = chunk_base_y + CHUNK_SIZE as i32 - 1;        // inclusive

        // Lowest world-y that's still dirt (e.g. -2 when thickness = 3).
        let lowest_dirt_y = SURFACE_Y - (DIRT_THICKNESS - 1);

        // ── Fast path 1: chunk entirely above the surface → all air ────────
        if chunk_base_y > SURFACE_Y {
            *out = VoxelChunk::empty();
            return;
        }

        // ── Fast path 2: chunk entirely below dirt band → all slate ────────
        if chunk_top_y < lowest_dirt_y {
            *out = VoxelChunk::filled(Voxel::full(SLATE_ID));
            return;
        }

        // ── Slow path: chunk straddles the surface band ────────────────────
        // Start from empty so anything above the surface is already air
        // without needing per-cell branching for it.
        *out = VoxelChunk::empty();

        let dirt  = Voxel::full(DIRT_ID);
        let slate = Voxel::full(SLATE_ID);

        for ly in 0..CHUNK_SIZE {
            let world_y = chunk_base_y + ly as i32;

            let voxel = if world_y > SURFACE_Y {
                continue;                       // air, already zeroed
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

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// DEV FUNCTIONS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

use crate::plugin::graphics::block_material::{VoxelMaterial, VoxelMaterialExtension};
use crate::plugin::worldgen::main::ActiveWorldGenerator;
use crate::plugin::graphics::block_textures::create_texture_array;
use crate::plugin::chunk::{StaticChunk, NeedsRemeshing};
use crate::plugin::dimension::DimensionId;

/// How many chunks out from origin to pre-spawn on each axis.
/// Total chunk count = (2*R + 1)^3 — with R=8 that's 4913 chunks.
/// Drop this to 2 or 3 if startup feels heavy while testing.
const DEV_CHUNK_RADIUS: i32 = 2;

pub fn setup_dev_chunks(
    mut commands:     Commands,
    mut images:       ResMut<Assets<Image>>,
    mut vox_material: ResMut<Assets<VoxelMaterial>>,
    worldgen:         Res<ActiveWorldGenerator>,
) {
    // ── base texture array ────────────────────────────────────────────────
    // Layer 0: purple-black  (used by FaceTextures::Simple when base=0)
    // Layer 1: slate
    // Layer 2: limestone
    // to add more as registry grows
    let array_texture = create_texture_array(
        &[
            "assets\\textures\\extra\\missing_tex.png",
            "assets\\textures\\terrain\\dirt.png",
            "assets\\textures\\terrain\\slate.png",
        ],
        &mut images,
    );

    // ── overlay texture array ─────────────────────────────────────────────
    // Layer 0: transparent (NO_OVERLAY = 0)
    // Layer 1: grass overlay (tinted green via FaceTextures::Tinted)
    let array_overlay = create_texture_array(
        &[
            "assets\\textures\\extra\\missing_tex.png",
            "assets\\textures\\tinted\\grass_top.png",
            "assets\\textures\\tinted\\grass_side.png",
        ],
        &mut images,
    );
    
    let material_handle = vox_material.add(VoxelMaterial {
        base: StandardMaterial {
            base_color: Color::from(bevy::color::palettes::basic::WHITE),
            metallic: 0.0,
            perceptual_roughness: 0.8,
            ..default()
        },
        extension: VoxelMaterialExtension {
            array_texture,
            array_overlay,
        },
    });

    // ── chunk generation + spawn ──────────────────────────────────────────
    let dim_id = DimensionId::OVERWORLD;

    for cx in -DEV_CHUNK_RADIUS..=DEV_CHUNK_RADIUS {
        for cy in -DEV_CHUNK_RADIUS..=DEV_CHUNK_RADIUS {
            for cz in -DEV_CHUNK_RADIUS..=DEV_CHUNK_RADIUS {
                let chunk_pos = IVec3::new(cx, cy, cz);

                let mut chunk_data = VoxelChunk::empty();
                worldgen.generate_chunk(chunk_pos, &mut chunk_data);

                commands.spawn((
                    StaticChunk { dimension: dim_id, position: chunk_pos },
                    chunk_data.clone(),
                    MeshMaterial3d(material_handle.clone()),
                    NeedsRemeshing,
                ));
            }
        }
    }
}