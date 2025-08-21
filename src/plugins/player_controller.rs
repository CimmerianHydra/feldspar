use bevy::prelude::*;
use bevy::input::mouse::{MouseMotion};

pub struct PlayerControllerPlugin;

impl Plugin for PlayerControllerPlugin {
    fn build(&self, app: &mut App) {
        use bevy::input::{gamepad, keyboard, mouse, touch};
        app

        .insert_resource(PlayerConfig {default_speed : 5.0})
        .insert_resource(MouseConfig {sensitivity : 0.12})

        .add_systems(PreUpdate,
            (
                handle_input_mouse,
                handle_input_movement,
            )
                .chain()
                .after(mouse::mouse_button_input_system)
                .after(keyboard::keyboard_input_system)
                .after(gamepad::gamepad_event_processing_system)
                .after(gamepad::gamepad_connection_system)
                .after(touch::touch_screen_input_system),
        );
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
    sensitivity : f32,
}


// FUNCTIONALITIES
pub fn mouse_lock() {todo!()}
pub fn mouse_release() {todo!()}


// UPDATE
fn handle_input_movement(
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

fn handle_input_mouse(
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
        ctl.yaw   -= delta.x * cfg.sensitivity;
        ctl.pitch -= delta.y * cfg.sensitivity;
        ctl.pitch = ctl.pitch.clamp(-89.9, 89.9);

        let yaw = Quat::from_rotation_y(ctl.yaw.to_radians());
        let pitch = Quat::from_rotation_x(ctl.pitch.to_radians());
        t.rotation = yaw * pitch;
    }
}