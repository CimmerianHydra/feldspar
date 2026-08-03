//! Item definitions, the registry that holds them, and the JSON that fills
//! it.
//!
//! As with blocks, `ItemID` itself lives in [`crate::voxel::ids`] and is
//! re-exported here.


pub mod asset;
pub mod behaviors;
pub mod display;
pub mod registry;
pub use crate::voxel::ItemID;

pub use asset::{
    load_item_definitions_sys, parse_hex_color, resolve_display, DisplayJson, ItemDefinitionAsset,
    ItemDefinitionAssets,
};
pub use behaviors::{
    ItemBehavior, ItemBehaviorData, ItemBehaviorRegistry, ItemBehaviors,
    ItemFamily, ItemLoadContext, RegisterItemBehaviorExtension,
};
pub use display::{DisplayLayer, ItemDisplay};
pub use registry::{ItemDefinition, ItemKind, ItemRegistry};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// STACKS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Default per-slot stack ceiling. An item may declare its own; this is what
/// `max_stack` falls back to.
pub const MAX_STACK: u16 = 99;

/// A quantity of one item, and nothing else.
///
/// Lives with the item rather than with the inventory that usually holds one,
/// because a recipe's result is a stack too and `content::recipe` must not
/// reach up into `sim`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemStack {
    pub id:    ItemID,
    pub count: u16,
}

impl ItemStack {
    pub fn new(id: ItemID, count: u16) -> Self { Self { id, count } }
}
