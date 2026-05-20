use bevy::prelude::*;

use crate::plugin::ui::main::*;
use crate::plugin::inventory::main::{Inventory, InventoryChangedEvent};
use crate::plugin::ui::item::build_ui_item_display;
use crate::plugin::inventory::item_registry::ItemRegistry;
use crate::plugin::inventory::main::ItemStack;
use crate::plugin::state::UIState;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// BASIC SLOT BUILDER
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub fn build_base_ui_item_slot() -> impl Bundle
{
    (
        Node {
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
    )
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ITEM SLOTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Component)]
pub struct InventorySlot {
    pub source_entity: Entity, // The inventory entity associated with this slot
    pub slot_index: usize,
}

/// Builder function that returns a bundle of all relevant components for a hotbar item slot.
fn build_inventory_ui_item_slot(
    source_entity: Entity,
    slot_index: usize,
) -> impl Bundle {
    (
        build_base_ui_item_slot(),
        InventorySlot { source_entity, slot_index },
        Pickable { should_block_lower: true, is_hoverable: true },
    )
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// INVENTORY UI
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Event)]
pub struct InventoryUISpawnRequest {
    pub source_entity: Entity, // The inventory entity we would like to view
}

#[derive(Event)]
pub struct InventoryUISyncRequest {
    pub entity: Entity, // The inventory entity we would like to view
    pub index: usize,
}

pub fn build_inventory_ui(
    source_entity: Entity,
    capacity: usize,
    max_cols: usize,
) -> impl Bundle {
    let cols = capacity.min(max_cols) as u16;

    (Node {
            display: Display::Grid,
            align_content: AlignContent::FlexStart,
            grid_template_columns: RepeatedGridTrack::auto(cols),
            border_radius: BorderRadius::all(UI_PANEL_RADIUS),
            border: UiRect::all(UI_BORDER_THICKN),
            padding: UiRect::all(UI_PANEL_PADDING),
            ..default()
        },
        BorderColor::all(UI_BORDER_COLOR),
        BackgroundColor(UI_PANEL_COLOR),
        Pickable::IGNORE,

        // Once this bundle is spawned, this will automatically spawn as many children as needed, building the correct item slots.
        Children::spawn(
            SpawnIter(
                (0..capacity).into_iter().map(move |i| { build_inventory_ui_item_slot(source_entity, i) })
            )
        )
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
    pub button:         PointerButton,
}

pub fn inventory_ui_click_obs(
    mut click: On<Pointer<Click>>,
    mut commands: Commands,
    available_slots: Query<&InventorySlot>,
) {
    let clicked_entity = click.entity;
    let button: PointerButton = click.button;
    if let Ok(slot_data) = available_slots.get(clicked_entity) {
        let entity = slot_data.source_entity;
        let slot_index = slot_data.slot_index;
        commands.trigger(InventoryClickedEvent{ entity, slot_index, button });
        click.propagate(false);
    }
}


// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// REBUILDING UI FOR ALL INVENTORIES
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Redraw a single slot: wipe any existing item-display child, then respawn
/// one if the slot is non-empty. Single source of truth for "what does a slot
/// look like in the UI", used by both the full-populate pass and the
/// per-slot sync observer.
fn render_slot_contents(
    commands:      &mut Commands,
    slot_ui_entity: Entity,
    stack:          Option<ItemStack>,
    item_registry:  &ItemRegistry,
) {
    commands.entity(slot_ui_entity).despawn_children();

    if let Some(stack) = stack {
        let display_entity = commands
            .spawn(build_ui_item_display(
                &item_registry.get(stack.id).display,
                stack.count,
            ))
            .id();
        commands.entity(slot_ui_entity).add_child(display_entity);
    }
}

/// On-demand sync: when the event is received, redraw only the slot that was
/// affected. Slots whose `source_entity` doesn't match the event target are
/// skipped, so this is cheap even with many UIs (or none) in existence.
pub fn inventory_sync_obs(
    event: On<InventoryUISyncRequest>,
    mut commands: Commands,
    slots_q: Query<(Entity, &InventorySlot)>,
    inventory_q: Query<&Inventory>,
    item_registry: Res<ItemRegistry>,
) {
    // The inventory the event is about must still exist and be readable.
    let Ok(inventory) = inventory_q.get(event.entity) else { return };

    // The slot index must be in range. Bounds violation here means somebody
    // emitted a bogus event — bail rather than panic.
    let Some(&stack) = inventory.slots().get(event.index) else { return };

    // Redraw every slot UI that points at this (inventory, index).
    // Normally that's one entity; the loop also handles the edge case where
    // multiple UIs view the same inventory.
    for (slot_ui_entity, slot_data) in slots_q.iter() {
        if slot_data.source_entity != event.entity { continue; }
        if slot_data.slot_index    != event.index  { continue; }
        render_slot_contents(&mut commands, slot_ui_entity, stack, &item_registry);
    }
}

pub fn inventory_changed_to_ui_sync_obs(
    event: On<InventoryChangedEvent>,
    mut commands: Commands,
) {
    let entity = event.entity;
    let index = event.index;

    commands.trigger(InventoryUISyncRequest {
        entity, index
    });
}

pub fn show_requested_inventory_obs(
    view_requests: On<InventoryUISpawnRequest>,
    mut commands: Commands,
    inventory_q: Query<(Entity, &Inventory)>,
) {
    let requested_inventory = view_requests.source_entity;
    if let Ok((source_entity, inventory)) = inventory_q.get(requested_inventory) {

        let ui_bundle = build_inventory_ui(source_entity, inventory.capacity(), 9);

        let root_bundle = (
            Node {
                width: percent(100),
                height: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            DespawnOnExit(UIState::Inventory),
            ZIndex(100),
            Pickable::IGNORE,
            children![
                ui_bundle,
            ]
        );

        commands.spawn(root_bundle);

        // Send a sync request for all nonempty slots
        for (i, slot) in inventory.slots().iter().enumerate() {
            if slot.is_some() {
                commands.trigger(InventoryUISyncRequest {
                    entity: requested_inventory,
                    index:  i,
                });
            }
        }
    }
}

