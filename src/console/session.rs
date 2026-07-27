use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::*;
use std::collections::VecDeque;

use crate::command::{CommandSource, PendingCommands};
use crate::sim::player::Player;
use crate::ui::screen::{TextFocus, UiScreenCommandsExt};

const HISTORY_CAPACITY: usize = 64;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – INPUT BUFFER
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The line being typed. Lives on the player, not on the screen, so it survives
/// closing and reopening the console — and so a second player would get their own.
#[derive(Component, Default, Debug)]
pub struct ConsoleBuffer {
    text:  String,
    /// Byte offset into `text`, kept on a char boundary by construction.
    caret: usize,
}

impl ConsoleBuffer {
    pub fn text(&self) -> &str { &self.text }

    pub fn insert_str(&mut self, text: &str) {
        self.text.insert_str(self.caret, text);
        self.caret += text.len();
    }

    pub fn backspace(&mut self) {
        let previous = self.previous_boundary();
        if previous != self.caret {
            self.text.replace_range(previous..self.caret, "");
            self.caret = previous;
        }
    }

    pub fn delete(&mut self) {
        let next = self.next_boundary();
        if next != self.caret {
            self.text.replace_range(self.caret..next, "");
        }
    }

    pub fn move_left(&mut self)  { self.caret = self.previous_boundary(); }
    pub fn move_right(&mut self) { self.caret = self.next_boundary(); }
    pub fn move_home(&mut self)  { self.caret = 0; }
    pub fn move_end(&mut self)   { self.caret = self.text.len(); }

    pub fn clear(&mut self) {
        self.text.clear();
        self.caret = 0;
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text  = text.into();
        self.caret = self.text.len();
    }

    /// Hand the line over and reset, in one move.
    pub fn take(&mut self) -> String {
        self.caret = 0;
        std::mem::take(&mut self.text)
    }

    /// bevy_ui has no text-editing widget, so the caret is drawn as a glyph.
    /// Crude, and completely legible for a console.
    pub fn render_with_caret(&self) -> String {
        format!("{}|{}", &self.text[..self.caret], &self.text[self.caret..])
    }

    // Both of these clamp at the ends, so the caret can never land mid-glyph
    // and split a multi-byte character.
    fn previous_boundary(&self) -> usize {
        self.text[..self.caret]
            .chars()
            .next_back()
            .map_or(0, |character| self.caret - character.len_utf8())
    }

    fn next_boundary(&self) -> usize {
        self.text[self.caret..]
            .chars()
            .next()
            .map_or(self.caret, |character| self.caret + character.len_utf8())
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – HISTORY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Component, Default, Debug)]
pub struct ConsoleHistory {
    /// Newest first, so index 0 is the last thing submitted.
    entries: VecDeque<String>,
    /// `None` means "editing a fresh line"; `Some(i)` means showing entries[i].
    cursor:  Option<usize>,
    /// The half-typed line stashed when history navigation started, so walking
    /// up and back down returns you to what you were writing.
    draft:   String,
}

impl ConsoleHistory {
    pub fn remember(&mut self, line: &str) {
        self.cursor = None;
        self.draft.clear();

        if line.trim().is_empty() { return; }
        // Hammering the same command shouldn't fill the history with copies.
        if self.entries.front().is_some_and(|newest| newest == line) { return; }

        self.entries.push_front(line.to_string());
        if self.entries.len() > HISTORY_CAPACITY {
            self.entries.pop_back();
        }
    }

    pub fn older(&mut self, current: &str) -> Option<String> {
        if self.entries.is_empty() { return None; }

        let index = match self.cursor {
            None => {
                self.draft = current.to_string();
                0
            }
            Some(index) => (index + 1).min(self.entries.len() - 1),
        };

        self.cursor = Some(index);
        self.entries.get(index).cloned()
    }

    pub fn newer(&mut self) -> Option<String> {
        match self.cursor {
            None => None,
            Some(0) => {
                self.cursor = None;
                Some(std::mem::take(&mut self.draft))
            }
            Some(index) => {
                self.cursor = Some(index - 1);
                self.entries.get(index - 1).cloned()
            }
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – SYSTEMS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Same shape as `sim::player::append_player_inventory_sys`: the console
/// hangs its own state off the player without the controller having to know
/// it exists.
pub fn append_console_session_sys(
    mut commands: Commands,
    new_players:  Query<Entity, Added<Player>>,
) {
    for player in new_players.iter() {
        commands
            .entity(player)
            .insert((ConsoleBuffer::default(), ConsoleHistory::default()));
    }
}

/// The single owner of the keyboard while the console is focused.
///
/// Runs every frame whether or not the console is open, and that's deliberate:
/// a reader that stops reading lets messages expire, then gets handed a
/// two-frame backlog when it resumes — the key that opened the console among
/// them. Draining and discarding keeps the cursor honest.
pub fn ingest_console_keys_sys(
    mut keys:     MessageReader<KeyboardInput>,
    mut commands: Commands,
    mut pending:  ResMut<PendingCommands>,
    mut focused:  Query<(Entity, &mut ConsoleBuffer, &mut ConsoleHistory), With<TextFocus>>,
) {
    let Ok((player, mut buffer, mut history)) = focused.single_mut() else {
        keys.clear();
        return;
    };

    for key in keys.read() {
        if key.state != ButtonState::Pressed { continue; }

        // `logical_key`, never `key_code`: the logical key is what the layout
        // actually produced, so this works on a keyboard that isn't US ANSI.
        match &key.logical_key {
            Key::Character(characters) => {
                // Control characters can ride inside Character on some
                // platforms and would corrupt the buffer.
                if characters.chars().any(char::is_control) { continue; }
                buffer.insert_str(characters);
            }
            Key::Space      => buffer.insert_str(" "),
            Key::Backspace  => buffer.backspace(),
            Key::Delete     => buffer.delete(),
            Key::ArrowLeft  => buffer.move_left(),
            Key::ArrowRight => buffer.move_right(),
            Key::Home       => buffer.move_home(),
            Key::End        => buffer.move_end(),

            Key::ArrowUp => {
                if let Some(line) = history.older(buffer.text()) {
                    buffer.set_text(line);
                }
            }
            Key::ArrowDown => {
                if let Some(line) = history.newer() {
                    buffer.set_text(line);
                }
            }

            Key::Enter => {
                let line = buffer.take();
                if !line.trim().is_empty() {
                    history.remember(&line);
                    pending.push(CommandSource::Player(player), line);
                }
            }

            // Escape closes, because UiBack can't fire while contexts are off.
            Key::Escape => {
                buffer.clear();
                commands.pop_ui_screen(player);
            }

            // Tab is reserved for completion.
            _ => {}
        }
    }
}
