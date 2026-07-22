use bevy::prelude::*;
use bevy_enhanced_input::context::ContextActivity;

use crate::plugin::block::behavior::{BlockEntitySpawner, BlockSpawnContext};
use crate::plugin::block::entities::{BlockEntityEvent, Interactable};
use crate::plugin::inventory::main::Inventory;

use crate::plugin::inventory::player::{PlayerHotbar, PlayerInventory};
use crate::plugin::ui::cursor::CursorLockRequest;
use crate::plugin::ui::player::{MAX_PLAYER_INVENTORY_UI_COLS, build_player_ui_with_top_panel};
use crate::plugin::ui::inventory::build_inventory_ui;
use crate::plugin::controller::player::{GameInput, PauseMenuInput};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// BARREL
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Everything the barrel is, in one place.
///
/// This whole file is the modularity test: nothing outside it mentions
/// barrels except the single `register_block_behavior("barrel", ...)` line
/// and the string `"barrel"` in `barrel.json`. Delete the file and the two
/// references, and the game still builds.
#[derive(Clone, Copy, Debug)]
pub struct BarrelSpawner {
    pub slots: usize,
}

impl Default for BarrelSpawner {
    fn default() -> Self { Self { slots: 27 } }
}

impl BlockEntitySpawner for BarrelSpawner {
    fn spawn(&self, ctx: &mut BlockSpawnContext) {
        ctx.insert((
            Inventory::new(self.slots),
            Interactable,
            Barrel,
        ));

        // The handler ships with the behavior. No central match statement
        // ever learns that barrels exist.
        ctx.observe(on_barrel_interact);
    }

    fn despawn(&self, _commands: &mut Commands, _root: Entity) {
        // TODO: spill contents onto the floor once item entities exist.
    }
}

/// Marker, so systems that care specifically about barrels (hoppers,
/// filters, the future logistics network) can query for them.
#[derive(Component, Debug)]
pub struct Barrel;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// INTERACTION
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Attached per-entity by `BarrelSpawner`, so it only ever fires for the
/// barrel that was actually clicked.
fn on_barrel_interact(
    event: On<BlockEntityEvent>,
    mut commands: Commands,
    inventories: Query<&Inventory>,
    hotbars: Query<(Entity, &Inventory), (With<PlayerHotbar>, Without<PlayerInventory>)>,
    player_invs: Query<(Entity, &Inventory), (With<PlayerInventory>, Without<PlayerHotbar>)>,
) {

    // TEMPORARY: for now there's only one player out there, but we need to have functions that
    // retrieve specifically the player's inventory info
    let Ok(hotbar_data) = hotbars.single() else { return; };
    let Ok(player_data) = player_invs.single() else { return; };

    let barrel = event.entity;

    let capacity = inventories
        .get(barrel)
        .map(|inv| inv.capacity())
        .unwrap_or(0);

    info!(
        "Barrel {barrel} used at {} (face {:?}) by player {} — {capacity} slots",
        event.at.pos, event.face, event.player,
    );

    // TODO: Spawn UI session on right click.
    let top_panel = build_inventory_ui(event.entity, capacity, MAX_PLAYER_INVENTORY_UI_COLS);
    commands.spawn(build_player_ui_with_top_panel(hotbar_data, player_data, top_panel, Some(event.entity)));
    commands.trigger(CursorLockRequest::Unlock);
    commands.entity(event.player).insert((ContextActivity::<GameInput>::INACTIVE, ContextActivity::<PauseMenuInput>::ACTIVE));
}