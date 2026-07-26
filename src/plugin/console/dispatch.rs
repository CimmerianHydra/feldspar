use bevy::prelude::*;

use crate::plugin::console::args::CommandContext;
use crate::plugin::console::command::*;
use crate::plugin::console::log::ConsoleLog;
use crate::plugin::console::parser::{bind_args, parse_line};
use crate::plugin::console::registry::{CommandRegistry, ConsolePermissions};

/// Ceiling on commands executed per frame. A safety valve for scripts and
/// aliases, not a performance concern.
const MAX_COMMANDS_PER_FRAME: usize = 32;

/// Exclusive because running a one-shot system needs `&mut World`. Gated by
/// `has_pending_commands`, so the sync point only exists on frames where
/// something was actually submitted.
pub fn dispatch_commands_sys(world: &mut World) {
    let submissions = world
        .resource_mut::<PendingCommands>()
        .take_up_to(MAX_COMMANDS_PER_FRAME);

    for submission in submissions {
        record(world, Severity::Echo, format!("> {}", submission.raw.trim()));

        match execute(world, &submission) {
            Ok(Some(feedback)) => record(world, feedback.severity, feedback.text),
            Ok(None)           => {}
            Err(error)         => record(world, Severity::Error, error.to_string()),
        }
    }
}

fn execute(world: &mut World, submission: &Submission) -> Result<Option<Feedback>, CommandError> {
    let Some(invocation) = parse_line(&submission.raw)? else { return Ok(None) };

    // Everything needed from the registry is pulled out inside this block, so
    // the borrow is dead before the handler gets the world mutably. Note what
    // has to survive the boundary: one `Copy` handle and one owned context.
    // That's the whole reason this design stays borrow-checker-friendly.
    let (handler, context) = {
        let registry = world.resource::<CommandRegistry>();
        let granted  = world.resource::<ConsolePermissions>().granted;

        let Some(id) = registry.by_name(&invocation.name) else {
            return Err(CommandError::UnknownCommand {
                name:        invocation.name.clone(),
                suggestions: registry.suggest(&invocation.name),
            });
        };
        let entry = registry.get(id);

        if entry.spec.level > granted {
            return Err(CommandError::NotPermitted {
                name:  entry.spec.name.clone(),
                level: entry.spec.level,
            });
        }
        if entry.spec.requires_player && submission.source.player().is_none() {
            return Err(CommandError::RequiresPlayer);
        }

        let args = bind_args(&entry.spec, &invocation.args)?;

        (
            entry.handler,
            CommandContext::new(submission.source, submission.raw.clone(), args),
        )
    };

    // Two distinct failure domains. The outer Err means the console's plumbing
    // broke (stale id, unregistered system, re-entrant call); the inner one
    // means the command itself refused. They are not the same bug, so they
    // don't get folded into one message.
    match world.run_system_with(handler, context) {
        Ok(result)  => result.map(Some),
        Err(error)  => Err(CommandError::Internal(format!("handler did not run: {error:?}"))),
    }
}

/// One place formats and files every outcome, which is why no handler ever has
/// to. Mirroring into `bevy::log` means commands are visible in the terminal
/// before any console UI exists.
fn record(world: &mut World, severity: Severity, text: String) {
    match severity {
        Severity::Error => bevy::log::error!("console: {text}"),
        Severity::Warn  => bevy::log::warn!("console: {text}"),
        Severity::Echo  => bevy::log::debug!("console: {text}"),
        _               => bevy::log::info!("console: {text}"),
    }

    world.resource_mut::<ConsoleLog>().push(severity, text);
}