use bevy::prelude::*;

use crate::plugin::crafting::spatial::{inventory_machine_craft_request_obs, spatial_inventory_change_recompute_recipe_obs};
use crate::plugin::inventory::main::ItemStack;
use crate::plugin::loader::recipe_maps::MatchedRecipe;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct CraftingPlugin;

impl Plugin for CraftingPlugin {
    fn build(&self, app: &mut App) {
        app
        .add_observer(spatial_inventory_change_recompute_recipe_obs)
        .add_observer(inventory_machine_craft_request_obs)
        ;
    }
}

/// Marker entity for machine objects.
/// Not sure how useful this is going to be.
#[derive(Component)]
pub struct Machine;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// EVENTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// "This machine's deduced output changed" — drives ghost-slot UIs.
#[derive(EntityEvent)]
pub struct MachineRecipeChanged {
    #[event_target]
    pub entity: Entity,
}

/// UI → data: requesting a given entity to begin its craft.
#[derive(Event)]
pub struct CraftRequest {
    pub entity: Entity,
}

/// Recipe-logic cache: the machine's current best match against its inputs.
/// Recomputed ONLY when an input-change event fires — GT's "notifiable
/// handler" pattern. Never polled.
#[derive(Component, Default)]
pub struct CurrentRecipe(pub Option<MatchedRecipe>);

/// Data → world: a craft actually happened. This is the first member of the
/// event family automated machines will emit on recipe completion — stats,
/// advancements, sounds, and quest systems hook here, never into the
/// mutation code itself.
#[derive(EntityEvent)]
pub struct CraftExecuted {
    #[event_target]
    pub entity: Entity,
    pub result: ItemStack,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// RELATIONSHIPS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Machines require entities as their inputs and outputs. This can be modeled
/// as a relationship within Bevy.

/// Input → machine. This IS the relationship (source of truth).
#[derive(Component)]
#[relationship(relationship_target = Inputs)]
pub struct InputOf {
    pub machine_entity: Entity,
}

/// Machine → inputs. Auto-maintained inverse; never insert or mutate this
/// yourself — Bevy does, whenever the component above is added/removed.
#[derive(Component, Deref)]
#[relationship_target(relationship = InputOf)]
pub struct Inputs(Vec<Entity>);