use bevy::prelude::*;

use crate::plugin::inventory::main::*;
use crate::plugin::inventory::item_registry::*;
use crate::plugin::controls::{MouseEvent, MouseAction};

pub const HOTBAR_CAPACITY: usize = 9;

/// Marks an entity as "the player's inventory".
/// Useful for distinguishing player from world containers in queries.
#[derive(Component)]
pub struct PlayerInventory;

/// Marker that identifies the player's hotbar. This is a shared resource
/// so that multiple systems (i.e. the UI) can poll from it.
#[derive(Resource, Default)]
pub struct PlayerHotbar {
    selected_slot_index: usize,
}

impl PlayerHotbar {
    pub fn new() -> Self {
        PlayerHotbar { selected_slot_index: 0 }
    }
}

#[derive(Event)]
pub struct PlayerHotbarSelectedChange {
    pub old_index: usize,
    pub new_index: usize,
}

/// Simple spawning of the player's inventory. For now, we spawn it empty and then add
/// items to it using the populate_player_inventory function.
pub fn spawn_player_inventory_sys(
    mut commands: Commands,
) {
    let mut new_inventory = Inventory::new(9);

    commands.spawn((
        PlayerInventory,
        new_inventory,
    ));

    commands.trigger(PlayerHotbarSelectedChange {
        old_index: 0,
        new_index: 0,
    });
}

/// Hardcoded function to spawn some items into the player's inventory.
/// Since I hardcoded a few blocks in the block registry, I'll add them here.
pub fn populate_player_inventory_once(
    mut commands: Commands,
    mut player_inventory_query: Query<(Entity, &mut Inventory), With<PlayerInventory>>, 
    item_registry: Res<ItemRegistry>,
) {
    if let Ok((entity, mut inventory)) = player_inventory_query.single_mut() {
        for id in 1..5 {
            let item_id = ItemID(id as u16);
            let result = inventory.insert(item_id, 5, &item_registry);

            bevy::log::info!("Added [{}]x{} to player inventory.", item_registry.get(item_id).name, 5);
            commands.trigger(InventoryChangedEvent {
                entity,
                index: id - 1,
                result,
            });
        };
    }
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