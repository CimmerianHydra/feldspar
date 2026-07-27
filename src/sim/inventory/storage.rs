use bevy::prelude::*;
use std::collections::HashMap;

use crate::content::item::{ItemID, ItemRegistry, ItemStack};
use crate::sim::inventory::stack::TransferResult;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – THE SLOT-BASED INVENTORY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

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
// SECTION 2 – EVENTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Fired whenever an Inventory's contents change. Lets UI diff and redraw.
#[derive(EntityEvent)]
pub struct InventoryChangedEvent {
    #[event_target]
    pub entity: Entity,
    pub index: usize,
}

/// A resolved click on a slot.
///
/// Defined on the data side, not in `ui`, so the exchange logic in
/// [`super::cursor`] never has to import a widget. The UI produces one of
/// these; what happens next is simulation.
#[derive(EntityEvent)]
pub struct InventoryClickedEvent {
    #[event_target]
    pub entity:     Entity,
    pub slot_index: usize,
    pub button:     PointerButton,
}
