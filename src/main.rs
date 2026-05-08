use bevy::prelude::*;
use bevy::light::CascadeShadowConfigBuilder;

mod plugin;
use plugin::camera::{FreeCameraPlugin, FreeCamera};
use plugin::meshing::MeshingPlugin;
use plugin::block_registry::BlockRegistryPlugin;
use plugin::block_interaction::{BlockInteractionPlugin, DDARay};
use plugin::chunk::ChunkPlugin;
use plugin::ui::UIPlugin;
use plugin::weather::WeatherPlugin;
use plugin::state::StatePlugin;


fn main() {
    App::new()
        // Plugins
        .add_plugins(DefaultPlugins)
        .add_plugins(StatePlugin)
        .add_plugins(FreeCameraPlugin)
        .add_plugins(MeshingPlugin)
        .add_plugins(ChunkPlugin)
        .add_plugins(UIPlugin)
        .add_plugins(WeatherPlugin)
        .add_plugins(BlockInteractionPlugin)
        .add_plugins(BlockRegistryPlugin)

        // Game systems (that can't fit into any one previous plugin neatly)
        .add_systems(PreStartup, setup)
        .add_systems(Startup, add_dda_ray_to_camera)

        // .add_systems(Update, debug_sys)

        .run();
}

/// set up a simple 3D scene
fn setup(
    mut commands: Commands,
) {
    // light
    // directional 'sun' light
    commands.spawn((
        DirectionalLight {
            illuminance: light_consts::lux::OVERCAST_DAY,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform {
            translation: Vec3::new(0.0, 16.0, 0.0),
            rotation: Quat::from_rotation_x(- std::f32::consts::PI / 4.) * Quat::from_rotation_y(- std::f32::consts::PI / 4.),
            ..default()
        },
        // The default cascade config is designed to handle large scenes.
        // As this example has a much smaller world, we can tighten the shadow
        // bounds for better visual quality.
        CascadeShadowConfigBuilder {
            first_cascade_far_bound: 4.0,
            maximum_distance: 100.0,
            ..default()
        }
        .build(),
    ));
}

fn add_dda_ray_to_camera(mut commands: Commands, query: Query<Entity, With<FreeCamera>>) {
    if let Ok(entity) = query.single() {
        commands.entity(entity).insert(DDARay { max_distance: 5.0 });
    }
}

/*
fn debug_sys(
    camera_parameters: Query<(&DDARay, &Transform), With<FreeCamera>>,
) {
    if let Ok((ray, transform)) = camera_parameters.single() {
        bevy::log::info!("Camera position: {:?}, direction: {:?}", transform.translation, transform.rotation * Vec3::Z);
    }
}
*/
