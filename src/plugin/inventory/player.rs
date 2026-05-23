use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;

use crate::plugin::controller::player::{SelectItem, HotbarSelection};

use crate::plugin::inventory::main::*;
use crate::plugin::inventory::cursor::*;

use crate::plugin::controller::main::MouseScrollEvent;
use crate::plugin::controller::player::Player;
use crate::plugin::ui::hotbar::HotbarUISpawnRequest;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// HOTBAR EVENTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Updates the hotbar resource globally, which allows the system to sync both UI
/// and player held items.
pub fn sync_hotbar_on_mouse_scroll_obs(
    event: On<MouseScrollEvent>,
    mut commands: Commands,
    mut hotbar_q: Query<(Entity, &mut PlayerHotbar, &Inventory)>,
) {
    let Ok((hotbar_entity, mut hotbar_data, inventory_data)) = hotbar_q.single_mut() else { return; };

    let capacity = inventory_data.capacity();
    let old_index = hotbar_data.highlighted_slot;
    let new_index = match *event.event() {
        MouseScrollEvent::ScrollDown => (old_index + 1) % capacity,
        MouseScrollEvent::ScrollUp => (old_index + capacity - 1) % capacity,
    };

    hotbar_data.highlighted_slot = new_index;

    commands.trigger(InventoryChangedEvent {
        entity: hotbar_entity,
        index: new_index,
    });
    commands.trigger(InventoryChangedEvent {
        entity: hotbar_entity,
        index: old_index,
    });
}

/// Updates the hotbar resource globally, which allows the system to sync both UI
/// and player held items.
pub fn sync_hotbar_on_input_action_obs(
    event: On<Start<SelectItem>>,
    action_data_query: Query<&HotbarSelection>,
    mut commands: Commands,
    mut hotbar_q: Query<(Entity, &mut PlayerHotbar, &Inventory)>,
) {
    let Ok((hotbar_entity, mut hotbar_data, inventory_data)) = hotbar_q.single_mut() else { return; };
    let Ok(action_data) = action_data_query.get(event.action) else { return; };

    let capacity = inventory_data.capacity();
    let old_index = hotbar_data.highlighted_slot;
    let new_index = action_data.index % capacity;

    hotbar_data.highlighted_slot = new_index;

    commands.trigger(InventoryChangedEvent {
        entity: hotbar_entity,
        index: new_index,
    });
    commands.trigger(InventoryChangedEvent {
        entity: hotbar_entity,
        index: old_index,
    });
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Player Inventory
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Marks an entity as "the player's inventory".
/// Useful for distinguishing player from world containers in queries.
#[derive(Component)]
pub struct PlayerInventory;

/// Marks an entity as "the player's hotbar".
/// This hooks into the hotbar display and update system (TODO).
/// In the future, the UI hooks will be changed, as the system will be able to display ANY
/// inventory as the player's UI hotbar.
#[derive(Component)]
pub struct PlayerHotbar {
    pub highlighted_slot: usize,
}



// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SYSTEMS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Simple spawning of the player's inventory. For now, we spawn it empty and then add
/// items to it using the populate_player_inventory function.
/// In the future, these entities will have to become children of a player so we can start
/// to distinguish players in a multiplayer world.
pub fn append_player_inventory_sys(
    mut commands: Commands,
    mut player_query: Query<Entity, Added<Player>>,
) {
    for new_player in player_query.iter() {
        let new_inventory = commands.spawn((
                PlayerInventory,
                Inventory::new(27),
        )).id();

        let new_hotbar = commands.spawn((
            PlayerHotbar { highlighted_slot: 0 },
            Inventory::new(9),
        )).observe(on_hotbar_changed).id();

        let new_cursor_inventory = commands.spawn((
            CursorInventory,
            Inventory::new(1),
        )).id();

        let new_equipment = commands.spawn((
            PlayerEquipment { right_hand: None },
        )).id();

        commands.entity(new_player).add_children(&[
            new_inventory,
            new_hotbar,
            new_cursor_inventory,
            new_equipment
        ]);

        commands.trigger(HotbarUISpawnRequest {
        source_entity: new_hotbar,
        });
    }
}



// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Player Held Item / Player Equipment (WIP)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Resource, Default)]
pub struct PlayerHeldItems{
    pub right_hand: Option<ItemStack>,
}

// WIP: we need to start using this for block placement too instead of a global resource
#[derive(Component)]
pub struct PlayerEquipment{
    pub right_hand: Option<ItemStack>,
}

pub fn on_hotbar_changed(
    event: On<InventoryChangedEvent>,
    mut held_items: ResMut<PlayerHeldItems>,
    hotbar_inventory_query: Query<(&PlayerHotbar, &Inventory)>,
    mut player_equipment: Query<&mut PlayerEquipment>,
) {
    let Ok((hotbar_data, inventory_data)) = hotbar_inventory_query.get(event.entity) else { return; };

    let currently_highlighted_slot = hotbar_data.highlighted_slot;
    held_items.right_hand = inventory_data.slots()[currently_highlighted_slot];

    let Ok(mut equipment_data) = player_equipment.single_mut() else { return; };
    equipment_data.right_hand = inventory_data.slots()[currently_highlighted_slot];
}

