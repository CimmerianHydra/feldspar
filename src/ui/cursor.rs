use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::content::item::ItemRegistry;
use crate::sim::inventory::{CursorInventory, Inventory, InventoryChangedEvent};
use crate::ui::inventory::InventorySlot;
use crate::ui::theme::*;
use crate::ui::widget::build_ui_item_display;
use crate::GameState;

pub const CURSOR_UI_ZINDEX: i32 = 1000;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – CURSOR LOCKING
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Event)]
pub enum CursorLockRequest {
    Lock,
    Unlock,
}

pub fn cursor_lock_request_obs(
    cursor_request: On<CursorLockRequest>,
    mut cursor_options: Single<&mut CursorOptions, With<PrimaryWindow>>,
) {
    match cursor_request.event() {
        CursorLockRequest::Lock => {
            cursor_options.grab_mode = CursorGrabMode::Locked;
            cursor_options.visible = false;
        }
        CursorLockRequest::Unlock => {
            cursor_options.grab_mode = CursorGrabMode::None;
            cursor_options.visible = true;
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – THE HELD STACK
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Component)]
pub struct CursorSlot;

/// If the mouse is moving over this area, every `MouseFollower` snaps to it.
#[derive(Component)]
pub struct MouseFollowerArea;

#[derive(Component)]
pub struct MouseFollower;

/// Presentation observing simulation: the moment a `CursorInventory`
/// appears anywhere, give it something to look like.
pub fn spawn_cursor_item_display_sys(
    mut commands: Commands,
    new_cursors: Query<Entity, (Added<CursorInventory>, With<Inventory>)>,
) {
    for new_cursor in new_cursors.iter() {
        let cursor_inventory_slot = (
            Node {
                width: SLOT_SIZE,
                height: SLOT_SIZE,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            CursorSlot,
            InventorySlot {
                source_entity: new_cursor,
                slot_index: 0,
            },
            MouseFollower,
            UiTransform::default(),
            Pickable::IGNORE,
        );

        // Large node to house the display.
        let cursor_display_parent = (
            Node {
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
}

pub fn on_cursor_move(
    event: On<Pointer<Move>>,
    follower_area_q: Single<Entity, With<MouseFollowerArea>>,
    mut follower_q: Query<&mut UiTransform, With<MouseFollower>>,
) {
    if event.entity == *follower_area_q {
        for mut follower_transform in follower_q.iter_mut() {
            let mouse_position = event.pointer_location.position;
            follower_transform.translation =
                Val2::new(Val::Px(mouse_position.x), Val::Px(mouse_position.y));
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
    let Ok((cursor_entity, cursor_inventory)) = cursor_inventory_q.single() else { return };
    let Ok(cursor_slot) = cursor_slot_q.single() else { return };
    if event.entity != cursor_entity { return; }

    commands.entity(cursor_slot).despawn_children();

    if let Some(stack) = cursor_inventory.slots()[0] {
        let display = &item_registry.get(stack.id).display;
        let ui_item_display_entity = commands
            .spawn(build_ui_item_display(display, stack.count))
            .id();
        commands.entity(cursor_slot).add_child(ui_item_display_entity);
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – CROSSHAIR  (it's kinda like a cursor, right?)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub fn spawn_crosshair_sys(mut commands: Commands) {
    // For now, just a small square as a placeholder for a crosshair.
    let crosshair_bundle = (
        Node {
            width: px(10),
            height: px(10),
            ..default()
        },
        BackgroundColor(Color::srgb(1.0, 1.0, 1.0)),
        Pickable::IGNORE,
    );

    let crosshair_parent = (
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_direction: FlexDirection::Column,
            ..default()
        },
        Pickable::IGNORE,
        children![crosshair_bundle],
    );

    commands.spawn(crosshair_parent);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct UiCursorPlugin;

impl Plugin for UiCursorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::InGame), spawn_crosshair_sys)
            .add_systems(Update, spawn_cursor_item_display_sys)
            .add_observer(cursor_lock_request_obs)
            .add_observer(sync_cursor_inventory_obs);
    }
}
