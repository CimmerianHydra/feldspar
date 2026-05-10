
use bevy::prelude::*;
use bevy::input::mouse::{MouseWheel, MouseScrollUnit};

use crate::plugin::state::*;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – Plugin Definition
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct ControlsPlugin;

impl Plugin for ControlsPlugin {
    fn build(&self, app: &mut App) {
        // Add systems related to block registry management here
        app
        
        .add_systems(Update, (
            mouse_click_handling_sys,
            mouse_scroll_handling_sys,
        ).run_if(in_state(UIState::Game)))
        ;
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – Systems and Events
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━


/// These events are supposed to be used whenever the user is IN GAME and
/// their controls are active. For this reason, they are by default disabled
/// during menu navigation.

#[derive(Event)]
pub struct MouseEvent {
    pub action: MouseAction,
}

#[derive(PartialEq)]
pub enum MouseAction {
    Primary,
    Secondary,
    Middle,
    ScrollUp,
    ScrollDown,
}

// Placeholder, just sends events when the player clicks.
fn mouse_click_handling_sys(
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    mut commands: Commands,
) {
    if mouse_button_input.just_pressed(MouseButton::Left) {
        commands.trigger(MouseEvent{ action: MouseAction::Primary });
    }
    if mouse_button_input.just_pressed(MouseButton::Right) {
        commands.trigger(MouseEvent{ action: MouseAction::Secondary });
    }
    if mouse_button_input.just_pressed(MouseButton::Middle) {
        commands.trigger(MouseEvent{ action: MouseAction::Middle });
    }
    
}

fn mouse_scroll_handling_sys(
    // Placeholder, just sends events when the player clicks.
    mut scroll_wheel: MessageReader<MouseWheel>,
    mut commands: Commands,
) {
    for event in scroll_wheel.read() {
        let scroll_direction = match event.unit {
            MouseScrollUnit::Line => { event.y.signum() },
            MouseScrollUnit::Pixel => { (event.y / MouseScrollUnit::SCROLL_UNIT_CONVERSION_FACTOR).signum() },
        };
        if scroll_direction < 0.0 {
            commands.trigger(MouseEvent{ action: MouseAction::ScrollDown });
        } else {
            commands.trigger(MouseEvent{ action: MouseAction::ScrollUp });
        }
    }
}