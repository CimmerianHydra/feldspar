use bevy::prelude::*;
use serde::Deserialize;

use crate::content::block::behaviors::{BlockBehavior, BlockSpawnContext};
use crate::sim::block_entity::Interactable;
use crate::sim::inventory::storage::Inventory;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// BARREL
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Marker, so systems that care specifically about barrels (hoppers,
/// filters, the future logistics network) can query for them.
#[derive(Component, Debug)]
pub struct Barrel;

/// Everything the barrel is, in one place.
///
/// This whole file is the modularity test, and it is now a much stronger
/// test than it used to be: the barrel declares `Inventory + Interactable`
/// and stops. It does not build a screen, does not know a screen exists,
/// and does not close one when it dies. `ui::inventory` observes
/// `BlockEntityEvent` filtered on `With<Inventory>`, so every machine that
/// holds items gets its UI for free and none of them can drift out of sync
/// with the others.
///
/// Nothing outside this file mentions barrels except the single
/// `.register_block_behavior::<BarrelBehavior>()` line and the string
/// `"barrel"` in `barrel.json`. Delete the file and those two references,
/// and the game still builds — and still opens inventories for everything
/// else.
///
/// Note there is no `fn build`: a barrel needs no systems of its own. The
/// default is a no-op, so the absence is the declaration.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BarrelBehavior {
    pub slots: usize,
}

impl Default for BarrelBehavior {
    fn default() -> Self {
        Self { slots: 27 }
    }
}

impl BlockBehavior for BarrelBehavior {
    const NAME: &'static str = "barrel";

    /// Touching `ctx` is what promotes the voxel to an entity. A behaviour
    /// that never calls into the context — `orientable`, say — leaves the
    /// block a bare voxel, which is why a wrenchable stair costs nothing.
    fn on_place(&self, ctx: &mut BlockSpawnContext) {
        // "Spawn an entity with the following bundle of components:"
        ctx.insert((Inventory::storage(self.slots), Interactable, Barrel));
    }

    fn on_break(&self, _commands: &mut Commands, _root: Entity) {
        // Screens viewing this barrel close themselves — see
        // `ui::inventory::close_screens_for_removed_inventory_obs`.
        // TODO: spill items on floor.
    }
}
