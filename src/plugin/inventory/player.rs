use bevy::prelude::*;

use crate::plugin::inventory::main::*;
use crate::plugin::inventory::item_registry::*;
use crate::plugin::controller::main::MouseScrollEvent;
use crate::plugin::ui::hotbar::HotbarUISpawnRequest;
use crate::plugin::ui::inventory::InventoryClickedEvent;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Hotbar Data
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Updates the hotbar resource globally, which allows the system to sync both UI
/// and player held items.
pub fn update_hotbar_obs(
    event: On<MouseScrollEvent>,
    mut commands: Commands,
    mut hotbar_q: Query<(Entity, &mut PlayerHotbar, &Inventory)>,
) {
    let Ok((hotbar_entity, mut hotbar_data, inventory_data)) = hotbar_q.single_mut() else { return; };

    let capacity = inventory_data.capacity();
    let old_index = hotbar_data.highlighted_slot;
    let new_index = match *event.event() {
        MouseScrollEvent::ScrollDown => (old_index + 1) % capacity,
        MouseScrollEvent::ScrollUp => (old_index + capacity - 1) % capacity,
    };

    hotbar_data.highlighted_slot = new_index;

    commands.trigger(InventoryChangedEvent {
        entity: hotbar_entity,
        index: new_index,
    });
    commands.trigger(InventoryChangedEvent {
        entity: hotbar_entity,
        index: old_index,
    });
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Player Inventory
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Marks an entity as "the player's inventory".
/// Useful for distinguishing player from world containers in queries.
#[derive(Component)]
pub struct PlayerInventory;

/// Marks an entity as "the player's hotbar".
/// This hooks into the hotbar display and update system (TODO).
#[derive(Component)]
pub struct PlayerHotbar {
    pub highlighted_slot: usize,
}

/// Marker component. Used for the singular inventory slot, which allows users to 
/// pick item stacks with their cursor.
#[derive(Component)]
pub struct CursorInventory;

/// Simple spawning of the player's inventory. For now, we spawn it empty and then add
/// items to it using the populate_player_inventory function.
/// In the future, these entities will have to become children of a player so we can start
/// to distinguish players in a multiplayer world.
pub fn spawn_player_inventory_sys(
    mut commands: Commands,
) {
    commands.spawn((
        PlayerInventory,
        Inventory::new(27),
    ));

    let hotbar_entity = commands.spawn((
        PlayerHotbar { highlighted_slot: 0 },
        Inventory::new(9),
    )).observe(on_hotbar_changed).id();

    commands.spawn((
        CursorInventory,
        Inventory::new(1),
    ));

    commands.spawn((
        PlayerEquipment { right_hand: None },
    ));

    commands.trigger(HotbarUISpawnRequest {
        source_entity: hotbar_entity,
    });
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SYSTEMS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Hardcoded function to spawn some items into the player's inventory.
/// Since I hardcoded a few blocks in the block registry, I'll add them here.
pub fn dev_populate_player_inventory(
    mut commands: Commands,
    mut player_hotbar_query: Query<(Entity, &mut Inventory), With<PlayerHotbar>>,
    mut player_inventory_query: Query<(Entity, &mut Inventory), (With<PlayerInventory>, Without<PlayerHotbar>)>,
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

    if let Ok((entity, mut inventory)) = player_inventory_query.single_mut() {
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

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Player Held Item / Player Equipment (in the future)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Resource, Default)]
pub struct PlayerHeldItems{
    pub right_hand: Option<ItemStack>,
}

#[derive(Component)]
pub struct PlayerEquipment{
    pub right_hand: Option<ItemStack>,
}

pub fn on_hotbar_changed(
    event: On<InventoryChangedEvent>,
    mut held_items: ResMut<PlayerHeldItems>,
    hotbar_inventory_query: Query<(&PlayerHotbar, &Inventory)>,
    player_equipment: Query<&PlayerEquipment>,
) {
    let Ok((hotbar_data, inventory_data)) = hotbar_inventory_query.get(event.entity) else { return; };

    let currently_highlighted_slot = hotbar_data.highlighted_slot;
    held_items.right_hand = inventory_data.slots()[currently_highlighted_slot];
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Cursor interactions
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub fn inventory_ui_click_obs(
    event: On<InventoryClickedEvent>,
    mut commands: Commands,
    mut inventory_query: Query<&mut Inventory, Without<CursorInventory>>,
    mut cursor_query:    Query<(Entity, &mut Inventory), With<CursorInventory>>,
    item_registry: Res<ItemRegistry>,
) {
    let target_entity = event.entity;
    let slot_index    = event.slot_index;
    let button = event.button;

    let Ok((cursor_entity, mut cursor_inv)) = cursor_query.single_mut() else { return };
    let Ok(mut target_inv) = inventory_query.get_mut(target_entity) else { return };

    // Snapshot both relevant slots up-front. ItemStack is Copy, so these
    // are cheap snapshots, not borrows. We can freely mutate the inventories below.
    let cursor_stack = cursor_inv.slots()[0];
    let target_stack = target_inv.slots()[slot_index];

    // Did each side actually change? Used to decide what events to fire.
    let mut cursor_changed = false;
    let mut target_changed = false;

    match (cursor_stack, target_stack, button) {

        // Pick up: target -> cursor.
        (None, Some(t), b) => {
            let to_extract = if b == PointerButton::Primary {t.count} else {t.count.div_ceil(2)};
            let extracted = target_inv.extract_from_slot(t.id, to_extract, slot_index);
            if extracted.transferred > 0 {
                cursor_inv.insert_at_slot(t.id, extracted.transferred, 0, &item_registry);
                cursor_changed = true;
                target_changed = true;
            }
        }

        // Place down: cursor -> target.
        (Some(c), None, b) => {
            let to_place = if b == PointerButton::Primary {c.count} else {1 as u16};
            let inserted = target_inv.insert_at_slot(c.id, to_place, slot_index, &item_registry);
            if inserted.transferred > 0 {
                cursor_inv.extract_from_slot(c.id, inserted.transferred, 0);
                cursor_changed = true;
                target_changed = true;
            }
        }

        // Merge: top off the target, cursor keeps the remainder.
        (Some(c), Some(t), b) if c.id == t.id => {
            let to_merge = if b == PointerButton::Primary {c.count} else {1 as u16};
            let inserted = target_inv.insert_at_slot(c.id, to_merge, slot_index, &item_registry);
            if inserted.transferred > 0 {
                cursor_inv.extract_from_slot(c.id, inserted.transferred, 0);
                cursor_changed = true;
                target_changed = true;
            }
        }

        // Swap: different items in cursor and target, swap them.
        (Some(c), Some(t), PointerButton::Primary) => {
            let extracted_from_target = target_inv.extract_from_slot(t.id, t.count, slot_index);
            let extracted_from_cursor = cursor_inv.extract_from_slot(c.id, c.count, 0);

            
            if extracted_from_cursor.remainder > 0 || extracted_from_target.remainder > 0 {
                // If something is left in either slot, the swap failed, so undo everything.
                target_inv.insert_at_slot(t.id, extracted_from_target.transferred, slot_index, &item_registry);
                cursor_inv.insert_at_slot(c.id, extracted_from_cursor.transferred, 0, &item_registry);
            } else {
                // If there's no remainder, proceed to swap.
                target_inv.insert_at_slot(c.id, extracted_from_cursor.transferred, slot_index, &item_registry);
                cursor_inv.insert_at_slot(t.id, extracted_from_target.transferred, 0, &item_registry);

                cursor_changed = true;
                target_changed = true;
            }
        }

        // Any other case: no op.
        (_, _, _) => {}
    }

    if cursor_changed {
        commands.trigger(InventoryChangedEvent {
            entity: cursor_entity,
            index:  0,
        });
    }
    if target_changed {
        commands.trigger(InventoryChangedEvent {
            entity: target_entity,
            index:  slot_index,
        });
    }
}