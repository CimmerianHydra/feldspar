use bevy::prelude::*;
use std::collections::HashMap;

use crate::plugin::inventory::item_registry::ItemID;
use crate::plugin::inventory::main::ItemStack;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct SpatialCraftingPlugin;

impl Plugin for SpatialCraftingPlugin {
    fn build(&self, app: &mut App) {
        app
            // UI observers live in the ui::crafting module and are added there
            // (parallel to how ui::inventory hosts the click observer).
            // Once the recognizer/matcher are finished, they'll be added here.
        ;
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 — Placement & SpatialInventory
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A single item drop in the crafting area: a position (local panel
/// coordinates in pixels, top-left origin) and the stack sitting there.
#[derive(Clone, Copy, Debug)]
pub struct Placement {
    pub pos:   Vec2,
    pub stack: ItemStack,
}

/// Stable identifier for a placement. Backed by the index into
/// `placements`; sparse slot reuse keeps IDs small without invalidating
/// existing IDs when other placements are removed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlacementID(pub usize);

/// A 2D, freeform-positioned inventory: items live at (x, y) inside a
/// rectangular crafting area, not in numbered slots.
///
/// **Dual-structure design**, matching slot-based `Inventory`:
/// - `placements` → indexed list for UI tracking and shape recognition
/// - `totals`     → HashMap for O(1) "do I have enough of X" queries used
///                  by the recipe matcher
///
/// Both are kept in sync on every mutation — never touch one without the other.
#[derive(Component)]
pub struct SpatialInventory {
    placements: Vec<Option<Placement>>,
    totals:     HashMap<ItemID, u16>,
    /// Size of the crafting area in local panel pixels. Positions outside
    /// `[0, width] x [0, height]` are rejected by `place`.
    width:  f32,
    height: f32,
}

impl SpatialInventory {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            placements: Vec::new(),
            totals:     HashMap::new(),
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
        pos.x >= 0.0 && pos.x <= self.width
            && pos.y >= 0.0 && pos.y <= self.height
    }

    pub fn get(&self, id: PlacementID) -> Option<&Placement> {
        self.placements.get(id.0).and_then(|p| p.as_ref())
    }

    /// Iterate non-empty placements with their IDs.
    /// This is what the shape recognizer will consume.
    pub fn iter(&self) -> impl Iterator<Item = (PlacementID, &Placement)> {
        self.placements.iter().enumerate().filter_map(|(i, p)| {
            p.as_ref().map(|p| (PlacementID(i), p))
        })
    }

    pub fn is_empty(&self) -> bool { self.totals.is_empty() }

    // ── Mutations ────────────────────────────────────────────────────────

    /// Place an entire `ItemStack` at `pos`. Returns the new placement's
    /// stable ID, or `None` if the position is out of bounds or count is 0.
    ///
    /// No merging with nearby placements at this stage — the recognizer
    /// will handle "close enough to be one point" if/when we want that.
    pub fn place(&mut self, pos: Vec2, stack: ItemStack) -> Option<PlacementID> {
        if !self.contains(pos) || stack.count == 0 { return None; }

        let p = Placement { pos, stack };

        // Reuse the first vacant slot to keep IDs dense.
        let id = if let Some(idx) = self.placements.iter().position(|s| s.is_none()) {
            self.placements[idx] = Some(p);
            PlacementID(idx)
        } else {
            self.placements.push(Some(p));
            PlacementID(self.placements.len() - 1)
        };

        *self.totals.entry(stack.id).or_insert(0) += stack.count;
        Some(id)
    }

    /// Remove the placement with this ID, returning the stack that was there.
    pub fn remove(&mut self, id: PlacementID) -> Option<ItemStack> {
        let slot = self.placements.get_mut(id.0)?;
        let placement = slot.take()?;

        if let Some(total) = self.totals.get_mut(&placement.stack.id) {
            *total = total.saturating_sub(placement.stack.count);
            if *total == 0 { self.totals.remove(&placement.stack.id); }
        }

        Some(placement.stack)
    }

    /// Strip every placement. Caller is responsible for firing
    /// `SpatialInventoryChangedEvent { change: SpatialChange::Cleared, .. }`.
    pub fn clear(&mut self) {
        self.placements.clear();
        self.totals.clear();
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 — Events
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Fired whenever a `SpatialInventory` changes. The `change` variant lets
/// observers do incremental UI updates (single spawn/despawn) instead of
/// rebuilding the whole panel.
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
    Cleared,
}