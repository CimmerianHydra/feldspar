//! The app's states.
//!
//! A **leaf module**: it imports nothing from the rest of the crate, and any
//! layer may name it. That is what keeps `GameState` from being an upward
//! dependency when `render::sky` wants `OnEnter(GameState::InGame)`. The
//! enums are re-exported at the crate root, so call sites read
//! `use crate::GameState;` rather than reaching into `app`.

use avian3d::prelude::*;
use bevy::prelude::*;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – THE STATES
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Where the app is in its lifecycle.
#[derive(States, Debug, Clone, Eq, PartialEq, Hash, Default)]
pub enum GameState {
    #[default]
    AssetLoading,
    InGame,
}

/// Whether the world is ticking. Driven by the UI stack (a pause menu stops
/// it) rather than set by feature code.
#[derive(States, Debug, Clone, Eq, PartialEq, Hash, Default)]
pub enum GameUpdate {
    #[default]
    Disabled,
    Enabled,
}

#[derive(States, Debug, Clone, Eq, PartialEq, Hash, Default)]
pub enum GameMode {
    #[default]
    Creative,
    Survival,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – CONSEQUENCES
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn pause_physics_sys(mut time: ResMut<Time<Physics>>) {
    time.pause();
    debug!("Physics was paused.");
}

fn resume_physics_sys(mut time: ResMut<Time<Physics>>) {
    time.unpause();
    debug!("Physics was unpaused.");
}

/// Entering the game starts the world ticking.
///
/// The UI reconciler also re-enables this whenever a player's screen stack
/// empties out; this covers the very first frame, before any stack exists.
fn enable_game_update_sys(mut commands: Commands) {
    commands.set_state(GameUpdate::Enabled);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct StatePlugin;

impl Plugin for StatePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .init_state::<GameUpdate>()
            .init_state::<GameMode>()
            .add_systems(OnEnter(GameState::InGame), enable_game_update_sys)
            .add_systems(OnEnter(GameUpdate::Enabled), resume_physics_sys)
            .add_systems(OnEnter(GameUpdate::Disabled), pause_physics_sys);
    }
}
