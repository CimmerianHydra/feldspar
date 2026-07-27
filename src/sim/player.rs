use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::content::item::ItemStack;
use crate::sim::crafting::machine::{CurrentRecipe, InputOf, Machine};
use crate::sim::crafting::spatial::InventoryMachine;
use crate::sim::inventory::cursor::CursorInventory;
use crate::sim::inventory::spatial::SpatialInventory;
use crate::sim::inventory::storage::{Inventory, InventoryChangedEvent};
use crate::space::BlockPos;
use crate::voxel::{Direction, Voxel};

const SPATIAL_CRAFTING_PANEL_WIDTH:  f32 = 520.0;
const SPATIAL_CRAFTING_PANEL_HEIGHT: f32 = 260.0;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – WHAT A PLAYER IS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Marks an entity as a player.
///
/// It lives in `sim`, not in `player`, and the split is deliberate: this is
/// *what a player is* — something with inventories, a hotbar, a held item —
/// while [`crate::player`] is *how a human drives one*, all camera, input
/// bindings and movement. Everything from `ui` downward can name this;
/// nothing below `player` needs to know a keyboard exists.
#[derive(Component)]
pub struct Player;

/// Marks an entity as "the player's inventory".
/// Useful for distinguishing player from world containers in queries.
#[derive(Component)]
pub struct PlayerInventory;

/// Marks an entity as "the player's hotbar". This hooks into the hotbar
/// display and update system.
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

#[derive(Resource, Default)]
pub struct PlayerHeldItems {
    pub right_hand: Option<ItemStack>,
}

// WIP: we need to start using this for block placement too instead of a
// global resource.
#[derive(Component)]
pub struct PlayerEquipment {
    pub right_hand: Option<ItemStack>,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – ACCESS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

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
        self.inventories.get(entity).ok()
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
        self.spatial.get(grid).ok()
    }

    /// The entities any player-inventory screen is a view of. Feed this to
    /// `UIPushOptions::viewing`, then chain whatever the caller is showing
    /// on top.
    pub fn ui_sources(&self, player: Entity) -> Option<[Entity; 2]> {
        let set = self.get_from_player(player)?;
        Some([set.main, set.hotbar])
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – ASSEMBLY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Hangs a full set of inventories off any freshly spawned player.
///
/// Reacts to `Added<Player>` rather than being called by whatever spawned
/// the player, so the controller never has to know inventories exist — and
/// the console hangs its own buffer off the player the same way.
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

        // The player is given a special kind of machine entity, performing
        // instant spatial recipes with instant processing speed. This is
        // essentially Minecraft's grid crafting, but for Feldspar.
        let new_crafting_machine = commands.spawn((
            Machine,
            InventoryMachine,
            CurrentRecipe::default(),
        )).id();

        let new_crafting_spatial_inventory = commands.spawn((
            SpatialInventory::new(SPATIAL_CRAFTING_PANEL_WIDTH, SPATIAL_CRAFTING_PANEL_HEIGHT),
            InputOf { machine_entity: new_crafting_machine },
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
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 4 – HOTBAR SELECTION
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// "Move the hotbar highlight."
///
/// Bindings live in [`crate::player::input`]; what a selection *means* lives
/// here. That inversion is why `sim` never has to name a scroll wheel or a
/// number key.
#[derive(Event, Clone, Copy, PartialEq, Eq, Debug)]
pub enum HotbarSelect {
    Index(usize),
    Next,
    Previous,
}

pub fn hotbar_select_obs(
    event: On<HotbarSelect>,
    mut commands: Commands,
    mut hotbar_q: Query<(Entity, &mut PlayerHotbar, &Inventory)>,
) {
    let Ok((hotbar_entity, mut hotbar_data, inventory_data)) = hotbar_q.single_mut() else { return };

    let capacity = inventory_data.capacity();
    if capacity == 0 { return; }

    let old_index = hotbar_data.highlighted_slot;
    let new_index = match *event.event() {
        HotbarSelect::Index(i)  => i % capacity,
        HotbarSelect::Next      => (old_index + 1) % capacity,
        HotbarSelect::Previous  => (old_index + capacity - 1) % capacity,
    };

    hotbar_data.highlighted_slot = new_index;

    // Both slots redraw: the one gaining the highlight and the one losing it.
    commands.trigger(InventoryChangedEvent { entity: hotbar_entity, index: new_index });
    commands.trigger(InventoryChangedEvent { entity: hotbar_entity, index: old_index });
}

/// Keeps the held-item mirrors in step with whatever the hotbar is showing.
///
/// Attached per-hotbar-entity at spawn, so it only ever fires for the hotbar
/// that actually changed.
pub fn on_hotbar_changed(
    event: On<InventoryChangedEvent>,
    mut held_items: ResMut<PlayerHeldItems>,
    hotbar_inventory_query: Query<(&PlayerHotbar, &Inventory)>,
    mut player_equipment: Query<&mut PlayerEquipment>,
) {
    let Ok((hotbar_data, inventory_data)) = hotbar_inventory_query.get(event.entity) else { return };

    let currently_highlighted_slot = hotbar_data.highlighted_slot;
    held_items.right_hand = inventory_data.slots()[currently_highlighted_slot];

    let Ok(mut equipment_data) = player_equipment.single_mut() else { return };
    equipment_data.right_hand = inventory_data.slots()[currently_highlighted_slot];
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 5 – LOOK TARGET
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// What the player is currently pointing at.
///
/// One variant for voxels, whatever space they're in — `BlockPos` carries
/// the terrain-versus-ship distinction, and no consumer downstream has to
/// branch on it.
///
/// The *result* of the raycast lives here rather than in `player` because
/// two very different consumers read it: block placement (input) and the
/// highlight box (presentation). Presentation may not import input, so the
/// answer sits below both and each reads it without knowing about the other.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LookTarget {
    Block { at: BlockPos, voxel: Voxel, face: Direction },
    Mob   { entity: Entity },
}

#[derive(Resource, Default)]
pub struct PlayerLookTarget {
    pub target: Option<LookTarget>,
}

/// Fired when the look target changes, for tooltips and anything else that
/// shouldn't re-raycast.
#[derive(Event, Clone, Copy, Debug)]
pub struct LookTargetChanged(pub Option<LookTarget>);

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct PlayerStatePlugin;

impl Plugin for PlayerStatePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerHeldItems>()
            .init_resource::<PlayerLookTarget>()
            .add_systems(Update, append_player_inventory_sys)
            .add_observer(hotbar_select_obs);
    }
}
