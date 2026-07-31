//! One file per block that does something.
//!
//! Each behavior registers itself by name in `BlockBehaviorRegistry`, and a
//! block JSON opts in by listing that name in its `"behaviors"` array. There
//! is no central match statement, and adding a machine touches exactly two
//! files: a new one here, and one line in the plugin below.


pub mod barrel;
pub mod chute;
pub mod extractor;

use barrel::{BarrelSpawner};
use chute::{ChuteSpawner, ChutePlugin};
use extractor::{ExtractorSpawner, ExtractorPlugin};

use bevy::prelude::*;
use crate::{content::block::RegisterBlockBehaviorExtension};

pub struct BehaviorsPlugin;

impl Plugin for BehaviorsPlugin {
    fn build(&self, app: &mut App) {
        app

        .register_block_behavior("chute", ChuteSpawner::default())
        .add_plugins(ChutePlugin)

        .register_block_behavior("extractor", ExtractorSpawner::default())
        .add_plugins(ExtractorPlugin)
        
        .register_block_behavior("barrel", BarrelSpawner { slots: 27 })
        ;
    }
}
