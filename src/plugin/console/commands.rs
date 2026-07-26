use bevy::prelude::*;

use crate::plugin::console::args::{ArgSpec, CommandContext};
use crate::plugin::console::command::{CommandError, CommandResult, Feedback};
use crate::plugin::console::registry::{
    CommandEntry, CommandLevel, CommandRegistry, CommandSpec, ConsolePermissions,
};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – /help
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub fn help_command_spec() -> CommandSpec {
    CommandSpec::new("help")
        .alias("?")
        .help("List every command, or explain one in detail.")
        .arg(ArgSpec::text("command").optional())
}

pub fn help_cmd(
    In(context):  In<CommandContext>,
    registry:     Res<CommandRegistry>,
    permissions:  Res<ConsolePermissions>,
) -> CommandResult {
    // `command` is optional with no default, so this is the accessor to use.
    match context.opt_text("command") {
        Some(name) => describe_one(&registry, name),
        None       => Ok(Feedback::info(list_all(&registry, permissions.granted))),
    }
}

/// Every line here is derived from the spec. Nothing about a command is
/// documented twice, so nothing can be documented inconsistently.
fn describe_one(registry: &CommandRegistry, name: &str) -> CommandResult {
    // Tolerate `/help /give` as well as `/help give`.
    let needle = name.strip_prefix('/').unwrap_or(name).to_ascii_lowercase();

    let Some(id) = registry.by_name(&needle) else {
        return Err(CommandError::UnknownCommand {
            name:        needle.clone(),
            suggestions: registry.suggest(&needle),
        });
    };

    let spec  = &registry.get(id).spec;
    let mut lines = vec![spec.usage()];

    if !spec.help.is_empty() {
        lines.push(format!("  {}", spec.help));
    }
    if !spec.aliases.is_empty() {
        lines.push(format!("  aliases: {}", spec.aliases.join(", ")));
    }
    if spec.level != CommandLevel::Everyone {
        lines.push(format!("  requires: {:?}", spec.level));
    }
    if spec.requires_player {
        lines.push("  has to be run by a player".to_string());
    }

    if !spec.args.is_empty() {
        lines.push("  arguments:".to_string());

        let rendered: Vec<String> = spec.args.iter().map(ArgSpec::render_usage).collect();
        let width = rendered.iter().map(|text| text.chars().count()).max().unwrap_or(0);

        for (arg, text) in spec.args.iter().zip(rendered.iter()) {
            let padding = " ".repeat(width - text.chars().count());
            lines.push(format!("    {text}{padding}   {}", arg.kind.expectation()));
        }
    }

    Ok(Feedback::info(lines.join("\n")))
}

/// Commands above the granted level are hidden rather than listed-and-refused.
fn list_all(registry: &CommandRegistry, granted: CommandLevel) -> String {
    let mut visible: Vec<&CommandEntry> = registry
        .iter()
        .filter(|entry| entry.spec.level <= granted)
        .collect();

    // The registry is insertion-ordered, which is plugin-registration order —
    // stable, but meaningless to a reader.
    visible.sort_by(|left, right| left.spec.name.cmp(&right.spec.name));

    if visible.is_empty() {
        return "no commands available".to_string();
    }

    let usages: Vec<String> = visible.iter().map(|entry| entry.spec.usage()).collect();
    let width = usages.iter().map(|text| text.chars().count()).max().unwrap_or(0);

    let mut lines = vec![format!("{} command(s):", visible.len())];
    for (entry, usage) in visible.iter().zip(usages.iter()) {
        if entry.spec.help.is_empty() {
            lines.push(format!("  {usage}"));
        } else {
            let padding = " ".repeat(width - usage.chars().count());
            lines.push(format!("  {usage}{padding}   {}", entry.spec.help));
        }
    }

    lines.join("\n")
}