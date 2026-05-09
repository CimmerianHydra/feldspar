use bevy::prelude::*;

use crate::plugin::inventory::main::*;
use crate::plugin::controls::{MouseEvent, MouseAction};

pub const HOTBAR_CAPACITY: usize = 9;

/// Marks an entity as "the player's inventory".
/// Useful for distinguishing player from world containers in queries.
#[derive(Component)]
pub struct PlayerInventory;

#[derive(Resource, Default)]
pub struct PlayerHotbar {
    selected_slot_index: usize,
    capacity: usize,
}

impl PlayerHotbar {
    pub fn new() -> Self {
        PlayerHotbar { selected_slot_index: 0, capacity: HOTBAR_CAPACITY }
    }
}

#[derive(Event)]
pub struct PlayerHotbarSelectedChange {
    pub old_index: usize,
    pub new_index: usize,
}

pub fn spawn_player_inventory(
    mut commands: Commands,
    item_registry: Res<ItemRegistry>,
) {
    let mut new_inventory = Inventory::new(9);

    for id in 0..3 {
        new_inventory.insert(ItemID(id as u16), 1, &item_registry);
    };

    commands.spawn((
        new_inventory,
        PlayerInventory,
    ));
}


/// Updates the hotbar resource globally, which allows the system to sync both UI
/// and player held items.
pub fn update_hotbar_obs(
    event: On<MouseEvent>,
    mut commands: Commands,
    mut hotbar: ResMut<PlayerHotbar>,
) {
    if event.action == MouseAction::ScrollDown {
        let old_index = hotbar.selected_slot_index;
        let new_index = (old_index + 1) % HOTBAR_CAPACITY;
        commands.trigger(PlayerHotbarSelectedChange {
            old_index,
            new_index,
        });
        hotbar.selected_slot_index = new_index;
    } else if event.action == MouseAction::ScrollUp {
        let old_index = hotbar.selected_slot_index;
        let new_index = (old_index + HOTBAR_CAPACITY - 1) % HOTBAR_CAPACITY;
        commands.trigger(PlayerHotbarSelectedChange {
            old_index,
            new_index,
        });
        hotbar.selected_slot_index = new_index;
    };
}