use bevy::prelude::*;

pub struct StatePlugin;

impl Plugin for StatePlugin {
    fn build(&self, app: &mut App) {
        // Add systems related to UI here
        app

        .init_state::<GameUpdateState>()
        .init_state::<UIState>()
        .init_state::<GameMode>()
        
        .add_systems(Update, toggle_pause_sys)

        ;
    }
}

// Toggles between pause and unpause.
fn toggle_pause_sys(
    input: Res<ButtonInput<KeyCode>>,
    game_state: Res<State<GameUpdateState>>,
    mut next_game_state: ResMut<NextState<GameUpdateState>>,
    ui_state: Res<State<UIState>>,
    mut next_ui_state: ResMut<NextState<UIState>>,
) {
    if input.just_pressed(KeyCode::Escape) {
        match game_state.get() {
            GameUpdateState::Running => next_game_state.set(GameUpdateState::Paused),
            GameUpdateState::Paused => next_game_state.set(GameUpdateState::Running),
            _ => return,
        }
        match ui_state.get() {
            UIState::Game => next_ui_state.set(UIState::Menu),
            UIState::Menu => next_ui_state.set(UIState::Game),
        }
    }
}

#[derive(States, Debug, Clone, Eq, PartialEq, Hash, Default)]
pub enum GameUpdateState {
    #[default]
    Loading,
    Running,
    Paused,
}

#[derive(States, Debug, Clone, Eq, PartialEq, Hash, Default)]
pub enum UIState {
    #[default]
    Game,
    Menu,
}

#[derive(States, Debug, Clone, Eq, PartialEq, Hash, Default)]
pub enum GameMode {
    #[default]
    Creative,
    Survival,
}