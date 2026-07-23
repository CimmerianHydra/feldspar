use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;

use crate::plugin::crafting::main::CurrentRecipe;
use crate::plugin::crafting::main::InputOf;
use crate::plugin::crafting::main::Machine;
use crate::plugin::crafting::spatial::InventoryMachine;
use crate::plugin::inventory::main::*;
use crate::plugin::inventory::cursor::*;

use crate::plugin::controller::player::{SelectItem, HotbarSelection};
use crate::plugin::controller::main::MouseScrollEvent;
use crate::plugin::controller::player::Player;
use crate::plugin::inventory::spatial::SpatialInventory;
use crate::plugin::ui::hotbar::HotbarUISpawnRequest;


const SPATIAL_CRAFTING_PANEL_WIDTH: f32 = 520.0;
const SPATIAL_CRAFTING_PANEL_HEIGHT: f32 = 260.0;

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

/// Marks an entity as "the player's hotbar". This hooks into the hotbar display 
/// and update system.
#[derive(Component)]
pub struct PlayerHotbar {
    pub highlighted_slot: usize,
}

/// Typed lookup for everything hanging off a player.
#[derive(Component, Clone, Copy, Debug)]
pub struct PlayerInventories {
    pub main:             Entity,
    pub hotbar:           Entity,
    pub cursor:           Entity,
    pub equipment:        Entity,
    pub crafting_machine: Entity,
    pub crafting_grid:    Entity,
}

/// SystemParam to access inventory information of players.
#[derive(SystemParam)]
pub struct PlayerInventoryAccess<'w, 's> {
    sets:        Query<'w, 's, &'static PlayerInventories>,
    inventories: Query<'w, 's, &'static Inventory>,
    spatial:     Query<'w, 's, &'static SpatialInventory>,
}

impl PlayerInventoryAccess<'_, '_> {
    pub fn get_from_player(&self, player: Entity) -> Option<&PlayerInventories> {
        self.sets.get(player).ok()
    }

    fn inventory(&self, entity: Entity) -> Option<&Inventory> {
        self.inventories.get(entity).ok().map(|inv| inv)
    }

    pub fn main(&self, player: Entity) -> Option<&Inventory> {
        self.inventory(self.get_from_player(player)?.main)
    }

    pub fn hotbar(&self, player: Entity) -> Option<&Inventory> {
        self.inventory(self.get_from_player(player)?.hotbar)
    }

    pub fn cursor(&self, player: Entity) -> Option<&Inventory> {
        self.inventory(self.get_from_player(player)?.cursor)
    }

    pub fn crafting_grid(&self, player: Entity) -> Option<&SpatialInventory> {
        let grid = self.get_from_player(player)?.crafting_grid;
        self.spatial.get(grid).ok().map(|s| s)
    }

    /// The entities any player-inventory screen is a view of. Feed this to
    /// `ScreenSpec::viewing`, then chain whatever the caller is showing on top.
    pub fn ui_sources(&self, player: Entity) -> Option<[Entity; 2]> {
        let set = self.get_from_player(player)?;
        Some([set.main, set.hotbar])
    }
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
    player_query: Query<Entity, Added<Player>>,
) {
    for new_player in player_query.iter() {
        let new_inventory: Entity = commands.spawn((
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


        // The player is given a special kind of machine entity, performing instant
        // spatial recipes with instant processing speed.
        // This is essentially Minecraft's grid crafting, but for Feldspar.

        let new_crafting_machine = commands.spawn((
            Machine,
            InventoryMachine,
            CurrentRecipe::default(),
        )).id();

        let new_crafting_spatial_inventory = commands.spawn((
            SpatialInventory::new(SPATIAL_CRAFTING_PANEL_WIDTH, SPATIAL_CRAFTING_PANEL_HEIGHT),
            InputOf { machine_entity : new_crafting_machine }
        )).id();

        commands.entity(new_player).add_children(&[
            new_inventory,
            new_hotbar,
            new_cursor_inventory,
            new_equipment,
            new_crafting_machine,
        ]);
        
        commands.entity(new_player).insert(PlayerInventories {
            main:             new_inventory,
            hotbar:           new_hotbar,
            cursor:           new_cursor_inventory,
            equipment:        new_equipment,
            crafting_machine: new_crafting_machine,
            crafting_grid:    new_crafting_spatial_inventory,
        });

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

