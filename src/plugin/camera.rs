use bevy::{input::mouse::AccumulatedMouseMotion, prelude::*};
use std::f32::consts::FRAC_PI_2;

use crate::plugin::state::GameUpdateState;

// Contains camera plugins to be used in both development and production builds.


// Plugin designed to provide a simple free camera system for development purposes.
// Just adding the plugin spawns a camera with the FreeCamera component, and the system handles movement based on keyboard input.
pub struct FreeCameraPlugin;

impl Plugin for FreeCameraPlugin {
    fn build(&self, app: &mut App) {
        app
        .add_systems(PreStartup, camera_spawn_sys)
        .add_systems(Update, (
            camera_mouse_sys,
            camera_movement_sys,
        ).run_if(in_state(GameUpdateState::Running)))
        ;
    }
}

#[derive(Component, Default)]
pub struct FreeCamera {
    pub speed: f32,
    pub sensitivity: f32,
}

pub fn camera_spawn_sys(mut commands: Commands) {
    // Spawn a camera with the FreeCamera component
    commands.spawn((
        Camera3d::default(),
        FreeCamera {speed: 5.0,
            sensitivity: 0.005
        },
        Transform::from_xyz(0.0, 0.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

pub fn camera_movement_sys(
    time: Res<Time>,
    mut query: Query<(&FreeCamera, &mut Transform)>,
    keyboard_input: Res<ButtonInput<KeyCode>>) {
    // Logic for camera movement here
    if let Ok((camera, mut transform)) = query.single_mut() {
        let mut direction = Vec3::ZERO;

        if keyboard_input.pressed(KeyCode::KeyW) {
            direction.z -= 1.0;
        }
        if keyboard_input.pressed(KeyCode::KeyS) {
            direction.z += 1.0;
        }
        if keyboard_input.pressed(KeyCode::KeyA) {
            direction.x -= 1.0;
        }
        if keyboard_input.pressed(KeyCode::KeyD) {
            direction.x += 1.0;
        }
        if keyboard_input.pressed(KeyCode::Space) {
            direction += transform.rotation.inverse() * Vec3::Y;
        }
        if keyboard_input.pressed(KeyCode::ShiftLeft) {
            direction -= transform.rotation.inverse() * Vec3::Y;
        }

        // Normalize the direction vector to ensure consistent movement speed
        if direction.length() > 0.0 {
            direction = direction.normalize();
            let rotation = transform.rotation;
            transform.translation += rotation * direction * camera.speed * time.delta_secs();
        }
    }
}

pub fn camera_mouse_sys(
    mut query: Query<(&FreeCamera, &mut Transform)>,
    mouse_motion: Res<AccumulatedMouseMotion>) {
    // Logic for camera rotation based on mouse movement here

    if let Ok((camera, mut transform)) = query.single_mut() {

        let delta_x = mouse_motion.delta.x * camera.sensitivity;
        let delta_y = mouse_motion.delta.y * camera.sensitivity;

        let (yaw, pitch, roll) = transform.rotation.to_euler(EulerRot::YXZ);
        let yaw = yaw - delta_x;

        // If the pitch was ±¹⁄₂ π, the camera would look straight up or down.
        // When the user wants to move the camera back to the horizon, which way should the camera face?
        // The camera has no way of knowing what direction was "forward" before landing in that extreme position,
        // so the direction picked will for all intents and purposes be arbitrary.
        // Another issue is that for mathematical reasons, the yaw will effectively be flipped when the pitch is at the extremes.
        // To not run into these issues, we clamp the pitch to a safe range.
        const PITCH_LIMIT: f32 = FRAC_PI_2 - 0.01;
        let pitch = (pitch - delta_y).clamp(-PITCH_LIMIT, PITCH_LIMIT);

        transform.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, roll);
    }
}