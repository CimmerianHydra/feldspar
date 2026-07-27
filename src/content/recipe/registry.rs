use bevy::prelude::*;
use std::collections::HashMap;

use crate::content::item::{ItemID, ItemStack};
use crate::content::recipe::shape::{recognize, CraftShape};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – CANONICALIZATION
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Apply every symmetry permutation of `shape` to the vertex sequence and
/// keep the lexicographically smallest, comparing (item id, count).
///
/// The SAME function runs on recipes at registration and on live layouts at
/// match time, so the two can never disagree about orientation. `T` rides
/// along untouched (recipes carry `()`, live layouts carry whatever handle
/// the caller uses to address its own storage), which is how the matcher
/// learns which physical placement maps to which canonical vertex without a
/// second pass.
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
// SECTION 2 – THE REGISTRY  (the first "RecipeMap")
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// One registered recipe: per-vertex required counts in canonical order,
/// and what it produces.
#[derive(Clone, Debug)]
pub struct RecipeEntry {
    pub counts: Vec<u16>,
    pub result: ItemStack,
}

/// One placed ingredient, as the matcher wants to see it: where it sits,
/// what it is, and an opaque handle back to wherever the caller keeps it.
///
/// The handle is generic so that the registry never has to name
/// `SpatialInventory` or `PlacementID`, both of which live a layer up in
/// `sim`. A caller passes its own addressing scheme through and gets it
/// back in `MatchedRecipe::consume`.
#[derive(Clone, Copy, Debug)]
pub struct PlacedItem<T> {
    pub pos:    Vec2,
    pub stack:  ItemStack,
    pub handle: T,
}

/// A successful match against a live layout: what to give, and exactly how
/// much to drain from which placement.
#[derive(Clone, Debug)]
pub struct MatchedRecipe<T> {
    pub shape:   CraftShape,
    pub result:  ItemStack,
    pub consume: Vec<(T, u16)>,
}

/// Keyed by (shape, canonical item sequence) — items only, counts stay OUT
/// of the key because "at least N" can't be hash equality. The hash gets us
/// to a tiny bucket; a linear scan settles counts. Buckets are sorted by
/// descending total cost so a 3-iron recipe outranks a 1-iron recipe with
/// the same layout — stack-aware recipes are already just data.
#[derive(Resource, Default)]
pub struct SpatialRecipeRegistry {
    recipes: HashMap<(CraftShape, Vec<ItemID>), Vec<RecipeEntry>>,
}

impl SpatialRecipeRegistry {
    pub fn new() -> Self {
        Self { recipes: HashMap::new() }
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
    ///
    /// Takes a flat slice rather than an inventory, which is what keeps this
    /// module independent of how any particular machine stores its inputs.
    pub fn match_layout<T: Copy>(&self, placed: &[PlacedItem<T>]) -> Option<MatchedRecipe<T>> {
        let positions: Vec<Vec2> = placed.iter().map(|p| p.pos).collect();
        let (shape, order) = recognize(&positions)?;

        // Vertex-ordered triples, then canonical form.
        let verts: Vec<(ItemID, u16, T)> = order.iter()
            .map(|&i| (placed[i].stack.id, placed[i].stack.count, placed[i].handle))
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
                    .map(|((_, _, handle), &need)| (*handle, need))
                    .collect(),
            })
    }
}
