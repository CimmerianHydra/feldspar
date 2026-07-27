//! Machines, their cached recipe match, and the crafting that follows from
//! it.
//!
//! The recipe *vocabulary* and *registry* live in [`crate::content::recipe`];
//! what a machine does with a match lives here.

pub mod machine;
pub mod spatial;

pub use machine::{
    CraftExecuted, CraftRequest, CurrentRecipe, InputOf, Inputs, Machine, MachineRecipeChanged,
    SpatialMatch,
};
pub use spatial::InventoryMachine;

use bevy::prelude::*;

pub struct CraftingPlugin;

impl Plugin for CraftingPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(spatial::spatial_inventory_change_recompute_recipe_obs)
            .add_observer(spatial::inventory_machine_craft_request_obs);
    }
}
