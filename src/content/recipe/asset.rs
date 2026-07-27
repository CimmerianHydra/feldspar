use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use serde::Deserialize;

use crate::content::item::{ItemID, ItemRegistry, ItemStack};
use crate::content::recipe::registry::SpatialRecipeRegistry;
use crate::content::recipe::shape::CraftShape;

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
