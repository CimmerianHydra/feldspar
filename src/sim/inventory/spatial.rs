use bevy::prelude::*;
use std::collections::HashMap;

use crate::content::item::{ItemID, ItemRegistry, ItemStack};
use crate::content::recipe::PlacedItem;
use crate::sim::inventory::stack::TransferResult;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 — PLACEMENT & SPATIALINVENTORY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A single item drop in the crafting area: a position (local panel
/// coordinates in pixels, top-left origin) and the stack sitting there.
#[derive(Clone, Copy, Debug)]
pub struct Placement {
    pub pos:   Vec2,
    pub stack: ItemStack,
}

/// Stable identifier for a placement. Issued from a monotonically
/// increasing counter and NEVER reused within an inventory's lifetime,
/// so a stale ID can only miss — it can never alias to a different stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlacementID(pub u64);

/// A 2D, freeform-positioned inventory: items live at (x, y) inside a
/// rectangular area, not in numbered slots.
///
/// **Dual-structure design**, matching slot-based `Inventory`:
/// - `placements` → ID-addressed map for UI tracking and shape recognition
/// - `totals`     → HashMap for O(1) "do I have enough of X" queries used
///                  by the recipe matcher
///
/// Both are kept in sync on every mutation — never touch one without the other.
#[derive(Component)]
pub struct SpatialInventory {
    placements: HashMap<PlacementID, Placement>,
    totals:     HashMap<ItemID, u16>,
    next_id:    u64,
    /// Size of the area in local panel pixels. Positions outside
    /// `[0, width] x [0, height]` are rejected by `place`.
    width:  f32,
    height: f32,
}

impl SpatialInventory {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            placements: HashMap::new(),
            totals:     HashMap::new(),
            next_id:    0,
            width, height,
        }
    }

    // ── Read queries ─────────────────────────────────────────────────────

    #[inline]
    pub fn count(&self, item: ItemID) -> u16 {
        self.totals.get(&item).copied().unwrap_or(0)
    }

    #[inline] pub fn width(&self)  -> f32 { self.width }
    #[inline] pub fn height(&self) -> f32 { self.height }

    pub fn contains(&self, pos: Vec2) -> bool {
        (0.0 <= pos.x && pos.x <= self.width)
        &&
        (0.0 <= pos.y && pos.y <= self.height)
    }

    pub fn get(&self, id: PlacementID) -> Option<&Placement> {
        self.placements.get(&id)
    }

    /// Iterate placements with their IDs. Order is unspecified — anything
    /// order-sensitive must not depend on it.
    pub fn iter(&self) -> impl Iterator<Item = (PlacementID, &Placement)> {
        self.placements.iter().map(|(id, p)| (*id, p))
    }

    pub fn is_empty(&self) -> bool { self.placements.is_empty() }

    /// The contents in the flat form the recipe matcher wants.
    ///
    /// This is the seam that keeps `content::recipe` free of any knowledge
    /// about spatial inventories: it takes `PlacedItem<T>` for an arbitrary
    /// handle type, and gets `PlacementID` back out in `consume`.
    pub fn layout(&self) -> Vec<PlacedItem<PlacementID>> {
        self.placements
            .iter()
            .map(|(id, p)| PlacedItem { pos: p.pos, stack: p.stack, handle: *id })
            .collect()
    }

    /// Geometric hit-test: which placement (if any) does a click at `pos`
    /// land on, treating each as a square of `half_extent` half-size
    /// centred on its position? Overlaps resolve to the nearest centre.
    ///
    /// The half-extent is a parameter (not a stored field) so the data side
    /// stays agnostic of how big the UI happens to draw the icons.
    pub fn hit_test(&self, pos: Vec2, half_extent: f32) -> Option<PlacementID> {
        self.placements
            .iter()
            .filter(|(_, p)| {
                let d = (p.pos - pos).abs();
                d.x <= half_extent && d.y <= half_extent
            })
            .min_by(|(_, a), (_, b)| {
                a.pos.distance_squared(pos)
                    .partial_cmp(&b.pos.distance_squared(pos))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(id, _)| *id)
    }

    // ── Mutations ────────────────────────────────────────────────────────

    /// Place an entire `ItemStack` at `pos`. Returns the new placement's
    /// ID, or `None` if the position is out of bounds or the count is 0.
    pub fn place(&mut self, pos: Vec2, stack: ItemStack) -> Option<PlacementID> {
        if !self.contains(pos) || stack.count == 0 { return None; }

        let id = PlacementID(self.next_id);
        self.next_id += 1;

        self.placements.insert(id, Placement { pos, stack });
        *self.totals.entry(stack.id).or_insert(0) += stack.count;
        Some(id)
    }

    /// Remove the placement with this ID, returning the stack that was there.
    pub fn remove(&mut self, id: PlacementID) -> Option<ItemStack> {
        let placement = self.placements.remove(&id)?;

        if let Some(total) = self.totals.get_mut(&placement.stack.id) {
            *total = total.saturating_sub(placement.stack.count);
            if *total == 0 { self.totals.remove(&placement.stack.id); }
        }

        Some(placement.stack)
    }

    /// Top off the stack at `id` with up to `count` more of `item`, capped
    /// at the item's max stack size. Fails if the placement doesn't exist
    /// or holds a different item. Mirrors `Inventory::insert_at_slot`.
    pub fn insert_into_placement(
        &mut self,
        item:     ItemID,
        count:    u16,
        id:       PlacementID,
        registry: &ItemRegistry,
    ) -> TransferResult {
        if count == 0 {
            return TransferResult { transferred: 0, remainder: 0 };
        }

        let Some(placement) = self.placements.get_mut(&id) else {
            return TransferResult::failed(count);
        };
        if placement.stack.id != item {
            return TransferResult::failed(count);
        }

        let max_stack = registry.get(item).max_stack;
        let space = max_stack.saturating_sub(placement.stack.count);
        let added = count.min(space);
        placement.stack.count += added;

        if added > 0 {
            *self.totals.entry(item).or_insert(0) += added;
        }

        TransferResult { transferred: added, remainder: count - added }
    }

    /// Extract up to `count` of `item` from the placement at `id`. A fully
    /// drained placement is removed from the map — its ID is retired, not
    /// recycled. Mirrors `Inventory::extract_from_slot`.
    ///
    /// The caller decides which change event to fire: check
    /// `get(id).is_some()` afterwards — `Modified` if it survived,
    /// `Removed` if it drained.
    pub fn extract_from_placement(
        &mut self,
        item:  ItemID,
        count: u16,
        id:    PlacementID,
    ) -> TransferResult {
        if count == 0 {
            return TransferResult { transferred: 0, remainder: 0 };
        }

        let Some(placement) = self.placements.get_mut(&id) else {
            return TransferResult::failed(count);
        };
        if placement.stack.id != item {
            return TransferResult::failed(count);
        }

        let taken = count.min(placement.stack.count);
        placement.stack.count -= taken;
        let drained = placement.stack.count == 0;

        if let Some(total) = self.totals.get_mut(&item) {
            *total = total.saturating_sub(taken);
            if *total == 0 { self.totals.remove(&item); }
        }

        if drained {
            self.placements.remove(&id);
        }

        TransferResult { transferred: taken, remainder: count - taken }
    }

    /// Strip every placement. `next_id` is deliberately NOT reset — retired
    /// IDs stay retired. Caller is responsible for firing
    /// `SpatialInventoryChangedEvent { change: SpatialChange::Cleared, .. }`.
    pub fn clear(&mut self) {
        self.placements.clear();
        self.totals.clear();
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 — EVENTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Fired whenever a `SpatialInventory` changes. The `change` variant lets
/// observers do incremental UI updates instead of rebuilding the panel.
#[derive(EntityEvent)]
pub struct SpatialInventoryChangedEvent {
    #[event_target]
    pub entity: Entity,
    pub change: SpatialChange,
}

#[derive(Clone, Copy, Debug)]
pub enum SpatialChange {
    Placed(PlacementID),
    Removed(PlacementID),
    /// The stack at this placement changed count but kept its position.
    Modified(PlacementID),
}

/// A click on a spatial inventory, already resolved into area-local
/// coordinates and hit-tested against the data.
///
/// The UI owns the pointer geometry; everything downstream of this event is
/// simulation, which is why the event lives here and not in `ui`.
#[derive(EntityEvent)]
pub struct SpatialInventoryClickedEvent {
    #[event_target]
    pub entity:     Entity,
    pub local_pos:  Vec2,
    /// The placement that was clicked, if any.
    pub id:         Option<PlacementID>,
    pub button:     PointerButton,
}
