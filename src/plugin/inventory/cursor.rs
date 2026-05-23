use bevy::prelude::*;

use crate::plugin::inventory::main::*;

use crate::plugin::loader::item_registry::*;

use crate::plugin::ui::inventory::{InventoryClickedEvent};


/// Marker component. Used for the singular inventory slot, which allows users to 
/// pick item stacks with their cursor.
#[derive(Component)]
pub struct CursorInventory;

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