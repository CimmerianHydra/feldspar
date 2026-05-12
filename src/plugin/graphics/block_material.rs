use bevy::{
    prelude::*, reflect::TypePath, render::render_resource::AsBindGroup, shader::ShaderRef,
};
use bevy::pbr::MaterialPipeline;
use bevy::material::descriptor::RenderPipelineDescriptor;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::MaterialPipelineKey;
use bevy::material::specialize::SpecializedMeshPipelineError;

use bevy::{
    asset::RenderAssetUsages,
    image::Image,
    mesh::{MeshVertexAttribute},
    pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline},
    prelude::*,
    render::render_resource::{
        Extent3d,
        TextureDimension, TextureFormat, VertexFormat,
    },
};

use crate::plugin::meshing::{ATTRIBUTE_TEXTURE_LAYER, ATTRIBUTE_OVERLAY_LAYER, ATTRIBUTE_OVERLAY_TINT};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// MATERIAL PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct VoxelMaterialPlugin;

impl Plugin for VoxelMaterialPlugin {
    fn build(&self, app: &mut App) {
        // Add systems related to weather effects here
        app
        .add_plugins(MaterialPlugin::<VoxelMaterial>::default())
        ;
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// VOXEL MATERIAL
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