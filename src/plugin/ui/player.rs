use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;

use crate::plugin::ui::crafting::build_inventory_crafting_ui;
use crate::plugin::ui::inventory::*;

use crate::plugin::inventory::player::*;

use crate::plugin::controller::player::UiOpenPlayerInventory;
use crate::plugin::ui::screen::UiPushOptions;
use crate::plugin::ui::screen::UiScreenCommandsExt;


pub const MAX_PLAYER_INVENTORY_UI_COLS: usize = 9;

/// The standard player screen layout: caller's panel on top, player's main
/// inventory beneath it, hotbar at the bottom.
///
/// Returns `None` if the player has no `PlayerInventorySet` yet, or if one of
/// its handles points at a despawned entity — callers just `else { return }`.
///
/// Pass `()` as `top_panel` for a bare inventory screen.

pub fn build_player_ui_with_top_panel(
    player: Entity,
    player_inv_access: &PlayerInventoryAccess<'_, '_>,
    top_panel: impl Bundle,
) -> Option<impl Bundle> {
    // Both borrows end here — only the capacities and entity ids escape,
    // so the returned bundle owns everything it needs.
    let set = player_inv_access.get_from_player(player)?;

    let (main_entity, main_capacity) = {
        let inv = player_inv_access.main(player)?;
        (set.main, inv.capacity())
    };
    
    let (hotbar_entity, hotbar_capacity) = {
        let inv = player_inv_access.hotbar(player)?;
        (set.hotbar, inv.capacity())
    };

    Some((
        Node {
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_direction: FlexDirection::Column,
            ..default()
        },
        Pickable::IGNORE,
        children![
            top_panel,
            build_inventory_ui(main_entity,   main_capacity,   MAX_PLAYER_INVENTORY_UI_COLS),
            build_inventory_ui(hotbar_entity, hotbar_capacity, MAX_PLAYER_INVENTORY_UI_COLS),
        ],
    ))
}



// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLAYER INVENTORY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

// For now, by default, the player's "open inventory" action simply opens the inventory
// with the spatial crafting interface.

pub fn open_player_inventory_obs(
    event: On<Start<UiOpenPlayerInventory>>,
    mut commands: Commands,
    player_inv_access: PlayerInventoryAccess,
) {
    let player = event.context;

    let Some(set)          = player_inv_access.get_from_player(player).copied() else { return };
    let Some(spatial)      = player_inv_access.crafting_grid(player) else { return };

    let main_entity = set.main;
    let hotbar_entity = set.hotbar;
    let spatial_entity = set.crafting_grid;

    let involved_entities = vec![main_entity, hotbar_entity, spatial_entity];

    let top_panel = build_inventory_crafting_ui(set.crafting_machine, spatial_entity, spatial);

    let ui_options = UiPushOptions {
        dim: true,
        sources: involved_entities,
    };

    let Some(panel) = build_player_ui_with_top_panel(
            player,
            &player_inv_access,
            top_panel,
        ) else { error!("There was a problem opening the player's inventory."); return; };

    commands.push_ui_screen(
        player,
        ui_options,
        panel
    );
}