use bevy::prelude::*;
use bevy::ecs::system::SystemParam;
use std::collections::HashMap;

use crate::plugin::voxel::Voxel;
use crate::plugin::dimension::DimensionId;

use crate::plugin::voxel::{BlockShape, Direction};

// Contains chunk logic and plugins.

pub struct ChunkPlugin;

impl Plugin for ChunkPlugin {
    fn build(&self, app: &mut App) {
        app
            // ---- resources -------------------------------------------------
            .init_resource::<StaticWorld>()
            // ---- events ----------------------------------------------------
            
            // ---- reflect (useful for Bevy's inspector / serialization) ------
            .register_type::<StaticChunk>()
            .register_type::<MovingGrid>()

            // ---- systems ---------------------------------------------------
            // PreUpdate: registry must be ready before game logic runs.
            .add_systems(PreUpdate,register_new_chunks_sys)
            .add_systems(PreUpdate,unregister_removed_chunks_sys)
            // Startup: spawn a test chunk for development.
            .add_systems(Startup, spawn_test_chunk)
        ;
    }
}

// Example function to create a test chunk with the specified block ID
fn spawn_test_chunk(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut chunk_data = VoxelChunk::empty();
    let material_handle = materials.add(StandardMaterial::default());

    // Fill the bottom layer with stone (id = 1).
    for x in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            chunk_data.set(x, 0, z, Voxel::full(1));
        }
    }

    // Add a slab on top
    let slab = Voxel::new(1, BlockShape::Slope, Direction::North);
    chunk_data.set(8, 1, 8, slab);

    let dim_id = DimensionId::OVERWORLD;
    
    commands.spawn((
        StaticChunk {
            dimension: dim_id,
            position:  IVec3::new(0, 0, 0),
        },
        chunk_data.clone(),
        MeshMaterial3d(material_handle.clone()),
        NeedsRemeshing,
    ));
    commands.spawn((
        StaticChunk {
            dimension: dim_id,
            position:  IVec3::new(0, 0, 1),
        },
        chunk_data.clone(),
        MeshMaterial3d(material_handle.clone()),
        NeedsRemeshing,
    ));
    commands.spawn((
        StaticChunk {
            dimension: dim_id,
            position:  IVec3::new(1, 0, 0),
        },
        chunk_data.clone(),
        MeshMaterial3d(material_handle.clone()),
        NeedsRemeshing,
    ));
    commands.spawn((
        StaticChunk {
            dimension: dim_id,
            position:  IVec3::new(1, 0, 1),
        },
        chunk_data.clone(),
        MeshMaterial3d(material_handle.clone()),
        NeedsRemeshing,
    ));
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – VOXEL CHUNK  (chunk.rs)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub const CHUNK_SIZE:   usize = 16;
pub const CHUNK_VOLUME: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE; // 4096

/// Dense 16 × 16 × 16 voxel storage component.
///
/// Shared by both **static world chunks** and **moving grids** — the only
/// difference is which marker component sits alongside it.
///
/// ## Indexing
///
/// Local positions are in `[0, 15]³`.  The flat index is:
///
/// ```
/// index = x  |  (y << 4)  |  (z << 8)
///       = x  +   y * 16   +   z * 256
/// ```
///
/// X changes fastest (cache-friendly for east-west sweeps).

#[derive(Component, Debug, Clone)]
pub struct VoxelChunk {
    voxels: Box<[Voxel; CHUNK_VOLUME]>,
}

impl VoxelChunk {
    pub fn empty() -> Self {
        Self {
            voxels: Box::new([Voxel::AIR; CHUNK_VOLUME])
        }
    }

    /// Fill every voxel with the same block (useful for solid test chunks).
    pub fn filled(voxel: Voxel) -> Self {
        Self {
            voxels: Box::new([voxel; CHUNK_VOLUME])
        }
    }

    // ---- index helpers ------------------------------------------------------

    /// Converts (x, y, z) in [0,15] to a flat array index.
    ///
    /// Uses bit-ops for zero-cost conversion (CHUNK_SIZE is a power of two).
    #[inline(always)]
    fn idx(x: usize, y: usize, z: usize) -> usize {
        debug_assert!(x < CHUNK_SIZE, "x={x} out of bounds");
        debug_assert!(y < CHUNK_SIZE, "y={y} out of bounds");
        debug_assert!(z < CHUNK_SIZE, "z={z} out of bounds");
        x | (y << 4) | (z << 8)
    }

    // ---- read ---------------------------------------------------------------

    #[inline] pub fn get(&self, x: usize, y: usize, z: usize) -> Voxel {
        self.voxels[Self::idx(x, y, z)]
    }

    #[inline] pub fn get_local(&self, p: UVec3) -> Voxel {
        self.get(p.x as usize, p.y as usize, p.z as usize)
    }

    // ---- write --------------------------------------------------------------

    #[inline] pub fn set(&mut self, x: usize, y: usize, z: usize, v: Voxel) {
        self.voxels[Self::idx(x, y, z)] = v;
    }

    #[inline] pub fn set_local(&mut self, p: UVec3, v: Voxel) {
        self.set(p.x as usize, p.y as usize, p.z as usize, v);
    }

    // ---- iteration ----------------------------------------------------------

    /// Iterate every non-air voxel as `(local_pos, voxel)`.
    pub fn iter_non_air(&self) -> impl Iterator<Item = (UVec3, Voxel)> + '_ {
        self.voxels.iter().enumerate().filter_map(|(i, &v)| {
            if v.is_air() { return None; }
            let x = (i        & 0xF) as u32;
            let y = ((i >> 4) & 0xF) as u32;
            let z = ((i >> 8) & 0xF) as u32;
            Some((UVec3::new(x, y, z), v))
        })
    }

    /// Raw slice access (e.g. for bulk copy into a mesh buffer).
    #[inline] pub fn raw(&self) -> &[Voxel; CHUNK_VOLUME] { &self.voxels }
}

#[derive(Component)]
pub struct NeedsRemeshing;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 4 – MARKER COMPONENTS  (markers.rs)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Marks a `VoxelChunk` entity as a fixed, static world chunk.
///
/// The `register_new_chunks` system automatically inserts this into
/// `VoxelWorld` so lookups by coordinate work immediately.
///
/// ## Required bundle
/// ```rust
/// commands.spawn((
///     StaticChunk { dimension: DimensionId::OVERWORLD, position: IVec3::ZERO },
///     VoxelChunk::empty(),
/// ));
/// ```

#[derive(Component, Reflect)]
pub struct StaticChunk {
    /// Which dimension this chunk belongs to.
    pub dimension: DimensionId,
    /// Chunk-space position.  
    /// `world_block_pos = chunk_pos * 16 + local_pos`.
    pub position: IVec3,
}

// ---------------------------------------------------------------------------

/// Marks a `VoxelChunk` entity as a **moving** voxel grid (vehicle, ship, etc.).
///
/// Pair with `Transform` for world placement and with your physics body
/// component (e.g. `RigidBody` from `bevy_rapier`) for physics simulation.
///
/// For single-chunk vehicles just add `VoxelChunk`; for larger constructs
/// also add `MovingGridChunks`.
///
/// ## Required bundle (single-chunk)
/// ```rust
/// commands.spawn((
///     MovingGrid::default(),
///     VoxelChunk::empty(),
///     Transform::default(),
///     // RigidBody::Dynamic,  ← your physics crate
/// ));
/// ```
#[derive(Component, Reflect, Default)]
pub struct MovingGrid {
    /// Size in chunks.  `UVec3::ONE` (the default) = a single 16³ chunk.
    pub chunk_extent: UVec3,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 5 – STATIC WORLD RESOURCE
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Global registry of all loaded static chunks.
/// If a chunk is not in here, it's not loaded, and any access attempts should return `Voxel::AIR`.
///
/// Provides **O(1)** lookup of the `Entity` that owns a
/// `(DimensionId, chunk_pos)` pair.
///
/// You usually never touch this manually — the `register_new_chunks`
/// and `unregister_removed_chunks` systems keep it in sync automatically.


#[derive(Resource, Default)]
pub struct StaticWorld {
    chunks: HashMap<(DimensionId, IVec3), Entity>,
}

impl StaticWorld {
    // ---- registry (called by ECS systems) -----------------------------------

    pub fn insert(&mut self, dim: DimensionId, chunk_pos: IVec3, entity: Entity) {
        self.chunks.insert((dim, chunk_pos), entity);
    }

    pub fn remove(&mut self, dim: DimensionId, chunk_pos: IVec3) {
        self.chunks.remove(&(dim, chunk_pos));
    }

    /// Returns the `Entity` that owns the chunk at `chunk_pos` in `dim`,
    /// or `None` if the chunk is not currently loaded.
    #[inline]
    pub fn chunk_entity(&self, dim: DimensionId, chunk_pos: IVec3) -> Option<Entity> {
        self.chunks.get(&(dim, chunk_pos)).copied()
    }

    /// Returns all loaded chunk positions for a given dimension.
    pub fn loaded_chunks(&self, dim: DimensionId) -> impl Iterator<Item = IVec3> + '_ {
        self.chunks.keys()
            .filter(move |(d, _)| *d == dim)
            .map(|(_, p)| *p)
    }

    // ---- coordinate math (pure, no &self needed) ----------------------------

    /// Convert a **block-space world position** into `(chunk_pos, local_pos)`.
    ///
    /// Uses Euclidean (floor) division, so negative coordinates are handled
    /// correctly: block `(-1, 0, 0)` maps to chunk `(-1, 0, 0)`, local `(15, 0, 0)`.
    #[inline]
    pub fn to_chunk_local(world_block: IVec3) -> (IVec3, UVec3) {
        let s = CHUNK_SIZE as i32;
        let chunk = world_block.div_euclid(IVec3::splat(s));
        // After floor-div, remainder is always in [0, CHUNK_SIZE).
        let local = (world_block - chunk * s).as_uvec3();
        (chunk, local)
    }

    /// Inverse of `to_chunk_local`.
    #[inline]
    pub fn to_world_block(chunk_pos: IVec3, local: UVec3) -> IVec3 {
        chunk_pos * CHUNK_SIZE as i32 + local.as_ivec3()
    }

    /// World-block-space bounding box (inclusive min, exclusive max) of a chunk.
    pub fn chunk_bounds(chunk_pos: IVec3) -> (IVec3, IVec3) {
        let min = chunk_pos * CHUNK_SIZE as i32;
        (min, min + IVec3::splat(CHUNK_SIZE as i32))
    }

}

/// StaticWorldAccess and StaticWorldAccessMut provide (immutable or mutable, respectively) access
/// to the StaticWorld resource. It should be added as an input parameter to all systems that need
/// to be able to access the chunk data.

#[derive(SystemParam)]
pub struct StaticWorldAccess<'w, 's> {
    world: Res<'w, StaticWorld>,
    chunks: Query<'w, 's, &'static VoxelChunk>,
}

impl<'w, 's> StaticWorldAccess<'w, 's> {
    pub fn get_voxel(
        &self,
        world_pos: IVec3,
        dimension: DimensionId,
    ) -> Voxel {
        let (chunk_pos, local_pos) = StaticWorld::to_chunk_local(world_pos);

        if let Some(entity) = self.world.chunk_entity(dimension, chunk_pos) {
            if let Ok(chunk) = self.chunks.get(entity) {
                return chunk.get_local(local_pos);
            }
        }
        Voxel::AIR
    }

    pub fn get_chunk_entity(
        &self,
        world_pos: IVec3,
        dimension: DimensionId,
    ) -> Option<Entity> {
        let (chunk_pos, local_pos) = StaticWorld::to_chunk_local(world_pos);
        self.world.chunk_entity(dimension, chunk_pos)
    }
}

#[derive(SystemParam)]
pub struct StaticWorldAccessMut<'w, 's> {
    world: Res<'w, StaticWorld>,
    chunks: Query<'w, 's, &'static mut VoxelChunk>,
    commands: Commands<'w, 's>,
}

impl<'w, 's> StaticWorldAccessMut<'w, 's> {
    pub fn get_voxel(
        &self,
        world_pos: IVec3,
        dimension: DimensionId,
    ) -> Voxel {
        let (chunk_pos, local_pos) = StaticWorld::to_chunk_local(world_pos);

        if let Some(entity) = self.world.chunk_entity(dimension, chunk_pos) {
            if let Ok(chunk) = self.chunks.get(entity) {
                return chunk.get_local(local_pos);
            }
        }
        Voxel::AIR
    }

    pub fn set_voxel(
        &mut self,
        world_pos: IVec3,
        dimension: DimensionId,
        voxel: Voxel,
    ) {
        let (chunk_pos, local_pos) = StaticWorld::to_chunk_local(world_pos);

        if let Some(entity) = self.world.chunk_entity(dimension, chunk_pos) {
            if let Ok(mut chunk) = self.chunks.get_mut(entity) {
                chunk.set_local(local_pos, voxel);
                self.commands.entity(entity).insert(NeedsRemeshing);
            }
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 7 – SYSTEMS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Keeps `StaticWorld` up to date when new `StaticChunk` entities appear.
pub fn register_new_chunks_sys(
    mut voxel_world: ResMut<StaticWorld>,
    query: Query<(Entity, &StaticChunk), Added<StaticChunk>>,
) {
    for (entity, chunk) in &query {
        voxel_world.insert(chunk.dimension, chunk.position, entity);
    }
}

/// Removes entries from `StaticWorld` when `StaticChunk` entities are despawned.
pub fn unregister_removed_chunks_sys(
    mut voxel_world: ResMut<StaticWorld>,
    mut removed: RemovedComponents<StaticChunk>,
    query: Query<&StaticChunk>,
) {
    for entity in removed.read() {
        // The component may still be accessible in the same frame it was removed.
        if let Ok(chunk) = query.get(entity) {
            voxel_world.remove(chunk.dimension, chunk.position);
        }
    }
}