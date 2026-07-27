//! Spatial recipes: their shape vocabulary, the registry that indexes them,
//! and the JSON that fills it.
//!
//! The matcher is generic over how the caller addresses its own ingredients
//! (`PlacedItem<T>` in, `MatchedRecipe<T>` out), which is what lets
//! `sim::crafting` hand it `PlacementID`s without this module ever learning
//! that spatial inventories exist.

pub mod asset;
pub mod registry;
pub mod shape;

pub use asset::{
    populate_spatial_recipe_registry_sys, RecipeDefinitionAsset, RecipeStackAsset,
    SpatialRecipeAssets,
};
pub use registry::{
    canonicalize, MatchedRecipe, PlacedItem, RecipeEntry, SpatialRecipeRegistry,
};
pub use shape::{recognize, CraftShape};
