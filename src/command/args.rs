use bevy::prelude::*;

use crate::command::source::{CommandError, CommandSource};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – ARGUMENT KINDS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// What an argument is *supposed* to be. Making this first-class — rather than
/// letting each handler parse its own tokens — is what buys uniform errors now
/// and generic tab-completion later.
///
/// Extend as needed: Float, Vec3, PlayerRef, BlockName, Enum(&[&str]), Rest.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArgKind {
    Int,
    /// A single word, or a quoted string.
    Text,
    /// A registry item name. NOT resolved here: resolution needs world access,
    /// so this only guarantees a bare token was supplied. The handler resolves
    /// it, and owns the "did you mean" suggestion on a miss.
    ItemName,
}

impl ArgKind {
    /// Used verbatim in error messages, so phrase it as a noun phrase.
    pub fn expectation(self) -> &'static str {
        match self {
            ArgKind::Int      => "a whole number",
            ArgKind::Text     => "a word or quoted string",
            ArgKind::ItemName => "an item name",
        }
    }
}

/// A parsed argument. Several kinds legitimately map onto one variant.
#[derive(Clone, Debug, PartialEq)]
pub enum ArgValue {
    Int(i64),
    Text(String),
}

impl ArgValue {
    /// How a default is shown inside a generated usage string.
    pub fn render(&self) -> String {
        match self {
            ArgValue::Int(value)  => value.to_string(),
            ArgValue::Text(value) => value.clone(),
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – ARGUMENT SPEC
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Clone, Debug)]
pub struct ArgSpec {
    pub name:     String,
    pub kind:     ArgKind,
    pub required: bool,
    /// Substituted when an optional argument is omitted. `None` means the
    /// handler sees the argument as genuinely absent.
    pub default:  Option<ArgValue>,
}

impl ArgSpec {
    pub fn new(name: impl Into<String>, kind: ArgKind) -> Self {
        Self { name: name.into(), kind, required: true, default: None }
    }

    pub fn int(name: impl Into<String>)  -> Self { Self::new(name, ArgKind::Int) }
    pub fn text(name: impl Into<String>) -> Self { Self::new(name, ArgKind::Text) }
    pub fn item(name: impl Into<String>) -> Self { Self::new(name, ArgKind::ItemName) }

    /// May be omitted; the handler must reach for it with `opt_*`.
    pub fn optional(mut self) -> Self {
        self.required = false;
        self
    }

    /// May be omitted; the binder substitutes `value`, so the handler can use
    /// the infallible accessors.
    pub fn default_int(mut self, value: i64) -> Self {
        self.required = false;
        self.default  = Some(ArgValue::Int(value));
        self
    }

    pub fn default_text(mut self, value: impl Into<String>) -> Self {
        self.required = false;
        self.default  = Some(ArgValue::Text(value.into()));
        self
    }

    /// `<name>` required, `[name]` optional, `[name=default]` when defaulted.
    pub fn render_usage(&self) -> String {
        match (&self.default, self.required) {
            (Some(default), _) => format!("[{}={}]", self.name, default.render()),
            (None, true)       => format!("<{}>", self.name),
            (None, false)      => format!("[{}]", self.name),
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – COMMAND CONTEXT
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The single input every handler receives. Arguments are already tokenized,
/// type-checked, defaulted and arity-checked against the schema, so a handler
/// never sees a malformed invocation.
///
/// A `Vec` rather than a `HashMap`: commands have one to four arguments, and a
/// linear scan over three entries beats hashing plus an allocation.
#[derive(Debug)]
pub struct CommandContext {
    pub source: CommandSource,
    /// The line as submitted. Useful for echoing and for error context.
    pub raw:    String,
    args:       Vec<(String, ArgValue)>,
}

impl CommandContext {
    pub fn new(source: CommandSource, raw: String, args: Vec<(String, ArgValue)>) -> Self {
        Self { source, raw, args }
    }

    /// The acting player, or `RequiresPlayer`. Handlers that need an actor
    /// should also set `requires_player` on their spec, which makes the
    /// dispatcher reject the call before the handler even runs; this is the
    /// belt to that braces.
    pub fn player(&self) -> Result<Entity, CommandError> {
        self.source.player().ok_or(CommandError::RequiresPlayer)
    }

    fn find(&self, name: &str) -> Option<&ArgValue> {
        self.args.iter().find(|(key, _)| key == name).map(|(_, value)| value)
    }

    pub fn has(&self, name: &str) -> bool { self.find(name).is_some() }

    /// For arguments the schema guarantees: required, or optional with a
    /// default. A failure here means the handler and its schema disagree, which
    /// is a bug in *your* code — hence `Internal`, not a user-facing complaint.
    pub fn int(&self, name: &str) -> Result<i64, CommandError> {
        match self.find(name) {
            Some(ArgValue::Int(value)) => Ok(*value),
            Some(other) => Err(CommandError::Internal(format!(
                "argument '{name}' is {other:?}, not an Int"
            ))),
            None => Err(CommandError::Internal(format!(
                "argument '{name}' is not in the schema, or is optional with no default (use opt_int)"
            ))),
        }
    }

    pub fn text(&self, name: &str) -> Result<&str, CommandError> {
        match self.find(name) {
            Some(ArgValue::Text(value)) => Ok(value.as_str()),
            Some(other) => Err(CommandError::Internal(format!(
                "argument '{name}' is {other:?}, not Text"
            ))),
            None => Err(CommandError::Internal(format!(
                "argument '{name}' is not in the schema, or is optional with no default (use opt_text)"
            ))),
        }
    }

    /// For arguments declared `optional()` with no default.
    pub fn opt_int(&self, name: &str) -> Option<i64> {
        match self.find(name) {
            Some(ArgValue::Int(value)) => Some(*value),
            _ => None,
        }
    }

    pub fn opt_text(&self, name: &str) -> Option<&str> {
        match self.find(name) {
            Some(ArgValue::Text(value)) => Some(value.as_str()),
            _ => None,
        }
    }
}
