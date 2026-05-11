use bevy::{
    prelude::*, reflect::TypePath, render::render_resource::AsBindGroup, shader::ShaderRef,
};
use bevy::pbr::MaterialPipeline;
use bevy::material::descriptor::RenderPipelineDescriptor;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::MaterialPipelineKey;
use bevy::material::specialize::SpecializedMeshPipelineError;

use crate::plugin::meshing::{ATTRIBUTE_TEXTURE_LAYER, ATTRIBUTE_OVERLAY_LAYER, ATTRIBUTE_OVERLAY_TINT};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// MATERIAL PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct VoxelMaterialPlugin;

impl Plugin for VoxelMaterialPlugin {
    fn build(&self, app: &mut App) {
        // Add systems related to weather effects here
        app
        .add_plugins(MaterialPlugin::<VoxelBaseMaterial>::default())
        .add_plugins(MaterialPlugin::<CustomMaterial>::default())
        ;
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// VOXEL MATERIAL
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct VoxelBaseMaterial {
    #[texture(0, dimension = "2d_array")]
    #[sampler(1)]
    pub texture_array: Handle<Image>,

    #[texture(2, dimension = "2d_array")]
    #[sampler(3)]
    pub overlay_array: Handle<Image>,
}

impl Material for VoxelBaseMaterial {

    fn vertex_shader() -> ShaderRef {
        "shaders/voxel_base.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/voxel_base.wgsl".into()
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {

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
// VOXEL MATERIAL FOR LEARNING
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct CustomMaterial {
    #[texture(100, dimension = "2d_array")]
    #[sampler(101)]
    pub texture_array: Handle<Image>,
}

impl Material for CustomMaterial {

    fn vertex_shader() -> ShaderRef {
        "shaders/custom.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/custom.wgsl".into()
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {

        let vertex_layout = layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(2),

            ATTRIBUTE_TEXTURE_LAYER.at_shader_location(8),
        ])?;

        descriptor.vertex.buffers = vec![vertex_layout];

        Ok(())
    }
}
