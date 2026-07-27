//! # Layer 10 — the in-game console
//!
//! The screen you type into, and the buffer it types into.
//!
//! May import: everything below, including [`crate::ui`] and
//! [`crate::player`].
//!
//! The command *system* — registry, parser, dispatcher, scrollback — is
//! [`crate::command`], right at the bottom of the stack. That split is what
//! lets `sim::inventory` own `/give` with one `add_command` call while the
//! panel that displays its output lives up here next to the UI it draws on.

pub mod screen;
pub mod session;

pub use screen::ConsoleScreen;
pub use session::{ConsoleBuffer, ConsoleHistory};

use bevy::prelude::*;

use crate::ConsoleSet;

pub struct ConsolePlugin;

impl Plugin for ConsolePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, session::append_console_session_sys)
            .add_observer(screen::open_console_obs)
            // Key ingestion happens before the dispatcher runs, and the
            // views refresh after it, so a line submitted this frame is
            // executed and rendered before the frame ends. The ordering is
            // configured once in `app::schedule`; neither this plugin nor
            // `command` names a system belonging to the other.
            .add_systems(
                Update,
                session::ingest_console_keys_sys.in_set(ConsoleSet::Input),
            )
            .add_systems(
                Update,
                (
                    screen::refresh_console_log_sys,
                    screen::refresh_console_input_sys,
                )
                    .chain()
                    .in_set(ConsoleSet::Present),
            );
    }
}
