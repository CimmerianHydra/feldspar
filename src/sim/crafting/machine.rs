use bevy::prelude::*;

use crate::content::item::ItemStack;
use crate::content::recipe::MatchedRecipe;
use crate::sim::inventory::PlacementID;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – WHAT A MACHINE IS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Marker for machine entities.
#[derive(Component)]
pub struct Machine;

/// A recipe match against a spatial layout, with placements as handles.
///
/// The registry's matcher is generic over the handle type precisely so that
/// `content` never names `PlacementID`; this alias is where the two meet.
pub type SpatialMatch = MatchedRecipe<PlacementID>;

/// Recipe-logic cache: the machine's current best match against its inputs.
/// Recomputed ONLY when an input-change event fires — GT's "notifiable
/// handler" pattern. Never polled.
#[derive(Component, Default)]
pub struct CurrentRecipe(pub Option<SpatialMatch>);

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – EVENTS
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
// SECTION 3 – RELATIONSHIPS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

// Machines require entities as their inputs and outputs. Bevy models that
// natively as a relationship.

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
