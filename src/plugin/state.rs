use bevy::prelude::*;

pub struct StatePlugin;

impl Plugin for StatePlugin {
    fn build(&self, app: &mut App) {
        // Add systems related to UI here
        app

        .init_state::<GameUpdateState>()
        .init_state::<UIState>()
        .init_state::<GameMode>()
        
        .add_systems(Update, toggle_state_sys)

        ;
    }
}

// Toggles between pause and unpause.
fn toggle_state_sys(
    input: Res<ButtonInput<KeyCode>>,
    game_state: Res<State<GameUpdateState>>,
    mut next_game_state: ResMut<NextState<GameUpdateState>>,
    ui_state: Res<State<UIState>>,
    mut next_ui_state: ResMut<NextState<UIState>>,
) {
    if input.just_pressed(KeyCode::Escape) {
        match ui_state.get() {
            UIState::Game => {
                next_ui_state.set(UIState::PauseMenu);
                next_game_state.set(GameUpdateState::Paused);
            },
            _ => {
                next_ui_state.set(UIState::Game);
                next_game_state.set(GameUpdateState::Running);
            },
        }
    }

    if input.just_pressed(KeyCode::KeyI) {
        match ui_state.get() {
            UIState::Game => next_ui_state.set(UIState::Inventory),
            UIState::Inventory => next_ui_state.set(UIState::Game),
            UIState::PauseMenu => {},
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
    PauseMenu,
    Inventory,
}

#[derive(States, Debug, Clone, Eq, PartialEq, Hash, Default)]
pub enum GameMode {
    #[default]
    Creative,
    Survival,
}