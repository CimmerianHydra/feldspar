//! A free-flying camera for looking at things without a player body.
//!
//! Not added by `DevPlugin` — add `FreeCameraPlugin` yourself when you want
//! it, and drop `PlayerPlugin` in the same breath, since both would spawn a
//! `Camera3d`.

use bevy::{input::mouse::AccumulatedMouseMotion, prelude::*};
use std::f32::consts::FRAC_PI_2;

use crate::GameState;

#[derive(Component, Default)]
pub struct FreeCamera {
    pub speed: f32,
    pub sensitivity: f32,
}

pub fn camera_spawn_sys(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        FreeCamera { speed: 5.0, sensitivity: 0.005 },
        Transform::from_xyz(0.0, 4.0, 0.0).looking_at(Vec3::new(8.0, 0.0, 8.0), Vec3::Y),
        SpatialListener::default(),
    ));
}

pub fn camera_movement_sys(
    time: Res<Time>,
    mut query: Query<(&FreeCamera, &mut Transform)>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
) {
    let Ok((camera, mut transform)) = query.single_mut() else { return };

    let mut direction = Vec3::ZERO;

    if keyboard_input.pressed(KeyCode::KeyW) { direction.z -= 1.0; }
    if keyboard_input.pressed(KeyCode::KeyS) { direction.z += 1.0; }
    if keyboard_input.pressed(KeyCode::KeyA) { direction.x -= 1.0; }
    if keyboard_input.pressed(KeyCode::KeyD) { direction.x += 1.0; }
    if keyboard_input.pressed(KeyCode::Space) {
        direction += transform.rotation.inverse() * Vec3::Y;
    }
    if keyboard_input.pressed(KeyCode::ShiftLeft) {
        direction -= transform.rotation.inverse() * Vec3::Y;
    }

    // Normalize so diagonal movement isn't faster.
    if direction.length() > 0.0 {
        direction = direction.normalize();
        let rotation = transform.rotation;
        transform.translation += rotation * direction * camera.speed * time.delta_secs();
    }
}

pub fn camera_mouse_sys(
    mut query: Query<(&FreeCamera, &mut Transform)>,
    mouse_motion: Res<AccumulatedMouseMotion>,
) {
    let Ok((camera, mut transform)) = query.single_mut() else { return };

    let delta_x = mouse_motion.delta.x * camera.sensitivity;
    let delta_y = mouse_motion.delta.y * camera.sensitivity;

    let (yaw, pitch, roll) = transform.rotation.to_euler(EulerRot::YXZ);
    let yaw = yaw - delta_x;

    // At ±½π the camera looks straight up or down, and yaw effectively
    // flips — so clamp short of it.
    const PITCH_LIMIT: f32 = FRAC_PI_2 - 0.01;
    let pitch = (pitch - delta_y).clamp(-PITCH_LIMIT, PITCH_LIMIT);

    transform.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, roll);
}

pub struct FreeCameraPlugin;

impl Plugin for FreeCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, camera_spawn_sys)
            .add_systems(
                Update,
                (camera_mouse_sys, camera_movement_sys).run_if(in_state(GameState::InGame)),
            );
    }
}
