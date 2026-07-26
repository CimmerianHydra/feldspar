use crate::plugin::console::args::{ArgKind, ArgSpec, ArgValue};
use crate::plugin::console::command::CommandError;
use crate::plugin::console::registry::CommandSpec;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – TOKENIZING
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A line split into a command name and its raw argument tokens, before any
/// schema is consulted.
#[derive(Clone, Debug)]
pub struct Invocation {
    pub name: String,
    pub args: Vec<String>,
}

/// Split on whitespace, honoring double quotes so multi-word values survive.
/// Backslash escapes `\"` and `\\` inside quotes; outside quotes a backslash is
/// just a character, which keeps Windows paths usable.
pub fn tokenize(line: &str) -> Result<Vec<String>, CommandError> {
    let mut tokens  = Vec::new();
    let mut current = String::new();
    let mut in_token = false;
    let mut quoted   = false;
    let mut escaped  = false;

    for character in line.chars() {
        if escaped {
            current.push(character);
            escaped = false;
            continue;
        }

        match character {
            '\\' if quoted => escaped = true,
            '"' => {
                quoted = !quoted;
                // An opening quote starts a token even if it ends up empty, so
                // `""` is a deliberately supplied empty argument.
                in_token = true;
            }
            character if character.is_whitespace() && !quoted => {
                if in_token {
                    tokens.push(std::mem::take(&mut current));
                    in_token = false;
                }
            }
            character => {
                current.push(character);
                in_token = true;
            }
        }
    }

    if quoted {
        return Err(CommandError::Malformed("unterminated quote".into()));
    }
    if escaped {
        return Err(CommandError::Malformed("line ends in a trailing backslash".into()));
    }
    if in_token {
        tokens.push(current);
    }

    Ok(tokens)
}

/// Turn a raw line into an invocation. `Ok(None)` means "nothing to do":
/// a blank line, or a comment — the latter so autoexec files can be annotated.
///
/// The leading `/` is optional and stripped. It stays meaningful for later: once
/// a chat box exists, `/` is what distinguishes a command from a message.
pub fn parse_line(raw: &str) -> Result<Option<Invocation>, CommandError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
        return Ok(None);
    }

    let mut tokens = tokenize(trimmed)?.into_iter();
    let Some(head) = tokens.next() else { return Ok(None) };

    let name = head.strip_prefix('/').unwrap_or(&head).to_ascii_lowercase();
    if name.is_empty() {
        return Ok(None);
    }

    Ok(Some(Invocation { name, args: tokens.collect() }))
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – BINDING
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Match raw tokens against a schema: arity, types, defaults. Every error it
/// returns carries the generated usage line, so the console can explain itself
/// without any handler writing error prose.
pub fn bind_args(
    spec:   &CommandSpec,
    tokens: &[String],
) -> Result<Vec<(String, ArgValue)>, CommandError> {
    if tokens.len() > spec.args.len() {
        return Err(CommandError::TooManyArguments {
            expected: spec.args.len(),
            got:      tokens.len(),
            usage:    spec.usage(),
        });
    }

    let mut bound = Vec::with_capacity(spec.args.len());

    for (index, arg) in spec.args.iter().enumerate() {
        match tokens.get(index) {
            Some(token) => bound.push((arg.name.clone(), parse_token(spec, arg, token)?)),
            None if arg.required => {
                return Err(CommandError::MissingArgument {
                    name:  arg.name.clone(),
                    usage: spec.usage(),
                });
            }
            None => {
                if let Some(default) = &arg.default {
                    bound.push((arg.name.clone(), default.clone()));
                }
            }
        }
    }

    Ok(bound)
}

fn parse_token(spec: &CommandSpec, arg: &ArgSpec, token: &str) -> Result<ArgValue, CommandError> {
    match arg.kind {
        ArgKind::Int => token.parse::<i64>().map(ArgValue::Int).map_err(|_| {
            CommandError::BadArgument {
                name:     arg.name.clone(),
                expected: arg.kind.expectation().to_string(),
                got:      token.to_string(),
                usage:    spec.usage(),
            }
        }),
        // Names are passed through untouched; resolution (and any
        // case-folding policy) belongs to whoever owns the registry.
        ArgKind::Text | ArgKind::ItemName => Ok(ArgValue::Text(token.to_string())),
    }
}