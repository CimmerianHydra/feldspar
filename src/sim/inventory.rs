//! Inventories: slot-based storage, freeform spatial storage, the cursor
//! that moves stacks between them, and the `/give` command.
//!
//! `ItemStack` and `MAX_STACK` live in [`crate::content::item`] and are
//! re-exported here — a recipe's result is a stack too, and content must not
//! reach up into simulation to describe one.

pub mod commands;
pub mod cursor;
pub mod spatial;
pub mod stack;
pub mod storage;

pub use crate::content::item::{ItemStack, MAX_STACK};

pub use cursor::CursorInventory;
pub use spatial::{
    Placement, PlacementID, SpatialChange, SpatialInventory, SpatialInventoryChangedEvent,
    SpatialInventoryClickedEvent,
};
pub use stack::{insert_and_notify, transfer_items, TransferResult};
pub use storage::{Inventory, InventoryChangedEvent, InventoryClickedEvent};

use bevy::prelude::*;

use crate::command::ConsoleAppExt;

pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.add_command(commands::give_command_spec(), commands::give_cmd)
            .add_observer(cursor::inventory_click_obs)
            .add_observer(cursor::spatial_inventory_click_obs);
    }
}
