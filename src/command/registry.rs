use bevy::ecs::system::SystemId;
use bevy::prelude::*;
use std::collections::HashMap;

use crate::command::args::{ArgSpec, CommandContext};
use crate::command::source::CommandResult;

/// How many alternatives an "unknown command" error offers.
const SUGGESTION_LIMIT: usize = 5;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – PERMISSION LEVEL
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Ordered, so the dispatcher's check is a single comparison.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub enum CommandLevel {
    #[default]
    Everyone,
    /// Changes the world in a way normal play couldn't. `/give` belongs here.
    Cheat,
    Admin,
}

/// What the dispatcher currently allows. Grants everything for now — the point
/// is that specs already carry their level, so tightening this later is a
/// change to one resource and not to every command.
///
/// Eventually: per-player, and/or derived from `GameMode::Creative`.
#[derive(Resource, Clone, Copy, Debug)]
pub struct ConsolePermissions {
    pub granted: CommandLevel,
}

impl Default for ConsolePermissions {
    fn default() -> Self {
        Self { granted: CommandLevel::Admin }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – COMMAND SPEC
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct CommandID(pub u16);

/// Everything about a command except its behavior. Built by the feature plugin
/// that owns the command, then married to a handler at registration.
#[derive(Clone, Debug)]
pub struct CommandSpec {
    pub name:            String,
    pub aliases:         Vec<String>,
    pub help:            String,
    pub args:            Vec<ArgSpec>,
    /// Rejected before the handler runs when the source has no actor.
    pub requires_player: bool,
    pub level:           CommandLevel,
}

impl CommandSpec {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name:            name.into(),
            aliases:         Vec::new(),
            help:            String::new(),
            args:            Vec::new(),
            requires_player: false,
            level:           CommandLevel::Everyone,
        }
    }

    pub fn alias(mut self, alias: impl Into<String>) -> Self {
        self.aliases.push(alias.into());
        self
    }

    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.help = help.into();
        self
    }

    pub fn arg(mut self, arg: ArgSpec) -> Self {
        self.args.push(arg);
        self
    }

    pub fn requires_player(mut self) -> Self {
        self.requires_player = true;
        self
    }

    pub fn level(mut self, level: CommandLevel) -> Self {
        self.level = level;
        self
    }

    /// Generated, never authored. A hand-written usage string is a comment that
    /// silently goes stale the first time an argument is added.
    pub fn usage(&self) -> String {
        let mut usage = format!("/{}", self.name);
        for arg in &self.args {
            usage.push(' ');
            usage.push_str(&arg.render_usage());
        }
        usage
    }

    /// Structural checks that can only be authoring mistakes. Panics, because a
    /// malformed command should kill the app at boot, not misbehave at runtime.
    fn validate(&self) {
        assert!(!self.name.is_empty(), "console: a command cannot have an empty name");
        assert!(
            !self.name.chars().any(char::is_whitespace),
            "console: command name '{}' contains whitespace",
            self.name
        );
        // Lookup lowercases the typed name, so registered names must be lowercase
        // or they'd be unreachable.
        assert!(
            self.name == self.name.to_ascii_lowercase(),
            "console: command name '{}' must be lowercase",
            self.name
        );

        let mut seen_optional = false;
        for (index, arg) in self.args.iter().enumerate() {
            if arg.required && seen_optional {
                panic!(
                    "console: '{}' declares required argument <{}> after an optional one — \
                     nobody could ever supply it positionally",
                    self.name, arg.name
                );
            }
            if !arg.required {
                seen_optional = true;
            }
            if self.args[..index].iter().any(|other| other.name == arg.name) {
                panic!("console: '{}' declares argument '{}' twice", self.name, arg.name);
            }
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – REGISTRY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A spec plus its behavior. The handler is a `SystemId`, which is `Copy` and
/// carries no type parameters beyond the shared input/output pair — that is
/// precisely why commands with wildly different system params can live side by
/// side in one `Vec` with no boxing and no trait objects.
pub struct CommandEntry {
    pub spec:    CommandSpec,
    pub handler: SystemId<In<CommandContext>, CommandResult>,
}

/// Mirror of ItemRegistry / BlockRegistry — same pattern.
#[derive(Resource, Default)]
pub struct CommandRegistry {
    commands:        Vec<CommandEntry>,
    /// Names *and* aliases, all lowercase, all pointing at the same entry.
    name_to_command: HashMap<String, CommandID>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self { commands: Vec::new(), name_to_command: HashMap::new() }
    }

    pub fn get(&self, id: CommandID) -> &CommandEntry {
        &self.commands[id.0 as usize]
    }

    pub fn by_name(&self, name: &str) -> Option<CommandID> {
        self.name_to_command.get(name).copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = &CommandEntry> {
        self.commands.iter()
    }

    pub fn len(&self) -> usize { self.commands.len() }

    pub fn register(
        &mut self,
        spec:    CommandSpec,
        handler: SystemId<In<CommandContext>, CommandResult>,
    ) -> CommandID {
        spec.validate();

        let id = CommandID(self.commands.len() as u16);
        let names: Vec<String> = std::iter::once(spec.name.clone())
            .chain(spec.aliases.iter().cloned())
            .collect();

        // Check every name before inserting any, so a rejected registration
        // can't leave the map half-populated.
        for name in &names {
            if let Some(existing) = self.name_to_command.get(name).copied() {
                panic!(
                    "console: '{}' is already taken by {} — names and aliases must be unique",
                    name,
                    self.get(existing).spec.usage()
                );
            }
        }

        for name in names {
            self.name_to_command.insert(name, id);
        }
        self.commands.push(CommandEntry { spec, handler });
        id
    }

    /// Cheap alternatives for a name that didn't resolve: prefix matches first,
    /// then substring. Sorted, because HashMap iteration order is arbitrary and
    /// error messages that reshuffle themselves are maddening.
    pub fn suggest(&self, name: &str) -> Vec<String> {
        let needle = name.to_ascii_lowercase();

        let mut prefixed: Vec<String> = Vec::new();
        let mut contained: Vec<String> = Vec::new();
        for key in self.name_to_command.keys() {
            if key.starts_with(&needle) {
                prefixed.push(key.clone());
            } else if key.contains(&needle) {
                contained.push(key.clone());
            }
        }

        prefixed.sort();
        contained.sort();
        prefixed.extend(contained);
        prefixed.truncate(SUGGESTION_LIMIT);
        prefixed
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 4 – REGISTRATION
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The one line a feature plugin writes to own a command.
pub trait ConsoleAppExt {
    fn add_command<M, S>(&mut self, spec: CommandSpec, handler: S) -> &mut Self
    where
        S: IntoSystem<In<CommandContext>, CommandResult, M> + 'static;
}

impl ConsoleAppExt for App {
    fn add_command<M, S>(&mut self, spec: CommandSpec, handler: S) -> &mut Self
    where
        S: IntoSystem<In<CommandContext>, CommandResult, M> + 'static,
    {
        // Idempotent, and the reason registration is order-independent: a
        // feature plugin can register commands whether or not CommandPlugin has
        // been added yet.
        self.init_resource::<CommandRegistry>();

        // The handler is stored *in the world* as an entity; what comes back is
        // a typed handle. Nothing runs yet — the system is initialized lazily on
        // its first execution.
        let handler = self.world_mut().register_system(handler);

        self.world_mut()
            .resource_mut::<CommandRegistry>()
            .register(spec, handler);

        self
    }
}
