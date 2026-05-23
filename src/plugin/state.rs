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
        .init_state::<GameUpdate>()
        .init_state::<GameMode>()

        .add_systems(OnEnter(GameUpdate::Enabled), resume_physics_sys)
        .add_systems(OnEnter(GameUpdate::Disabled), pause_physics_sys)

        .add_observer(open_pause_menu_action_obs)
        .add_observer(close_pause_menu_action_obs)
        ;
    }
}

// Toggles between pause and unpause.
fn open_pause_menu_action_obs(
    event: On<Start<OpenPauseMenu>>,
    mut commands: Commands,
) {
    commands.set_state(GameUpdate::Disabled);
}

fn close_pause_menu_action_obs(
    event: On<Start<ClosePauseMenu>>,
    mut commands: Commands,
) {
    commands.set_state(GameUpdate::Enabled);
}

fn pause_physics_sys(
    mut time: ResMut<Time<Physics>>,
) {
    time.pause();
    bevy::log::debug!("Physics was paused.")
}

fn resume_physics_sys(
    mut time: ResMut<Time<Physics>>,
) {
    time.unpause();
    bevy::log::debug!("Physics was unpaused.")
}



#[derive(States, Debug, Clone, Eq, PartialEq, Hash, Default)]
pub enum GameState {
    #[default]
    AssetLoading,
    InGame,
}

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