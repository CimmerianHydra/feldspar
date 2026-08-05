use serde::Deserialize;

use crate::{content::block::{BlockBehaviors, behaviors::BlockBehavior}, sim::behaviors::blocks::face_targeted::FaceTargeted};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// CONFIGURABLE
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Declares that right-clicking this block changes its state, even though it
/// has no block entity to route the change to.
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

#[derive(Clone, Copy, Default, Debug, Deserialize)]
pub struct Configurable;

impl BlockBehavior for Configurable {
    const NAME: &'static str = "configurable";

    fn apply(self, into: &mut BlockBehaviors) {
        into.push(FaceTargeted);
        into.push(self);
    }
}