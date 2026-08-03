use bevy::prelude::*;
use serde::Deserialize;

use crate::content::item::behaviors::{ItemBehavior, ItemBehaviors, ItemLoadContext};
use crate::voxel::BlockID;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLACES BLOCK
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// This item, used against a face, writes a block into the world.
///
/// ## The one behaviour that has real resolution work to do
///
/// JSON names a block; the game runs on `BlockID`s. Names are the stable
/// currency — they survive save files, mod load order and a block being
/// added in the middle of the alphabet — while IDs are a runtime detail
/// assigned by registration order. So the name is what's authored, and the
/// ID is filled in here, once, at load.
///
/// This is also the whole reason item definitions load *after* every block
/// has been registered, and the reason [`ItemLoadContext`] exists at all.
///
/// ```json
/// { "places_block": { "block": "slate" } }
/// ```
///
/// ## Why one struct and not two
///
/// A separate `PlacesBlockConfig` → `PlacesBlock` pair would be tidier in
/// the abstract and worse in practice: the bag would hold a type that no
/// JSON name maps to, and `get::<PlacesBlock>()` would silently miss for
/// anyone who reached for the config type by mistake. One struct with a
/// skipped field keeps the authored name available for diagnostics and
/// makes the resolved-vs-unresolved distinction visible in the type.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PlacesBlock {
    /// Block name as authored. Kept after resolution so a log line can say
    /// what was asked for, not just what it became.
    pub block: String,
    /// Filled in by `apply`. `None` never survives resolution — a behaviour
    /// that fails to resolve drops itself rather than entering the bag half
    /// built.
    #[serde(skip)]
    pub block_id: Option<BlockID>,
}

impl PlacesBlock {
    /// Construct one already resolved, for items generated in Rust rather
    /// than authored in JSON — the block→item pass in `render::icons`.
    pub fn resolved(name: impl Into<String>, block_id: BlockID) -> Self {
        Self { block: name.into(), block_id: Some(block_id) }
    }

    /// The block this item places. Always `Some` for anything that made it
    /// into a bag.
    pub fn block_id(&self) -> Option<BlockID> {
        self.block_id
    }
}

impl ItemBehavior for PlacesBlock {
    const NAME: &'static str = "places_block";

    fn apply(mut self, ctx: ItemLoadContext<'_>, into: &mut ItemBehaviors) {
        let Some(block_id) = ctx.blocks.by_name(self.block.clone()) else {
            warn!(
                "item '{}': places_block names unknown block '{}' — behavior \
                 skipped, so this item will place nothing.",
                ctx.item_name, self.block,
            );
            return;
        };

        self.block_id = Some(block_id);
        into.push(self);
    }
}
