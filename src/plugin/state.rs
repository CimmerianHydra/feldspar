use bevy::prelude::*;



pub struct StatePlugin;

impl Plugin for StatePlugin {
    fn build(&self, app: &mut App) {
        // Add systems related to UI here

        app
        .init_state::<GameUpdateState>()
        .add_systems(Update, toggle_pause_sys)
        ;
    }
}

// Toggles between pause and unpause.
fn toggle_pause_sys(
    input: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameUpdateState>>,
    state: Res<State<GameUpdateState>>,
) {
    if input.just_pressed(KeyCode::Escape) {
        match state.get() {
            GameUpdateState::Running => next_state.set(GameUpdateState::Paused),
            GameUpdateState::Paused => next_state.set(GameUpdateState::Running),
        }
    }
}

#[derive(States, Debug, Clone, Eq, PartialEq, Hash, Default)]
pub enum GameUpdateState {
    #[default]
    Running,
    Paused,
}