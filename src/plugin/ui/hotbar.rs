use bevy::prelude::*;

use crate::plugin::ui::main::*;
use crate::plugin::ui::item::{build_ui_item_display};
use crate::plugin::inventory::item_registry::ItemRegistry;
use crate::plugin::inventory::main::{Inventory, InventoryChangedEvent};
use crate::plugin::inventory::player::*;




const UI_HL_BORDER_COLOR: Color = Color::srgba_u8(250, 250, 250, 255);
const UI_HL_BORDER_THICKN: Val = Val::Px(4.0);

#[derive(Component)]
pub struct HotbarSlot {
    index: usize,
}

/// Builder function that returns a bundle of all relevant components for a hotbar item slot.
fn build_hotbar_item_slot(
    index: usize,
) -> impl Bundle {
        (Node {
        width: SLOT_SIZE,
        height: SLOT_SIZE,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        flex_direction: FlexDirection::Column,
        border_radius: BorderRadius::all(UI_PANEL_RADIUS),
        border: UiRect::all(UI_BORDER_THICKN),
        margin: UiRect::all(SLOT_GAP),
        ..default()
        },
        BorderColor::all(UI_BORDER_COLOR),
        BackgroundColor(UI_SLOT_COLOR),
        HotbarSlot { index },
    )
}

/// Builder function that returns a bundle of all relevant components for a hotbar item slot with highlight.
fn build_hotbar_item_slot_highlighted(
    index: usize,
) -> impl Bundle {
        (Node {
        width: SLOT_SIZE,
        height: SLOT_SIZE,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        flex_direction: FlexDirection::Column,
        border_radius: BorderRadius::all(UI_PANEL_RADIUS),
        border: UiRect::all(UI_HL_BORDER_THICKN),
        margin: UiRect::all(SLOT_GAP),
        ..default()
        },
        BorderColor::all(UI_HL_BORDER_COLOR),
        BackgroundColor(UI_SLOT_COLOR),
        HotbarSlot { index },
    )
}

fn build_hotbar_ui() -> impl Bundle {
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
    )
}


pub fn spawn_hotbar_sys(
    mut commands: Commands,
) {
    let game_ui_root = (Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::FlexEnd,
            flex_direction: FlexDirection::Column,
            ..default()
        },
        ZIndex(0),
    );

    let hotbar_panel = build_hotbar_ui();

    commands.spawn(game_ui_root)
        .with_children(|parent| {
            parent.spawn(hotbar_panel)
                .with_children(|hotbar| {
                    // Spawn first hotbar slot already highlighted
                    let slot_node = build_hotbar_item_slot_highlighted(0);
                    hotbar.spawn(slot_node);

                    // Spawn the other hotbar slots not highlighted
                    for index in 1..HOTBAR_CAPACITY {
                        let slot_node = build_hotbar_item_slot(index);
                        hotbar.spawn(slot_node);
                    }
                });
        });
}

pub fn sync_hotbar_highlight_obs(
    event: On<PlayerHotbarSelectionChange>,
    mut query: Query<(&mut Node, &mut BorderColor, &HotbarSlot)>
) {
    let new_index = event.new_index;

    for (mut node, mut border_color, slot_data) in query.iter_mut() {
        if slot_data.index == new_index {
            *border_color = BorderColor::all(UI_HL_BORDER_COLOR);
            node.border = UiRect::all(UI_HL_BORDER_THICKN);
        } else {
            *border_color = BorderColor::all(UI_BORDER_COLOR);
            node.border = UiRect::all(UI_BORDER_THICKN);
        }
    }
}


/// Redraws the hotbar when the player inventory changes, if the affected slots are in the hotbar
/// This is a temporary function. We need to have a much better system in place for updating
/// inventory UIs later on.

pub fn sync_hotbar_item_display_obs(
    event: On<InventoryChangedEvent>,
    mut commands: Commands,
    hotbar_query: Query<(Entity, &HotbarSlot)>,
    player_inventory_query: Query<&Inventory, With<PlayerInventory>>,
    item_registry: Res<ItemRegistry>,
) {
    if let Ok(player_inventory) = player_inventory_query.get(event.entity) {
        let affected_index = event.index;

        // If the affected index is in the hotbar, we update the hotbar
        if affected_index < HOTBAR_CAPACITY {

            let slots = player_inventory.slots();

            // Update the hotbar slot by refreshing it entirely. It's fine for now
            for (slot_entity, slot_data) in hotbar_query.iter() {

                if slot_data.index == affected_index {
                    // Remove everything
                    commands.entity(slot_entity).despawn_children();

                    if let Some(stack) = slots[affected_index] {
                        // Add the image of the stack if there's something there
                        let slot_image = commands.spawn(build_ui_item_display(&item_registry.get(stack.id).display, stack.count)).id();
                        commands.entity(slot_entity).add_child(slot_image);
                    }
                }
            }
        }
    }
}