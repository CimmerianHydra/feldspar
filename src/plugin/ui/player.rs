use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;

use crate::plugin::ui::inventory::*;
use crate::plugin::ui::cursor::*;
use crate::plugin::inventory::main::{Inventory, InventoryChangedEvent};
use crate::plugin::inventory::player::*;
use crate::plugin::controller::player::{OpenPlayerInventory, ClosePlayerInventory};


pub const MAX_PLAYER_INVENTORY_UI_COLS: usize = 9;

/// Builds a full UI for the player: a top panel (whatever machine/chest/etc the
/// caller wants to show), the player's main inventory, and the player's hotbar,
/// stacked vertically over a dimmed fullscreen background (the typical root).
fn build_player_ui_with_top_panel(
    hotbar_data:  (Entity, &Inventory),
    player_data:  (Entity, &Inventory),
    top_panel:    impl Bundle,
    top_entity:   Option<Entity>,
) -> impl Bundle {
    let (hotbar_entity, hotbar_inv) = hotbar_data;
    let (player_entity, player_inv) = player_data;
    let mut involved_entities = vec![player_entity, hotbar_entity];
    if let Some(ui_entity) = top_entity { involved_entities.push(ui_entity); };

    (
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_direction: FlexDirection::Column,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
        ZIndex(100),
        Pickable::IGNORE,
        EntityUISession {
            source_entities: involved_entities,
        },
        children![
            top_panel,
            build_inventory_ui(player_entity, player_inv.capacity(), MAX_PLAYER_INVENTORY_UI_COLS),
            build_inventory_ui(hotbar_entity, hotbar_inv.capacity(), MAX_PLAYER_INVENTORY_UI_COLS),
        ],
    )
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLAYER INVENTORY - redo for multiplayer in the future
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━


// Since actions are now children of the player entity, we can leverage that
// in the future to specifically grab the caller's PlayerInventory and PlayerHotbar.

pub fn open_inventory_player_input_action_obs(
    _action: On<Start<OpenPlayerInventory>>,
    mut commands: Commands,
    player_hotbar_query: Query<(Entity, &Inventory), (With<PlayerHotbar>, Without<PlayerInventory>)>,
    player_inventory_query: Query<(Entity, &Inventory), (With<PlayerInventory>, Without<PlayerHotbar>)>,
) {
    let Ok(hotbar_data) = player_hotbar_query.single() else { return; };
    let Ok(player_data) = player_inventory_query.single() else { return; };

    commands.spawn(
            build_player_ui_with_top_panel(
            hotbar_data,
            player_data,
            (),
            None),
        );
    commands.trigger(CursorLockRequest::Unlock);
}

pub fn close_inventory_player_input_action_obs(
    _action: On<Start<ClosePlayerInventory>>,
    mut commands: Commands,
    player_inventory_query: Query<Entity, With<PlayerInventory>>,
) {
    let Ok(player_inventory_entity) = player_inventory_query.single() else { return; };

    commands.trigger(EntityUISessionEndRequest {
        context: player_inventory_entity,
        source_entity: player_inventory_entity,
    });
    commands.trigger(CursorLockRequest::Lock);
}