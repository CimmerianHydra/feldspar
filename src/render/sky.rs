use bevy::light::CascadeShadowConfigBuilder;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::MaterialPipeline;
use bevy::pbr::MaterialPipelineKey;
use bevy::render::render_resource::RenderPipelineDescriptor;
use bevy::render::render_resource::SpecializedMeshPipelineError;
use bevy::{
    prelude::*, reflect::TypePath, render::render_resource::AsBindGroup, shader::ShaderRef,
};

use crate::GameState;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SKY MATERIAL
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

const SKY_SHADER_ASSET_PATH: &str = "shaders\\sky_shader.wgsl";

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct SkyMaterial {
    #[uniform(0)]
    horizon_color: LinearRgba,
    #[uniform(1)]
    top_color: LinearRgba,
}

impl Material for SkyMaterial {
    fn fragment_shader() -> ShaderRef {
        SKY_SHADER_ASSET_PATH.into()
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None; // render inside
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            depth.depth_write_enabled = false;
        }
        Ok(())
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SKY & SUN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Marker for sky boxes.
#[derive(Component)]
pub struct SkyBox;

fn spawn_sky(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<SkyMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(1000.0))), // very large
        MeshMaterial3d(materials.add(SkyMaterial {
            horizon_color: LinearRgba::new(0.7, 0.85, 1.0, 1.0),
            top_color: LinearRgba::new(0.2, 0.5, 1.0, 1.0),
        })),
        Transform::from_translation(Vec3::ZERO),
        SkyBox,
        bevy::light::NotShadowCaster, // Prevents casting shadows
    ));
}

/// Forces all skyboxes to follow the one Camera3d in the scene.
fn follow_camera(
    camera_query: Query<&Transform, With<Camera3d>>,
    mut sky_query: Query<&mut Transform, (With<SkyBox>, Without<Camera3d>)>,
) {
    if let Ok(camera_transform) = camera_query.single() {
        for mut transform in sky_query.iter_mut() {
            transform.translation = camera_transform.translation;
        }
    }
}

fn spawn_sunlight(mut commands: Commands) {
    commands.spawn((
        DirectionalLight {
            illuminance: light_consts::lux::AMBIENT_DAYLIGHT,
            shadows_enabled: true,
            ..default()
        },
        Transform {
            translation: Vec3::new(0.0, 16.0, 0.0),
            rotation: Quat::from_rotation_x(-std::f32::consts::PI / 4.)
                * Quat::from_rotation_y(-std::f32::consts::PI / 4.),
            ..default()
        },
        // The default cascade config is designed to handle large scenes.
        // As this world is much smaller, we can tighten the shadow bounds
        // for better visual quality.
        CascadeShadowConfigBuilder::default().build(),
    ));
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<SkyMaterial>::default())
            .add_systems(OnEnter(GameState::InGame), (spawn_sky, spawn_sunlight))
            .add_systems(Update, follow_camera);
    }
}
