//! Logistics mechanics for items, fluids and more.


use bevy::prelude::*;
use mover::ItemTransportPlugin;

pub mod mover;
pub mod network;
pub struct TransportPlugin;

impl Plugin for TransportPlugin {
    fn build(&self, app: &mut App) {
        app
        .add_plugins(ItemTransportPlugin)
        ;
    }
}
