//! # Layer 9 — the player
//!
//! How a human drives a `Player`: input bindings, the camera rig, movement,
//! and turning a look direction into place/break intent.
//!
//! May import: everything below, including [`crate::ui`].
//!
//! What a player *is* — inventories, held item, look target — lives in
//! [`crate::sim::player`], one layer down, so that `ui` can read it without
//! ever seeing a keybinding. This module is the translation layer between a
//! keyboard and everything beneath it.

pub mod camera;
pub mod input;
pub mod interaction;
pub mod movement;

pub use camera::FPSCamera;
pub use input::{
    ConsoleToggle, GameInput, HotbarSelection, MouseScrollEvent, PrimaryFire, SecondaryFire, UiInput,
};
pub use interaction::DDARay;

use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;

use crate::player::camera::build_player_camera;
use crate::player::input::{build_game_input_actions, build_ui_input_actions, GameInput as GameCtx, UiInput as UiCtx};
use crate::player::movement::{build_player_body, CAM_LOCAL_Y};
use crate::sim::player::Player;
use crate::ui::screen::UiStack;
use crate::GameState;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SPAWNING
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Assembles a player out of the bundles each submodule contributes.
///
/// Note what it does *not* do: no `.observe(...)` calls, because every
/// submodule registers its own global observers, and no inventory setup,
/// because `sim::player` reacts to `Added<Player>` on its own. Adding a
/// capability to the player never edits this function.
fn spawn_player_sys(mut commands: Commands) {
    commands.spawn((
        Player,
        Name::new("Player"),
        InheritedVisibility::default(),
        Transform::from_xyz(0.0, 20.0, 0.0),
        build_player_body(),
        // Defaults to locking the cursor on startup.
        UiStack::default(),
        build_game_input_actions(),
        build_ui_input_actions(),
        ContextActivity::<GameCtx>::ACTIVE,
        ContextActivity::<UiCtx>::INACTIVE,
        children![build_player_camera(CAM_LOCAL_Y)],
    ));
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            input::PlayerInputPlugin,
            camera::PlayerCameraPlugin,
            movement::PlayerMovementPlugin,
            interaction::PlayerInteractionPlugin,
        ))
        .add_systems(OnEnter(GameState::InGame), spawn_player_sys);
    }
}
