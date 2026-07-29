//! # Layer 11 — the app
//!
//! Wiring. States, system sets, and the order the world is loaded in.
//!
//! May import: everything. Nothing may import `app::loading` — but
//! [`state`] and [`schedule`] are deliberate exceptions: they are leaf
//! modules with no crate-internal dependencies of their own, re-exported at
//! the crate root, and any layer may name them.


pub mod loading;
pub mod progress;
pub mod schedule;
pub mod state;

pub use progress::LoadReport;
pub use schedule::{BakeSet, ConsoleSet, GameplaySet, LoadSet};
pub use state::{GameMode, GameState, GameUpdate};

use bevy::prelude::*;

pub struct AppPlugin;

impl Plugin for AppPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            state::StatePlugin,
            schedule::SchedulePlugin,
            loading::LoadingPlugin,
        ));
    }
}
