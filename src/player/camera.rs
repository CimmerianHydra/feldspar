use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;
use std::f32::consts::FRAC_PI_2;

use crate::player::input::Look;
use crate::player::interaction::DDARay;
use crate::sim::player::Player;

pub const DEFAULT_SENSITIVITY: f32 = 0.0022;
pub const DEFAULT_REACH:       f32 = 8.0;
const PITCH_LIMIT: f32 = FRAC_PI_2 - 0.01;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – THE CAMERA
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Component)]
pub struct FPSCamera {
    pub sensitivity: f32,
}

/// The camera rig that hangs off a player body.
///
/// `local_y` is the eye height relative to the body's centre; the body owns
/// yaw and this owns pitch, which is why neither transform needs a Euler
/// round-trip.
pub fn build_player_camera(local_y: f32) -> impl Bundle {
    (
        FPSCamera { sensitivity: DEFAULT_SENSITIVITY },
        DDARay { max_distance: DEFAULT_REACH },
        Camera3d::default(),
        Transform::from_xyz(0.0, local_y, 0.0),
        SpatialListener::default(),
    )
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – LOOK
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//
// The rotation is split:
//   - Body owns yaw (rotation around +Y).
//   - Camera child owns pitch (rotation around +X), clamped.
//
// Because each transform holds only one axis of rotation, we don't need the
// full YXZ Euler round-trip — `from_rotation_y` / `from_rotation_x` suffice.

fn on_look_fire(
    look: On<Fire<Look>>,
    mut bodies: Query<(&mut Transform, &Children), With<Player>>,
    mut cameras: Query<(&mut Transform, &FPSCamera), Without<Player>>,
) {
    let Ok((mut body_tf, children)) = bodies.get_mut(look.context) else { return };

    // Body yaw.
    let (yaw, _, _) = body_tf.rotation.to_euler(EulerRot::YXZ);
    // Sensitivity is pulled from whichever child is the FPS camera.
    // We need it before we can apply yaw, so peek at the first matching child.
    let sensitivity = children
        .iter()
        .find_map(|c| cameras.get(c).ok().map(|(_, cam)| cam.sensitivity))
        .unwrap_or(DEFAULT_SENSITIVITY);

    body_tf.rotation = Quat::from_rotation_y(yaw + look.value.x * sensitivity);

    // Camera pitch.
    for &child in children {
        if let Ok((mut cam_tf, _)) = cameras.get_mut(child) {
            let (_, pitch, _) = cam_tf.rotation.to_euler(EulerRot::YXZ);
            let new_pitch = (pitch + look.value.y * sensitivity)
                .clamp(-PITCH_LIMIT, PITCH_LIMIT);
            cam_tf.rotation = Quat::from_rotation_x(new_pitch);
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct PlayerCameraPlugin;

impl Plugin for PlayerCameraPlugin {
    fn build(&self, app: &mut App) {
        // A global observer rather than one attached at spawn: the action
        // carries its context entity, so this works for any number of
        // players without the spawner having to wire anything up.
        app.add_observer(on_look_fire);
    }
}
