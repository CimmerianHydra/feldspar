use bevy::prelude::*;
use std::collections::HashMap;

use crate::plugin::block_registry::{BlockRegistry, BlockID, initialize_registry_sys};

use crate::plugin::inventory::*;


// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – Item Registry
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ItemID(pub u16);

/// What kind of thing this item is.
#[derive(Clone, Debug)]
pub enum ItemKind {
    /// Can be placed into the world as a block.
    Block { block_id: BlockID },
    /// Pure resource — ore, wire, circuit board, etc.
    Resource,
    /// A tool with optional durability cap.
    Tool { max_durability: Option<u32> },
}

pub struct ItemDefinition {
    pub id:           ItemID,
    pub name:         String,
    pub display_name: String,
    pub max_stack:    u32,       // e.g. 100 for ore, 1 for unique tools
    pub kind:         ItemKind,
}

/// Mirror of BlockRegistry — same pattern so both feel uniform.
#[derive(Resource)]
pub struct ItemRegistry {
    items: Vec<ItemDefinition>,
    /// Fast reverse lookup: BlockID → the item that places it
    block_to_item: HashMap<BlockID, ItemID>,
}

impl ItemRegistry {
    pub fn new() -> Self {
        Self { items: Vec::new(), block_to_item: HashMap::new() }
    }

    pub fn get(&self, id: ItemID) -> &ItemDefinition {
        &self.items[id.0 as usize]
    }

    pub fn item_for_block(&self, block: BlockID) -> Option<ItemID> {
        self.block_to_item.get(&block).copied()
    }

    pub fn register(&mut self, def: ItemDefinition) -> ItemID {
        let id = ItemID(self.items.len() as u16);

        // If this item places a block, record the reverse link
        if let ItemKind::Block { block_id } = def.kind {
            self.block_to_item.insert(block_id, id);
        }

        self.items.push(ItemDefinition { id, ..def });
        id
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – ItemStack and Inventory Storage
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub const MAX_STACK: u32 = 999;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemStack {
    pub item:  ItemID,
    pub count: u32,
}

/// Returned by insert/extract to tell the caller what actually happened.
#[derive(Debug)]
pub struct TransferResult {
    /// How many items were actually moved.
    pub transferred: u32,
    /// How many were left over (couldn't fit / weren't available).
    pub remainder:   u32,
}

/// A fixed-size inventory.
///
/// **Dual-structure design:**
/// - `slots`  → ordered Vec for UI rendering and slot-specific manipulation
/// - `totals` → HashMap for O(1) "how many X do I have" queries used by
///              automation belts, inserters, filters, etc.
///
/// Both are kept in sync on every mutation — never touch one without the other.
#[derive(Component)]
pub struct Inventory {
    slots:     Vec<Option<ItemStack>>,
    totals:    HashMap<ItemID, u32>,
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
    pub fn count(&self, item: ItemID) -> u32 {
        self.totals.get(&item).copied().unwrap_or(0)
    }

    #[inline]
    pub fn has_at_least(&self, item: ItemID, n: u32) -> bool {
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
    pub fn free_capacity_for(&self, item: ItemID, registry: &ItemRegistry) -> u32 {
        let max_stack = registry.get(item).max_stack;
        let mut space = 0u32;
        for slot in &self.slots {
            match slot {
                None => space += max_stack,
                Some(s) if s.item == item => space += max_stack.saturating_sub(s.count),
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
        count:    u32,
        registry: &ItemRegistry,
    ) -> TransferResult {
        let max_stack = registry.get(item).max_stack;
        let mut remaining = count;

        // Pass 1: top-off existing stacks
        for slot in self.slots.iter_mut() {
            if remaining == 0 { break; }
            if let Some(s) = slot {
                if s.item == item && s.count < max_stack {
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
                *slot = Some(ItemStack { item, count: added });
                remaining -= added;
                *self.totals.entry(item).or_insert(0) += added;
            }
        }

        let transferred = count - remaining;
        TransferResult { transferred, remainder: remaining }
    }

    /// Extract up to `count` of `item`. Returns how many were actually taken.
    /// Drains from the last matching slot first (avoids sliding elements).
    pub fn extract(&mut self, item: ItemID, count: u32) -> TransferResult {
        let mut remaining = count;

        for slot in self.slots.iter_mut().rev() {
            if remaining == 0 { break; }
            if let Some(s) = slot {
                if s.item == item {
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
/// Zero allocations, no locking — call it in a regular Bevy system.
pub fn transfer_items(
    from:     &mut Inventory,
    to:       &mut Inventory,
    item:     ItemID,
    count:    u32,
    registry: &ItemRegistry,
) -> u32 {
    // Fast-reject: source doesn't have enough, or destination is full
    let available = from.count(item);
    if available == 0 { return 0; }

    let wanted    = count.min(available);
    let insertable = to.free_capacity_for(item, registry);
    let to_move   = wanted.min(insertable);

    if to_move == 0 { return 0; }

    from.extract(item, to_move);
    to.insert(item, to_move, registry);
    to_move
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 4 – Inventory Events
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Fired whenever an Inventory's contents change. Lets UI diff and redraw.
#[derive(Event)]
pub struct InventoryChangedEvent {
    pub entity: Entity,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 5 – Plugin
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app
            .insert_resource(ItemRegistry::new())
            
            .insert_resource(player_inventory::PlayerHotbar::new())

            .add_systems(Startup, initialize_item_registry_sys.after(initialize_registry_sys))

            .add_systems(PostStartup, player_inventory::spawn_player_inventory)

            .add_observer(player_inventory::update_hotbar_obs)
        ;
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 6 – Example Systems
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn initialize_item_registry_sys(
    block_registry: Res<BlockRegistry>,
    mut item_registry: ResMut<ItemRegistry>,
) {
    // First we register all the blocks as items.
    for id in 0..block_registry.size() {
        let block = block_registry.get(BlockID(id as u16));
        item_registry.register(
            ItemDefinition {
                id: ItemID(0 as u16),
                name: block.name.clone(),
                display_name: block.display_name.clone(),
                max_stack: MAX_STACK,
                kind: ItemKind::Block { block_id: BlockID(id as u16) },
            }
        );
    }
}