use bevy::prelude::*;

mod plugin;
use plugin::controller::freecamera::{FreeCameraPlugin, FreeCamera};
use plugin::geometry::meshing::MeshingPlugin;
use plugin::block_interaction::BlockInteractionPlugin;
use plugin::chunk::ChunkPlugin;
use plugin::ui::main::UIPlugin;
use plugin::weather::WeatherPlugin;
use plugin::state::StatePlugin;
use plugin::controller::main::ControlsPlugin;
use plugin::inventory::main::InventoryPlugin;
use plugin::graphics::block_material::VoxelMaterialPlugin;
use plugin::worldgen::main::WorldgenPlugin;
use plugin::controller::player::PlayerControllerPlugin;
use plugin::audio::block::BlockAudioPlugin;
use plugin::crafting::main::CraftingPlugin;
use plugin::loader::main::AssetLoaderPlugin;

use bevy::{input::common_conditions::input_toggle_active};
use bevy_inspector_egui::{bevy_egui::EguiPlugin, quick::WorldInspectorPlugin};
use avian3d::PhysicsPlugins;
use plugin::state::GameState;


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
        .add_plugins(InventoryPlugin)
        .add_plugins(BlockInteractionPlugin)
        .add_plugins(WorldgenPlugin)
        .add_plugins(WeatherPlugin)
        .add_plugins(BlockAudioPlugin)
        .add_plugins(CraftingPlugin)
        
        .add_plugins(EguiPlugin::default())
        .add_plugins(
            WorldInspectorPlugin::default().run_if(input_toggle_active(false, KeyCode::F3)),
        )

        .add_systems(OnEnter(GameState::InGame), setup_dev_chunks)
        .add_systems(OnEnter(GameState::InGame), dev_lock_cursor)

        .add_systems(Update, enable_game.after(setup_dev_chunks))

        .add_systems(Update, dev_populate_player_inventory)
        .add_systems(Update, dev_populate_player_hotbar)

        .run();
}

use crate::plugin::inventory::main::*;
use crate::plugin::inventory::player::*;
use crate::plugin::loader::item_registry::*;
use crate::plugin::state::GameUpdate;
use crate::plugin::ui::cursor::CursorLockRequest;

fn enable_game(
    mut commands: Commands,
) {
    commands.set_state(GameUpdate::Enabled);
}

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
        for id in 1..7 {
            let item_id = ItemID(id as u16);
            let result = inventory.insert(item_id, 1, &item_registry);

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


use crate::plugin::chunk::{StaticChunk, NeedsRemeshing, VoxelChunk};
use crate::plugin::dimension::DimensionID;
use crate::plugin::loader::texture_assets::VoxelMaterialHandle;
use crate::plugin::worldgen::main::{WorldGenerator, ActiveWorldGenerator};

/// How many chunks out from origin to pre-spawn on each axis.
/// Total chunk count = (2*R + 1)^3 — with R=8 that's 4913 chunks.
/// Drop this to 2 or 3 if startup feels heavy while testing.
const DEV_CHUNK_RADIUS: i32 = 4;
const DEV_CHUNK_HEIGHT: i32 = 4;

fn setup_dev_chunks(
    mut commands:                   Commands,
    terrain_material_handle_res:    Res<VoxelMaterialHandle>,
    worldgen:                       Res<ActiveWorldGenerator>,
) {
    // ── chunk generation + spawn ──────────────────────────────────────────
    let dim_id = DimensionID::OVERWORLD;

    for cx in -DEV_CHUNK_RADIUS..=DEV_CHUNK_RADIUS {
        for cy in -DEV_CHUNK_HEIGHT..=DEV_CHUNK_HEIGHT {
            for cz in -DEV_CHUNK_RADIUS..=DEV_CHUNK_RADIUS {
                let chunk_pos = IVec3::new(cx, cy, cz);

                let mut chunk_data = VoxelChunk::empty();
                worldgen.generate_chunk(chunk_pos, &mut chunk_data);

                bevy::log::debug!("Generating static chunk at position ({}, {}, {})", cx, cy, cz);

                commands.spawn((
                    StaticChunk { dimension: dim_id, position: chunk_pos },
                    chunk_data.clone(),
                    MeshMaterial3d(terrain_material_handle_res.0.clone()),
                    NeedsRemeshing,
                ));
            }
        }
    }
}