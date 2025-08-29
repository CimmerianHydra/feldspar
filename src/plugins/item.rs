use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Serialize, Deserialize};


/// ----------------- ITEM DEFINITION AND REGISTERING -----------------
/// 
/// This section is dedicated to all the logical definitions and components of Items.
/// 


// Item "kind". Items are fundamental game objects and represent a certain item's "immutable" properties.
#[derive(Asset, Serialize, Deserialize, Clone, Reflect)]
pub struct Item {
    pub id: ItemId,
    pub name: String,
    pub icon_path: String,
    pub max_stack: u16,
    pub tags: Vec<String>,
}

#[derive(Reflect, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ItemId(pub u32);


#[derive(Resource, Default)]
pub struct ItemRegistry {
    pub items: std::collections::BTreeMap<ItemId, Handle<Item>>,
    pub name_to_id: std::collections::BTreeMap<String, ItemId>,
}


#[derive(Reflect, Serialize, Deserialize, Clone)]
pub struct ItemInstance {
    pub item: Item,
    pub qty: u16,
    pub uid: Option<uuid::Uuid>,   // present when state is not default
}

/// <===================== GAME MECHANICS COMPONENTS =====================> ///
/// 
/// These will influence how items play out in game and will be drawn in the UI.
/// 


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


/// <===================== UI =====================> ///
/// 
/// Functions and useful components to decide how the item will be drawn within the UI.
/// 

/// How the item will be represented in the UI.
#[derive(Component)]
pub enum UiItemAsset {
    Atlas,
    Model,
    Texture,
    AnimTexture,
    Other
}

pub fn update_item_ui_node() {
    // Might need to be called very sparsely
    
}

pub fn build_item_ui_node() {

    // If item has durability, add percentage.
    // If item has count > 1, add number.

}


