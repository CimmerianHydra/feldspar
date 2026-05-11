use bevy::prelude::*;
use bevy::render::mesh::{Mesh, Indices, PrimitiveTopology};
use bevy::asset::{RenderAssetUsages};

use crate::plugin::chunk::{CHUNK_SIZE, VoxelChunk, StaticChunk, NeedsRemeshing};
use crate::plugin::graphics::block_material::VoxelBaseMaterial;
use crate::plugin::graphics::block_textures::{BlockAppearance, FaceTextures};
use crate::plugin::state::GameUpdateState;
use crate::plugin::voxel::{BlockShape, Direction};
use crate::plugin::shape::shape_quads;
use crate::plugin::block_registry::{BlockID, BlockDefinition, BlockRegistry};

use bevy::mesh::MeshVertexAttribute;
use bevy::render::render_resource::VertexFormat;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct MeshingPlugin;

impl Plugin for MeshingPlugin {
    fn build(&self, app: &mut App) {
        // Add systems related to block meshing here
        app
        .add_systems(Update, (
            add_transform_to_static_chunk_sys,
            sync_static_chunk_transform_sys,
            update_dirty_mesh_sys
        ).run_if(in_state(GameUpdateState::Running)))
        ;
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// BASIC DEFINITIONS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━


pub const BLOCK_SIZE : f32 = 1.0;

pub const ATTRIBUTE_TEXTURE_LAYER: MeshVertexAttribute =
    MeshVertexAttribute::new(
        "Texture_Layer",
        1000,
        VertexFormat::Uint32,
    );

pub const ATTRIBUTE_OVERLAY_LAYER: MeshVertexAttribute =
    MeshVertexAttribute::new(
        "Overlay_Layer",
        1001,
        VertexFormat::Uint32,
    );

pub const ATTRIBUTE_OVERLAY_TINT: MeshVertexAttribute =
    MeshVertexAttribute::new(
        "Overlay_Tint",
        1002,
        VertexFormat::Float32x4,
    );

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// UPDATE SCHEDULE SYSTEMS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

// Systems related to block meshing can be defined here
fn update_dirty_mesh_sys(
    mut commands: Commands,
    chunk_query: Query<(Entity, &VoxelChunk), With<NeedsRemeshing>>,
    mut meshes: ResMut<Assets<Mesh>>,
    block_registry: Res<BlockRegistry>,
    )  {
    // Block mesh generation logic from chunk/grid data.

    // For every chunk that has changed, we will generate a mesh based on the block data.
    for (entity, voxel_chunk) in chunk_query.iter() {

        let new_handle = meshes.add(build_chunk_mesh(voxel_chunk, &block_registry));
        commands.entity(entity).insert(Mesh3d(new_handle));
        commands.entity(entity).remove::<NeedsRemeshing>();

    }
}


// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// MESHING - CORE FUNCTIONS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━


/// Helper function to figure out the correct texture to give each face.
fn resolve_face_texture<'a>(
    appearance: &'a BlockAppearance,
    face_dir: Option<Direction>,
) -> &'a FaceTextures {
    match appearance {
        BlockAppearance::Uniform(ft) => ft,

        BlockAppearance::TopBottomSides { up, down, side } => match face_dir {
            Some(Direction::Up)   => up,
            Some(Direction::Down) => down,
            _                     => side, // sides AND interior (None) faces
        },

        BlockAppearance::PerFace { up, down, north, south, east, west } => {
            match face_dir {
                Some(Direction::Up)    => up,
                Some(Direction::Down)  => down,
                Some(Direction::North) => north,
                Some(Direction::South) => south,
                Some(Direction::East)  => east,
                Some(Direction::West)  => west,
                None => up, // interior face fallback — pick any side
            }
        }
    }
}

/// Helper function that figures out whether a certain block has a neighbor
/// in the same chunk in the specified direction.
fn neighbor_pos(pos: UVec3, face: Direction) -> Option<UVec3> {
    let (x, y, z) = (pos.x as i32, pos.y as i32, pos.z as i32);

    let offset = face.as_ivec3();

    let np = IVec3::new(x, y, z) + offset;

    if np.x < 0 || np.y < 0 || np.z < 0 ||
       np.x >= CHUNK_SIZE as i32 ||
       np.y >= CHUNK_SIZE as i32 ||
       np.z >= CHUNK_SIZE as i32 {
        return None;
    }

    Some(np.as_uvec3())
}


// TODO: Implement greedy meshing and face culling to optimize block rendering, making use of the shapes and blockstates.

fn build_chunk_mesh(chunk: &VoxelChunk, registry: &BlockRegistry) -> Mesh {
    let mut positions      = Vec::<[f32; 3]>::new();
    let mut normals        = Vec::<[f32; 3]>::new();
    let mut uvs            = Vec::<[f32; 2]>::new();
    let mut indices             = Vec::<u32>::new();
    let mut texture_layers      = Vec::<u32>::new();
    let mut overlay_layers      = Vec::<u32>::new();
    let mut overlay_tints  = Vec::<[f32; 4]>::new();
    let mut index_offset   = 0u32;

    let face_uvs = [[0.,0.],[1.,0.],[1.,1.],[0.,1.]];

    for (pos, voxel) in chunk.iter_non_air() {
        let block_def  = registry.get(BlockID(voxel.id()));
        let appearance = &block_def.appearance;
        let quads      = shape_quads(voxel.shape(), voxel.facing());

        for quad in &quads {
            let visible = match quad.face_dir {
                None => true, // It's an internal face, so we need to always render it
                Some(face_dir) => match neighbor_pos(pos, face_dir) {
                    None    => true,
                    Some(npos) => {
                        let neighbor = chunk.get_local(npos);
                        !neighbor.covers_face(face_dir.opposite())
                    }
                },
            };
            if !visible { continue; }

            // ── resolve texture data for this quad ────────────────────────
            let face_tex = resolve_face_texture(appearance, quad.face_dir);
            let (base_layer, ov_layer, tint): (u32, u32, [f32; 4]) = match face_tex {
                FaceTextures::Default(b, o) => (
                    *b, *o,
                    [1.0, 1.0, 1.0, 1.0],
                ),
                FaceTextures::Tinted(b, o, color) => (
                    *b, *o,
                    // Convert Bevy Color to a linear [f32; 4] for the shader
                    {
                        let c = color.to_linear();
                        [c.red, c.green, c.blue, c.alpha]
                    },
                ),
            };

            for (i, &vert) in quad.verts.iter().enumerate() {
                positions.push((vert + pos.as_vec3()).to_array());
                normals.push(quad.normal.to_array());
                uvs.push(face_uvs[i]);
                texture_layers.push(base_layer);
                overlay_layers.push(ov_layer);
                overlay_tints.push(tint);
            }

            indices.extend_from_slice(&[
                index_offset,     index_offset + 1, index_offset + 2,
                index_offset,     index_offset + 2, index_offset + 3,
            ]);
            index_offset += 4;
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION,   positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL,     normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0,       uvs);
    mesh.insert_attribute(ATTRIBUTE_TEXTURE_LAYER,    texture_layers);
    mesh.insert_attribute(ATTRIBUTE_OVERLAY_LAYER,    overlay_layers);
    mesh.insert_attribute(ATTRIBUTE_OVERLAY_TINT,     overlay_tints);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}


// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SYSTEMS AT STARTUP PHASE
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn sync_static_chunk_transform_sys(
    mut query: Query<(&StaticChunk, &mut Transform), Changed<StaticChunk>>,
) {
    for (chunk, mut transform) in &mut query {
        transform.translation =
            (chunk.position * CHUNK_SIZE as i32).as_vec3();
    }
}

fn add_transform_to_static_chunk_sys(
    mut commands: Commands,
    query: Query<(Entity, &StaticChunk), Added<StaticChunk>>,
) {
    for (entity, chunk) in &query {
        let translation =
            (chunk.position * CHUNK_SIZE as i32).as_vec3();

        commands.entity(entity).insert(
            Transform::from_translation(translation)
        );
    }
}