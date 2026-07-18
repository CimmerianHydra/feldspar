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
pub struct InventorySpatialCraftingMachine {
    pub input_entity : Entity,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TRIGGERS AND EVENTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Input changed → re-search the RecipeMap, update the cache, notify UIs.
pub fn inventory_recompute_recipe_obs(
    event: On<SpatialInventoryChangedEvent>,
    spatial: Query<(Entity, &SpatialInventory)>,
    mut commands: Commands,
    mut machines: Query<(Entity, &InventorySpatialCraftingMachine, &mut CurrentRecipe)>,
    recipes: Res<SpatialRecipeRegistry>,
) {
    let Ok((input_entity, input_data)) = spatial.get(event.entity) else { return };
    for (machine, machine_data, mut current) in machines.iter_mut() {
        if machine_data.input_entity != input_entity { continue; }
        current.0 = recipes.match_inventory(input_data);
        commands.trigger(MachineRecipeChanged { entity: machine });
    }
}

/// Execute a craft from the inventory crafting machine: validate cursor acceptance,
/// re-validate ingredients against LIVE data, consume, deliver. The consumption's
/// own change events re-trigger the recompute above, so the ghost slot refreshes (or
/// empties) automatically — repeat-crafting costs zero extra code.
pub fn inventory_spatial_craft_request_obs(
    event: On<CraftRequest>,
    mut commands: Commands,
    machines: Query<(&InventorySpatialCraftingMachine, &CurrentRecipe)>,
    mut spatial: Query<&mut SpatialInventory>,
    mut cursor_q: Query<(Entity, &mut Inventory), With<CursorInventory>>,
    item_registry: Res<ItemRegistry>,
) {
    let Ok((machine, current)) = machines.get(event.entity) else { return };
    let Some(recipe) = current.0.clone() else { return };
    let Ok(mut input_inventory) = spatial.get_mut(machine.input_entity) else { return };
    let Ok((cursor_entity, mut cursor_inv)) = cursor_q.single_mut() else { return };

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
        commands.trigger(SpatialInventoryChangedEvent { entity: machine.input_entity, change });
    }

    cursor_inv.insert_at_slot(recipe.result.id, recipe.result.count, 0, &item_registry);
    commands.trigger(InventoryChangedEvent { entity: cursor_entity, index: 0 });
    commands.trigger(CraftExecuted { entity: event.entity, result: recipe.result });
}