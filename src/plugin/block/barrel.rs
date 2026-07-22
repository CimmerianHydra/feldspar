use bevy::prelude::*;

use crate::plugin::block::behavior::{BlockEntitySpawner, BlockSpawnContext};
use crate::plugin::block::entities::{BlockEntityEvent, Interactable};
use crate::plugin::inventory::main::Inventory;

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
    inventories: Query<&Inventory>,
) {
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
}