use bevy::prelude::*;
use std::collections::HashMap;

use crate::content::item::{ItemID, ItemRegistry, ItemStack};
use crate::sim::inventory::stack::TransferResult;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – SLOT ROLES
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// What automated machinery is allowed to do to a slot.
///
/// Roles constrain *automation only*. A player rummaging in a furnace by
/// hand is not bound by them, which is why the manual API in section 3 does
/// not consult this and the automation API in section 4 is the only thing
/// that does.
///
/// The default is `Buffer` — deny — on purpose. If the default were
/// permissive, a machine that forgot to lock its working slots would let
/// players pull half-finished recipes out through a chute, and nobody would
/// notice for months. Denying by default fails the other way: "my chute
/// isn't working", discovered in ten seconds.
///
/// There is no `Fuel` role, and there shouldn't be: a fuel slot is an
/// `Input` with a filter on it. Roles say *whether*, filters will say
/// *what*, and keeping those separate stops this enum growing a variant per
/// machine.
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum SlotRole {
    /// A machine's private working space. No automated access at all.
    #[default]
    Buffer,
    /// Free automated access both ways. What a barrel is made of.
    Storage,
    /// Automation may insert but never extract. A machine's ingredients.
    Input,
    /// Automation may extract but never insert. A machine's products.
    Output,
}

impl SlotRole {
    #[inline]
    pub fn accepts_insert(self) -> bool {
        matches!(self, Self::Storage | Self::Input)
    }

    #[inline]
    pub fn accepts_extract(self) -> bool {
        matches!(self, Self::Storage | Self::Output)
    }
}

/// A single slot's worth of automated transfer: where, what, and how much.
///
/// Returned by the peek methods, which never mutate. The point of handing
/// back the slot index is that the caller can then commit to exactly that
/// slot and fire an `InventoryChangedEvent` with an exact index, rather
/// than telling the UI to redraw everything.
#[derive(Clone, Copy, Debug)]
pub struct SlotOffer {
    pub slot: usize,
    pub item: ItemID,
    /// Items available to take, or space available to fill.
    pub count: u16,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – THE SLOT-BASED INVENTORY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A fixed-size inventory.
///
/// **Parallel-structure design:**
/// - `slots`  → ordered Vec for UI rendering and slot-specific manipulation
/// - `roles`  → same length as `slots`, immutable in practice; what
///              automation may do to each one
/// - `totals` → HashMap for O(1) "how many X do I have" queries
///
/// `slots` and `totals` must never be touched independently. `roles` is set
/// at construction and read-only thereafter, so it cannot drift.
///
/// **`totals` counts every slot, whatever its role.** So [`Inventory::count`]
/// answers a storage question, not an automation question: a furnace holding
/// five iron in an `Input` slot honestly reports five iron, and an extractor
/// that believes it will find nothing to take. Automation goes through
/// section 4, always.
#[derive(Component)]
pub struct Inventory {
    slots:  Vec<Option<ItemStack>>,
    roles:  Box<[SlotRole]>,
    totals: HashMap<ItemID, u16>,
}

impl Inventory {
    /// The general constructor. Capacity comes from the layout, so the two
    /// can never disagree.
    ///
    /// A furnace is `from_roles([Input, Input, Output])`; every uniform
    /// constructor below is sugar over this.
    pub fn from_roles(roles: impl IntoIterator<Item = SlotRole>) -> Self {
        let roles: Box<[SlotRole]> = roles.into_iter().collect();
        Self {
            slots: vec![None; roles.len()],
            totals: HashMap::new(),
            roles,
        }
    }

    fn uniform(capacity: usize, role: SlotRole) -> Self {
        Self::from_roles(std::iter::repeat_n(role, capacity))
    }

    /// Closed to automation. The safe default for anything that hasn't
    /// thought about it yet.
    pub fn new(capacity: usize) -> Self {
        Self::uniform(capacity, SlotRole::Buffer)
    }

    pub fn storage(capacity: usize) -> Self {
        Self::uniform(capacity, SlotRole::Storage)
    }

    pub fn input(capacity: usize) -> Self {
        Self::uniform(capacity, SlotRole::Input)
    }

    pub fn output(capacity: usize) -> Self {
        Self::uniform(capacity, SlotRole::Output)
    }

    // ── Read-only queries (hot path for automation) ──────────────────────

    /// Total across **all** slots regardless of role. See the type docs.
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

    /// How many more of `item` could fit, respecting max_stack from the
    /// registry. Ignores roles — for the automation answer, ask
    /// [`Inventory::automation_insertable`], whose `count` is exactly the
    /// space in the one slot an inserter would actually use.
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

    // ── Layout ───────────────────────────────────────────────────────────

    #[inline]
    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    pub fn slots(&self) -> &[Option<ItemStack>] {
        &self.slots
    }

    pub fn roles(&self) -> &[SlotRole] {
        &self.roles
    }

    #[inline]
    pub fn role(&self, slot: usize) -> Option<SlotRole> {
        self.roles.get(slot).copied()
    }

    /// For machines that reconfigure themselves — a filter block changing
    /// which of its slots automation may reach, say. Not part of the normal
    /// path; layouts are otherwise fixed at construction.
    pub fn set_role(&mut self, slot: usize, role: SlotRole) {
        if let Some(existing) = self.roles.get_mut(slot) {
            *existing = role;
        }
    }

    // ━━━ SECTION 3 – MANUAL ACCESS ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    //
    // The player's path. Deliberately ignores roles: a human with the
    // screen open may do as they like. `cursor.rs` calls into here and
    // needs no changes.

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

                    // Update totals. `get_mut` rather than `unwrap` — if the
                    // two structures ever disagree, a wrong count is a much
                    // better outcome than a panic in the middle of a tick.
                    if let Some(total) = self.totals.get_mut(&item) {
                        *total = total.saturating_sub(taken);
                        if *total == 0 { self.totals.remove(&item); }
                    }

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
        if count == 0 || slot >= self.slots.len() {
            return TransferResult { transferred: 0, remainder: count };
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
        if count == 0 || slot >= self.slots.len() {
            return TransferResult { transferred: 0, remainder: count };
        }

        let Some(s) = self.slots[slot].as_mut() else {
            return TransferResult::failed(count);
        };

        if s.id != item {
            return TransferResult::failed(count);
        }

        let taken = count.min(s.count);
        s.count -= taken;

        if let Some(total) = self.totals.get_mut(&item) {
            *total = total.saturating_sub(taken);
            if *total == 0 {
                self.totals.remove(&item);
            }
        }

        if s.count == 0 {
            self.slots[slot] = None;
        }

        TransferResult {
            transferred: taken,
            remainder:   count - taken,
        }
    }

    // ━━━ SECTION 4 – AUTOMATED ACCESS ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    //
    // The only family that consults roles. Split into peek (`&self`, never
    // mutates) and commit (`&mut self`, one slot), so a transfer can work
    // out the exact amount from both ends before touching either.

    /// Every stack an extractor could take, in round-robin order starting
    /// at `cursor`.
    ///
    /// An iterator rather than a single answer because the first candidate
    /// may be something the destination refuses. Walking to the next one is
    /// what stops a chute jamming forever on a stack nobody wants — the
    /// oldest and most-complained-about hopper behaviour there is.
    pub fn automation_offers(&self, cursor: usize) -> impl Iterator<Item = SlotOffer> + '_ {
        let n = self.slots.len();

        (0..n).filter_map(move |i| {
            let slot = (cursor + i) % n;

            if !self.roles[slot].accepts_extract() {
                return None;
            }

            let stack = self.slots[slot]?;
            Some(SlotOffer { slot, item: stack.id, count: stack.count })
        })
    }

    /// Where an inserter would put `item`, and how much fits *there*.
    ///
    /// One slot, not a spread. Topping off a matching partial stack beats
    /// opening a fresh one, so automation cannot fragment a destination the
    /// way a careless bulk insert would.
    ///
    /// `None` means the destination is closed or full — which is exactly
    /// the "only move if there is space" test, with no separate query.
    pub fn automation_insertable(
        &self,
        item: ItemID,
        registry: &ItemRegistry,
    ) -> Option<SlotOffer> {
        let max_stack = registry.get(item).max_stack;
        let mut first_empty: Option<usize> = None;

        for (slot, (held, role)) in self.slots.iter().zip(self.roles.iter()).enumerate() {
            if !role.accepts_insert() {
                continue;
            }

            match held {
                Some(s) if s.id == item && s.count < max_stack => {
                    return Some(SlotOffer { slot, item, count: max_stack - s.count });
                }
                None if first_empty.is_none() => first_empty = Some(slot),
                _ => {}
            }
        }

        first_empty.map(|slot| SlotOffer { slot, item, count: max_stack })
    }

    /// Commit an insertion the caller has already sized with
    /// [`Inventory::automation_insertable`]. Re-checks the role, so this is
    /// safe to call on its own.
    pub fn automation_insert_at(
        &mut self,
        slot: usize,
        item: ItemID,
        count: u16,
        registry: &ItemRegistry,
    ) -> TransferResult {
        match self.role(slot) {
            Some(role) if role.accepts_insert() => {
                self.insert_at_slot(item, count, slot, registry)
            }
            _ => TransferResult::failed(count),
        }
    }

    /// Commit an extraction the caller has already sized with
    /// [`Inventory::automation_offers`].
    pub fn automation_extract_at(
        &mut self,
        slot: usize,
        item: ItemID,
        count: u16,
    ) -> TransferResult {
        match self.role(slot) {
            Some(role) if role.accepts_extract() => {
                self.extract_from_slot(item, count, slot)
            }
            _ => TransferResult::failed(count),
        }
    }

    /// Cheap "is there anything here for me?", for extractor sleep logic.
    pub fn has_extractable(&self) -> bool {
        self.automation_offers(0).next().is_some()
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 5 – AUTOMATED TRANSFER
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//
// Lives here rather than in `transport` because every automated device
// wants it — chutes now, pipe extractors and storage buses later. Move it
// to its own `sim::inventory::automation` module if this file gets long.

/// Move up to `batch` items from one inventory to another, obeying roles at
/// both ends.
///
/// Decides the amount from both sides *before* mutating either, so there is
/// no window in which items exist nowhere and no rollback path to get
/// wrong. Returns how many moved; zero means blocked, which is the caller's
/// cue to go back to sleep.
///
/// `cursor` is the caller's own round-robin position over the source's
/// slots. It belongs to the device, not the inventory: two chutes drawing
/// from one barrel should rotate independently.
pub fn transfer_automated(
    commands: &mut Commands,
    source: (Entity, &mut Inventory),
    sink: (Entity, &mut Inventory),
    batch: u16,
    cursor: &mut usize,
    registry: &ItemRegistry,
) -> u16 {
    let (source_entity, source) = source;
    let (sink_entity, sink) = sink;

    if batch == 0 {
        return 0;
    }

    // ── plan ─────────────────────────────────────────────────────────────
    //
    // Walk the source's offers until one the sink will accept. Collected
    // eagerly because the plan borrows `source` immutably and the commit
    // needs it mutably.
    let plan = source
        .automation_offers(*cursor)
        .find_map(|take| {
            let give = sink.automation_insertable(take.item, registry)?;
            let moved = batch.min(take.count).min(give.count);
            (moved > 0).then_some((take, give, moved))
        });

    let Some((take, give, moved)) = plan else { return 0 };

    // ── commit ───────────────────────────────────────────────────────────
    let taken = source.automation_extract_at(take.slot, take.item, moved);
    if taken.transferred == 0 {
        return 0;
    }

    let given = sink.automation_insert_at(give.slot, take.item, taken.transferred, registry);

    // Should be exact — `give.count` was measured against this very slot.
    // If it somehow isn't, put the difference back rather than lose it: the
    // source slot definitely has room, we just emptied part of it.
    let leftover = taken.transferred - given.transferred;
    if leftover > 0 {
        warn!(
            "automated transfer: {leftover} item(s) rejected after sizing, returning to source"
        );
        source.insert_at_slot(take.item, leftover, take.slot, registry);
    }

    // Advance past the slot we just used, so a source holding several item
    // types serves them all instead of draining the first one forever.
    *cursor = (take.slot + 1) % source.capacity().max(1);

    if given.transferred > 0 {
        commands.trigger(InventoryChangedEvent { entity: source_entity, index: take.slot });
        commands.trigger(InventoryChangedEvent { entity: sink_entity,   index: give.slot });
    }

    given.transferred
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 6 – EVENTS
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