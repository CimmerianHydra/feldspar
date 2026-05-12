use bevy::prelude::*;

use crate::plugin::chunk::VoxelChunk;
use crate::plugin::worldgen::{flat::FlatGenerator, hills::HillsGenerator};


// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct WorldgenPlugin;

pub const DEV_SEED: u64 = 0;

impl Plugin for WorldgenPlugin {
    fn build(&self, app: &mut App) {
        // Default to a flat world with seed 0.
        // Swap for `ActiveWorldGenerator::Hills(...)` once the hills generator
        // is in place, or replace at runtime when load-game UI exists.
        app
        
        .insert_resource(ActiveWorldGenerator::Hills(HillsGenerator::new(DEV_SEED)))

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

use crate::plugin::graphics::block_material::{VoxelMaterial, VoxelMaterialExtension};
use crate::plugin::graphics::block_textures::create_texture_array;
use crate::plugin::chunk::{StaticChunk, NeedsRemeshing};
use crate::plugin::dimension::DimensionId;

/// How many chunks out from origin to pre-spawn on each axis.
/// Total chunk count = (2*R + 1)^3 — with R=8 that's 4913 chunks.
/// Drop this to 2 or 3 if startup feels heavy while testing.
const DEV_CHUNK_RADIUS: i32 = 16;
const DEV_CHUNK_HEIGHT: i32 = 4;

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
        for cy in -DEV_CHUNK_HEIGHT..=DEV_CHUNK_HEIGHT {
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