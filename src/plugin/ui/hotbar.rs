use bevy::prelude::*;

use crate::plugin::ui::inventory::*;
use crate::plugin::ui::main::*;
use crate::plugin::inventory::main::*;
use crate::plugin::inventory::player::*;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// BASIC DEFINITIONS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

const UI_HL_BORDER_COLOR: Color = Color::srgba_u8(250, 250, 250, 255);
const UI_HL_BORDER_THICKN: Val = Val::Px(4.0);


/// Component that tags the given UI slot as a hotbar slot, and which hotbar slot index it has.
/// This does not need to be the same as the slot it has for the player's inventory,
/// allowing us in the future to quickswap hotbars.

#[derive(Component)]
pub struct HotbarSlot {
    hotbar_slot_index: usize,
}

/// Builder function that returns a bundle of all relevant components for a hotbar item slot.
fn build_hotbar_item_slot(
    source_entity: Entity,
    slot_index: usize,
) -> impl Bundle {
    (
        build_base_ui_item_slot(),
        HotbarSlot { hotbar_slot_index: slot_index },
        InventorySlot { source_entity, slot_index },
    )
}

/// Builder function that generates the entire hotbar. Spawn this bundle as an entity to spawn a hotbar.
fn build_hotbar_ui_panel(
    source_entity: Entity,
    capacity: usize,
) -> impl Bundle {
    (Node {
            display: Display::Flex,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_direction: FlexDirection::Row,
            border_radius: BorderRadius::all(UI_PANEL_RADIUS),
            border: UiRect::all(UI_BORDER_THICKN),
            padding: UiRect::all(UI_PANEL_PADDING),
            margin: UiRect::bottom(Val::Px(20.)),
            ..default()
        },
        BorderColor::all(UI_BORDER_COLOR),
        BackgroundColor(UI_PANEL_COLOR),
        // Once this bundle is spawned, this will automatically spawn as many children as needed, building the correct item slots.
        Children::spawn(
            SpawnIter(
                (0..capacity).into_iter().map(move |i| { build_hotbar_item_slot(source_entity, i) })
            )
        )
    )
}


// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// HOTBAR UI EVENTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Event)]
pub struct HotbarUISpawnRequest {
    pub source_entity: Entity, // The hotbar entity we would like to view
}

pub fn show_requested_hotbar_obs(
    view_requests: On<HotbarUISpawnRequest>,
    mut commands: Commands,
    player_hotbar_query: Query<&Inventory, With<PlayerHotbar>>,
) {
    let hotbar_entity = view_requests.source_entity;

    let Ok(inventory_data) = player_hotbar_query.get(view_requests.source_entity) else {
        bevy::log::error!("Could not find hotbar!");
        return;
    };

    let game_ui_root = (Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::FlexEnd,
            flex_direction: FlexDirection::Column,
            ..default()
        },
        ZIndex(0),
        children![
            build_hotbar_ui_panel(hotbar_entity, inventory_data.capacity()),
        ],
    );

    commands.spawn(game_ui_root);

    // Send a sync request for all nonempty slots
    for (i, slot) in inventory_data.slots().iter().enumerate() {
        if slot.is_some() {
            commands.trigger(InventoryUISyncRequest {
                entity: hotbar_entity,
                index:  i,
            });
        }
    }
}

/// Poll every visible hotbar UI and just change the highlight every time there's an UI update request.
/// Maybe I'll make updating the highlight its own request at some point.
/// The inventory of the hotbar is always being updated by virtue of the fact that every UI element with InventorySlot
/// is kept in sync by plugin::ui::inventory::inventory_sync_obs.
pub fn sync_hotbar_highlight_sys(
    event: On<InventoryUISyncRequest>,
    player_hotbar_query: Query<&PlayerHotbar>,
    mut slot_query: Query<(&mut Node, &mut BorderColor, &HotbarSlot, &InventorySlot)>
) {
    let entity = event.entity;

    // The inventory the event is about must still exist and be readable.
    let Ok(hotbar_data) = player_hotbar_query.get(entity) else { return };

    for (
        mut node,
        mut border_color,
        hotbar_slot_data,
        slot_data
    ) in slot_query.iter_mut() {

        if !(slot_data.source_entity == entity) { continue; }

        if hotbar_slot_data.hotbar_slot_index == hotbar_data.highlighted_slot {
            *border_color = BorderColor::all(UI_HL_BORDER_COLOR);
            node.border = UiRect::all(UI_HL_BORDER_THICKN);
        } else {
            *border_color = BorderColor::all(UI_BORDER_COLOR);
            node.border = UiRect::all(UI_BORDER_THICKN);
        }
    }
}