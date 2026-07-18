use bevy::prelude::*;

use crate::plugin::{crafting::spatial::InventoryMachine, ui::main::*};
use crate::plugin::ui::item::build_ui_item_display;
use crate::plugin::loader::item_registry::ItemRegistry;
use crate::plugin::inventory::spatial::*;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// CONSTANTS & MARKERS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

const SPATIAL_ITEM_ICON_SIZE: f32 = 64.0;

/// Marker on the UI node that visualizes a `SpatialInventory`.
/// Back-reference mirrors `InventorySlot::source_entity`.
#[derive(Component)]
pub struct SpatialInventoryNode {
    pub source_entity: Entity,
}

/// Marker on each item child inside a `SpatialInventoryArea`. The sync
/// observer uses these to find and despawn the right node when a
/// placement is removed.
#[derive(Component)]
pub struct SpatialPlacementNode {
    pub source_entity: Entity,
    pub placement_id:  PlacementID,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PANEL & PLACEMENT BUILDERS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub fn build_inventory_crafting_ui(
    machine_entity: Entity,
    spatial_entity: Entity,
    spatial_data:  &SpatialInventory,
) -> impl Bundle {

    (Node {
            display: Display::Flex,
            align_content: AlignContent::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(UI_PANEL_RADIUS),
            border: UiRect::all(UI_BORDER_THICKN),
            padding: UiRect::all(UI_PANEL_PADDING),
            width: px(844.0),
            ..default()
        },
        BorderColor::all(UI_BORDER_COLOR),
        BackgroundColor(UI_PANEL_COLOR),
        Pickable::IGNORE,

        // Once this bundle is spawned, this will automatically spawn as many children as needed.
        children![
            build_spatial_inventory_panel(spatial_entity, spatial_data),
            build_recipe_output_slot(machine_entity)
            ]
    )
}


/// Build the crafting area panel. Items will be added as absolutely-
/// positioned children by the sync observer as placements happen.
pub fn build_spatial_inventory_panel(
    source_entity: Entity,
    spatial_data:  &SpatialInventory,
) -> impl Bundle {
    (Node {
            width:          Val::Px(spatial_data.width()),
            height:         Val::Px(spatial_data.height()),
            position_type:  PositionType::Relative,
            border_radius:  BorderRadius::all(UI_PANEL_RADIUS),
            border:         UiRect::all(UI_BORDER_THICKN),
            margin:         UiRect::all(SLOT_GAP),
            ..default()
        },
        BorderColor::all(UI_BORDER_COLOR),
        BackgroundColor(UI_SLOT_COLOR),
        SpatialInventoryNode { source_entity },
        Pickable { should_block_lower: true, is_hoverable: true },
    )
}

/// Build one absolutely-positioned child for a placed item.
/// `pos` is taken to represent the CENTER of the icon, so we offset by
/// half-size when computing `left`/`top` — this feels more natural with
/// freeform placement.
fn build_placement_node(
    source_entity: Entity,
    placement_id:  PlacementID,
    placement:     &Placement,
    item_registry: &ItemRegistry,
) -> impl Bundle {
    let half = SPATIAL_ITEM_ICON_SIZE * 0.5;

    (Node {
            position_type: PositionType::Absolute,
            left: Val::Px(placement.pos.x - half),
            top:  Val::Px(placement.pos.y - half),
            width:  Val::Px(SPATIAL_ITEM_ICON_SIZE),
            height: Val::Px(SPATIAL_ITEM_ICON_SIZE),
            ..default()
        },
        SpatialPlacementNode { source_entity, placement_id },
        // Placements don't catch clicks, it's the spatial inventory ui that
        // checks whether they were hit by a click or not. They're a mirage.
        Pickable::IGNORE,
        children![
            build_ui_item_display(
                &item_registry.get(placement.stack.id).display,
                placement.stack.count,
            )
        ],
    )
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// EVENTS & SPAWNING
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Internal event raised once a click on the area panel has been resolved
/// into area-local coordinates. Keeps the "find the panel" pointer math
/// separate from the "mutate inventories" logic.
#[derive(EntityEvent)]
pub struct SpatialInventoryClickedEvent {
    #[event_target]
    pub entity:     Entity, // the SpatialInventory entity
    pub local_pos:  Vec2,
    pub id:         Option<PlacementID>, // The PlacementID that was clicked, if any
    pub button:     PointerButton, // The mouse button used to click
}


// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// CLICK HANDLERS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━


/// The single click entry point for spatial inventories. Owns ALL geometry:
/// converts the pointer position into area-local coordinates, discards
/// out-of-bounds clicks, and hit-tests the click against the DATA's
/// placement positions (placement nodes are click-transparent visuals).
/// Emits a fully resolved `SpatialInventoryClickedEvent` for the data side.
pub fn spatial_inventory_click_obs(
    click: On<Pointer<Click>>,
    mut commands: Commands,
    node_query: Query<(&SpatialInventoryNode, &ComputedNode)>,
    inv_query: Query<&SpatialInventory>,
) {
    let Ok((node, panel)) = node_query.get(click.entity) else { return };
    let Ok(spatial) = inv_query.get(node.source_entity) else { return };
    let Some(local_pos_three_dimensional) = click.hit.position else { return };

    // Local position in terms of 
    let local_pos = local_pos_three_dimensional.truncate() * panel.size() + panel.size() * 0.5;

    // Out-of-bounds clicks are discarded before any event exists.
    if !spatial.contains(local_pos) { return; }

    let id = spatial.hit_test(local_pos, SPATIAL_ITEM_ICON_SIZE * 0.5);

    commands.trigger(SpatialInventoryClickedEvent {
        entity:    node.source_entity,
        local_pos,
        id,
        button:    click.button,
    });
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SYNC
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Reacts to a `SpatialInventoryChangedEvent` by doing the minimum work:
/// spawn one node, despawn one node, edit one node...
/// Handles the edge case of multiple UIs viewing the same inventory.
pub fn spatial_inventory_changed_to_ui_sync_obs(
    event: On<SpatialInventoryChangedEvent>,
    mut commands: Commands,
    areas: Query<(Entity, &SpatialInventoryNode)>,
    placement_nodes: Query<(Entity, &SpatialPlacementNode)>,
    spatial_q: Query<&SpatialInventory>,
    item_registry: Res<ItemRegistry>,
) {
    let target = event.entity;

    match event.change {
        SpatialChange::Placed(id) => {
            let Ok(spatial) = spatial_q.get(target) else { return };
            let Some(placement) = spatial.get(id) else { return };

            for (area_entity, area) in areas.iter() {
                if area.source_entity != target { continue; }
                let child = commands.spawn(build_placement_node(
                    target, id, placement, &item_registry,
                )).id();
                commands.entity(area_entity).add_child(child);
            }
        }
        SpatialChange::Removed(id) => {
            for (node_entity, node) in placement_nodes.iter() {
                if node.source_entity != target { continue; }
                if node.placement_id  != id     { continue; }
                commands.entity(node_entity).despawn();
            }
        }
        SpatialChange::Modified(id) => {
            let Ok(spatial) = spatial_q.get(target) else { return };
            let Some(placement) = spatial.get(id) else { return };

            // Rebuild the node: despawn stale, spawn fresh. Cheap enough,
            // and reuses the exact spawn path `Placed` uses.
            for (node_entity, node) in placement_nodes.iter() {
                if node.source_entity == target && node.placement_id == id {
                    commands.entity(node_entity).despawn();
                }
            }
            for (area_entity, area) in areas.iter() {
                if area.source_entity != target { continue; }
                let child = commands.spawn(build_placement_node(
                    target, id, placement, &item_registry,
                )).id();
                commands.entity(area_entity).add_child(child);
            }
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// CRAFTING-RELATED UI STUFF
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━


use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
};
use crate::plugin::crafting::main::{CurrentRecipe, MachineRecipeChanged, CraftRequest};

const OUTPUT_GHOST_SLOT_SIZE: Val = Val::Px(64.0); // match your inventory slot size

/// The ghost output: a pure VIEW of the machine's CurrentRecipe cache.
/// It stores nothing — clicking it asks the machine to craft.
#[derive(Component)]
#[component(on_add = recipe_output_slot_on_add)]
pub struct RecipeOutputSlot {
    pub machine: Entity,
}

/// Same pattern as InventorySlot: a freshly spawned slot requests its own
/// first sync, so no UI builder ever has to remember to.
fn recipe_output_slot_on_add(mut world: DeferredWorld, ctx: HookContext) {
    let machine = world.entity(ctx.entity)
        .get::<RecipeOutputSlot>()
        .expect("on_add runs after insertion")
        .machine;
    world.commands().trigger(MachineRecipeChanged { entity: machine });
}

pub fn build_recipe_output_slot(machine: Entity) -> impl Bundle {
    (Node {
            width:  OUTPUT_GHOST_SLOT_SIZE,
            height: OUTPUT_GHOST_SLOT_SIZE,
            border: UiRect::all(UI_BORDER_THICKN),
            border_radius: BorderRadius::all(UI_PANEL_RADIUS),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        BorderColor::all(UI_BORDER_COLOR),
        BackgroundColor(UI_PANEL_COLOR),
        RecipeOutputSlot { machine },
        Pickable { should_block_lower: true, is_hoverable: true },
    )
}

/// MachineOutputChanged → redraw every ghost slot viewing that machine.
pub fn recipe_output_sync_obs(
    event: On<MachineRecipeChanged>,
    mut commands: Commands,
    machines: Query<&CurrentRecipe>,
    slots: Query<(Entity, &RecipeOutputSlot)>,
    item_registry: Res<ItemRegistry>,
) {
    let Ok(current) = machines.get(event.entity) else { return };

    for (slot_entity, slot) in slots.iter() {
        if slot.machine != event.entity { continue; }

        commands.entity(slot_entity).despawn_related::<Children>();

        if let Some(m) = &current.0 {
            let icon = commands.spawn(build_ui_item_display(
                &item_registry.get(m.result.id).display,
                m.result.count,
            )).id();
            commands.entity(slot_entity).add_child(icon);
        }
    }
}

/// Clicks on the ghost slot become craft requests. Acceptance logic lives
/// entirely on the data side.
pub fn recipe_output_click_obs(
    mut click: On<Pointer<Click>>,
    mut commands: Commands,
    slots: Query<&RecipeOutputSlot>,
) {
    let Ok(slot) = slots.get(click.entity) else { return };
    click.propagate(false);
    commands.trigger(CraftRequest { entity: slot.machine });
}