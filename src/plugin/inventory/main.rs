use bevy::prelude::*;
use std::collections::HashMap;

use crate::plugin::inventory::player::{CursorInventory, PlayerHotbarSelection,
    dev_populate_player_inventory, spawn_player_inventory_sys, update_held_items_obs, update_hotbar_obs
};
use crate::plugin::inventory::item_registry::*;
use crate::plugin::state::UIState;
use crate::plugin::ui::inventory::{InventoryClickedEvent, InventoryUISpawnRequest};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app
            // Resources
            .insert_resource(PlayerHotbarSelection::new())

            // Startup Systems
            .add_systems(Startup, spawn_player_inventory_sys)

            // Update Systems

            // DEVELOPMENT SYSTEMS TO TEST THINGS
            .add_systems(Update, dev_populate_player_inventory.run_if(run_once))
            .add_systems(Update, dev_spawn_dummy_inventory.run_if(run_once))
            .add_systems(OnEnter(UIState::Inventory), dev_show_dummy_inventory_request_obs)

            // Event Observers
            .add_observer(update_hotbar_obs)
            .add_observer(update_held_items_obs)
            .add_observer(inventory_ui_click_obs)


        ;
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – ItemStack and Inventory Storage
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub const MAX_STACK: u16 = 99;

/// Represents a stack of items only by id and number. Needs to be used by
/// inventories as a lightweight way of keeping tabs on the number of items
/// and their location.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemStack {
    pub id:  ItemID,
    pub count: u16,
}

/// Returned by insert/extract to tell the caller what actually happened.
#[derive(Debug)]
pub struct TransferResult {
    /// How many items were actually moved.
    pub transferred: u16,
    /// How many were left over (couldn't fit / weren't available).
    pub remainder:   u16,
}

impl TransferResult {
    pub fn failed(count: u16) -> Self {
        TransferResult { transferred: 0, remainder: count }
    }
}

/// A fixed-size inventory.
///
/// **Dual-structure design:**
/// - `slots`  → ordered Vec for UI rendering and slot-specific manipulation
/// - `totals` → HashMap for O(1) "how many X do I have" queries used by
///              automation, inserters, filters, etc.
///
/// Both are kept in sync on every mutation — never touch one without the other.

#[derive(Component)]
pub struct Inventory {
    slots:     Vec<Option<ItemStack>>,
    totals:    HashMap<ItemID, u16>,
    capacity:  usize,
}

impl Inventory {
    pub fn new(capacity: usize) -> Self {
        Self {
            slots:    vec![None; capacity],
            totals:   HashMap::new(),
            capacity,
        }
    }

    // ── Read-only queries (hot path for automation) ──────────────────────

    #[inline]
    pub fn count(&self, item: ItemID) -> u16 {
        self.totals.get(&item).copied().unwrap_or(0)
    }

    #[inline]
    pub fn has_at_least(&self, item: ItemID, n: u16) -> bool {
        self.count(item) >= n
    }

    #[inline]
    pub fn is_full(&self) -> bool {
        self.slots.iter().all(|s| s.is_some())
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.totals.is_empty()
    }

    /// How many more of `item` could fit, respecting max_stack from the registry.
    pub fn free_capacity_for(&self, item: ItemID, registry: &ItemRegistry) -> u16 {
        let max_stack = registry.get(item).max_stack;
        let mut space = 0u16;
        for slot in &self.slots {
            match slot {
                None => space += max_stack,
                Some(s) if s.id == item => space += max_stack.saturating_sub(s.count),
                _ => {}
            }
        }
        space
    }

    // ── Mutations ────────────────────────────────────────────────────────

    /// Insert up to `count` of `item`. Returns how many were actually inserted.
    /// Prefers filling existing partial stacks before opening new slots.
    pub fn insert(
        &mut self,
        item:     ItemID,
        count:    u16,
        registry: &ItemRegistry,
    ) -> TransferResult {
        let max_stack = registry.get(item).max_stack;
        let mut remaining = count;

        // Pass 1: top-off existing stacks
        for slot in self.slots.iter_mut() {
            if remaining == 0 { break; }
            if let Some(s) = slot {
                if s.id == item && s.count < max_stack {
                    let space = max_stack - s.count;
                    let added = remaining.min(space);
                    s.count  += added;
                    remaining -= added;
                    *self.totals.entry(item).or_insert(0) += added;
                }
            }
        }

        // Pass 2: open new slots
        for slot in self.slots.iter_mut() {
            if remaining == 0 { break; }
            if slot.is_none() {
                let added = remaining.min(max_stack);
                *slot = Some(ItemStack { id: item, count: added });
                remaining -= added;
                *self.totals.entry(item).or_insert(0) += added;
            }
        }

        let transferred = count - remaining;
        TransferResult { transferred, remainder: remaining }
    }

    /// Extract up to `count` of `item`. Returns how many were actually taken.
    /// Drains from the last matching slot first (avoids sliding elements).
    pub fn extract(&mut self, item: ItemID, count: u16) -> TransferResult {
        let mut remaining = count;

        for slot in self.slots.iter_mut().rev() {
            if remaining == 0 { break; }
            if let Some(s) = slot {
                if s.id == item {
                    let taken = remaining.min(s.count);
                    s.count  -= taken;
                    remaining -= taken;

                    // Update totals map
                    let total = self.totals.get_mut(&item).unwrap();
                    *total -= taken;
                    if *total == 0 { self.totals.remove(&item); }

                    // Clear the slot if empty
                    if s.count == 0 { *slot = None; }
                }
            }
        }

        let transferred = count - remaining;
        TransferResult { transferred, remainder: remaining }
    }

    pub fn insert_at_slot(
        &mut self,
        item:     ItemID,
        count:    u16,
        slot:     usize,
        registry: &ItemRegistry,
    ) -> TransferResult {
        if count == 0 {
            return TransferResult { transferred: 0, remainder: 0 };
        }

        let max_stack = registry.get(item).max_stack;

        let added = match self.slots[slot].as_mut() {
            // Empty slot → place a fresh stack, capped at max_stack.
            None => {
                let added = count.min(max_stack);
                self.slots[slot] = Some(ItemStack { id: item, count: added });
                added
            }
            // Same item already present → top it off.
            Some(s) if s.id == item => {
                let space = max_stack.saturating_sub(s.count);
                let added = count.min(space);
                s.count += added;
                added
            }
            // Different item → cannot insert here.
            Some(_) => return TransferResult::failed(count),
        };

        if added > 0 {
            *self.totals.entry(item).or_insert(0) += added;
        }

        TransferResult {
            transferred: added,
            remainder:   count - added,
        }
    }

    pub fn extract_from_slot(
        &mut self,
        item:  ItemID,
        count: u16,
        slot:  usize,
    ) -> TransferResult {
        if count == 0 {
            return TransferResult { transferred: 0, remainder: 0 };
        }

        let Some(s) = self.slots[slot].as_mut() else {
            return TransferResult::failed(count);
        };

        if s.id != item {
            return TransferResult::failed(count);
        }

        let taken = count.min(s.count);
        s.count -= taken;

        // Update totals
        if let Some(total) = self.totals.get_mut(&item) {
            *total -= taken;
            if *total == 0 {
                self.totals.remove(&item);
            }
        }

        // Clear slot if drained
        if s.count == 0 {
            self.slots[slot] = None;
        }

        TransferResult {
            transferred: taken,
            remainder:   count - taken,
        }
    }

    // ── UI iteration ─────────────────────────────────────────────────────

    pub fn slots(&self) -> &[Option<ItemStack>] {
        &self.slots
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – Transfer Utilities
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Move up to `count` of `item` from one inventory to another.
/// Returns how many were actually transferred.
///
/// This is THE hot-path function for belts, inserters, pipes, etc.

pub fn transfer_items(
    from:     &mut Inventory,
    to:       &mut Inventory,
    item:     ItemID,
    count:    u16,
    registry: &ItemRegistry,
) -> TransferResult {
    // Fast-reject: source doesn't have enough, or destination is full
    let available = from.count(item);
    if available == 0 { return TransferResult::failed(count); }

    let wanted    = count.min(available);
    let insertable = to.free_capacity_for(item, registry);
    let to_move   = wanted.min(insertable);

    if to_move == 0 { return TransferResult::failed(count); }

    from.extract(item, to_move);
    to.insert(item, to_move, registry);
    TransferResult { transferred: to_move, remainder: insertable }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 4 – Inventory Events
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Fired whenever an Inventory's contents change. Lets UI diff and redraw.
#[derive(EntityEvent)]
pub struct InventoryChangedEvent {
    #[event_target]
    pub entity: Entity,
    pub index: usize,
}

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
        // Both empty: no op.
        (None, None, _) => {}

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


// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// DEV FUNCTIONS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub fn dev_spawn_dummy_inventory(
    mut commands: Commands,
    item_registry: Res<ItemRegistry>,
) {
    let mut new_inventory = Inventory::new(27);
    new_inventory.insert_at_slot(ItemID(1), 40, 0, &item_registry);
    new_inventory.insert_at_slot(ItemID(1), 40, 1, &item_registry);
    new_inventory.insert_at_slot(ItemID(1), 40, 2, &item_registry);

    bevy::log::info!("Added stuff to dummy inventory.");
    commands.spawn(
        (new_inventory,
        Name::new("Dummy"))
    );
}

pub fn dev_show_dummy_inventory_request_obs(
    mut commands: Commands,
    dummy_inventory_q: Query<(Entity, &Name)>,
) {
    for (entity, name) in dummy_inventory_q.iter() {
        if name.contains("Dummy") {
            commands.trigger(InventoryUISpawnRequest { source_entity: entity });
        }
    }
}