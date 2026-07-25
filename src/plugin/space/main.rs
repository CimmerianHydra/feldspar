use avian3d::prelude::*;
use bevy::{
    ecs::{lifecycle::HookContext, system::SystemParam, world::DeferredWorld},
    prelude::*,
};
use std::collections::HashMap;

use crate::plugin::geometry::collision::NeedsColliderRebuild;
use crate::plugin::geometry::meshing::NeedsRemeshing;
use crate::plugin::geometry::voxel::{Direction, Voxel};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Owns the concept of a *voxel space*: an entity that holds a `ChunkMap`,
/// defines a coordinate frame, and owns a rigid body.
///
/// Dimensions and moving grids are the same kind of thing here, differing
/// only in which `RigidBody` variant they carry. Chunks are never bodies —
/// they contribute collision shape to whatever space they belong to. One
/// rule, no exceptions, and no entity has two authorities writing its
/// transform.
pub struct SpacePlugin;

impl Plugin for SpacePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DimensionRegistry>();

        // Spawned directly against the World, not from a startup system, so
        // the overworld exists before any system can try to put a chunk in it.
        spawn_dimension(app.world_mut(), DimensionID::OVERWORLD, "Overworld");
    }
}

/// Spawn a dimension space. The `Dimension` hook registers it in
/// `DimensionRegistry` automatically.
///
/// It's a static body: terrain doesn't move, and its chunks attach their
/// colliders to it through the hierarchy exactly the way a ship's do.
pub fn spawn_dimension(world: &mut World, id: DimensionID, name: &str) -> Entity {
    world
        .spawn((
            Dimension(id),
            ChunkMap::default(),
            RigidBody::Static,
            // Identity frame. Present so the same transform math works for
            // dimensions and grids without a special case.
            Transform::default(),
            Visibility::default(),
            Name::new(format!("Dimension<{name}>")),
        ))
        .id()
}

/// Bundle for a movable voxel construct - ship, vehicle, station.
///
/// Mass, inertia, and center of mass are all derived from the colliders its
/// chunks contribute, so don't set them by hand; set `ColliderDensity` on
/// the chunks instead (see `CHUNK_COLLIDER_DENSITY` in the mesher).
pub fn build_moving_grid(transform: Transform, name: &str) -> impl Bundle {
    (
        MovingGrid,
        ChunkMap::default(),
        RigidBody::Dynamic,
        transform,
        Visibility::default(),
        Name::new(format!("Grid<{name}>")),
    )
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Component, Reflect)]
pub struct DimensionID(pub u8);

impl DimensionID {
    pub const OVERWORLD:    Self = Self(0);
    pub const UNDERWORLD:   Self = Self(1);
    pub const LUA:          Self = Self(2);
    pub const MARS:         Self = Self(3);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – ADDRESSING
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// How the rest of the codebase *names* a block: a coordinate frame plus a
/// position inside it.
///
/// `space` is an entity carrying a `ChunkMap` — a dimension or a grid,
/// indistinguishably. `pos` is absolute block coordinates within that
/// frame, so for a ship it's ship-local and stays constant while the ship
/// flies around.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct BlockPos {
    pub space: Entity,
    pub pos:   IVec3,
}

impl BlockPos {
    #[inline]
    pub fn new(space: Entity, pos: IVec3) -> Self { Self { space, pos } }

    /// Offset within the same space. Never crosses a space boundary.
    #[inline]
    pub fn offset(self, delta: IVec3) -> Self {
        Self { space: self.space, pos: self.pos + delta }
    }

    #[inline]
    pub fn neighbor(self, dir: Direction) -> Self { self.offset(dir.as_ivec3()) }
}

/// How the engine *files* a block: which chunk entity owns it, and where
/// inside that chunk.
///
/// This is the canonical form, stored on long-lived components. When a ship
/// splits, chunk entities get reassigned to a new space but their identity
/// and contents don't change — so anything holding a `VoxelAddress`
/// survives the surgery untouched, where a `BlockPos` would go stale.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct VoxelAddress {
    pub chunk: Entity,
    pub local: UVec3,
}

// ---- pure coordinate math ------------------------------------------------

/// Split an in-space block position into `(chunk coord, local position)`.
/// Euclidean division, so negatives behave: block `(-1,0,0)` is chunk
/// `(-1,0,0)`, local `(15,0,0)`.
#[inline]
pub fn to_chunk_local(block: IVec3) -> (IVec3, UVec3) {
    let s = CHUNK_SIZE as i32;
    let chunk = block.div_euclid(IVec3::splat(s));
    (chunk, (block - chunk * s).as_uvec3())
}

/// Inverse of `to_chunk_local`.
#[inline]
pub fn to_space_pos(chunk: IVec3, local: UVec3) -> IVec3 {
    chunk * CHUNK_SIZE as i32 + local.as_ivec3()
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – SPACES
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
    /// Fill every voxel with air blocks.
    pub fn empty() -> Self {
        Self {
            voxels: Box::new([Voxel::AIR; CHUNK_VOLUME])
        }
    }

    /// Fill every voxel with the same block.
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

    /// Short-circuiting checker to see if all voxels are air.
    pub fn is_all_air(&self) -> bool {
        !self.raw().iter().any(|v| !v.is_air())
    }

    /// Raw slice access (e.g. for bulk copy into a mesh buffer).
    #[inline] pub fn raw(&self) -> &[Voxel; CHUNK_VOLUME] { &self.voxels }
}



/// Chunk index for one space. Never edited by hand — `ChunkSlot`'s hooks
/// keep it in sync.
#[derive(Component, Default, Debug)]
pub struct ChunkMap {
    chunks: HashMap<IVec3, Entity>,
}

impl ChunkMap {
    #[inline]
    pub fn get(&self, coord: IVec3) -> Option<Entity> { self.chunks.get(&coord).copied() }
    #[inline]
    pub fn contains(&self, coord: IVec3) -> bool { self.chunks.contains_key(&coord) }

    pub fn coords(&self) -> impl Iterator<Item = IVec3> + '_ { self.chunks.keys().copied() }
    pub fn iter(&self) -> impl Iterator<Item = (IVec3, Entity)> + '_ {
        self.chunks.iter().map(|(c, e)| (*c, *e))
    }
    pub fn len(&self) -> usize { self.chunks.len() }
    pub fn is_empty(&self) -> bool { self.chunks.is_empty() }

    fn insert(&mut self, coord: IVec3, chunk: Entity) { self.chunks.insert(coord, chunk); }
    fn remove(&mut self, coord: IVec3) { self.chunks.remove(&coord); }
}

/// Marks a space as a dimension — the kind that doesn't move.
#[derive(Component, Reflect, Clone, Copy, Debug)]
#[component(on_add = dimension_on_add, on_remove = dimension_on_remove)]
pub struct Dimension(pub DimensionID);

/// Marks a space as a movable voxel construct.
#[derive(Component, Reflect, Default, Debug)]
pub struct MovingGrid;

/// Stable-ID lookup for dimensions. Save files reference `DimensionID`,
/// which survives across runs; `Entity` does not.
#[derive(Resource, Default)]
pub struct DimensionRegistry {
    by_id: HashMap<DimensionID, Entity>,
}

impl DimensionRegistry {
    #[inline]
    pub fn get(&self, id: DimensionID) -> Option<Entity> { self.by_id.get(&id).copied() }

    /// Panics only if `SpacePlugin` wasn't added, which is a setup error
    /// rather than a runtime condition.
    #[inline]
    pub fn overworld(&self) -> Entity {
        self.get(DimensionID::OVERWORLD)
            .expect("SpacePlugin must be added before anything touches the overworld")
    }
}

fn dimension_on_add(mut world: DeferredWorld, ctx: HookContext) {
    let Some(&Dimension(id)) = world.entity(ctx.entity).get::<Dimension>() else { return };
    let entity = ctx.entity;
    if let Some(mut reg) = world.get_resource_mut::<DimensionRegistry>() {
        if reg.by_id.insert(id, entity).is_some() {
            warn!("dimension {id:?} registered twice — later entity wins");
        }
    }
}

fn dimension_on_remove(mut world: DeferredWorld, ctx: HookContext) {
    let Some(&Dimension(id)) = world.entity(ctx.entity).get::<Dimension>() else { return };
    if let Some(mut reg) = world.get_resource_mut::<DimensionRegistry>() {
        if reg.by_id.get(&id) == Some(&ctx.entity) {
            reg.by_id.remove(&id);
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 4 – CHUNK MEMBERSHIP
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Says which space a chunk belongs to and where it sits in that space.
///
/// One component, four jobs, all through its hooks:
///   1. registers the chunk in the space's `ChunkMap`
///   2. parents it to the space, which is also how avian finds the rigid
///      body its colliders belong to
///   3. positions it via a local `Transform`
///   4. unregisters cleanly on removal or despawn
///
/// Spawning a chunk is therefore just
/// `commands.spawn((ChunkSlot { space, coord }, VoxelChunk::empty()))`,
/// and cutting one loose to become a ship is a change to `space` plus a
/// re-parent — no rigid body bookkeeping, because the chunk never had one.
#[derive(Component, Reflect, Clone, Copy, Debug)]
#[component(on_add = chunk_slot_on_add, on_remove = chunk_slot_on_remove)]
pub struct ChunkSlot {
    pub space: Entity,
    pub coord: IVec3,
}

/// Local translation of a chunk within its space.
#[inline]
pub fn chunk_translation(coord: IVec3) -> Vec3 {
    (coord * CHUNK_SIZE as i32).as_vec3()
}

fn chunk_slot_on_add(mut world: DeferredWorld, ctx: HookContext) {
    let entity = ctx.entity;
    let Some(&ChunkSlot { space, coord }) = world.entity(entity).get::<ChunkSlot>() else { return };

    world.commands().queue(move |world: &mut World| {
        let Ok(mut space_ref) = world.get_entity_mut(space) else {
            warn!("chunk at {coord} points at a space entity that doesn't exist");
            return;
        };
        let Some(mut map) = space_ref.get_mut::<ChunkMap>() else {
            warn!("chunk at {coord} points at an entity with no ChunkMap — not a space");
            return;
        };
        map.insert(coord, entity);

        // Parent + place. The transform is local to the space, so a grid
        // chunk needs no per-frame syncing — Bevy's propagation does it,
        // and it's the only writer, because chunks aren't rigid bodies.
        if let Ok(mut chunk_ref) = world.get_entity_mut(entity) {
            chunk_ref.insert((
                ChildOf(space),
                Transform::from_translation(chunk_translation(coord)),
            ));
        }
    });
}

fn chunk_slot_on_remove(mut world: DeferredWorld, ctx: HookContext) {
    let entity = ctx.entity;
    let Some(&ChunkSlot { space, coord }) = world.entity(entity).get::<ChunkSlot>() else { return };

    world.commands().queue(move |world: &mut World| {
        let Ok(mut space_ref) = world.get_entity_mut(space) else { return };
        let Some(mut map) = space_ref.get_mut::<ChunkMap>() else { return };
        // Only clear the slot if it still points at us.
        if map.get(coord) == Some(entity) {
            map.remove(coord);
        }
    });
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 5 – BLOCK-ENTITY INDEX
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Sparse map of local block positions -> block-entities, on the chunk
/// entity beside `VoxelChunk`.
///
/// Chunk-relative by construction, so it works identically for terrain and
/// ships. Only present on chunks that contain block-entities, so plain
/// terrain pays nothing.
#[derive(Component, Default, Debug)]
pub struct ChunkBlockEntities {
    map: HashMap<UVec3, Entity>,
}

impl ChunkBlockEntities {
    pub fn insert(&mut self, local: UVec3, entity: Entity) { self.map.insert(local, entity); }
    pub fn remove(&mut self, local: UVec3) -> Option<Entity> { self.map.remove(&local) }
    #[inline]
    pub fn get(&self, local: UVec3) -> Option<Entity> { self.map.get(&local).copied() }
    pub fn is_empty(&self) -> bool { self.map.is_empty() }
    pub fn len(&self) -> usize { self.map.len() }
    pub fn iter(&self) -> impl Iterator<Item = (UVec3, Entity)> + '_ {
        self.map.iter().map(|(l, e)| (*l, *e))
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 6 – READ ACCESS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Space-agnostic read access to voxels, block-entities, and world-space
/// geometry. The only difference from the old `StaticWorldAccess` at a call
/// site is passing a `BlockPos` instead of `(IVec3, DimensionID)`.
#[derive(SystemParam)]
pub struct VoxelWorld<'w, 's> {
    spaces: Query<'w, 's, (&'static ChunkMap, &'static GlobalTransform)>,
    slots:  Query<'w, 's, &'static ChunkSlot>,
    chunks: Query<'w, 's, &'static VoxelChunk>,
    tables: Query<'w, 's, &'static ChunkBlockEntities>,
}

impl<'w, 's> VoxelWorld<'w, 's> {
    // ---- resolution ------------------------------------------------------

    /// `BlockPos` -> `VoxelAddress`. `None` means the chunk isn't loaded, or
    /// `space` isn't a space.
    pub fn resolve(&self, at: BlockPos) -> Option<VoxelAddress> {
        let (map, _) = self.spaces.get(at.space).ok()?;
        let (coord, local) = to_chunk_local(at.pos);
        Some(VoxelAddress { chunk: map.get(coord)?, local })
    }

    /// `VoxelAddress` -> `BlockPos`. Recomputed live from the chunk's own
    /// `ChunkSlot`, so it stays correct after a grid split or dock.
    pub fn locate(&self, address: VoxelAddress) -> Option<BlockPos> {
        let slot = self.slots.get(address.chunk).ok()?;
        Some(BlockPos::new(slot.space, to_space_pos(slot.coord, address.local)))
    }

    // ---- voxels ----------------------------------------------------------

    /// Unloaded chunks read as air.
    pub fn get_voxel(&self, at: BlockPos) -> Voxel {
        self.resolve(at)
            .and_then(|a| self.chunks.get(a.chunk).ok().map(|c| c.get_local(a.local)))
            .unwrap_or(Voxel::AIR)
    }

    /// Neighbor lookup stays strictly inside one space. A block on the edge
    /// of a ship has air outside it, not whatever terrain happens to
    /// overlap — cross-space interaction should be a deliberate mechanic,
    /// never an accident of coordinate math.
    #[inline]
    pub fn get_neighbor(&self, at: BlockPos, dir: Direction) -> Voxel {
        self.get_voxel(at.neighbor(dir))
    }

    // ---- block entities --------------------------------------------------

    pub fn block_entity_at(&self, at: BlockPos) -> Option<Entity> {
        let address = self.resolve(at)?;
        self.tables.get(address.chunk).ok()?.get(address.local)
    }

    pub fn block_entity_at_address(&self, address: VoxelAddress) -> Option<Entity> {
        self.tables.get(address.chunk).ok()?.get(address.local)
    }

    // ---- geometry --------------------------------------------------------

    /// World-space center of a block, wherever its space happens to be.
    /// What spatial audio, particles, and tooltips want.
    pub fn world_position(&self, at: BlockPos) -> Option<Vec3> {
        let (_, space_tf) = self.spaces.get(at.space).ok()?;
        Some(space_tf.transform_point(at.pos.as_vec3() + Vec3::splat(0.5)))
    }

    /// Full world-space frame of a block, including the space's rotation.
    /// Use this to orient a highlight or a block-mounted model on a ship.
    pub fn world_transform(&self, at: BlockPos) -> Option<Transform> {
        let (_, space_tf) = self.spaces.get(at.space).ok()?;
        let (scale, rotation, _) = space_tf.to_scale_rotation_translation();
        Some(Transform {
            translation: space_tf.transform_point(at.pos.as_vec3()),
            rotation,
            scale,
        })
    }

    /// Rotate a space-local `Direction` into world space. Machine logic
    /// should stay in local directions; only physics, gravity, and lighting
    /// need this.
    pub fn world_direction(&self, space: Entity, dir: Direction) -> Option<Vec3> {
        let (_, space_tf) = self.spaces.get(space).ok()?;
        Some(space_tf.affine().matrix3 * dir.as_vec3())
    }

    /// World-space point -> block position in the given space. Used when
    /// translating a physics hit or a player's feet into grid coordinates.
    pub fn world_to_block(&self, space: Entity, world_point: Vec3) -> Option<BlockPos> {
        let (_, space_tf) = self.spaces.get(space).ok()?;
        let local = space_tf.affine().inverse().transform_point3(world_point);
        Some(BlockPos::new(space, local.floor().as_ivec3()))
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 7 – WRITE ACCESS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(SystemParam)]
pub struct VoxelWorldMut<'w, 's> {
    spaces:   Query<'w, 's, &'static ChunkMap>,
    chunks:   Query<'w, 's, &'static mut VoxelChunk>,
    commands: Commands<'w, 's>,
}

impl<'w, 's> VoxelWorldMut<'w, 's> {
    pub fn resolve(&self, at: BlockPos) -> Option<VoxelAddress> {
        let map = self.spaces.get(at.space).ok()?;
        let (coord, local) = to_chunk_local(at.pos);
        Some(VoxelAddress { chunk: map.get(coord)?, local })
    }

    pub fn get_voxel(&self, at: BlockPos) -> Voxel {
        self.resolve(at)
            .and_then(|a| self.chunks.get(a.chunk).ok().map(|c| c.get_local(a.local)))
            .unwrap_or(Voxel::AIR)
    }

    /// Write a voxel and report what was there before.
    ///
    /// `None` means nothing was written — the chunk isn't loaded. That
    /// distinction is what lets the write observer emit truthful
    /// place/break events instead of optimistic ones.
    pub fn set_voxel(&mut self, at: BlockPos, voxel: Voxel) -> Option<Voxel> {
        let address = self.resolve(at)?;
        let mut chunk = self.chunks.get_mut(address.chunk).ok()?;

        let old = chunk.get_local(address.local);
        if old == voxel { return Some(old); }   // no-op, skip the remesh

        chunk.set_local(address.local, voxel);
        self.commands.entity(address.chunk).insert((NeedsRemeshing, NeedsColliderRebuild));
        Some(old)
    }
}
