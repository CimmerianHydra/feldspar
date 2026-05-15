use bevy::prelude::*;
use bevy::light::CascadeShadowConfigBuilder;

mod plugin;
use plugin::controller::freecamera::{FreeCameraPlugin, FreeCamera};
use plugin::geometry::meshing::MeshingPlugin;
use plugin::block_registry::BlockRegistryPlugin;
use plugin::block_interaction::{BlockInteractionPlugin, DDARay};
use plugin::chunk::ChunkPlugin;
use plugin::ui::UIPlugin;
use plugin::weather::WeatherPlugin;
use plugin::state::StatePlugin;
use plugin::controller::main::ControlsPlugin;
use plugin::inventory::main::InventoryPlugin;
use plugin::inventory::item_registry::ItemRegistryPlugin;
use plugin::graphics::block_material::{VoxelMaterialPlugin, VoxelMaterial};
use plugin::worldgen::main::WorldgenPlugin;
use plugin::controller::player::PlayerControllerPlugin;

use bevy::{input::common_conditions::input_toggle_active};
use bevy_inspector_egui::{bevy_egui::EguiPlugin, quick::WorldInspectorPlugin};
use avian3d::PhysicsPlugins;

fn main() {
    App::new()
        // Plugins
        .add_plugins(DefaultPlugins)
        .add_plugins(PhysicsPlugins::default())
        .add_plugins(StatePlugin)
        //.add_plugins(FreeCameraPlugin)
        .add_plugins(PlayerControllerPlugin)
        .add_plugins(ControlsPlugin)
        .add_plugins(VoxelMaterialPlugin)
        .add_plugins(MeshingPlugin)
        .add_plugins(ChunkPlugin)
        .add_plugins(UIPlugin)
        .add_plugins(BlockRegistryPlugin)
        .add_plugins(ItemRegistryPlugin)
        .add_plugins(InventoryPlugin)
        .add_plugins(BlockInteractionPlugin)
        .add_plugins(WorldgenPlugin)
        .add_plugins(WeatherPlugin)

        .add_plugins(EguiPlugin::default())
        .add_plugins(
            WorldInspectorPlugin::default().run_if(input_toggle_active(false, KeyCode::F3)),
        )

        // Game systems (that can't fit into any one previous plugin neatly)
        .add_systems(PreStartup, setup)

        // .add_systems(Update, debug_sys)

        // Only run this once to generate the world
        .add_systems(Update, crate::plugin::worldgen::main::setup_dev_chunks.run_if(run_once))

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
            illuminance: light_consts::lux::AMBIENT_DAYLIGHT,
            shadows_enabled: true,
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
            ..default()
        }.build(),
    ));
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