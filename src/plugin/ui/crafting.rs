use bevy::prelude::*;

use crate::plugin::ui::main::*;
use crate::plugin::ui::item::build_ui_item_display;
use crate::plugin::inventory::item_registry::ItemRegistry;
use crate::plugin::inventory::main::{Inventory, InventoryChangedEvent, ItemStack};
use crate::plugin::inventory::player::CursorInventory;
use crate::plugin::crafting::main::*;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// CONSTANTS & MARKERS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

const SPATIAL_ITEM_ICON_PX: f32 = 48.0;
const SPATIAL_ITEM_ICON_SIZE: Val = Val::Px(SPATIAL_ITEM_ICON_PX);

/// Marker on the UI node that visualizes a `SpatialInventory`.
/// Back-reference mirrors `InventorySlot::source_entity`.
#[derive(Component)]
pub struct SpatialCraftingArea {
    pub source_entity: Entity,
}

/// Marker on each item child inside a `SpatialCraftingArea`. The sync
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

/// Build the crafting area panel. Items will be added as absolutely-
/// positioned children by the sync observer as placements happen.
pub fn build_crafting_area_ui(
    source_entity: Entity,
    width:  f32,
    height: f32,
) -> impl Bundle {
    (Node {
            width:         Val::Px(width),
            height:        Val::Px(height),
            position_type: PositionType::Relative,
            border_radius: BorderRadius::all(UI_PANEL_RADIUS),
            border:        UiRect::all(UI_BORDER_THICKN),
            padding:       UiRect::all(UI_PANEL_PADDING),
            ..default()
        },
        BorderColor::all(UI_BORDER_COLOR),
        BackgroundColor(UI_PANEL_COLOR),
        SpatialCraftingArea { source_entity },
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
    let half = SPATIAL_ITEM_ICON_PX * 0.5;

    (Node {
            position_type: PositionType::Absolute,
            left: Val::Px(placement.pos.x - half),
            top:  Val::Px(placement.pos.y - half),
            width:  SPATIAL_ITEM_ICON_SIZE,
            height: SPATIAL_ITEM_ICON_SIZE,
            ..default()
        },
        SpatialPlacementNode { source_entity, placement_id },
        // Placements catch their own clicks (for pickup) and block the
        // panel underneath so the area observer doesn't also fire.
        Pickable { should_block_lower: true, is_hoverable: true },
        children![
            build_ui_item_display(
                &item_registry.get(placement.stack.id).display,
                placement.stack.count,
            )
        ],
    )
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// CLICK → PLACE (empty space)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Internal event raised once a click on the area panel has been resolved
/// into area-local coordinates. Keeps the "find the panel" pointer math
/// separate from the "mutate inventories" logic.
#[derive(EntityEvent)]
pub struct SpatialAreaClickedEvent {
    #[event_target]
    pub entity:    Entity, // the SpatialInventory entity
    pub local_pos: Vec2,
}

/// First leg: a click landed on a `SpatialCraftingArea` panel. Convert the
/// screen-space pointer position into area-local coordinates and forward.
///
/// `GlobalTransform` of a UI node represents the node's center in
/// screen-pixel space, and `ComputedNode::size` gives its rendered size.
pub fn spatial_area_click_obs(
    mut click: On<Pointer<Click>>,
    mut commands: Commands,
    areas: Query<(&SpatialCraftingArea, &ComputedNode, &GlobalTransform)>,
) {
    let Ok((area, computed, transform)) = areas.get(click.entity) else { return };

    let panel_center   = transform.translation().truncate();
    let panel_size     = computed.size();
    let panel_top_left = panel_center - panel_size * 0.5;
    let local          = click.pointer_location.position - panel_top_left;

    commands.trigger(SpatialAreaClickedEvent {
        entity:    area.source_entity,
        local_pos: local,
    });
    click.propagate(false);
}

/// Second leg: drop the cursor's held stack into the spatial inventory at
/// the resolved local position. Snapshot-then-mutate, then fire change
/// events for cursor and target — same shape as `inventory_ui_click_obs`.
pub fn place_from_cursor_obs(
    event: On<SpatialAreaClickedEvent>,
    mut commands: Commands,
    mut spatial_q: Query<&mut SpatialInventory>,
    mut cursor_q:  Query<(Entity, &mut Inventory), With<CursorInventory>>,
) {
    let Ok((cursor_entity, mut cursor_inv)) = cursor_q.single_mut() else { return };
    let Ok(mut spatial) = spatial_q.get_mut(event.entity) else { return };

    let Some(stack) = cursor_inv.slots()[0] else { return };

    // Bail before mutating anything if the spot is out of bounds.
    if !spatial.contains(event.local_pos) { return; }

    let extracted = cursor_inv.extract_from_slot(stack.id, stack.count, 0);
    if extracted.transferred == 0 { return; }

    let Some(id) = spatial.place(
        event.local_pos,
        ItemStack { id: stack.id, count: extracted.transferred },
    ) else { return /* unreachable if `contains` passed */ };

    commands.trigger(SpatialInventoryChangedEvent {
        entity: event.entity,
        change: SpatialChange::Placed(id),
    });
    commands.trigger(InventoryChangedEvent {
        entity: cursor_entity,
        index:  0,
    });
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// CLICK → PICK UP (on an existing placement)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Clicking a placement returns it to the cursor. For now we only handle
/// the "cursor is empty" case; merge/swap semantics can come later.
pub fn placement_click_obs(
    mut click: On<Pointer<Click>>,
    mut commands: Commands,
    placements: Query<&SpatialPlacementNode>,
    mut spatial_q: Query<&mut SpatialInventory>,
    mut cursor_q:  Query<(Entity, &mut Inventory), With<CursorInventory>>,
    item_registry: Res<ItemRegistry>,
) {
    let Ok(node) = placements.get(click.entity) else { return };
    let Ok(mut spatial)                       = spatial_q.get_mut(node.source_entity) else { return };
    let Ok((cursor_entity, mut cursor_inv))   = cursor_q.single_mut() else { return };

    // TODO: cursor non-empty → swap or merge. For now: ignore.
    if cursor_inv.slots()[0].is_some() { return; }

    let Some(stack) = spatial.remove(node.placement_id) else { return };
    cursor_inv.insert_at_slot(stack.id, stack.count, 0, &item_registry);

    commands.trigger(SpatialInventoryChangedEvent {
        entity: node.source_entity,
        change: SpatialChange::Removed(node.placement_id),
    });
    commands.trigger(InventoryChangedEvent {
        entity: cursor_entity,
        index:  0,
    });
    click.propagate(false);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// INCREMENTAL SYNC
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Reacts to a `SpatialInventoryChangedEvent` by doing the minimum work:
/// spawn one node, despawn one node, or wipe all this area's children.
/// Handles the edge case of multiple UIs viewing the same inventory.
pub fn spatial_inventory_sync_obs(
    event: On<SpatialInventoryChangedEvent>,
    mut commands: Commands,
    areas: Query<(Entity, &SpatialCraftingArea)>,
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
        SpatialChange::Cleared => {
            for (node_entity, node) in placement_nodes.iter() {
                if node.source_entity != target { continue; }
                commands.entity(node_entity).despawn();
            }
        }
    }
}