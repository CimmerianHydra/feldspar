use bevy::prelude::*;
use std::collections::VecDeque;
use std::fmt;

use crate::command::registry::CommandLevel;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – COMMAND SOURCE
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Who issued a command. Carrying the actor here — rather than assuming a
/// single local player — is what lets `/give` know whose inventory to fill,
/// and is the one thing that would be painful to retrofit later.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CommandSource {
    /// Typed into a console by a player.
    Player(Entity),
    /// Read from an autoexec file or a macro. No actor.
    Script,
    /// Emitted by engine code: startup fixtures, keybinds, tests.
    Internal,
}

impl CommandSource {
    pub fn player(self) -> Option<Entity> {
        match self {
            CommandSource::Player(entity) => Some(entity),
            _ => None,
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – SUBMISSIONS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// One raw line handed to the console, unparsed.
#[derive(Clone, Debug)]
pub struct Submission {
    pub source: CommandSource,
    pub raw:    String,
}

/// THE seam of the whole feature. Anything that can get a `ResMut` to this can
/// run a command: the console UI, an autoexec file, a debug keybind, a test.
///
/// A queue rather than a message, deliberately — messages are double-buffered
/// with per-reader cursors, so a submission could be dropped if nothing read it
/// in time, and an exclusive system can't take a `MessageReader` anyway. Here,
/// "who drains this, and when" is explicit and every line is guaranteed to run.
#[derive(Resource, Default)]
pub struct PendingCommands {
    queue: VecDeque<Submission>,
}

impl PendingCommands {
    pub fn push(&mut self, source: CommandSource, raw: impl Into<String>) {
        self.queue.push_back(Submission { source, raw: raw.into() });
    }

    pub fn is_empty(&self) -> bool { self.queue.is_empty() }
    pub fn len(&self) -> usize { self.queue.len() }

    /// Detach up to `budget` submissions. The dispatcher works on this
    /// snapshot, so a handler that submits more commands queues them for the
    /// next frame instead of extending the loop it's running inside — that plus
    /// the budget makes a runaway script impossible rather than merely unlikely.
    pub fn take_up_to(&mut self, budget: usize) -> Vec<Submission> {
        let count = budget.min(self.queue.len());
        self.queue.drain(..count).collect()
    }
}

/// Run condition: keeps the exclusive dispatcher — and therefore its schedule
/// sync point — out of every frame where nothing was submitted.
pub fn has_pending_commands(pending: Res<PendingCommands>) -> bool {
    !pending.is_empty()
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – OUTCOMES
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Severity {
    /// The submitted line, echoed back so the feed reads as a transcript.
    Echo,
    Info,
    Success,
    /// Did something, but not all of it. `/give` that only partly fit.
    Warn,
    Error,
}

/// What a handler reports on success. Severity lives on the success side too:
/// a partial `/give` is neither a clean `Ok` nor an `Err`.
#[derive(Clone, Debug)]
pub struct Feedback {
    pub severity: Severity,
    pub text:     String,
}

impl Feedback {
    pub fn info(text: impl Into<String>)    -> Self { Self { severity: Severity::Info,    text: text.into() } }
    pub fn success(text: impl Into<String>) -> Self { Self { severity: Severity::Success, text: text.into() } }
    pub fn warn(text: impl Into<String>)    -> Self { Self { severity: Severity::Warn,    text: text.into() } }
}

/// The uniform return type of every handler. This — and only this — is what
/// makes a homogeneous registry of heterogeneous systems possible.
pub type CommandResult = Result<Feedback, CommandError>;

#[derive(Clone, Debug)]
pub enum CommandError {
    UnknownCommand   { name: String, suggestions: Vec<String> },
    MissingArgument  { name: String, usage: String },
    BadArgument      { name: String, expected: String, got: String, usage: String },
    TooManyArguments { expected: usize, got: usize, usage: String },
    /// The command acts on an actor, but the source has none.
    RequiresPlayer,
    NotPermitted     { name: String, level: CommandLevel },
    /// The line couldn't be tokenized at all.
    Malformed(String),
    /// The handler ran and failed for a gameplay reason.
    Failed(String),
    /// A bug in the console or in a handler's own schema, not user error.
    /// Deliberately distinct so it reads as "fix the code", not "type better".
    Internal(String),
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownCommand { name, suggestions } => {
                write!(f, "unknown command '{name}'")?;
                if !suggestions.is_empty() {
                    write!(f, " — did you mean: {}?", suggestions.join(", "))?;
                }
                Ok(())
            }
            Self::MissingArgument { name, usage } => {
                write!(f, "missing argument <{name}>\n  usage: {usage}")
            }
            Self::BadArgument { name, expected, got, usage } => {
                write!(f, "<{name}> expected {expected}, got '{got}'\n  usage: {usage}")
            }
            Self::TooManyArguments { expected, got, usage } => {
                write!(f, "expected at most {expected} argument(s), got {got}\n  usage: {usage}")
            }
            Self::RequiresPlayer      => write!(f, "this command has to be run by a player"),
            Self::NotPermitted { name, level } => write!(f, "'{name}' requires {level:?} permission"),
            Self::Malformed(why)      => write!(f, "malformed input: {why}"),
            Self::Failed(why)         => write!(f, "{why}"),
            Self::Internal(why)       => write!(f, "internal console error: {why}"),
        }
    }
}
