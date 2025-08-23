use bevy::prelude::*;
use bevy::window::PrimaryWindow;

pub struct FollowCursorPlugin;
impl Plugin for FollowCursorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_node_to_mouse_position);
    }
}

#[derive(Component, Default)]
pub struct FollowCursor {
    offset : Vec2,
    anchor : Vec2, // Expresses the anchor point of the element as a percentage.
} // For UI elements that ARE CURRENTLY BEING dragged


/// Check that no dragging is going on. Then, if mouse just pressed, give
/// the FollowCursor component to UI element.
pub fn pick(){} 

/// Check that dragging is going on. Then, if mouse just pressed, remove
/// the FollowCursor component from UI element.
pub fn drop(){}

/// Ensures the position of UI elements that are picked is attached to mouse.
pub fn update_node_to_mouse_position(
    window_q: Query<&Window, With<PrimaryWindow>>,
    ui_scale: Res<UiScale>,
    mut q_ui: Query<(&mut Node, &ComputedNode, &FollowCursor), With<FollowCursor>>,
) {
    let Ok(window) = window_q.single() else { return };

    if let Some(cursor) = window.cursor_position() {
        // Convert to "logical UI pixels" so values match Val::Px at the current UI scale
        let cursor = cursor / ui_scale.0;

        

        for (mut node, computed, follow_data) in &mut q_ui {
            // Absolute positions this node relative to its parent node’s box
            node.position_type = PositionType::Absolute;

            // OPTION A: top-left corner at the cursor
            // node.left = Val::Px(cursor.x);
            // node.top  = Val::Px(cursor.y);

            // OPTION B: center the node on the cursor (uses laid-out size)
            let size = computed.size / ui_scale.0;
            node.left = Val::Px(cursor.x - size.x * follow_data.anchor.x + follow_data.offset.x);
            node.top  = Val::Px(cursor.y - size.y * follow_data.anchor.y - follow_data.offset.y);
        }
    }
}