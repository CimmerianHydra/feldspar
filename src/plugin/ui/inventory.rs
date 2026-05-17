use bevy::prelude::*;

use crate::plugin::ui::main::*;
use crate::plugin::inventory::item_registry::ItemRegistry;
use crate::plugin::inventory::main::{Inventory, InventoryChangedEvent};

#[derive(Component)]
pub struct InventorySlot {
    pub index: usize,
}

/// Builder function that returns a bundle of all relevant components for a hotbar item slot.
fn build_inventory_ui_item_slot(
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
        InventorySlot { index },
    )
}

fn build_inventory_ui(
    rows: usize,
    cols: usize,
) -> impl Bundle {
    (Node {
            display: Display::Grid,
            border_radius: BorderRadius::all(UI_PANEL_RADIUS),
            border: UiRect::all(UI_BORDER_THICKN),
            padding: UiRect::all(UI_PANEL_PADDING),
            ..default()
        },
        BorderColor::all(UI_BORDER_COLOR),
        BackgroundColor(UI_PANEL_COLOR),
    )
}