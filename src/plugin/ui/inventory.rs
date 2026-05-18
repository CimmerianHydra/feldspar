use bevy::prelude::*;

use crate::plugin::ui::main::*;
use crate::plugin::inventory::item_registry::ItemRegistry;
use crate::plugin::inventory::main::{Inventory, InventoryChangedEvent};


// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ITEM SLOTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Component)]
pub struct InventorySlot {
    pub entity: Entity, // The inventory entity associated with this slot
    pub slot_index: usize,
}

#[derive(Component)]
pub struct PickableSlot;


/// Builder function that returns a bundle of all relevant components for a hotbar item slot.
fn build_inventory_ui_item_slot(
    entity: Entity,
    slot_index: usize,
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
        InventorySlot { entity, slot_index },
        Pickable { should_block_lower: true, is_hoverable: true },
    )
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// INVENTORY UI
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Component)]
pub struct VisibleInventoryUI;

fn build_inventory_ui(
    rows: u16,
) -> impl Bundle {
    (Node {
            display: Display::Grid,
            grid_template_rows: RepeatedGridTrack::auto(rows),
            border_radius: BorderRadius::all(UI_PANEL_RADIUS),
            border: UiRect::all(UI_BORDER_THICKN),
            padding: UiRect::all(UI_PANEL_PADDING),
            ..default()
        },
        BorderColor::all(UI_BORDER_COLOR),
        BackgroundColor(UI_PANEL_COLOR),
        Pickable::IGNORE,
    )
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// CLICK EVENTS FOR PICKABLE SLOTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(EntityEvent)]
pub struct InventoryClickedEvent {
    #[event_target]
    pub entity:         Entity,
    pub slot_index:     usize,
}

pub fn inventory_ui_click_obs(
    mut click: On<Pointer<Click>>,
    mut commands: Commands,
    pickable_slots: Query<&InventorySlot, With<PickableSlot>>,
) {
    let clicked_entity = click.entity;
    if let Ok(slot_data) = pickable_slots.get(clicked_entity) {
        let entity = slot_data.entity;
        let slot_index = slot_data.slot_index;
        commands.trigger(InventoryClickedEvent{ entity, slot_index });
    }
    click.propagate(false);
}

