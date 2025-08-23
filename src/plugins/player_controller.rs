use bevy::prelude::*;
use bevy::input::mouse::{MouseMotion};

pub const DEFAULT_PLAYER_SPEED : f32 = 10.0;
pub const DEFAULT_SENSITIVITY_X : f32 = 0.15;
pub const DEFAULT_SENSITIVITY_Y : f32 = 0.12;

pub struct PlayerControllerPlugin;

impl Plugin for PlayerControllerPlugin {
    fn build(&self, app: &mut App) {
        app

        .insert_resource(PlayerConfig {default_speed : DEFAULT_PLAYER_SPEED})
        .insert_resource(MouseConfig {sensitivity : Vec2::from((DEFAULT_SENSITIVITY_X, DEFAULT_SENSITIVITY_Y))});
    }
}

#[derive(Component, Default)]
pub struct PlayerController {
    yaw: f32,
    pitch: f32,
}

// Resources are global, thus we use this to set a default speed. Game mechanics may alter this.
#[derive(Resource)]
pub struct PlayerConfig {
    default_speed : f32,
}

// Resources are global, thus we use this to set a sens. This could potentially be loaded from config files.
#[derive(Resource)]
pub struct MouseConfig {
    sensitivity : Vec2,
}


// UPDATE
pub fn handle_input_movement(
    time: Res<Time>,
    input: Res<ButtonInput<KeyCode>>,
    cfg: Res<PlayerConfig>,
    mut q: Query<&mut Transform, With<PlayerController>>,
) {
    for mut t in &mut q {
        let mut dir = Vec3::ZERO;
        let forward : Vec3 = t.forward().into();
        let right : Vec3 = t.right().into();
        let speed_multiplier = cfg.default_speed;

        if input.pressed(KeyCode::KeyW) { dir += forward; }
        if input.pressed(KeyCode::KeyS) { dir -= forward; }
        if input.pressed(KeyCode::KeyD) { dir += right; }
        if input.pressed(KeyCode::KeyA) { dir -= right; }

        if dir != Vec3::ZERO {
            t.translation += dir.normalize() * speed_multiplier * time.delta_secs();
        }
    }
}

pub fn handle_input_mouse(
    mut motions: EventReader<MouseMotion>,
    cfg: Res<MouseConfig>,
    mut q: Query<(&mut Transform, &mut PlayerController)>,
) {
    // Accumulate mouse delta for this frame
    let mut delta = Vec2::ZERO;
    for m in motions.read() {
        delta += m.delta;
    }
    if delta == Vec2::ZERO {
        return;
    }

    for (mut t, mut ctl) in &mut q {
        ctl.yaw   -= delta.x * cfg.sensitivity.x;
        ctl.pitch -= delta.y * cfg.sensitivity.y;
        ctl.pitch = ctl.pitch.clamp(-89.9, 89.9);

        let yaw = Quat::from_rotation_y(ctl.yaw.to_radians());
        let pitch = Quat::from_rotation_x(ctl.pitch.to_radians());
        t.rotation = yaw * pitch;
    }
}