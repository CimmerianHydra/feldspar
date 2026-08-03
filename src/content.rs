//! # Layer 4 — content
//!
//! Definitions, the registries that hold them, and the JSON that fills them.
//! Blocks, items, recipes, substances, texture arrays, shape sets.
//!
//! May import: [`crate::voxel`], [`crate::space`].
//!
//! This layer owns *what things are*, never *what they do*. It hands out
//! `BlockID`s and `ItemID`s and describes what each one looks like, weighs,
//! and is made of. Behaviour lives in [`crate::sim`]; drawing lives in
//! [`crate::render`].
//!
//! The plugin below only installs resources and the asset types. Sequencing
//! the load is [`crate::app::loading`]'s job, because the order spans this
//! layer and the renderer's model baker.


use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use bevy_common_assets::json::JsonAssetPlugin;

use crate::content::block::asset::{BlockDefinitionAsset, BlockDefinitionAssets};
use crate::content::block::registry::BlockRegistry;
use crate::content::block::behaviors::BlockBehaviorRegistry;
use crate::content::item::behaviors::ItemBehaviorRegistry;
use crate::content::item::asset::{ItemDefinitionAsset, ItemDefinitionAssets};
use crate::content::item::registry::ItemRegistry;
use crate::content::recipe::asset::{RecipeDefinitionAsset, SpatialRecipeAssets};
use crate::content::recipe::registry::SpatialRecipeRegistry;
use crate::content::shape_set::{ShapeSetAsset, ShapeSetAssets, ShapeSetRegistry};
use crate::content::substance::{
    PartProfileAsset, PartProfileAssets, PartRegistry, SubstanceDefinitionAssets, SubstanceFileAsset,
    SubstanceRegistry,
};
use crate::content::texture::{BlockTextureAssets, TextureRegistry};
use crate::GameState;
use crate::content::model_source::{BbModel, ModelSourceAssets, ModelSourceRegistry};


pub mod behavior;
pub mod block;
pub mod item;
pub mod model_source;
pub mod recipe;
pub mod shape_set;
pub mod substance;
pub mod texture;
/// Inserted by `bevy_asset_loader` the instant every collection above has
/// been built. `app::loading` gates the elaboration sequence on it, so no
/// step ever runs against a half-parsed world.
#[derive(Resource, Default)]
pub struct ContentFilesReady;
pub struct ContentPlugin;

impl Plugin for ContentPlugin {
    fn build(&self, app: &mut App) {
        app
            // ---- registries ------------------------------------------------
            .init_resource::<TextureRegistry>()
            .init_resource::<BlockRegistry>()
            .init_resource::<BlockBehaviorRegistry>()
            .init_resource::<ItemBehaviorRegistry>()
            .init_resource::<ShapeSetRegistry>()
            .init_resource::<ItemRegistry>()
            .init_resource::<SubstanceRegistry>()
            .init_resource::<PartRegistry>()
            .init_resource::<SpatialRecipeRegistry>()
            .init_resource::<ModelSourceRegistry>()

            // ---- parsers for the *.json definition files --------------------
            .add_plugins(JsonAssetPlugin::<BlockDefinitionAsset>::new(&["block.json"]))
            .add_plugins(JsonAssetPlugin::<RecipeDefinitionAsset>::new(&["recipe.json"]))
            .add_plugins(JsonAssetPlugin::<ItemDefinitionAsset>::new(&["item.json"]))
            .add_plugins(JsonAssetPlugin::<SubstanceFileAsset>::new(&["substance.json"]))
            .add_plugins(JsonAssetPlugin::<PartProfileAsset>::new(&["part.json"]))
            .add_plugins(JsonAssetPlugin::<ShapeSetAsset>::new(&["shapeset.json"]))
            .add_plugins(JsonAssetPlugin::<BbModel>::new(&["bbmodel"]))

            // ---- gate the state transition on every file being parsed -------
            .add_loading_state(
                LoadingState::new(GameState::AssetLoading)
                    .load_collection::<BlockDefinitionAssets>()
                    .load_collection::<BlockTextureAssets>()
                    .load_collection::<ShapeSetAssets>()
                    .load_collection::<ItemDefinitionAssets>()
                    .load_collection::<SubstanceDefinitionAssets>()
                    .load_collection::<PartProfileAssets>()
                    .load_collection::<SpatialRecipeAssets>()
                    .load_collection::<ModelSourceAssets>()
                    .finally_init_resource::<ContentFilesReady>()
            );
    }
}
