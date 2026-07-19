use bevy::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

use crate::plugin::inventory::spatial::{PlacementID, SpatialInventory};
use crate::plugin::crafting::shape::{recognize, CraftShape};
use crate::plugin::inventory::main::ItemStack;
use crate::plugin::loader::item_registry::{ItemID, ItemRegistry};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// CANONICALIZATION
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Apply every symmetry permutation of `shape` to the vertex sequence and
/// keep the lexicographically smallest, comparing (item id, count).
///
/// The SAME function runs on recipes at registration and on live layouts at
/// match time, so the two can never disagree about orientation. `T` rides
/// along untouched (recipes carry `()`, live layouts carry `PlacementID`),
/// which is how the matcher learns which physical placement maps to which
/// canonical vertex without a second pass.
pub fn canonicalize<T: Copy>(
    shape: CraftShape,
    verts: &[(ItemID, u16, T)],
) -> Option<Vec<(ItemID, u16, T)>> {
    let perms = shape.symmetry_perms();
    if perms.is_empty() || verts.len() != shape.arity() { return None; }

    perms.iter()
        .map(|perm| perm.iter().map(|&i| verts[i]).collect::<Vec<_>>())
        .min_by(|a, b| {
            let ka = a.iter().map(|(id, n, _)| (id.0, *n));
            let kb = b.iter().map(|(id, n, _)| (id.0, *n));
            ka.cmp(kb)
        })
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// REGISTRY  (the first "RecipeMap")
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// One registered recipe: per-vertex required counts in canonical order,
/// and what it produces.
#[derive(Clone, Debug)]
pub struct RecipeEntry {
    pub counts: Vec<u16>,
    pub result: ItemStack,
}

/// A successful match against a live layout: what to give, and exactly how
/// much to drain from which physical placement.
#[derive(Clone, Debug)]
pub struct MatchedRecipe {
    pub shape:   CraftShape,
    pub result:  ItemStack,
    pub consume: Vec<(PlacementID, u16)>,
}

/// Keyed by (shape, canonical item sequence) — items only, counts stay OUT
/// of the key because "at least N" can't be hash equality. The hash gets us
/// to a tiny bucket; a linear scan settles counts. Buckets are sorted by
/// descending total cost so a 3-iron recipe outranks a 1-iron recipe with
/// the same layout — stack-aware recipes are already just data.
#[derive(Resource)]
pub struct SpatialRecipeRegistry {
    recipes: HashMap<(CraftShape, Vec<ItemID>), Vec<RecipeEntry>>,
}

impl SpatialRecipeRegistry {

    pub fn new() -> Self {
        Self {
            recipes: HashMap::new(),
        }
    }

    pub fn register(
        &mut self,
        shape:  CraftShape,
        verts:  Vec<(ItemID, u16)>,  // author's vertex order
        result: ItemStack,
    ) -> bool {
        let tagged: Vec<(ItemID, u16, ())> =
            verts.iter().map(|&(id, n)| (id, n, ())).collect();
        let Some(canon) = canonicalize(shape, &tagged) else { return false; };

        let key    = (shape, canon.iter().map(|(id, _, _)| *id).collect());
        let counts = canon.iter().map(|(_, n, _)| *n).collect();

        let bucket = self.recipes.entry(key).or_default();
        bucket.push(RecipeEntry { counts, result });
        bucket.sort_by_key(|e| std::cmp::Reverse(e.counts.iter().map(|&n| n as u32).sum::<u32>()));
        true
    }

    /// The full pipeline: positions → shape → canonical key → bucket scan.
    pub fn match_inventory(&self, spatial: &SpatialInventory) -> Option<MatchedRecipe> {
        let entries: Vec<(PlacementID, Vec2, ItemStack)> = spatial
            .iter()
            .map(|(id, p)| (id, p.pos, p.stack))
            .collect();

        let positions: Vec<Vec2> = entries.iter().map(|(_, pos, _)| *pos).collect();
        let (shape, order) = recognize(&positions)?;

        // Vertex-ordered triples, then canonical form.
        let verts: Vec<(ItemID, u16, PlacementID)> = order.iter()
            .map(|&i| (entries[i].2.id, entries[i].2.count, entries[i].0))
            .collect();
        let canon = canonicalize(shape, &verts)?;

        let key = (shape, canon.iter().map(|(id, _, _)| *id).collect::<Vec<_>>());
        let bucket = self.recipes.get(&key)?;

        // Sorted-descending bucket → first hit is the richest satisfiable
        // recipe. Positional ≥ against canonical counts is valid because
        // canonicalization sorted equal-item runs by count on both sides.
        bucket.iter()
            .find(|entry| {
                canon.iter().zip(&entry.counts).all(|((_, have, _), need)| have >= need)
            })
            .map(|entry| MatchedRecipe {
                shape,
                result:  entry.result,
                consume: canon.iter().zip(&entry.counts)
                    .map(|((_, _, pid), &need)| (*pid, need))
                    .collect(),
            })
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// JSON  →  REGISTRY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn one() -> u16 { 1 }

#[derive(Asset, TypePath, Deserialize, Debug, Clone)]
pub struct RecipeDefinitionAsset {
    pub shape:    CraftShape,
    /// In the shape's conventional vertex order (Line3: [end, middle, end]).
    /// Orientation doesn't matter — canonicalization erases it — but WHICH
    /// vertex is the middle does.
    pub vertices: Vec<RecipeStackAsset>,
    pub result:   RecipeStackAsset,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RecipeStackAsset {
    pub item: String,
    #[serde(default = "one")]
    pub count: u16,
}

use bevy_asset_loader::prelude::*;

#[derive(AssetCollection, Resource)]
pub struct SpatialRecipeAssets {
    #[asset(path = "templates\\recipes", collection(typed))]
    pub recipes: Vec<Handle<RecipeDefinitionAsset>>,
}

pub fn populate_spatial_recipe_registry_sys(
    collection:    Res<SpatialRecipeAssets>,
    assets:        Res<Assets<RecipeDefinitionAsset>>,
    item_registry: Res<ItemRegistry>,
    mut registry:  ResMut<SpatialRecipeRegistry>,
) {
    for handle in &collection.recipes {
        let Some(def) = assets.get(handle) else { continue };

        if def.vertices.len() != def.shape.arity() {
            warn!("recipe skipped: {:?} needs {} vertices, got {}",
                  def.shape, def.shape.arity(), def.vertices.len());
            continue;
        }

        // Names are durable, IDs are not — resolve through the registry,
        // same rule as blocks.
        let resolve = |name: &str| item_registry.by_name(name.to_string());

        let verts: Option<Vec<(ItemID, u16)>> = def.vertices.iter()
            .map(|v| resolve(&v.item).map(|id| (id, v.count.max(1))))
            .collect();
        let (Some(verts), Some(result_id)) = (verts, resolve(&def.result.item)) else {
            warn!("recipe skipped: unresolved item name in {:?}", def);
            continue;
        };

        registry.register(
            def.shape,
            verts,
            ItemStack { id: result_id, count: def.result.count.max(1) },
        );
    }
    info!("Spatial recipe registry populated.");
}