use bevy::prelude::*;

mod plugins;
use plugins::player_controller::{PlayerControllerPlugin, PlayerController};
use plugins::camera_follow::{CameraFollowPlugin, CameraFollow};

#[derive(Component)]
struct Block;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(PlayerControllerPlugin)
        .add_plugins(CameraFollowPlugin)
        .add_systems(Startup, setup)
        // .add_systems(Update, debugging)
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
        Transform::from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
        CameraFollow { target : player, offset : Vec3::ZERO }
    ));
    
}

fn build_entity_default_player(
    commands : &mut Commands,
) -> Entity {
    commands
        .spawn((
            GlobalTransform::default(),
            Transform::default(),
            PlayerController::default(),
        ))
        .id()
}

fn debugging(
    mut q: Query<&mut Transform, With<PlayerController>>,
) {
    for t in &mut q {
        info!("The current player translation is: {:?}", t.translation)
    }
}