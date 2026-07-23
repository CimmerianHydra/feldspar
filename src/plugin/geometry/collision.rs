use avian3d::prelude::*;
use bevy::prelude::*;

use crate::plugin::chunk::{VoxelChunk, CHUNK_SIZE};
use crate::plugin::loader::block_registry::{BlockID, BlockRegistry};
use crate::plugin::space::prelude::ChunkSlot;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// CONFIG
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Mass per cubic block for grid chunks. Avian derives the grid's total
/// mass, center of mass, and inertia tensor from its colliders, so this is
/// the only mass knob a construct needs.
///
/// TODO: replace with per-voxel density from `BlockMaterial`, summed across
/// the chunk. A stone hull should out-mass a wooden one.
pub const CHUNK_COLLIDER_DENSITY: f32 = 0.4;

/// Force a collider rebuild. Insert this wherever your mesher *applies* a
/// finished mesh, so the trimesh path picks up the new geometry on the same
/// frame rather than one behind.
#[derive(Component, Default, Debug)]
pub struct NeedsColliderRebuild;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// THE SYSTEM
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Gives every chunk a collision shape appropriate to the body it belongs
/// to. Dimension-owned chunks build a trimesh, which is good for static objects
/// while MovingGrid-owned chunks build using a greedy algo.
pub fn update_chunk_collider_sys(
    mut commands: Commands,
    meshes: Res<Assets<Mesh>>,
    registry: Res<BlockRegistry>,
    chunks: Query<
        (Entity, &ChunkSlot, &VoxelChunk, Option<&Mesh3d>),
        Or<(Changed<VoxelChunk>, With<NeedsColliderRebuild>)>,
    >,
    bodies: Query<&RigidBody>,
) {
    for (entity, slot, chunk, mesh_handle) in chunks.iter() {
        let body = bodies.get(slot.space).copied().unwrap_or(RigidBody::Static);

        let collider = match body {
            // Static geometry: a trimesh off the render mesh. Hollow, but
            // that's fine for something that never moves, and it handles
            // slabs, stairs, and every other non-cube shape for free.
            RigidBody::Static => mesh_handle
                .and_then(|h| meshes.get(&h.0))
                .and_then(Collider::trimesh_from_mesh),

            // Moving bodies: boxes with real interior volume. A dynamic
            // body made of a trimesh tunnels through geometry and has no
            // usable inertia tensor, because a hollow surface has no
            // sensible mass distribution.
            _ => box_collider_from_chunk(chunk, &registry),
        };

        let mut e = commands.entity(entity);
        e.remove::<NeedsColliderRebuild>();

        match collider {
            Some(collider) => { e.insert((collider, ColliderDensity(CHUNK_COLLIDER_DENSITY))); }
            // An all-air chunk. Strip any stale shape rather than leaving
            // collision where blocks used to be.
            None => { e.remove::<Collider>(); }
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// VOXELS -> BOXES
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Compound of merged boxes covering the chunk's collidable voxels.
///
/// TODO: non-cube blocks are approximated as full cubes here. The fix is a
/// convex hull per `BlockShape`, excluded from the merge pass and appended
/// to the compound individually — the same treatment greedy meshing already
/// gives non-cube blocks on the render side.
pub fn box_collider_from_chunk(chunk: &VoxelChunk, registry: &BlockRegistry) -> Option<Collider> {
    let n = CHUNK_SIZE as usize;
    let mut solid = vec![false; n * n * n];

    for (local, voxel) in chunk.iter_non_air() {
        if registry.get(BlockID(voxel.id())).has_collision {
            solid[flat_index(local.x as usize, local.y as usize, local.z as usize)] = true;
        }
    }

    let boxes = greedy_boxes(&solid);
    if boxes.is_empty() { return None; }

    let shapes: Vec<(Vec3, Quat, Collider)> = boxes
        .into_iter()
        .map(|(min, size)| {
            let size = size.as_vec3();
            (
                min.as_vec3() + size * 0.5,
                Quat::IDENTITY,
                // Avian takes full side lengths here, not half-extents.
                Collider::cuboid(size.x, size.y, size.z),
            )
        })
        .collect();

    Some(Collider::compound(shapes))
}

#[inline]
fn flat_index(x: usize, y: usize, z: usize) -> usize {
    let n = CHUNK_SIZE as usize;
    (x * n + y) * n + z
}

/// Merge a solidity mask into as few axis-aligned boxes as a cheap greedy
/// pass can manage.
///
/// Same idea as greedy meshing, one dimension up: grow a run along Z, widen
/// it along Y while every cell in the run stays solid, then along X while
/// the whole slab does. A solid 16³ chunk collapses from 4096 cuboids to
/// one; a typical hull lands in the tens. That difference is the whole
/// reason this is worth writing rather than emitting a box per voxel — the
/// broad phase cost of a compound scales with its child count.
fn greedy_boxes(solid: &[bool]) -> Vec<(UVec3, UVec3)> {
    let n = CHUNK_SIZE as usize;
    let mut used = vec![false; n * n * n];
    let mut out = Vec::new();

    for x in 0..n {
        for y in 0..n {
            for z in 0..n {
                let start = flat_index(x, y, z);
                if !solid[start] || used[start] { continue; }

                // Grow along Z.
                let mut dz = 1;
                while z + dz < n {
                    let i = flat_index(x, y, z + dz);
                    if !solid[i] || used[i] { break; }
                    dz += 1;
                }

                // Widen along Y, one full Z-run at a time.
                let mut dy = 1;
                'grow_y: while y + dy < n {
                    for zz in z..z + dz {
                        let i = flat_index(x, y + dy, zz);
                        if !solid[i] || used[i] { break 'grow_y; }
                    }
                    dy += 1;
                }

                // Deepen along X, one full YZ-slab at a time.
                let mut dx = 1;
                'grow_x: while x + dx < n {
                    for yy in y..y + dy {
                        for zz in z..z + dz {
                            let i = flat_index(x + dx, yy, zz);
                            if !solid[i] || used[i] { break 'grow_x; }
                        }
                    }
                    dx += 1;
                }

                for xx in x..x + dx {
                    for yy in y..y + dy {
                        for zz in z..z + dz {
                            used[flat_index(xx, yy, zz)] = true;
                        }
                    }
                }

                out.push((
                    UVec3::new(x as u32, y as u32, z as u32),
                    UVec3::new(dx as u32, dy as u32, dz as u32),
                ));
            }
        }
    }

    out
}