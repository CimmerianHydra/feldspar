//! Logistics mechanics for items, fluids and more.

pub mod mover;

use bevy::prelude::*;
use mover::ItemTransportPlugin;

pub struct TransportPlugin;

impl Plugin for TransportPlugin {
    fn build(&self, app: &mut App) {
        app
        .add_plugins(ItemTransportPlugin)
        ;
    }
}