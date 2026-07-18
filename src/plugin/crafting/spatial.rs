use bevy::prelude::*;


use crate::plugin::inventory::cursor::CursorInventory;
use crate::plugin::loader::item_registry::ItemRegistry;
use crate::plugin::loader::recipe_maps::SpatialRecipeRegistry;

use crate::plugin::crafting::main::*;
use crate::plugin::inventory::main::{Inventory, InventoryChangedEvent};
use crate::plugin::inventory::spatial::*;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// BASIC DEFINITIONS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Machine-type marker: a player-driven spatial crafter. The GregTech
/// analogue is the MetaTileEntity class; the components alongside it
/// (SpatialInventory, CurrentRecipe) are its handlers and recipe logic.
#[derive(Component)]
pub struct InventoryMachine;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TRIGGERS AND EVENTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Input changed → re-search the RecipeMap, update the cache, notify UIs.
pub fn spatial_inventory_change_recompute_recipe_obs(
    event: On<SpatialInventoryChangedEvent>,
    spatial_inventory_q: Query<(&SpatialInventory, &InputOf)>,
    mut commands: Commands,
    mut machines: Query<(Entity, &mut CurrentRecipe)>,
    recipes: Res<SpatialRecipeRegistry>,
) {
    let Ok((inv_data, input_of)) = spatial_inventory_q.get(event.entity) else { return };
    let Ok((machine, mut current)) = machines.get_mut(input_of.machine_entity) else { return };

    current.0 = recipes.match_inventory(inv_data);
    commands.trigger(MachineRecipeChanged { entity: machine });
}

/// Execute a craft from the inventory crafting machine: validate cursor acceptance,
/// re-validate ingredients against LIVE data, consume, deliver. The consumption's
/// own change events re-trigger the recompute above, so the ghost slot refreshes (or
/// empties) automatically — repeat-crafting costs zero extra code.
pub fn inventory_machine_craft_request_obs(
    event: On<CraftRequest>,
    mut commands: Commands,
    machines: Query<(&CurrentRecipe, &Inputs), With<InventoryMachine>>,
    mut spatial: Query<&mut SpatialInventory>,
    mut cursor_q: Query<(Entity, &mut Inventory), With<CursorInventory>>,
    item_registry: Res<ItemRegistry>,
) {
    // Machines with no recipe and no inputs can't craft.
    let Ok((current_recipe, inputs)) = machines.get(event.entity) else { return };

    // Return early if no recipe is set in the machine.
    let Some(recipe) = current_recipe.0.clone() else { return };

    // No cursor? No craft. This is a special machine that only ever sends items to the cursor.
    let Ok((cursor_entity, mut cursor_inv)) = cursor_q.single_mut() else { return };

    // For every available inventory of this crafting machine (there's going to be only one), craft the recipe.
    let Some(inv_entity) = inputs.first() else { return; };
    let inv_entity = *inv_entity;

    let Ok(mut input_inventory) = spatial.get_mut(inv_entity) else { return };

    // The cursor must be able to accept the WHOLE result.
    let max_stack = item_registry.get(recipe.result.id).max_stack;
    let can_accept = match cursor_inv.slots()[0] {
        None => recipe.result.count <= max_stack,
        Some(c) => c.id == recipe.result.id
                && c.count.saturating_add(recipe.result.count) <= max_stack,
    };
    if !can_accept { return; }

    // Never trust the cache: it could be one event behind live data.
    for &(pid, need) in &recipe.consume {
        match input_inventory.get(pid) {
            Some(p) if p.stack.count >= need => {}
            _ => return,
        }
    }

    // Consume. Partial stacks keep their positions (extract preserves pos),
    // so the arrangement survives for the next craft.
    for &(pid, need) in &recipe.consume {
        if need == 0 { continue; }
        let item = input_inventory.get(pid).map(|p| p.stack.id).unwrap();
        input_inventory.extract_from_placement(item, need, pid);

        let change = if input_inventory.get(pid).is_some() {
            SpatialChange::Modified(pid)
        } else {
            SpatialChange::Removed(pid)
        };
        commands.trigger(SpatialInventoryChangedEvent { entity: inv_entity, change });
    }

    cursor_inv.insert_at_slot(recipe.result.id, recipe.result.count, 0, &item_registry);
    commands.trigger(InventoryChangedEvent { entity: cursor_entity, index: 0 });
    commands.trigger(CraftExecuted { entity: event.entity, result: recipe.result });
}