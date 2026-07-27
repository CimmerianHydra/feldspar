use bevy::prelude::*;

use crate::content::item::{ItemID, ItemRegistry, ItemStack};
use crate::sim::inventory::storage::{Inventory, InventoryChangedEvent};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – TRANSFER RESULTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

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

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – TRANSFER UTILITIES
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Move up to `count` of `item` from one inventory to another.
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

    let wanted     = count.min(available);
    let insertable = to.free_capacity_for(item, registry);
    let to_move    = wanted.min(insertable);

    if to_move == 0 { return TransferResult::failed(count); }

    from.extract(item, to_move);
    to.insert(item, to_move, registry);
    TransferResult { transferred: to_move, remainder: insertable }
}

/// `insert` reports totals; the UI syncs per (inventory, slot). So: snapshot,
/// insert, diff, and fire one `InventoryChangedEvent` per slot that actually
/// moved.
///
/// Anything that mutates an inventory *after* the UI exists has to do this, or
/// the hotbar and any open screen silently go stale. The startup dev fixture got
/// away without it only because it ran before a single slot node was spawned.
///
/// The snapshot is a memcpy of `capacity` Copy-sized slots — cheap enough for
/// commands and one-off interactions. The real fix is for `insert`/`extract` to
/// report their touched slots directly, which belts and inserters will want too;
/// this is the stopgap that keeps `Inventory` untouched for now.
pub fn insert_and_notify(
    commands:  &mut Commands,
    entity:    Entity,
    inventory: &mut Inventory,
    item:      ItemID,
    count:     u16,
    registry:  &ItemRegistry,
) -> TransferResult {
    let before: Vec<Option<ItemStack>> = inventory.slots().to_vec();
    let result = inventory.insert(item, count, registry);

    for (index, (old, new)) in before.iter().zip(inventory.slots().iter()).enumerate() {
        if old != new {
            commands.trigger(InventoryChangedEvent { entity, index });
        }
    }

    result
}
