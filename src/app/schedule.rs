//! Named system sets, and the one place their order is decided.
//!
//! A **leaf module**: it imports only `app::state`, and any layer may name
//! it. The sets are re-exported at the crate root.
//!
//! ## Why this exists
//!
//! Two systems in different features often have to run in a particular
//! order — the mesher before the collider builder, the raycast before the
//! highlight, key ingestion before command dispatch. Expressing that with
//! `.after(other_feature::some_system)` makes one feature import another's
//! internals for no reason other than scheduling.
//!
//! Instead each plugin declares which *set* it belongs to, and the order
//! between sets is configured exactly once, here. `render` never names a
//! `physics` system; `console` never names a `command` system.
//!
//! The same trick handles state gating: `in_state(GameState::InGame)` is
//! applied to `GameplaySet` and `BakeSet` once, rather than repeated in
//! every plugin that happens to need it.

use bevy::prelude::*;

use crate::app::state::GameState;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// THE SETS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The in-game frame, in three beats. Gated on `GameState::InGame`.
#[derive(SystemSet, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum GameplaySet {
    /// Sampling the world on the player's behalf: raycasts, intent.
    Input,
    /// Gameplay reacting to that intent.
    Simulate,
    /// Views catching up with whatever just happened.
    Present,
}

/// Rebuilding the derived representations of a chunk. Gated on
/// `GameState::InGame` and ordered after gameplay.
///
/// `Collide` runs after `Mesh` because a static chunk's collider is a
/// trimesh taken from the render mesh.
#[derive(SystemSet, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum BakeSet {
    Mesh,
    Collide,
}

/// A console line submitted this frame is executed and rendered before the
/// frame ends. Ungated: the console works during loading too.
#[derive(SystemSet, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ConsoleSet {
    /// Keystrokes into the buffer.
    Input,
    /// The dispatcher and the chat-queue drain.
    Dispatch,
    /// The views that show the result.
    Present,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct SchedulePlugin;

impl Plugin for SchedulePlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            Update,
            (
                GameplaySet::Input,
                GameplaySet::Simulate,
                GameplaySet::Present,
                BakeSet::Mesh,
                BakeSet::Collide,
            )
                .chain()
                .run_if(in_state(GameState::InGame)),
        )
        .configure_sets(
            Update,
            (ConsoleSet::Input, ConsoleSet::Dispatch, ConsoleSet::Present).chain(),
        );
    }
}
