use bevy::prelude::*;
use bevy::light::CascadeShadowConfigBuilder;

mod plugin;
use plugin::controller::freecamera::{FreeCameraPlugin, FreeCamera};
use plugin::geometry::meshing::MeshingPlugin;
use plugin::block_registry::{BlockRegistryPlugin, BlockDefinition, BlockID, BlockRegistry};
use plugin::block_interaction::BlockInteractionPlugin;
use plugin::chunk::ChunkPlugin;
use plugin::ui::main::UIPlugin;
use plugin::weather::WeatherPlugin;
use plugin::state::{StatePlugin, GameState};
use plugin::voxel::BlockShape;
use plugin::controller::main::ControlsPlugin;
use plugin::inventory::main::InventoryPlugin;
use plugin::inventory::item_registry::{ItemRegistryPlugin, populate_item_registry_sys};
use plugin::graphics::block_material::{VoxelMaterialPlugin, VoxelMaterial};
use plugin::graphics::block_textures::{BlockAppearance, FaceTextures};
use plugin::worldgen::main::WorldgenPlugin;
use plugin::controller::player::PlayerControllerPlugin;
use plugin::audio::block::BlockAudioPlugin;
use plugin::crafting::main::SpatialCraftingPlugin;
use plugin::loader::main::AssetLoaderPlugin;

use bevy::{input::common_conditions::input_toggle_active};
use bevy_inspector_egui::{bevy_egui::EguiPlugin, quick::WorldInspectorPlugin};
use avian3d::PhysicsPlugins;



fn main() {
    App::new()
        // Plugins
        .add_plugins(DefaultPlugins)
        .add_plugins(PhysicsPlugins::default())
        .add_plugins(StatePlugin)
        //.add_plugins(FreeCameraPlugin)
        .add_plugins(PlayerControllerPlugin)
        .add_plugins(ControlsPlugin)
        .add_plugins(VoxelMaterialPlugin)
        .add_plugins(AssetLoaderPlugin)
        .add_plugins(MeshingPlugin)
        .add_plugins(ChunkPlugin)
        .add_plugins(UIPlugin)
        .add_plugins(BlockRegistryPlugin)
        .add_plugins(ItemRegistryPlugin)
        .add_plugins(InventoryPlugin)
        .add_plugins(SpatialCraftingPlugin)
        .add_plugins(BlockInteractionPlugin)
        .add_plugins(WorldgenPlugin)
        .add_plugins(WeatherPlugin)
        .add_plugins(BlockAudioPlugin)

        .add_plugins(EguiPlugin::default())
        .add_plugins(
            WorldInspectorPlugin::default().run_if(input_toggle_active(false, KeyCode::F3)),
        )

        .add_systems(OnEnter(GameState::Running), crate::plugin::worldgen::main::setup_dev_chunks)
        .add_systems(OnEnter(GameState::Running), dev_populate_player_inventory)
        .add_systems(OnEnter(GameState::Running), dev_populate_player_hotbar)
        .add_systems(OnEnter(GameState::Running), dev_lock_cursor)

        .run();
}


use crate::plugin::inventory::main::*;
use crate::plugin::inventory::player::*;
use crate::plugin::inventory::item_registry::*;
use crate::plugin::ui::cursor::CursorLockRequest;

/// Hardcoded function to spawn some items into the player's inventory.
/// Since I hardcoded a few blocks in the block registry, I'll add them here.
pub fn dev_populate_player_inventory(
    mut commands: Commands,
    mut player_inventory_query: Query<(Entity, &mut Inventory), Added<PlayerInventory>>,
    item_registry: Res<ItemRegistry>,
) {
    for (entity, mut inventory) in player_inventory_query.iter_mut() {
        for slot in 0..3 {
            let item_id = ItemID(1);
            let result = inventory.insert_at_slot(item_id, 40, slot, &item_registry);

            bevy::log::info!("Added [{}]x{} to player inventory.", item_registry.get(item_id).name, result.transferred);
            commands.trigger(InventoryChangedEvent {
                entity,
                index: slot,
            });
        };
    }
}

/// Hardcoded function to spawn some items into the player's inventory.
/// Since I hardcoded a few blocks in the block registry, I'll add them here.
pub fn dev_populate_player_hotbar(
    mut commands: Commands,
    mut player_hotbar_query: Query<(Entity, &mut Inventory), Added<PlayerHotbar>>,
    item_registry: Res<ItemRegistry>,
) {
    if let Ok((entity, mut inventory)) = player_hotbar_query.single_mut() {
        for id in 1..5 {
            let item_id = ItemID(id as u16);
            let result = inventory.insert(item_id, 5, &item_registry);

            bevy::log::info!("Added [{}]x{} to player hotbar.", item_registry.get(item_id).name, result.transferred);
            commands.trigger(InventoryChangedEvent {
                entity,
                index: id - 1,
            });
        };
    }
}



/// Lock the cursor to the screen at the beginning of the game
pub fn dev_lock_cursor(
    mut commands: Commands,
) {
    commands.trigger(CursorLockRequest::Lock);
}