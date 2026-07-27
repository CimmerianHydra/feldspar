use bevy::prelude::*;

use crate::content::block::BlockRegistry;
use crate::render::mesh::model::ModelArena;
use crate::render::mesh::shapes;
use crate::voxel::{BlockRotation, BlockShape, ModelTable, VariantKey};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// GEOMETRY BAKING
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Fill in every block definition's `ModelTable` and build the arena those
/// tables index into.
///
/// This lives in `render`, not in `content`, even though it writes into the
/// block registry — because a model arena is a rendering structure and
/// content must not depend on the renderer. What content owns is the
/// `ModelID`s, which are vocabulary; what the renderer owns is the geometry
/// they name. The registry is the meeting point, written once at load and
/// read-only from then on.
pub fn bake_block_geometry_sys(
    mut commands: Commands,
    mut registry: ResMut<BlockRegistry>,
) {
    let mut arena = ModelArena::new();
    bake_all(&mut registry, &mut arena);
    info!("Model arena baked: {} models, {} quads",
          arena.model_count(), arena.quad_count());
    commands.insert_resource(arena);
}

pub fn bake_all(registry: &mut BlockRegistry, arena: &mut ModelArena) {
    // Index 0 is air, which has no geometry.
    for i in 1..registry.definitions.len() {
        let shape = registry.definitions[i].shape.clone();
        let name  = registry.definitions[i].name.clone();

        let table = match &shape {
            BlockShape::Cube =>
                ModelTable::single(arena.bake(&shapes::cube(), BlockRotation::IDENTITY)),

            BlockShape::Slab =>
                ModelTable::from_rotations(
                    arena.bake_rotations(&shapes::panel(8.0), BlockRotation::all())),

            BlockShape::Panel =>
                ModelTable::from_rotations(
                    arena.bake_rotations(&shapes::panel(1.0), BlockRotation::all())),

            BlockShape::Stair =>
                ModelTable::from_rotations(
                    arena.bake_rotations(&shapes::stair(), BlockRotation::all())),

            BlockShape::Slope =>
                ModelTable::from_rotations(
                    arena.bake_rotations(&shapes::slope(), BlockRotation::all())),

            // The 64 masks, baked in order, indexed by 6 connection bits.
            BlockShape::Pipe => {
                let ids = shapes::all_pipe_variants()
                    .iter()
                    .map(|els| arena.bake(els, BlockRotation::IDENTITY))
                    .collect();
                ModelTable::new(VariantKey::stateful(6), ids)
            }

            // Blockbench import — parser is future work; fall back to a
            // cube so the match stays total and nothing panics.
            BlockShape::Custom(_path) =>
                ModelTable::single(arena.bake(&shapes::cube(), BlockRotation::IDENTITY)),
        };

        table.key.validate(&name);
        registry.definitions[i].models = table;
    }
}
