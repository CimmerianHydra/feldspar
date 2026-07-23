use bevy::prelude::*;

use crate::plugin::block::behavior::{BlockEntitySpawner, BlockSpawnContext};
use crate::plugin::block::entities::{BlockEntityEvent, Interactable};
use crate::plugin::inventory::main::Inventory;

use crate::plugin::inventory::player:: PlayerInventoryAccess;
use crate::plugin::ui::player::{MAX_PLAYER_INVENTORY_UI_COLS, build_player_ui_with_top_panel};
use crate::plugin::ui::inventory::{EntityUISessionEndRequest, build_inventory_ui};
use crate::plugin::ui::screen::{UiPushOptions, UiScreenCommandsExt};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// BARREL
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Marker, so systems that care specifically about barrels (hoppers,
/// filters, the future logistics network) can query for them.
#[derive(Component, Debug)]
pub struct Barrel;

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

    fn despawn(&self, commands: &mut Commands, root: Entity) {
        commands.trigger(
            EntityUISessionEndRequest {
                context: root,
                source_entity: root,
            }
        );
        // TODO: spill items on floor.
    }
}


// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// INTERACTION
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Attached per-entity by `BarrelSpawner`, so it only ever fires for the
/// barrel that was actually clicked.
fn on_barrel_interact(
    event: On<BlockEntityEvent>,
    mut commands: Commands,
    inventories: Query<&Inventory>,
    players: PlayerInventoryAccess,
) {
    let (barrel, player) = (event.entity, event.player);

    let Ok(capacity) = inventories.get(barrel).map(|inv| inv.capacity()) else { return };
    let Some(sources) = players.ui_sources(player) else { return };
    let Some(ui) = build_player_ui_with_top_panel(
        player,
        &players,
        build_inventory_ui(barrel, capacity, MAX_PLAYER_INVENTORY_UI_COLS),
    ) else { return };

    commands.push_ui_screen(
        player,
        UiPushOptions::new().viewing(sources).viewing([barrel]),
        ui,
    );
}