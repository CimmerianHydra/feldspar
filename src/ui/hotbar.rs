use bevy::prelude::*;

use crate::sim::inventory::Inventory;
use crate::sim::player::PlayerHotbar;
use crate::ui::inventory::{InventorySlot, InventoryUISyncRequest};
use crate::ui::screen::EntityUISession;
use crate::ui::theme::*;
use crate::ui::widget::build_base_ui_item_slot;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – BUILDERS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Tags a UI slot as a hotbar slot, and which hotbar index it has.
///
/// This does not need to be the same as the slot it has in the player's
/// inventory, which is what will allow quick-swapping hotbars later.
#[derive(Component)]
pub struct HotbarSlot {
    hotbar_slot_index: usize,
}

fn build_hotbar_item_slot(source_entity: Entity, slot_index: usize) -> impl Bundle {
    (
        build_base_ui_item_slot(),
        HotbarSlot { hotbar_slot_index: slot_index },
        InventorySlot { source_entity, slot_index },
    )
}

fn build_hotbar_ui_panel(source_entity: Entity, capacity: usize) -> impl Bundle {
    (
        Node {
            display: Display::Flex,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_direction: FlexDirection::Row,
            border_radius: BorderRadius::all(UI_PANEL_RADIUS),
            border: UiRect::all(UI_BORDER_THICKN),
            padding: UiRect::all(UI_PANEL_PADDING),
            margin: UiRect::bottom(Val::Px(20.)),
            ..default()
        },
        BorderColor::all(UI_BORDER_COLOR),
        BackgroundColor(UI_PANEL_COLOR),
        Children::spawn(SpawnIter(
            (0..capacity).map(move |i| build_hotbar_item_slot(source_entity, i)),
        )),
    )
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – SPAWNING
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Presentation observing simulation: a hotbar appears in the world, so a
/// hotbar appears on screen. Nothing on the data side asks for this, which
/// is why `sim::player` can assemble a player's inventories without naming a
/// single UI symbol.
fn spawn_hotbar_ui_sys(
    mut commands: Commands,
    new_hotbars: Query<(Entity, &Inventory), Added<PlayerHotbar>>,
) {
    for (hotbar_entity, inventory_data) in new_hotbars.iter() {
        let game_ui_root = (
            Node {
                width: percent(100),
                height: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexEnd,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            ZIndex(0),
            EntityUISession { source_entities: vec![hotbar_entity] },
            children![build_hotbar_ui_panel(hotbar_entity, inventory_data.capacity())],
        );

        commands.spawn(game_ui_root);

        // Send a sync request for all non-empty slots.
        for (i, slot) in inventory_data.slots().iter().enumerate() {
            if slot.is_some() {
                commands.trigger(InventoryUISyncRequest {
                    entity: hotbar_entity,
                    index:  i,
                });
            }
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – HIGHLIGHT
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Poll every visible hotbar UI and update the highlight on any UI refresh.
///
/// The *contents* of the hotbar are kept in sync by
/// `ui::inventory::inventory_sync_obs`, since every hotbar slot also carries
/// an `InventorySlot`; this only owns the border.
fn sync_hotbar_highlight_obs(
    event: On<InventoryUISyncRequest>,
    player_hotbar_query: Query<&PlayerHotbar>,
    mut slot_query: Query<(&mut Node, &mut BorderColor, &HotbarSlot, &InventorySlot)>,
) {
    let entity = event.entity;

    // The inventory the event is about must still exist and be readable.
    let Ok(hotbar_data) = player_hotbar_query.get(entity) else { return };

    for (mut node, mut border_color, hotbar_slot_data, slot_data) in slot_query.iter_mut() {
        if slot_data.source_entity != entity { continue; }

        if hotbar_slot_data.hotbar_slot_index == hotbar_data.highlighted_slot {
            *border_color = BorderColor::all(UI_HL_BORDER_COLOR);
            node.border = UiRect::all(UI_HL_BORDER_THICKN);
        } else {
            *border_color = BorderColor::all(UI_BORDER_COLOR);
            node.border = UiRect::all(UI_BORDER_THICKN);
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct UiHotbarPlugin;

impl Plugin for UiHotbarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, spawn_hotbar_ui_sys)
            .add_observer(sync_hotbar_highlight_obs);
    }
}
