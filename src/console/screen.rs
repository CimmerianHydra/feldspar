use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;

use crate::command::log::{ConsoleLog, LogLine};
use crate::command::Severity;
use crate::console::session::ConsoleBuffer;
use crate::player::input::ConsoleToggle;
use crate::ui::screen::{UIPushOptions, UiScreenCommandsExt};
use crate::ui::theme::*;

const CONSOLE_FONT_SIZE: f32 = 14.0;
/// Also caps the rebuild cost, since a change respawns every visible line.
const CONSOLE_VISIBLE_LINES: usize = 14;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – MARKERS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Component)]
pub struct ConsoleScreen;

#[derive(Component)]
pub struct ConsoleLogView;

#[derive(Component)]
pub struct ConsoleInputView;

fn severity_color(severity: Severity) -> Color {
    match severity {
        Severity::Echo    => Color::srgba_u8(150, 158, 180, 255),
        Severity::Info    => Color::srgba_u8(225, 230, 240, 255),
        Severity::Success => Color::srgba_u8(140, 220, 150, 255),
        Severity::Warn    => Color::srgba_u8(240, 200, 110, 255),
        Severity::Error   => Color::srgba_u8(240, 120, 120, 255),
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – BUILDERS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn build_console_ui() -> impl Bundle {
    (
        Node {
            width:           percent(100),
            height:          percent(100),
            flex_direction:  FlexDirection::Column,
            justify_content: JustifyContent::FlexEnd,
            padding:         UiRect::all(px(8.0)),
            row_gap:         px(4.0),
            ..default()
        },
        Pickable::IGNORE,
        children![build_console_log_view(), build_console_input_view()],
    )
}

fn build_console_log_view() -> impl Bundle {
    (
        Node {
            width:          percent(100),
            flex_direction: FlexDirection::Column,
            padding:        UiRect::all(UI_PANEL_PADDING),
            border_radius:  BorderRadius::all(UI_PANEL_RADIUS),
            overflow:       Overflow::clip(),
            ..default()
        },
        BackgroundColor(UI_PANEL_COLOR),
        ConsoleLogView,
        Pickable::IGNORE,
    )
}

fn build_console_log_line(line: &LogLine) -> impl Bundle {
    (
        Text::new(line.text.clone()),
        TextFont { font_size: CONSOLE_FONT_SIZE, ..default() },
        TextColor(severity_color(line.severity)),
        Pickable::IGNORE,
    )
}

fn build_console_input_view() -> impl Bundle {
    (
        Node {
            width:         percent(100),
            padding:       UiRect::all(UI_PANEL_PADDING),
            border:        UiRect::all(UI_BORDER_THICKN),
            border_radius: BorderRadius::all(UI_PANEL_RADIUS),
            ..default()
        },
        BackgroundColor(UI_PANEL_COLOR),
        BorderColor::all(UI_BORDER_COLOR),
        Pickable::IGNORE,
        children![(
            Text::new("> |"),
            TextFont { font_size: CONSOLE_FONT_SIZE, ..default() },
            TextColor(severity_color(Severity::Info)),
            ConsoleInputView,
            Pickable::IGNORE,
        )],
    )
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – OPENING
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Undimmed, because a debug console you can't see the world through is a poor
/// debug console — and the reconciler never disables `GameUpdate` for a
/// non-empty stack, so the game keeps running underneath.
pub fn open_console_obs(
    event:         On<Start<ConsoleToggle>>,
    mut commands:  Commands,
    open_consoles: Query<&ConsoleScreen>,
) {
    // Defensive: the action can't fire while the console is up, since GameInput
    // is inactive. Cheap insurance against a future second binding.
    if !open_consoles.is_empty() { return; }

    let player = event.context;
    let screen = commands.push_ui_screen(
        player,
        UIPushOptions::new().dimmed(false).capturing_text(),
        build_console_ui(),
    );
    commands.entity(screen).insert(ConsoleScreen);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 4 – REFRESH
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// `Ref` rather than a `Changed` filter, because there are two reasons to
/// redraw: the log grew, or the view was just spawned and has to catch up on
/// everything logged while the console was shut.
pub fn refresh_console_log_sys(
    mut commands: Commands,
    log:          Res<ConsoleLog>,
    views:        Query<(Entity, Ref<ConsoleLogView>)>,
) {
    for (view, marker) in views.iter() {
        if !log.is_changed() && !marker.is_added() { continue; }

        commands.entity(view).despawn_children();

        let lines: Vec<Entity> = log
            .tail(CONSOLE_VISIBLE_LINES)
            .map(|line| commands.spawn(build_console_log_line(line)).id())
            .collect();

        commands.entity(view).add_children(&lines);
    }
}

pub fn refresh_console_input_sys(
    buffers:   Query<Ref<ConsoleBuffer>>,
    mut views: Query<(&mut Text, Ref<ConsoleInputView>)>,
) {
    // One local player for now; this becomes a lookup through the screen's
    // owner the day there are two.
    let Ok(buffer) = buffers.single() else { return };

    for (mut text, marker) in views.iter_mut() {
        if !buffer.is_changed() && !marker.is_added() { continue; }
        *text = Text::new(format!("> {}", buffer.render_with_caret()));
    }
}
