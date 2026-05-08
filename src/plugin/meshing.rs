use bevy::prelude::*;
use bevy::render::mesh::{Mesh, Indices, PrimitiveTopology};
use bevy::asset::{RenderAssetUsages};

use crate::plugin::chunk::{CHUNK_SIZE, VoxelChunk, StaticChunk, NeedsRemeshing};
use crate::plugin::voxel::{BlockShape, Direction};
use crate::plugin::shape::shape_quads;

// Contains block meshing logic and plugins.
pub const BLOCK_SIZE : f32 = 1.0;
pub const BLOCK_HALF_SIZE : f32 = BLOCK_SIZE / 2.0;
pub const STATIC_MESH_DISPLACEMENT : Vec3 = Vec3::splat(BLOCK_HALF_SIZE); // To center the mesh on the block coordinates.

pub struct MeshingPlugin;

impl Plugin for MeshingPlugin {
    fn build(&self, app: &mut App) {
        // Add systems related to block meshing here
        app
        .add_systems(Update, add_transform_to_static_chunk_sys)
        .add_systems(Update, sync_static_chunk_transform_sys)
        .add_systems(Update, update_dirty_mesh_sys)
        ;
    }
}

// Systems related to block meshing can be defined here

fn update_dirty_mesh_sys(
    mut commands: Commands,
    chunk_query: Query<(Entity, &VoxelChunk), With<NeedsRemeshing>>,
    mut meshes: ResMut<Assets<Mesh>>,
    )
    {
    // Block mesh generation logic from chunk/grid data.

    // For every chunk that has changed, we will generate a mesh based on the block data.
    for (entity, voxel_chunk) in chunk_query.iter() {

        let new_handle = meshes.add(build_chunk_mesh(voxel_chunk));
        commands.entity(entity).insert(Mesh3d(new_handle));
        commands.entity(entity).remove::<NeedsRemeshing>();

    }
}

fn face_vertices(face: Direction) -> ([Vec3; 4], Vec3) {
    let normal = face.as_vec3();
    let vertices = match face {
        // Normals follow the CCW winding convention.
        Direction::Up => 
            [
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.0, 1.0, 1.0),
                Vec3::new(1.0, 1.0, 1.0),
                Vec3::new(1.0, 1.0, 0.0),
            ],
        Direction::Down =>
            [
                Vec3::new(0.0, 0.0, 1.0),
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 1.0),
            ],
        Direction::North =>
            [
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(1.0, 1.0, 0.0),
            ],
        Direction::South =>
            [
                Vec3::new(0.0, 0.0, 1.0),
                Vec3::new(1.0, 0.0, 1.0),
                Vec3::new(1.0, 1.0, 1.0),
                Vec3::new(0.0, 1.0, 1.0),
            ],
        Direction::East =>
            [
                Vec3::new(1.0, 0.0, 1.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(1.0, 1.0, 0.0),
                Vec3::new(1.0, 1.0, 1.0),
            ],
        Direction::West =>
            [
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
                Vec3::new(0.0, 1.0, 1.0),
                Vec3::new(0.0, 1.0, 0.0),
            ],
    };
    return (vertices, normal)
}

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
fn build_chunk_mesh(chunk: &VoxelChunk) -> Mesh {
    let mut positions       = Vec::<[f32; 3]>::new();
    let mut normals         = Vec::<[f32; 3]>::new();
    let mut uvs             = Vec::<[f32; 2]>::new();
    let mut indices              = Vec::<u32>::new();
    let mut index_offset              = 0u32;

    // UV layout: vertex 0→(0,0), 1→(1,0), 2→(1,1), 3→(0,1)
    let face_uvs = [[0.,0.],[1.,0.],[1.,1.],[0.,1.]];

    for (pos, voxel) in chunk.iter_non_air() {
        let quads = shape_quads(voxel.shape(), voxel.facing());

        for quad in &quads {
            let visible = match quad.face_dir {
                // Interior face (riser, inner slab wall): always render.
                None => true,
                Some(face_dir) => match neighbor_pos(pos, face_dir) {
                    // At the chunk boundary: always render.
                    None => true,
                    Some(npos) => {
                        let neighbor = chunk.get_local(npos);
                        // Cull only if the neighbour fully covers the shared boundary.
                        !neighbor.covers_face(face_dir.opposite())
                    }
                },
            };

            if !visible { continue; }

            for (i, &vert) in quad.verts.iter().enumerate() {
                positions.push((vert + pos.as_vec3()).to_array());
                normals.push(quad.normal.to_array());
                uvs.push(face_uvs[i]);
            }

            indices.extend_from_slice(&[
                index_offset,     index_offset + 1, index_offset + 2,
                index_offset,     index_offset + 2, index_offset + 3,
            ]);
            index_offset += 4;
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}


// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 4 – Helper Systems
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