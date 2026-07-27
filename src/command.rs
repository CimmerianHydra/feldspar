//! # Layer 2 — the command system
//!
//! Registry, schema, parser, dispatcher and scrollback for slash commands.
//! Deliberately knows nothing about the game: no blocks, no items, no player.
//! That is what lets any feature above it own a command with one line —
//! `app.add_command(spec, handler)` — without the command system growing a
//! central match statement.
//!
//! The *player-facing* console (the on-screen panel, the typing buffer) is a
//! separate feature much higher up, in [`crate::console`]. Splitting them is
//! what keeps `sim` able to register `/give` without importing a UI.

pub mod args;
pub mod chat;
pub mod dispatch;
pub mod help;
pub mod log;
pub mod parser;
pub mod registry;
pub mod source;

pub use args::{ArgKind, ArgSpec, ArgValue, CommandContext};
pub use chat::{console_log_layer, CHAT_TARGET};
pub use log::{ConsoleLog, LogLine};
pub use registry::{
    CommandEntry, CommandID, CommandLevel, CommandRegistry, CommandSpec, ConsoleAppExt,
    ConsolePermissions,
};
pub use source::{
    CommandError, CommandResult, CommandSource, Feedback, PendingCommands, Severity, Submission,
};

use bevy::prelude::*;

use crate::command::chat::drain_chat_queue_sys;
use crate::command::dispatch::dispatch_commands_sys;
use crate::command::help::{help_cmd, help_command_spec};
use crate::command::source::has_pending_commands;
use crate::ConsoleSet;

pub struct CommandPlugin;

impl Plugin for CommandPlugin {
    fn build(&self, app: &mut App) {
        app
            // `add_command` does this too, so ordering against feature plugins
            // never matters.
            .init_resource::<CommandRegistry>()
            .init_resource::<PendingCommands>()
            .init_resource::<ConsolePermissions>()
            .init_resource::<ConsoleLog>()

            .add_command(help_command_spec(), help_cmd)

            // A line submitted this frame is parsed and executed before the
            // frame ends; `ConsoleSet::Present`, which the console screen
            // registers into, is configured to run after this. Nothing about a
            // command feels deferred.
            .add_systems(
                Update,
                (
                    dispatch_commands_sys.run_if(has_pending_commands),
                    drain_chat_queue_sys,
                )
                    .chain()
                    .in_set(ConsoleSet::Dispatch),
            );
    }
}
