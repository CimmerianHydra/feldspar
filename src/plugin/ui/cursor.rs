use bevy::prelude::*;

use crate::plugin::inventory::item_registry;
use crate::plugin::inventory::item_registry::ItemRegistry;
use crate::plugin::inventory::main::Inventory;
use crate::plugin::inventory::main::InventoryChangedEvent;
use crate::plugin::ui::item::build_ui_item_display;
use crate::plugin::ui::main::*;
use crate::plugin::ui::inventory::*;
use crate::plugin::inventory::player::CursorInventory;

pub const CURSOR_UI_ZINDEX: i32 = 1000;

#[derive(Component)]
pub struct CursorSlot;

// If the mouse is moving over this area, every MouseFollower will snap to it.
#[derive(Component)]
pub struct MouseFollowerArea;

#[derive(Component)]
pub struct MouseFollower;

pub fn spawn_cursor_item_display_sys(
    mut commands: Commands,
    cursor_inventory_query: Query<Entity, With<CursorInventory>>,
) {
    let Ok(cursor_inventory_entity) = cursor_inventory_query.single() else {
        bevy::log::error!("Cursor inventory does not exist! Please spawn a cursor inventory.");
        return;
    };

    let cursor_inventory_slot = (Node {
        width: SLOT_SIZE,
        height: SLOT_SIZE,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        flex_direction: FlexDirection::Column,
        margin: UiRect::all(SLOT_GAP),
        ..default()
        },
        CursorSlot,
        InventorySlot { 
            entity: cursor_inventory_entity,
            slot_index: 0
        },
        MouseFollower,
        UiTransform::default(),
        Pickable::IGNORE,
    );

    // Large node to house the display
    let cursor_display_parent = (Node {
            width: percent(100),
            height: percent(100),
            ..default()
        },
        MouseFollowerArea,
        ZIndex(CURSOR_UI_ZINDEX),
        Visibility::Visible,
        Pickable { should_block_lower: false, is_hoverable: true },
        children![cursor_inventory_slot],
    );

    commands.spawn(cursor_display_parent).observe(on_cursor_move);
}


pub fn on_cursor_move(
    event: On<Pointer<Move>>,
    follower_area_q: Single<Entity, With<MouseFollowerArea>>,
    mut follower_q: Query<&mut UiTransform, With<MouseFollower>>,
) {
    if event.entity == *follower_area_q {
        for mut follower_transform in follower_q.iter_mut() {
            let mouse_position = event.pointer_location.position;

            let mouse_x = Val::Px(mouse_position.x);
            let mouse_y = Val::Px(mouse_position.y);
            follower_transform.translation = Val2::new(mouse_x, mouse_y);
        }
    }
}

pub fn sync_cursor_inventory_obs(
    event: On<InventoryChangedEvent>,
    mut commands: Commands,
    cursor_inventory_q: Query<(Entity, &Inventory), With<CursorInventory>>,
    cursor_slot_q: Query<Entity, With<CursorSlot>>,
    item_registry: Res<ItemRegistry>,
) {
    bevy::log::info!("Cursor inventory change was detected!");
    if let Ok((cursor_entity, cursor_inventory)) = cursor_inventory_q.single() {
        if let Ok(cursor_slot) = cursor_slot_q.single() {
            if event.entity == cursor_entity {
                commands.entity(cursor_slot).despawn_children();
                if let Some(stack) = cursor_inventory.slots()[0] {
                    let item_id = stack.id;
                    let count = stack.count;
                    let display = &item_registry.get(item_id).display;
                    let ui_item_display = build_ui_item_display(display, count);

                    let ui_item_display_entity = commands.spawn(ui_item_display).id();
                    commands.entity(cursor_slot).add_child(ui_item_display_entity);
                }
            }
        }
    }
}


