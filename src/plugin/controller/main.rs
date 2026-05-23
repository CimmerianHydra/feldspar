
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
        
        .add_systems(Update, mouse_scroll_handling_sys)
        ;
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – Systems and Events
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━


/// For some reason bevy_enhanced_input doesn't have bindings for the scroll wheel
/// so these are to make up for that. I should be replacing them with bevy_enhanced_input
/// bindings as soon as I can find a way.

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

