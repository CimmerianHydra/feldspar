use bevy::prelude::*;

use crate::content::block::BlockRegistry;
use crate::content::block::TextureSlot;
use crate::content::model_source::ModelSourceRegistry;
use crate::render::mesh::model::ModelArena;
use crate::render::mesh::shapes;
use crate::voxel::{BlockRotation, BlockShape, ModelTable, VariantKey};
use crate::render::mesh::import;

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
    sources:      Res<ModelSourceRegistry>,
) {
    let mut arena = ModelArena::new();
    bake_all(&mut registry, &mut arena, &sources);
    info!("Model arena baked: {} models, {} quads",
          arena.model_count(), arena.quad_count());
    commands.insert_resource(arena);
}

pub fn bake_all(registry: &mut BlockRegistry, arena: &mut ModelArena, sources: &ModelSourceRegistry) {
    // Index 0 is air, which has no geometry.
    for i in 1..registry.definitions.len() {
        let shape = registry.definitions[i].shape.clone();
        let name  = registry.definitions[i].name.clone();

        let table = match &shape {
            BlockShape::Cube =>
                ModelTable::from_rotations(
                    arena.bake_rotations(&shapes::cube(), BlockRotation::all())),

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
            BlockShape::Custom(path) => match sources.get(path) {
                Some(doc) => {
                    // The slot table was resolved from this same document, so
                    // the two can only disagree if a block used a non-model
                    // appearance. Pad rather than panic — a wrong texture is a
                    // better failure than a crash in the mesher's inner loop,
                    // which is the only other place this could show up.
                    let needed = doc.slot_count();
                    let slots  = &mut registry.definitions[i].texture_slots;
                    if slots.len() < needed {
                        warn!("Block '{name}' resolved {} texture slots but model '{path}' \
                               declares {needed} surfaces — padding with 'missing'. Give it \
                               an appearance of kind 'model'.", slots.len());
                        slots.resize(needed, TextureSlot::MISSING);
                    }

                    let elements = import::import(doc, path);
                    if elements.is_empty() {
                        warn!("Model '{path}' produced no geometry for '{name}' — using a cube.");
                        ModelTable::single(arena.bake(&shapes::cube(), BlockRotation::IDENTITY))
                    } else {
                        ModelTable::single(arena.bake(&elements, BlockRotation::IDENTITY))
                    }
                }
                None => {
                    error!("Block '{name}' names model '{path}', which is not in the registry. \
                            Falling back to a cube.");
                    ModelTable::single(arena.bake(&shapes::cube(), BlockRotation::IDENTITY))
                }
            },
        };

        table.key.validate(&name);
        registry.definitions[i].models = table;
    }
}
