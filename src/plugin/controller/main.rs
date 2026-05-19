
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

#[derive(Event, PartialEq)]
pub enum MouseScrollEvent {
    ScrollUp,
    ScrollDown,
}

fn mouse_scroll_handling_sys(
    mut scroll_wheel: MessageReader<MouseWheel>,
    mut commands: Commands,
) {
    for event in scroll_wheel.read() {
        let scroll_direction = match event.unit {
            MouseScrollUnit::Line => { event.y.signum() },
            MouseScrollUnit::Pixel => { (event.y / MouseScrollUnit::SCROLL_UNIT_CONVERSION_FACTOR).signum() },
        };
        if scroll_direction < 0.0 {
            commands.trigger(MouseScrollEvent::ScrollDown );
        } else {
            commands.trigger(MouseScrollEvent::ScrollUp );
        }
    }
}

