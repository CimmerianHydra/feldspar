use bevy::prelude::*;
use bevy_common_assets::json::JsonAssetPlugin;
use bevy_asset_loader::prelude::*;

use crate::plugin::state::GameState;
use crate::plugin::inventory::item_registry::populate_item_registry_sys;
use crate::plugin::loader::texture_registry::*;

use crate::plugin::loader::block_assets::*;
use crate::plugin::loader::texture_assets::*;

pub struct AssetLoaderPlugin;

impl Plugin for AssetLoaderPlugin {
    fn build(&self, app: &mut App) {
        app

        .insert_resource(TextureRegistry::default())
        .insert_resource(VoxelMaterialHandle::default())

        // Parser for *.json block definition files
        .add_plugins(JsonAssetPlugin::<BlockDefinitionAsset>::new(&["json"]))

        // Gate Loading → Running on every block file being parsed
        .add_loading_state(
            LoadingState::new(GameState::Loading)
                .continue_to_state(GameState::Running)
                .load_collection::<BlockDefinitionAssets>()
                .load_collection::<BlockTextureAssets>()
        )

        // Gate Loading → Running on every block file being parsed
        .add_systems(
            OnExit(GameState::Loading),
            (
                assemble_texture_arrays_sys,
                populate_block_registry_sys,
                populate_item_registry_sys,
            ).chain(),
        )
        ;
    }
}