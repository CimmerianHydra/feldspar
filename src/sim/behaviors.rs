//! One file per thing that does something, and one line per file here.
//!
//! A behaviour file owns everything about itself: its JSON config struct,
//! its runtime data, its lifecycle hooks, and — via `fn build` — any systems
//! or observers it needs. Nothing outside it knows it exists except the
//! registration line below and the name in a JSON definition.
//!
//! Which means the deletion test is exact: remove the file, remove its line
//! here, and it is gone. No orphaned systems, no central match to prune, no
//! field left behind in a struct somewhere in `content`.

pub mod blocks;
pub mod items;

use bevy::prelude::*;

use crate::content::block::behaviors::RegisterBlockBehaviorExtension;
use crate::content::item::behaviors::RegisterItemBehaviorExtension;

use blocks::barrel::BarrelBehavior;
use blocks::chute::ChuteBehavior;
use blocks::extractor::ExtractorBehavior;
use blocks::face_targeted::FaceTargeted;
use blocks::orientable::Orientable;
use blocks::interacts::InteractsOnSecondary;
use blocks::pipe::ItemPipeBehavior;

use items::durability::Durability;
use items::fuel::Fuel;
use items::orients_blocks::OrientsBlocks;
use items::places_block::PlacesBlock;

pub struct BehaviorsPlugin;

impl Plugin for BehaviorsPlugin {
    fn build(&self, app: &mut App) {
        app
            // ── blocks ───────────────────────────────────────────────────
            .register_block_behavior::<FaceTargeted>()
            .register_block_behavior::<Orientable>()
            .register_block_behavior::<InteractsOnSecondary>()
            .register_block_behavior::<BarrelBehavior>()
            .register_block_behavior::<ChuteBehavior>()
            .register_block_behavior::<ExtractorBehavior>()
            .register_block_behavior::<ItemPipeBehavior>()
            // ── items ────────────────────────────────────────────────────
            .register_item_behavior::<PlacesBlock>()
            .register_item_behavior::<OrientsBlocks>()
            .register_item_behavior::<Fuel>()
            .register_item_behavior::<Durability>();
    }
}
