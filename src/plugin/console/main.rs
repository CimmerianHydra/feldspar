use bevy::prelude::*;


use crate::plugin::console::command::{has_pending_commands, PendingCommands};
use crate::plugin::console::dispatch::dispatch_commands_sys;
use crate::plugin::console::log::ConsoleLog;
use crate::plugin::console::registry::{CommandRegistry, ConsolePermissions};
use crate::plugin::console::commands::*;
use crate::plugin::console::registry::ConsoleAppExt;
use crate::plugin::console::chat::drain_chat_queue_sys;
use crate::plugin::console::session::{append_console_session_sys, ingest_console_keys_sys};
use crate::plugin::console::ui::{open_console_obs, refresh_console_input_sys, refresh_console_log_sys};


pub struct ConsolePlugin;

impl Plugin for ConsolePlugin {
    fn build(&self, app: &mut App) {
        app
            // `add_command` also does this, so ordering against feature plugins
            // never matters.
            .init_resource::<CommandRegistry>()
            .init_resource::<PendingCommands>()
            .init_resource::<ConsolePermissions>()
            .init_resource::<ConsoleLog>()

            .add_command(help_command_spec(), help_cmd)

            .add_systems(Update, append_console_session_sys)
            .add_observer(open_console_obs)

            // Ordered deliberately: a line submitted this frame is parsed,
            // executed, and rendered before the frame ends. Nothing about a
            // command feels deferred.
            .add_systems(Update, (
                ingest_console_keys_sys,
                dispatch_commands_sys.run_if(has_pending_commands),
                drain_chat_queue_sys,
                refresh_console_log_sys,
                refresh_console_input_sys,
            ).chain())
        ;
    }
}
