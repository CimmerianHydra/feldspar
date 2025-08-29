use bevy::prelude::*;
use serde::{Serialize, Deserialize};


/// ----------------- ITEM DEFINITION AND REGISTERING -----------------
/// 
/// This section is dedicated to all the logical definitions and components of Items.
/// 


// Item "kind". Items are fundamental game objects and represent a certain item's "immutable" properties.
/// The JSON-serialized asset (data-only).
#[derive(bevy::asset::Asset, bevy::reflect::TypePath, Deserialize, Clone, Debug)]
pub struct Item {
    pub id: ItemId,
    pub name: String,

    #[serde(default = "max_stack")]
    pub max_stack: u16,
    #[serde(default)]
    pub tags: Vec<String>,

    // Keep paths as strings (resolve to handles elsewhere)
    #[serde(default)]
    pub icon_path: Option<String>,
    #[serde(default)]
    pub model_path: Option<String>,
}

const fn max_stack() -> u16 { 64 }

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ItemId(pub u32);


/// Registry -> fast lookups by id or name
#[derive(Resource, Default)]
pub struct ItemRegistry {
    pub by_id: std::collections::BTreeMap<ItemId, Handle<Item>>,
    pub by_name: std::collections::BTreeMap<String, Handle<Item>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Component)]
pub struct ItemInstance {
    pub item: ItemId,
    #[serde(default = "one")] pub qty: u16,
}
const fn one() -> u16 { 1 }


impl ItemInstance {
    /// Start a fresh 1x instance from an `Item` asset, pre-filling defaults.
    pub fn new_from_item(item: &Item) -> Self {
        Self { item: item.id, qty: 1 }
    }

    /// Instances can stack only if both kind *and* state match exactly.
    #[inline]
    pub fn can_stack_with(&self, other: &Self) -> bool {
        self.item == other.item
    }

    /// Merge `other` into `self` up to `max_stack`. Returns how many of `other`
    /// could *not* be merged (0 means full merge).
    pub fn absorb_from(&mut self, other: &mut Self, max_stack: u16) -> u16 {
        // If they can't be stacked, return other

        if !self.can_stack_with(other) { return other.qty };
        let space = max_stack.saturating_sub(self.qty);
        let take = std::cmp::min(space, other.qty);
        self.qty += take;
        other.qty -= take;
        other.qty
    }
}

/// <===================== GAME MECHANICS COMPONENTS =====================> ///
/// 
/// These will influence how items play out in game and will be drawn in the UI.
/// 

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Durability { pub current: u32, pub max: u32 }

// #[derive(Component)]
// pub struct ItemDurability(pub f32);
// impl Default for ItemDurability {
//     fn default() -> Self {
//         Self(1.0)
//     }
// }

// #[derive(Component)]
// pub struct ItemStack(pub u32);
// impl Default for ItemStack {
//     fn default() -> Self {
//         Self(1)
//     }
// }


