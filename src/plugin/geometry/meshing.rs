use avian3d::collision::collider::Collider;
use avian3d::dynamics::rigid_body::RigidBody;
use bevy::prelude::*;
use bevy::mesh::{Mesh, Indices, PrimitiveTopology};
use bevy::asset::{RenderAssetUsages};

use crate::plugin::chunk::{CHUNK_SIZE, VoxelChunk, StaticChunk, NeedsRemeshing};
use crate::plugin::graphics::block_textures::{BlockAppearance, FaceTextures};
use crate::plugin::state::GameUpdateState;
use crate::plugin::voxel::{Direction};
use crate::plugin::geometry::quads::{Quad, shape_quads};
use crate::plugin::block_registry::{BlockID, BlockRegistry};

use bevy::mesh::MeshVertexAttribute;
use bevy::render::render_resource::VertexFormat;

use avian3d::prelude::Position;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct MeshingPlugin;

impl Plugin for MeshingPlugin {
    fn build(&self, app: &mut App) {
        // Add systems related to block meshing here
        app
        .add_systems(Update, (
            add_components_to_static_chunk_sys,
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

fn update_dirty_mesh_sys(
    mut commands: Commands,
    chunk_query: Query<(Entity, &VoxelChunk, Option<&Mesh3d>, Option<&Collider>), With<NeedsRemeshing>>,
    mut meshes: ResMut<Assets<Mesh>>,
    block_registry: Res<BlockRegistry>,
) {
    for (
        entity,
        voxel_chunk,
        existing_mesh,
        existing_collider,
    ) in chunk_query.iter() {
        let mut e = commands.entity(entity);

        // Quirk of Bevy: an empty mesh kinda breaks the system.
        // So if the chunk is all air, we need to remove the mesh entirely.
        // is_all_air is short circuiting, so we shouldn't be afraid to use it: we won't be
        // looping over every chunk. Especially the chunks that are completely full only add one operation.

        // Note to self: removing the mesh and the collider, or adding them, is very slow.
        // this causes some desyncs when the mesh needs to be generated from empty and added to the chunk entity.
        // Need to find a more performant way to keep the chunks "ready to have a mesh and collider" rather than
        // adding or removing them.
        // Still, this is fine for now, Bevy handles it async.

        if voxel_chunk.is_all_air() {
            // No geometry; drop any stale mesh handle.
            if existing_mesh.is_some() {
                e.remove::<Mesh3d>();
            }
            if existing_collider.is_some() {
                e.remove::<Collider>();
            }
        } else {
            let (new_mesh, new_collider) = build_chunk_data(voxel_chunk, &block_registry);
            e.insert(Mesh3d(meshes.add(new_mesh)));
            e.insert((new_collider));
        }

        e.remove::<NeedsRemeshing>();
    }
}


// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// MESHING - CORE FUNCTIONS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Helper function to figure out the correct texture to give each face.
fn resolve_face_texture(
    appearance: &BlockAppearance,
    face_dir: Direction,
    is_internal: bool,
) -> &FaceTextures {
    match appearance {
        BlockAppearance::Uniform(ft) => ft,

        BlockAppearance::TopBotSide { up, down, side } => match face_dir {
            Direction::Up   => up,
            Direction::Down => down,
            _               => side, // sides AND interior (None) faces
        },

        BlockAppearance::PerFace { up, down, north, south, east, west } => {
            match face_dir {
                Direction::Up    => up,
                Direction::Down  => down,
                Direction::North => north,
                Direction::South => south,
                Direction::East  => east,
                Direction::West  => west,
            }
        }
        BlockAppearance::UniformWithInternal{ ext, int} => {
            if is_internal { int } else { ext }
        }
    }
}

/// Helper function to figure out the correct texture data to provide to the voxel terrain shader.
fn resolve_texture_properties(face_texture: &FaceTextures) -> (u32, u32, [f32; 4]) {
    match face_texture {
        FaceTextures::Simple(b) => (
            *b, 0,
            [1.0, 1.0, 1.0, 1.0],
        ),
        FaceTextures::Bilayer(b, o) => (
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
    }
}

/// Helper function that figures out whether a certain block has a neighbor
/// in the same chunk in the specified direction.
/// All local coordinates only.
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

/// Helper function to figure out whether the given quad is visible, knowing the neighbor it's looking at.
/// TODO: include a more sophisticated coverage system to check visibility.
/// TODO: include chunk boundaries (augment voxelchunk with some "chunk context" input data).
fn is_visible(quad: &Quad, chunk: &VoxelChunk, pos: UVec3) -> bool {
    let visible = match quad.culling_direction {
        None => true, // It's an internal face, so we need to always render it
        Some(dir) => match neighbor_pos(pos, dir) {
            None  => true, // for now, and only for now, render the face if it's at the boundary of the chunk
            Some(npos) => {
                let neighbor = chunk.get_local(npos);
                !neighbor.covers_face(dir.opposite())
            }
        },
    };
    visible
}

/// Helper struct to pass around data exclusively related to meshing.
struct MeshingData {
    indices: Vec<u32>,
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    texture_layers: Vec<u32>,
    overlay_layers: Vec<u32>,
    overlay_tints: Vec<[f32; 4]>,
}

/// Helper struct to pass around data exclusively related to colliders.
struct ColliderData {
    indices: Vec<u32>,
    positions: Vec<[f32; 3]>,
}


// TODO: Implement greedy meshing and face culling to optimize block rendering, making use of the shapes and blockstates.
// TODO: Separate rendering and physics pipelines in a smarter way
fn build_chunk_data(chunk: &VoxelChunk, registry: &BlockRegistry) -> (Mesh, Collider) {
    let mut positions      = Vec::<[f32; 3]>::new();
    let mut normals        = Vec::<[f32; 3]>::new();
    let mut uvs            = Vec::<[f32; 2]>::new();
    let mut mesh_indices        = Vec::<u32>::new();
    let mut texture_layers      = Vec::<u32>::new();
    let mut overlay_layers      = Vec::<u32>::new();
    let mut overlay_tints  = Vec::<[f32; 4]>::new();
    let mut index_offset   = 0u32;

    let mut collider_indices   = Vec::<[u32; 3]>::new();
    let mut collider_positions     = Vec::<Vec3>::new();


    for (pos, voxel) in chunk.iter_non_air() {
        let block_id = BlockID(voxel.id());
        let block_def  = registry.get(block_id);
        let appearance = &block_def.appearance;
        let has_collision = block_def.has_collision;

        let quads      = shape_quads(voxel.shape(), voxel.facing());

        for quad in &quads {
            // ── Check visibility    WIP    ────────────────────────────────
            if !is_visible(quad, chunk, pos) { continue; }

            // ── Resolve texture data for this quad ────────────────────────
            let is_internal = quad.culling_direction == None;
            let face_tex = resolve_face_texture(appearance, quad.texture_direction, is_internal);
            let (base_layer, ov_layer, tint) = resolve_texture_properties(face_tex);

            // ── Push all mesh data ────────────────────────────────────────
            for (i, &vert) in quad.verts.iter().enumerate() {
                positions.push((vert + pos.as_vec3()).to_array());
                normals.push(quad.normal.to_array());
                uvs.push(quad.uvs[i].to_array());
                texture_layers.push(base_layer);
                overlay_layers.push(ov_layer);
                overlay_tints.push(tint);
            }

            mesh_indices.extend_from_slice(&[
                index_offset,     index_offset + 1, index_offset + 2,
                index_offset,     index_offset + 2, index_offset + 3,
            ]);


            // ── Push all collision data ────────────────────────────────────────
            if has_collision {
                for &vert in quad.verts.iter() {
                    collider_positions.push(vert + pos.as_vec3());
                }
                collider_indices.push([
                    index_offset,     index_offset + 1, index_offset + 2,
                ]);

                // Skip the degenerate triangle for the collider — Parry's trimesh
                // builder doesn't need it and zero-area faces can confuse contact normals.
                if quad.verts[2] != quad.verts[3] {
                        collider_indices.push([
                        index_offset,     index_offset + 1, index_offset + 2,
                    ]);
                }
            }

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
    mesh.insert_indices(Indices::U32(mesh_indices));

    let collider = Collider::trimesh(collider_positions, collider_indices);

    (mesh, collider)
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

fn add_components_to_static_chunk_sys(
    mut commands: Commands,
    query: Query<(Entity, &StaticChunk), Added<StaticChunk>>,
) {
    for (entity, chunk) in &query {
        let translation =
            (chunk.position * CHUNK_SIZE as i32).as_vec3();

        commands.entity(entity).insert(
            Transform::from_translation(translation)
        );
        commands.entity(entity).insert(
            (
                RigidBody::Static,
                Position::from(translation)
            )
        );
    }
}