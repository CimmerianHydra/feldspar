use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

use crate::voxel::BlockShape;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SHAPE SETS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

// A shape set is to a block what a part profile is to a substance: a NAMED
// list, written once, pointed at by many. "Every stone-like block also
// comes as a slab, a stair and a slope" is then one file, not an edit on
// fifty block definitions.
//
// Example — assets/templates/shapesets/base.shapeset.json:
// {
//   "set": "stone_family",
//   "shapes": ["Cube", "Slab", "Stair", "Slope", "Panel"]
// }
//
// MERGE SEMANTICS: two files declaring the same set name merge their shapes
// (dedup by value, declaration order preserved) — an addon file can add
// "Pipe" to "stone_family" without touching the base file. Same rule as
// PartRegistry::extend_profile.

/// One `*.shapeset.json` file: one named set.
#[derive(Deserialize, Asset, TypePath)]
pub struct ShapeSetAsset {
    pub set:    String,
    pub shapes: Vec<BlockShape>,
}

#[derive(AssetCollection, Resource)]
pub struct ShapeSetAssets {
    #[asset(path = "templates\\shapesets", collection(typed))]
    pub files: Vec<Handle<ShapeSetAsset>>,
}

#[derive(Resource, Default)]
pub struct ShapeSetRegistry {
    sets: HashMap<String, Vec<BlockShape>>,
}

impl ShapeSetRegistry {
    pub fn new() -> Self { Self::default() }

    /// The members of a set, or `None` if no file declared it.
    pub fn get(&self, name: &str) -> Option<&[BlockShape]> {
        self.sets.get(name).map(Vec::as_slice)
    }

    /// Append members, creating the set on first sight. Duplicates are
    /// dropped rather than warned about: two files legitimately listing
    /// `Cube` in the same set is not a mistake.
    pub fn extend_set(&mut self, name: &str, shapes: impl IntoIterator<Item = BlockShape>) {
        let list = self.sets.entry(name.to_string()).or_default();
        for shape in shapes {
            if !list.contains(&shape) {
                list.push(shape);
            }
        }
    }

    pub fn len(&self) -> usize { self.sets.len() }
    pub fn is_empty(&self) -> bool { self.sets.is_empty() }
}

/// Must run before `populate_block_registry_sys` — that system resolves
/// set names into shapes and cannot see a set that hasn't been parsed yet.
pub fn populate_shape_set_registry_sys(
    assets:       Res<ShapeSetAssets>,
    parsed:       Res<Assets<ShapeSetAsset>>,
    mut registry: ResMut<ShapeSetRegistry>,
) {
    for handle in &assets.files {
        let Some(file) = parsed.get(handle) else {
            error!("Shape set asset not parsed before resolution — check loading state.");
            continue;
        };
        registry.extend_set(&file.set, file.shapes.iter().cloned());
    }

    info!("ShapeSetRegistry: {} sets.", registry.len());
}
