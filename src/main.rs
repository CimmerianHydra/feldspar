use bevy::prelude::*;
use bevy::input::{gamepad, keyboard, mouse, touch};

mod plugins;
use plugins::player_controller::{
    PlayerControllerPlugin,
    PlayerController,
    handle_input_mouse,
    handle_input_movement,
};
use plugins::camera_follow::{
    CameraFollow,
    update_camera_transform_to_target,
};
use plugins::game_state::{GameState, PausePlugin};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(PausePlugin)
        .add_plugins(PlayerControllerPlugin)


        // Early setup (will eventually be removed)
        .add_systems(Startup, setup)


        // Player input handling
        .add_systems(PreUpdate,
            (
                        handle_input_mouse,
                        handle_input_movement,
                    )
                    .chain()
                    .run_if(in_state(GameState::Playing))
                    .after(mouse::mouse_button_input_system)
                    .after(keyboard::keyboard_input_system)
                    .after(gamepad::gamepad_event_processing_system)
                    .after(gamepad::gamepad_connection_system)
                    .after(touch::touch_screen_input_system)
        )

        // Camera Follow
        .add_systems(Update,
            (
                        update_camera_transform_to_target
                    )
                    .run_if(in_state(GameState::Playing))
        )

        // Run
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // circular base
    commands.spawn((
        Mesh3d(meshes.add(Circle::new(4.0))),
        MeshMaterial3d(materials.add(Color::WHITE)),
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    ));
    // cube
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(Color::srgb_u8(124, 144, 255))),
        Transform::from_xyz(0.0, 0.5, 0.0),
    ));
    // light
    commands.spawn((
        PointLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));

    let player = build_entity_default_player(&mut commands);

    // camera
    commands.spawn((
        Camera3d::default(),
        Transform::default(),
        CameraFollow { target : player, offset : Vec3::ZERO }
    ));
    
}

fn build_entity_default_player(
    commands : &mut Commands,
) -> Entity {
    commands
        .spawn((
            GlobalTransform::default(),
            Transform::from_xyz(-2.5, 4.5, 9.0),
            PlayerController::default(),
        ))
        .id()
}