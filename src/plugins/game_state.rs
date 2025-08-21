use bevy::prelude::*;
use bevy::window::{CursorGrabMode};

#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum GameState {
    #[default]
    Playing,
    Paused,
}

pub struct PausePlugin;

impl Plugin for PausePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            // Input: toggle on Esc
            .add_systems(Update, toggle_pause)
            // Enter/exit hooks
            .add_systems(OnEnter(GameState::Paused), free_mouse)
            .add_systems(OnExit(GameState::Paused), capture_mouse);
    }
}

/// Press Esc to toggle pause.
fn toggle_pause(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        next.set(match state.get() {
            GameState::Playing => GameState::Paused,
            GameState::Paused  => GameState::Playing,
        });
    }
}

// === Mouse capture helpers ===
// Lock & hide while playing
fn capture_mouse(mut q_window: Query<&mut Window>) {
    if let Ok(mut window) = q_window.single_mut() {
        window.cursor_options.visible = false;
        // Bevy will pick a supported mode per platform if locked/ confined is unavailable.
        window.cursor_options.grab_mode = CursorGrabMode::Locked;
    }
}

// Free & show while paused
fn free_mouse(mut q_window: Query<&mut Window>) {
    if let Ok(mut window) = q_window.single_mut() {
        window.cursor_options.visible = true;
        window.cursor_options.grab_mode = CursorGrabMode::None;
    }
}