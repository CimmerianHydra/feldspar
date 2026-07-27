use bevy::log::BoxedLayer;
use bevy::log::tracing::field::{Field, Visit};
use bevy::log::tracing::{Event, Level, Subscriber};
use bevy::log::tracing_subscriber::Layer;
use bevy::log::tracing_subscriber::layer::Context;
use bevy::prelude::*;
use std::fmt;
use std::sync::Mutex;

use crate::plugin::console::command::Severity;
use crate::plugin::console::log::ConsoleLog;

/// The tracing target that routes an event into the in-game console. Has to stay
/// in sync with the literal in the macros below — tracing needs a literal there,
/// so a const can't be substituted.
pub const CHAT_TARGET: &str = "console";

/// Bounded so a logging loop that outruns the drain leaks nothing worse than
/// dropped lines.
const CHAT_QUEUE_CAP: usize = 512;

/// Tracing events arrive on whatever thread emitted them, outside any system, so
/// a global is the only place they can land. Drained once a frame.
static CHAT_QUEUE: Mutex<Vec<(Severity, String)>> = Mutex::new(Vec::new());

pub fn push_chat_line(severity: Severity, text: String) {
    // Never panic on a logging path. A poisoned lock or a full queue drops the
    // line; it does not take the game down.
    if let Ok(mut queue) = CHAT_QUEUE.lock() {
        if queue.len() < CHAT_QUEUE_CAP {
            queue.push((severity, text));
        }
    }
}

pub fn drain_chat_queue_sys(mut log: ResMut<ConsoleLog>) {
    let Ok(mut queue) = CHAT_QUEUE.lock() else { return };
    // Checked before touching `log`, so an idle frame doesn't mark the resource
    // changed and trigger a pointless view rebuild.
    if queue.is_empty() { return; }

    for (severity, text) in queue.drain(..) {
        log.push(severity, text);
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TRACING LAYER
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

struct ConsoleCaptureLayer;

impl<S: Subscriber> Layer<S> for ConsoleCaptureLayer {
    fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
        let metadata = event.metadata();

        // Exactly this target, so wgpu and the asset server stay out of the
        // game. To see all your own logs in-game, widen this to
        // `!metadata.target().starts_with("feldspar")` — but note the dispatcher
        // already mirrors command feedback through `bevy::log`, so those lines
        // would then arrive twice.
        if metadata.target() != CHAT_TARGET { return; }

        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);
        let Some(message) = visitor.message else { return };

        let severity = match *metadata.level() {
            Level::ERROR => Severity::Error,
            Level::WARN  => Severity::Warn,
            _            => Severity::Info,
        };

        push_chat_line(severity, message);
    }
}

/// Tracing has no "just give me the text" accessor: the message is a field like
/// any other and has to be visited.
#[derive(Default)]
struct MessageVisitor {
    message: Option<String>,
}

impl Visit for MessageVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        // The message field is a `fmt::Arguments`, whose Debug is its Display —
        // so this is the formatted text, not a quoted rendering of it.
        if field.name() == "message" {
            self.message = Some(format!("{value:?}"));
        }
    }
}

/// Hand this to `LogPlugin::custom_layer`. It's a bare fn pointer and can't
/// capture anything, which is the other reason the queue is a global.
pub fn console_log_layer(_app: &mut App) -> Option<BoxedLayer> {
    Some(Box::new(ConsoleCaptureLayer))
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// MACROS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Print a line to the in-game console. Same format syntax as `info!`, callable
/// from anywhere — no system params, no resource access.
///
/// ```ignore
/// chat!("spawned {} chunks around {}", count, origin);
/// ```
#[macro_export]
macro_rules! chat {
    ($($arg:tt)*) => { ::bevy::log::info!(target: "console", $($arg)*) };
}

#[macro_export]
macro_rules! chat_warn {
    ($($arg:tt)*) => { ::bevy::log::warn!(target: "console", $($arg)*) };
}

#[macro_export]
macro_rules! chat_error {
    ($($arg:tt)*) => { ::bevy::log::error!(target: "console", $($arg)*) };
}