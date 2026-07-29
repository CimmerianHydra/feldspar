//! What the loading screen is allowed to know.
//!
//! A **leaf module**: it imports nothing from the rest of the crate, so
//! `app::loading` (which writes this) and `ui::splash` (which reads it) can
//! both name it without either importing the other. Same exception, and the
//! same justification, as [`crate::app::state`].
//!
//! The *bar* comes from `ProgressTracker<GameState>` — `iyes_progress` owns
//! the arithmetic. This owns only the words, because `iyes_progress` has no
//! notion of a label and the two change at different rates.

use bevy::prelude::*;

/// The caption on the splash screen: which step is running, and where it
/// sits in the sequence.
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct LoadReport {
    /// Shown verbatim. Written by the step's own entry in the load
    /// sequence, so the words live next to the work.
    pub label: &'static str,
    /// 0-based index of `label` within the sequence.
    pub step: usize,
    /// Length of the sequence, or **0 while it hasn't started** — during
    /// the file-reading phase there are no numbered steps yet, only files
    /// arriving. The splash uses this to decide whether to show a counter.
    pub steps: usize,
}

impl Default for LoadReport {
    fn default() -> Self {
        Self { label: "Loading assets...", step: 0, steps: 0 }
    }
}