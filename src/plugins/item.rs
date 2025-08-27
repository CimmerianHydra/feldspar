use std::default;

use bevy::prelude::*;

#[derive(Component)]
pub struct Item; // Item marker. Items are fundamental game objects and represent a (stack of) a certain item.

#[derive(Resource, Default)]
pub struct ItemData {
    pub name : String,
    pub tooltip : String,
    pub stackable : bool,
    pub tags : Vec<ItemTag>,
} // Represents abstract, permanent item data.

pub enum ItemTag {
    block,
    stone,
    wood,
    ore,
    iron,
    tool,
    ingot,
    gem,
}

/// <===================== GAME MECHANICS COMPONENTS =====================> ///
/// 
/// These will influence how items play out in game and will be drawn in the UI.
/// 

#[derive(Component)]
pub struct ItemDurability(pub f32);
impl Default for ItemDurability {
    fn default() -> Self {
        Self(1.0)
    }
}

#[derive(Component)]
pub struct ItemStack(pub u32);
impl Default for ItemStack {
    fn default() -> Self {
        Self(1)
    }
}


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


