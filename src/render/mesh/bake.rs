use bevy::prelude::*;

use crate::content::block::BlockRegistry;
use crate::content::block::TextureSlot;
use crate::content::model_def::{ModelDef, ModelDefRegistry, ModelLayout};
use crate::content::model_source::ModelSourceRegistry;
use crate::render::mesh::element::Element;
use crate::render::mesh::import;
use crate::render::mesh::model::{rotate_elements, ModelArena};
use crate::render::mesh::shapes;
use crate::voxel::{BlockRotation, BlockShape, ModelTable, RotationSet, VariantKey};

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
    sources: Res<ModelSourceRegistry>,
    defs: Res<ModelDefRegistry>,
) {
    let mut arena = ModelArena::new();
    bake_all(&mut registry, &mut arena, &sources, &defs);
    info!(
        "Model arena baked: {} models, {} quads",
        arena.model_count(),
        arena.quad_count()
    );
    commands.insert_resource(arena);
}

pub fn bake_all(
    registry: &mut BlockRegistry,
    arena: &mut ModelArena,
    sources: &ModelSourceRegistry,
    defs: &ModelDefRegistry,
) {
    // Index 0 is air, which has no geometry.
    for i in 1..registry.definitions.len() {
        let shape = registry.definitions[i].shape.clone();
        let name = registry.definitions[i].name.clone();

        let table = match &shape {
            // Every primitive bakes all 24 orientations.
            //
            // This is nearly free: the generators take no per-block input,
            // so all cube blocks in the game share one set of 24 arena
            // models — 144 quads, once, globally. The per-block cost is the
            // table itself, 32 entries of 4 bytes.
            //
            // Which rotations a block can actually be *placed* in is the
            // placement code's business. Baking generously here is what
            // lets a log rotate without the baker knowing what a log is.
            BlockShape::Cube => rotated(arena, &shapes::cube(), RotationSet::Full),
            BlockShape::Slab => rotated(arena, &shapes::panel(8.0), RotationSet::Full),
            BlockShape::Panel => rotated(arena, &shapes::panel(1.0), RotationSet::Full),
            BlockShape::Stair => rotated(arena, &shapes::stair(), RotationSet::Full),
            BlockShape::Slope => rotated(arena, &shapes::slope(), RotationSet::Full),

            // The 64 masks, baked in order, indexed by 6 connection bits.
            //
            // Still a generator rather than a model definition, but note
            // that a `parts` layout expresses exactly this — when you want
            // an authored pipe, it goes in a `.model.json` and this arm
            // stops being special.
            BlockShape::Pipe => {
                let ids = shapes::all_pipe_variants()
                    .iter()
                    .map(|els| arena.bake(els, BlockRotation::IDENTITY))
                    .collect();
                ModelTable::new(VariantKey::stateful(6), ids)
            }

            BlockShape::Custom(key) => {
                bake_custom(&name, key, arena, sources, defs, registry)
            }
        };

        table.key.validate(&name);
        registry.definitions[i].models = table;
    }
}

/// Bake one element list in every orientation a set names.
fn rotated(arena: &mut ModelArena, elements: &[Element], set: RotationSet) -> ModelTable {
    let rotations = set.rotations();
    let ids = arena.bake_rotations(elements, rotations.iter().copied());
    ModelTable::from_rotation_set(&rotations, ids)
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// CUSTOM MODELS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Resolve a `Custom(key)` through its definition and bake the whole
/// rotation × state cross product.
///
/// `key` is looked up in the definition registry first; a miss is treated
/// as a bare `.bbmodel` path, which is what every existing block does and
/// what keeps them working untouched.
fn bake_custom(
    name: &str,
    key: &str,
    arena: &mut ModelArena,
    sources: &ModelSourceRegistry,
    defs: &ModelDefRegistry,
    registry: &mut BlockRegistry,
) -> ModelTable {
    let owned;
    let def = match defs.get(key) {
        Some(def) => def,
        None => {
            if sources.get(key).is_none() {
                error!(
                    "Block '{name}' names model '{key}', which is neither a model \
                     definition nor a known geometry file. Falling back to a cube."
                );
                return ModelTable::single(arena.bake(&shapes::cube(), BlockRotation::IDENTITY));
            }
            owned = ModelDef::implicit(key);
            &owned
        }
    };

    // ── one element list per state value ─────────────────────────────────
    let states = match build_states(name, def, sources) {
        Some(states) if !states.iter().all(Vec::is_empty) => states,
        _ => {
            warn!("Model '{key}' produced no geometry for '{name}' — using a cube.");
            return ModelTable::single(arena.bake(&shapes::cube(), BlockRotation::IDENTITY));
        }
    };

    pad_texture_slots(name, key, def, sources, registry);

    // ── bake the cross product ───────────────────────────────────────────
    let rotations = def.rotations.rotations();
    let bits = def.state_bits();

    let per_rotation: Vec<Vec<_>> = rotations
        .iter()
        .map(|r| states.iter().map(|els| arena.bake(els, *r)).collect())
        .collect();

    // An unrotated model keeps a small state-only table — a lamp is 2
    // entries, not 64. Worth the branch: `validate` caps the whole thing at
    // 12 bits and a 6-bit connecting block would otherwise spend 5 of them
    // on rotations it never uses.
    if !def.rotations.uses_rotation() {
        return ModelTable::new(VariantKey::stateful(bits), per_rotation.into_iter().next().unwrap());
    }

    ModelTable::from_rotation_state(&rotations, bits, &per_rotation)
}

/// Expand a layout into `2^bits` element lists, indexed by state value.
fn build_states(
    name: &str,
    def: &ModelDef,
    sources: &ModelSourceRegistry,
) -> Option<Vec<Vec<Element>>> {
    match &def.layout {
        ModelLayout::Single(path) => Some(vec![import_geometry(name, path, sources)]),

        ModelLayout::Variants { bits, geometry } => Some(
            geometry
                .iter()
                .map(|path| import_geometry(name, path, sources))
                .collect::<Vec<_>>()
                .tap_len(1usize << bits),
        ),

        // The interesting one. Each part contributes to every state value
        // whose bit it names, so `bits + 1` files cover `2^bits` variants —
        // seven files for a pipe's sixty-four masks.
        ModelLayout::Parts { bits, parts } => {
            // Import and pre-rotate each part once, not once per state.
            let baked_parts: Vec<(Option<u8>, Vec<Element>)> = parts
                .iter()
                .map(|part| {
                    let mut elements = import_geometry(name, &part.geometry, sources);
                    if let Some(dir) = part.rotate {
                        elements = rotate_elements(&elements, BlockRotation::from_parts(dir, 0));
                    }
                    (part.when_bit, elements)
                })
                .collect();

            let mut out = Vec::with_capacity(1usize << bits);
            for state in 0..(1u32 << bits) {
                let mut elements = Vec::new();
                for (when_bit, part) in &baked_parts {
                    let present = match when_bit {
                        None => true,
                        Some(bit) => state & (1 << bit) != 0,
                    };
                    if present {
                        elements.extend_from_slice(part);
                    }
                }
                out.push(elements);
            }
            Some(out)
        }
    }
}

fn import_geometry(name: &str, path: &str, sources: &ModelSourceRegistry) -> Vec<Element> {
    match sources.get(path) {
        Some(doc) => import::import(doc, path),
        None => {
            warn!("Block '{name}': geometry '{path}' is not in the model registry — \
                   contributing nothing.");
            Vec::new()
        }
    }
}

/// Make sure the block resolved at least as many texture slots as the model
/// declares surfaces.
///
/// The slot table was resolved from the same definition, so the two can
/// only disagree if a block used a non-model appearance. Pad rather than
/// panic — a wrong texture is a better failure than a crash in the mesher's
/// inner loop, which is the only other place this could show up.
fn pad_texture_slots(
    name: &str,
    key: &str,
    def: &ModelDef,
    sources: &ModelSourceRegistry,
    registry: &mut BlockRegistry,
) {
    // An explicit surface list is the contract when several files
    // contribute; otherwise fall back to the widest single document, which
    // reproduces the old single-geometry behaviour exactly.
    let needed = if def.surfaces.is_empty() {
        def.geometry_paths()
            .iter()
            .filter_map(|path| sources.get(path))
            .map(|doc| doc.slot_count())
            .max()
            .unwrap_or(0)
    } else {
        def.surfaces.len()
    };

    let index = registry.by_name(name.to_string()).map(|id| id.0 as usize);
    let Some(index) = index else { return };
    let slots = &mut registry.definitions[index].texture_slots;

    if slots.len() < needed {
        warn!(
            "Block '{name}' resolved {} texture slots but model '{key}' declares \
             {needed} surfaces — padding with 'missing'. Give it an appearance of \
             kind 'model'.",
            slots.len()
        );
        slots.resize(needed, TextureSlot::MISSING);
    }
}

// ── small helper ─────────────────────────────────────────────────────────

/// Assert-and-return, so the `Variants` arm reads as one expression. The
/// loader already rejects a mismatched count, so this only fires if a
/// definition reached the baker without going through the registry.
trait TapLen: Sized {
    fn tap_len(self, expected: usize) -> Self;
}

impl<T> TapLen for Vec<T> {
    fn tap_len(self, expected: usize) -> Self {
        debug_assert_eq!(self.len(), expected, "state variant count mismatch");
        self
    }
}