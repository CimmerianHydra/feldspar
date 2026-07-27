//! # Layer 5 — simulation
//!
//! Gameplay. Block entities, inventories, crafting machines, per-block
//! behaviours, and the simulation-side notion of a player.
//!
//! May import: [`crate::voxel`], [`crate::command`], [`crate::space`],
//! [`crate::content`].
//!
//! **It never imports presentation.** A barrel declares that it has an
//! `Inventory` and is `Interactable`; whether anyone is looking at it, and
//! what that looks like, is not its problem. Everything here runs headless.


pub mod behaviors;
pub mod block_entity;
pub mod crafting;
pub mod inventory;
pub mod player;
pub use block_entity::{BlockEntityEvent, BlockEntityTag, Interactable};
pub use player::{
    HotbarSelect, LookTarget, LookTargetChanged, Player, PlayerEquipment, PlayerHeldItems,
    PlayerHotbar, PlayerInventories, PlayerInventory, PlayerInventoryAccess, PlayerLookTarget,
};

use bevy::prelude::*;

pub struct SimPlugin;

impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            block_entity::BlockEntityPlugin,
            inventory::InventoryPlugin,
            crafting::CraftingPlugin,
            behaviors::BehaviorsPlugin,
            player::PlayerStatePlugin,
        ));
    }
}
