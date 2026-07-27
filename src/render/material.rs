use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::render::render_resource::RenderPipelineDescriptor;
use bevy::render::render_resource::SpecializedMeshPipelineError;
use bevy::{
    image::Image,
    pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline},
};
use bevy::{prelude::*, render::render_resource::AsBindGroup, shader::ShaderRef};

use crate::content::texture::TextureArrays;
use crate::render::mesh::mesher::{
    ATTRIBUTE_OVERLAY_LAYER, ATTRIBUTE_OVERLAY_TINT, ATTRIBUTE_TEXTURE_LAYER,
};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – THE MATERIAL
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Convenience alias for the extended material type.
pub type VoxelMaterial = ExtendedMaterial<StandardMaterial, VoxelMaterialExtension>;

/// The "extension" part of the material. Adds a 2D-array texture binding to
/// `StandardMaterial`. Bind slots 0–99 are reserved for the base material, so
/// we start the extension's bindings at 100.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct VoxelMaterialExtension {
    #[texture(100, dimension = "2d_array")]
    #[sampler(101)]
    pub array_texture: Handle<Image>,

    #[texture(102, dimension = "2d_array")]
    #[sampler(103)]
    pub array_overlay: Handle<Image>,
}

impl MaterialExtension for VoxelMaterialExtension {
    fn vertex_shader() -> ShaderRef {
        "shaders/voxel_material.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/voxel_material.wgsl".into()
    }

    /// Wire the three custom attributes into the forward pipeline's vertex
    /// buffer layout. The prepass / shadow pipelines fall back to
    /// `StandardMaterial`'s built-in layout, which only needs position /
    /// normal / uv — our extra attributes are just along for the ride there.
    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let is_prepass_or_shadow = descriptor
            .label
            .as_ref()
            .is_some_and(|l| l.contains("prepass") || l.contains("shadow"));
        if is_prepass_or_shadow {
            return Ok(());
        }

        let vertex_layout = layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(2),
            ATTRIBUTE_TEXTURE_LAYER.at_shader_location(8),
            ATTRIBUTE_OVERLAY_LAYER.at_shader_location(9),
            ATTRIBUTE_OVERLAY_TINT.at_shader_location(10),
        ])?;
        descriptor.vertex.buffers = vec![vertex_layout];
        Ok(())
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – THE SHARED HANDLE
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The one voxel material every chunk and every block icon renders through.
#[derive(Resource, Default)]
pub struct VoxelMaterialHandle(pub Handle<VoxelMaterial>);

/// Build the material from the arrays `content::texture` assembled.
///
/// The split is the layering rule in miniature: assembling image layers and
/// naming them is content's job; deciding they are a PBR material with a
/// custom vertex layout is the renderer's.
pub fn build_voxel_material_sys(
    mut commands: Commands,
    arrays: Res<TextureArrays>,
    mut materials: ResMut<Assets<VoxelMaterial>>,
) {
    let handle = materials.add(VoxelMaterial {
        base: StandardMaterial {
            base_color:           Color::WHITE,
            metallic:             0.0,
            perceptual_roughness: 0.8,
            ..default()
        },
        extension: VoxelMaterialExtension {
            array_texture: arrays.base.clone(),
            array_overlay: arrays.overlay.clone(),
        },
    });
    commands.insert_resource(VoxelMaterialHandle(handle));

    info!("Generated global handle for voxel material.");
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct VoxelMaterialPlugin;

impl Plugin for VoxelMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<VoxelMaterial>::default())
            .init_resource::<VoxelMaterialHandle>();
    }
}
