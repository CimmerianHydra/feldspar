use serde::Deserialize;

use crate::content::block::behaviors::BlockBehavior;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// INTERACTS ON SECONDARY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Declares that right-clicking this block *does something*, even though it
/// has no block entity to route the click to.
///
/// `space::write` has named this type in a doc comment since the day
/// `BlockEvent::Interact` was added; this is the type, and the rung in
/// `player::interaction` that fires the event is the other half.
///
/// ## Why it cannot just be `Interactable`
///
/// [`crate::sim::block_entity::Interactable`] is a *component*, so wearing
/// it means being an entity. That is the right shape for a barrel and the
/// wrong one for a pipe: promoting every pipe voxel to an entity purely so
/// a right-click can find it would cost an entity and a
/// `ChunkBlockEntities` slot per cell, for a click that happens once in the
/// lifetime of that block. Declaring the capability on the *definition*
/// keeps the voxel a voxel.
///
/// Handlers observe `BlockEvent::Interact` and filter on the block's own
/// behaviours, exactly as `ui::inventory` observes `BlockEntityEvent` and
/// filters on `Inventory`. Nothing here knows who is listening.
#[derive(Clone, Copy, Default, Debug, Deserialize)]
pub struct InteractsOnSecondary;

impl BlockBehavior for InteractsOnSecondary {
    const NAME: &'static str = "interacts_on_secondary";
}