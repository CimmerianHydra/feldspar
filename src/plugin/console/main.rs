use bevy::prelude::*;

use crate::plugin::console::command::{has_pending_commands, PendingCommands};
use crate::plugin::console::dispatch::dispatch_commands_sys;
use crate::plugin::console::log::ConsoleLog;
use crate::plugin::console::registry::{CommandRegistry, ConsolePermissions};
use crate::plugin::console::commands::*;
use crate::plugin::console::registry::ConsoleAppExt;


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

            .add_systems(Update, dispatch_commands_sys.run_if(has_pending_commands))
        ;
    }
}
