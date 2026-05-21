use bevy::prelude::*;
use avian3d::prelude::*;
use bevy_enhanced_input::prelude::*;

use crate::plugin::controller::player::{OpenPauseMenu, ClosePauseMenu};

pub struct StatePlugin;

impl Plugin for StatePlugin {
    fn build(&self, app: &mut App) {
        // Add systems related to UI here
        app

        .init_state::<GameState>()
        .init_state::<GameMode>()


        ;
    }
}

// Toggles between pause and unpause.
fn open_pause_menu_action_obs(
    event: On<Start<OpenPauseMenu>>,
    mut time: ResMut<Time<Physics>>,
    mut next_state: NextState<GameState>,
) {
    time.pause();
    next_state.set(GameState::Paused);
}

fn close_pause_menu_action_obs(
    event: On<Start<ClosePauseMenu>>,
    mut time: ResMut<Time<Physics>>,
    mut next_state: NextState<GameState>,
) {
    time.unpause();
    next_state.set(GameState::Running);
}

#[derive(States, Debug, Clone, Eq, PartialEq, Hash, Default)]
pub enum GameState {
    #[default]
    Loading,
    Running,
    Paused,
}

#[derive(States, Debug, Clone, Eq, PartialEq, Hash, Default)]
pub enum GameMode {
    #[default]
    Creative,
    Survival,
}