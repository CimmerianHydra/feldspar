use bevy::prelude::*;
use std::collections::VecDeque;

use crate::command::source::Severity;

const DEFAULT_LOG_CAPACITY: usize = 256;

#[derive(Clone, Debug)]
pub struct LogLine {
    pub severity: Severity,
    pub text:     String,
}

/// Bounded scrollback. A resource for now; if the console ever becomes
/// per-player this becomes a component on the player, alongside the input
/// buffer and history.
#[derive(Resource)]
pub struct ConsoleLog {
    lines:    VecDeque<LogLine>,
    capacity: usize,
}

impl Default for ConsoleLog {
    fn default() -> Self {
        Self::with_capacity(DEFAULT_LOG_CAPACITY)
    }
}

impl ConsoleLog {
    pub fn with_capacity(capacity: usize) -> Self {
        Self { lines: VecDeque::with_capacity(capacity), capacity }
    }

    pub fn push(&mut self, severity: Severity, text: impl Into<String>) {
        if self.lines.len() == self.capacity {
            self.lines.pop_front();
        }
        self.lines.push_back(LogLine { severity, text: text.into() });
    }

    /// Oldest first — the order a UI renders top to bottom.
    pub fn iter(&self) -> impl Iterator<Item = &LogLine> {
        self.lines.iter()
    }

    pub fn clear(&mut self)      { self.lines.clear(); }
    pub fn len(&self)  -> usize  { self.lines.len() }
    pub fn is_empty(&self) -> bool { self.lines.is_empty() }

    /// The last `count` lines, oldest first.
    pub fn tail(&self, count: usize) -> impl Iterator<Item = &LogLine> {
        self.lines.iter().skip(self.lines.len().saturating_sub(count))
    }
}
