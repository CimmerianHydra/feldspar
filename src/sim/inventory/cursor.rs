use bevy::prelude::*;

use crate::content::item::{ItemRegistry, ItemStack};
use crate::sim::inventory::spatial::{
    SpatialChange, SpatialInventory, SpatialInventoryChangedEvent, SpatialInventoryClickedEvent,
};
use crate::sim::inventory::storage::{Inventory, InventoryChangedEvent, InventoryClickedEvent};

/// Marker component. Used for the singular inventory slot, which allows users
/// to pick item stacks up with their cursor.
#[derive(Component)]
pub struct CursorInventory;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – SLOT-BASED EXCHANGES
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Consumes a resolved click and performs all cursor <-> inventory exchanges.
/// Never touches UI; the change events it fires drive the visual sync.
///
/// - (no cursor, no target)        → nothing
/// - (cursor,    no target)        → place at target slot (all / one)
/// - (no cursor, target)           → pick up (all / half)
/// - (cursor,    same item)        → merge cursor into target slot (all / one)
/// - (cursor,    different item)   → swap; cursor stack lands at target slot
pub fn inventory_click_obs(
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
            let to_extract = if b == PointerButton::Primary { t.count } else { t.count.div_ceil(2) };
            let extracted = target_inv.extract_from_slot(t.id, to_extract, slot_index);
            if extracted.transferred > 0 {
                cursor_inv.insert_at_slot(t.id, extracted.transferred, 0, &item_registry);
                cursor_changed = true;
                target_changed = true;
            }
        }

        // Place down: cursor -> target.
        (Some(c), None, b) => {
            let to_place = if b == PointerButton::Primary { c.count } else { 1u16 };
            let inserted = target_inv.insert_at_slot(c.id, to_place, slot_index, &item_registry);
            if inserted.transferred > 0 {
                cursor_inv.extract_from_slot(c.id, inserted.transferred, 0);
                cursor_changed = true;
                target_changed = true;
            }
        }

        // Merge: top off the target, cursor keeps the remainder.
        (Some(c), Some(t), b) if c.id == t.id => {
            let to_merge = if b == PointerButton::Primary { c.count } else { 1u16 };
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

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – SPATIAL EXCHANGES
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Twin of `inventory_click_obs` for freeform placement: consumes a resolved
/// click and performs all cursor <-> spatial-inventory exchanges. Never
/// touches UI; the change events it fires drive the visual sync.
///
/// - (no cursor, no placement)   → nothing
/// - (cursor,    no placement)   → place at click point (all / one)
/// - (no cursor, placement)      → pick up (all / half)
/// - (cursor,    same item)      → merge cursor into placement (all / one)
/// - (cursor,    different item) → swap; cursor stack lands at click point
pub fn spatial_inventory_click_obs(
    event: On<SpatialInventoryClickedEvent>,
    mut commands: Commands,
    mut spatial_q: Query<&mut SpatialInventory>,
    mut cursor_q:  Query<(Entity, &mut Inventory), With<CursorInventory>>,
    item_registry: Res<ItemRegistry>,
) {
    let Ok(mut spatial) = spatial_q.get_mut(event.entity) else { return };
    let Ok((cursor_entity, mut cursor_inv)) = cursor_q.single_mut() else { return };

    // The producer already filters out-of-bounds clicks; re-validate anyway —
    // the data side never trusts its inputs.
    if !spatial.contains(event.local_pos) { return; }

    let button = event.button;

    // Snapshots — ItemStack is Copy, so we can mutate freely below.
    let cursor_stack = cursor_inv.slots()[0];

    let mut cursor_changed = false;

    match (cursor_stack, event.id) {

        // ── Nothing to exchange ──────────────────────────────────────────
        (None, None) => return,

        // ── Place: cursor → empty space at the click point ───────────────
        (Some(c), None) => {
            let to_place = if button == PointerButton::Primary { c.count } else { 1 };

            let extracted = cursor_inv.extract_from_slot(c.id, to_place, 0);
            if extracted.transferred == 0 { return; }

            let Some(id) = spatial.place(
                event.local_pos,
                ItemStack { id: c.id, count: extracted.transferred },
            ) else { return /* unreachable: bounds already validated */ };

            cursor_changed = true;
            commands.trigger(SpatialInventoryChangedEvent {
                entity: event.entity,
                change: SpatialChange::Placed(id),
            });
        }

        // ── Pick up: placement → empty cursor ────────────────────────────
        (None, Some(id)) => {
            let Some(p) = spatial.get(id).map(|p| p.stack) else { return };

            let wanted = if button == PointerButton::Primary {
                p.count
            } else {
                p.count.div_ceil(2)
            };
            // The cursor slot is empty, so it can hold at most one full stack.
            let wanted = wanted.min(item_registry.get(p.id).max_stack);

            let extracted = spatial.extract_from_placement(p.id, wanted, id);
            if extracted.transferred > 0 {
                cursor_inv.insert_at_slot(p.id, extracted.transferred, 0, &item_registry);
                cursor_changed = true;

                let change = if spatial.get(id).is_some() {
                    SpatialChange::Modified(id) // partial pickup
                } else {
                    SpatialChange::Removed(id)  // fully drained
                };
                commands.trigger(SpatialInventoryChangedEvent {
                    entity: event.entity,
                    change,
                });
            }
        }

        // ── Merge or swap ────────────────────────────────────────────────
        (Some(c), Some(id)) => {
            let Some(p) = spatial.get(id).map(|p| p.stack) else { return };

            if c.id == p.id {
                // Merge: cursor → placement, capped at max_stack.
                let to_insert = if button == PointerButton::Primary { c.count } else { 1 };

                let inserted =
                    spatial.insert_into_placement(c.id, to_insert, id, &item_registry);

                if inserted.transferred > 0 {
                    cursor_inv.extract_from_slot(c.id, inserted.transferred, 0);
                    cursor_changed = true;
                    commands.trigger(SpatialInventoryChangedEvent {
                        entity: event.entity,
                        change: SpatialChange::Modified(id),
                    });
                }
            } else {
                // Swap. Ordered so nothing can be lost: `place` cannot fail
                // once bounds are validated and the cursor stack is
                // non-empty, and the cursor slot is guaranteed empty by the
                // time the old stack lands in it.
                let Some(old) = spatial.remove(id) else { return };
                cursor_inv.extract_from_slot(c.id, c.count, 0);
                let new_id = spatial.place(event.local_pos, c)
                    .expect("in-bounds place of a non-empty stack cannot fail");
                cursor_inv.insert_at_slot(old.id, old.count, 0, &item_registry);
                cursor_changed = true;

                commands.trigger(SpatialInventoryChangedEvent {
                    entity: event.entity,
                    change: SpatialChange::Removed(id),
                });
                commands.trigger(SpatialInventoryChangedEvent {
                    entity: event.entity,
                    change: SpatialChange::Placed(new_id),
                });
            }
        }
    }

    if cursor_changed {
        commands.trigger(InventoryChangedEvent { entity: cursor_entity, index: 0 });
    }
}
