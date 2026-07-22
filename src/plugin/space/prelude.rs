use bevy::{
    ecs::{lifecycle::HookContext, system::SystemParam, world::DeferredWorld},
    prelude::*,
};
use std::collections::HashMap;

use crate::plugin::chunk::{NeedsRemeshing, VoxelChunk, CHUNK_SIZE};
use crate::plugin::dimension::DimensionID;
use crate::plugin::voxel::{Direction, Voxel};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Owns the concept of a *voxel space*: any entity that holds a `ChunkMap`
/// and defines a coordinate frame. Dimensions and moving grids are the same
/// kind of thing here, which is the entire point — everything downstream of
/// addressing stops caring which one it's talking to.
pub struct SpacePlugin;

impl Plugin for SpacePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DimensionRegistry>();

        // Spawned directly against the World, not via a startup system, so
        // the overworld entity is guaranteed to exist before *any* system
        // runs — including anything that spawns chunks.
        spawn_dimension(app.world_mut(), DimensionID::OVERWORLD, "Overworld");
    }
}

/// Spawn a dimension space. The `Dimension` hook registers it in
/// `DimensionRegistry` automatically.
pub fn spawn_dimension(world: &mut World, id: DimensionID, name: &str) -> Entity {
    world
        .spawn((
            Dimension(id),
            ChunkMap::default(),
            // Identity frame. Present so the same transform math works for
            // dimensions and grids without a special case.
            Transform::default(),
            Visibility::default(),
            Name::new(format!("Dimension<{name}>")),
        ))
        .id()
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – ADDRESSING
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// How the rest of the codebase *names* a block: a coordinate frame plus a
/// position inside it.
///
/// `space` is an entity carrying a `ChunkMap` — a dimension or a moving
/// grid, indistinguishably. `pos` is absolute block coordinates within that
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

    /// Offset within the same space. Never crosses a space boundary —
    /// that's deliberate, see the note on neighbor queries below.
    #[inline]
    pub fn offset(self, delta: IVec3) -> Self {
        Self { space: self.space, pos: self.pos + delta }
    }

    #[inline]
    pub fn neighbor(self, dir: Direction) -> Self {
        self.offset(dir.as_ivec3())
    }
}

/// How the engine *files* a block: which chunk entity owns it, and where
/// inside that chunk.
///
/// This is the canonical form, and it's what gets stored on long-lived
/// components. The reason is grid surgery: when a ship splits, chunk
/// entities get reassigned to a new space, but their identity and their
/// contents don't change. Anything holding a `VoxelAddress` survives a
/// split or a dock untouched; anything holding a `BlockPos` would go stale.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct VoxelAddress {
    pub chunk: Entity,
    pub local: UVec3,
}

// ---- pure coordinate math ------------------------------------------------

/// Split an in-space block position into `(chunk coord, local position)`.
///
/// Euclidean division, so negatives behave: block `(-1, 0, 0)` is chunk
/// `(-1, 0, 0)`, local `(15, 0, 0)`.
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

/// Chunk index for one space. The moving-grid twin of the old global
/// `StaticWorld` resource, except it's a component, so every space gets one
/// and there's no dimension key in the map.
///
/// Never edited by hand — `ChunkSlot`'s hooks keep it in sync.
#[derive(Component, Default, Debug)]
pub struct ChunkMap {
    chunks: HashMap<IVec3, Entity>,
}

impl ChunkMap {
    #[inline]
    pub fn get(&self, coord: IVec3) -> Option<Entity> {
        self.chunks.get(&coord).copied()
    }

    #[inline]
    pub fn contains(&self, coord: IVec3) -> bool { self.chunks.contains_key(&coord) }

    pub fn coords(&self) -> impl Iterator<Item = IVec3> + '_ {
        self.chunks.keys().copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (IVec3, Entity)> + '_ {
        self.chunks.iter().map(|(c, e)| (*c, *e))
    }

    pub fn len(&self) -> usize { self.chunks.len() }
    pub fn is_empty(&self) -> bool { self.chunks.is_empty() }

    // Crate-internal: only the ChunkSlot hooks should call these.
    fn insert(&mut self, coord: IVec3, chunk: Entity) { self.chunks.insert(coord, chunk); }
    fn remove(&mut self, coord: IVec3) { self.chunks.remove(&coord); }
}

/// Marks a space as a dimension (the non-moving kind).
#[derive(Component, Reflect, Clone, Copy, Debug)]
#[component(on_add = dimension_on_add, on_remove = dimension_on_remove)]
pub struct Dimension(pub DimensionID);

/// Marks a space as a movable voxel construct — ship, vehicle, station.
///
/// Pair with `ChunkMap`, `Transform`, and your physics body. Chunks
/// belonging to it are `ChildOf` this entity, so they inherit its motion
/// and so does everything attached to them.
#[derive(Component, Reflect, Default, Debug)]
pub struct MovingGrid;

/// Convenience bundle for spawning a grid space.
pub fn moving_grid(transform: Transform, name: &str) -> impl Bundle {
    (
        MovingGrid,
        ChunkMap::default(),
        transform,
        Visibility::default(),
        Name::new(format!("Grid<{name}>")),
    )
}

/// Stable-ID lookup for dimensions. Save files reference `DimensionID`,
/// which survives across runs; `Entity` does not.
#[derive(Resource, Default)]
pub struct DimensionRegistry {
    by_id: HashMap<DimensionID, Entity>,
}

impl DimensionRegistry {
    #[inline]
    pub fn get(&self, id: DimensionID) -> Option<Entity> {
        self.by_id.get(&id).copied()
    }

    /// Shorthand for the common case. Panics only if `SpacePlugin` wasn't
    /// added, which is a setup error rather than a runtime condition.
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
/// This one component does four jobs, all through its hooks:
///   1. registers the chunk in the space's `ChunkMap`
///   2. parents it to the space, so grid chunks inherit grid motion
///   3. positions it via a local `Transform`
///   4. unregisters cleanly on removal or despawn
///
/// Spawning a chunk is therefore just
/// `commands.spawn((ChunkSlot { space, coord }, VoxelChunk::empty()))`.
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
        // 1. Index it in the owning space.
        let Ok(mut space_ref) = world.get_entity_mut(space) else {
            warn!("chunk at {coord} points at a space entity that doesn't exist");
            return;
        };
        let Some(mut map) = space_ref.get_mut::<ChunkMap>() else {
            warn!("chunk at {coord} points at an entity with no ChunkMap — not a space");
            return;
        };
        map.insert(coord, entity);

        // 2 & 3. Parent + place. Transform is local to the space, so a grid
        // chunk needs no per-frame syncing: Bevy's propagation does it.
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
        // Only clear the slot if it still points at us — guards a
        // replace-in-place where the new chunk registered first.
        if map.get(coord) == Some(entity) {
            map.remove(coord);
        }
    });
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 5 – BLOCK-ENTITY INDEX
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Sparse map of local block positions -> block-entities, living on the
/// chunk entity beside `VoxelChunk`.
///
/// Chunk-relative by construction, so it works identically for terrain and
/// for ships without knowing the difference. Only present on chunks that
/// actually contain block-entities, so plain terrain pays nothing.
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
/// geometry. Replaces `StaticWorldAccess`; the only difference at the call
/// site is passing a `BlockPos` instead of `(IVec3, DimensionID)`.
#[derive(SystemParam)]
pub struct VoxelWorld<'w, 's> {
    spaces:  Query<'w, 's, (&'static ChunkMap, &'static GlobalTransform)>,
    slots:   Query<'w, 's, &'static ChunkSlot>,
    chunks:  Query<'w, 's, &'static VoxelChunk>,
    tables:  Query<'w, 's, &'static ChunkBlockEntities>,
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

    /// Unloaded chunks read as air, matching the old behavior.
    pub fn get_voxel(&self, at: BlockPos) -> Voxel {
        self.resolve(at)
            .and_then(|a| self.chunks.get(a.chunk).ok().map(|c| c.get_local(a.local)))
            .unwrap_or(Voxel::AIR)
    }

    /// Neighbor lookup stays strictly inside one space. A block on the edge
    /// of a ship has air outside it, not whatever terrain happens to overlap
    /// — cross-space interaction should be an explicit mechanic, never an
    /// accident of coordinate math.
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
    /// This is what spatial audio, particles, and tooltips want.
    pub fn world_position(&self, at: BlockPos) -> Option<Vec3> {
        let (_, space_tf) = self.spaces.get(at.space).ok()?;
        Some(space_tf.transform_point(at.pos.as_vec3() + Vec3::splat(0.5)))
    }

    /// Full world-space frame of a block, including the space's rotation.
    /// Use this to orient a highlight or a block-mounted model on a ship.
    pub fn world_transform(&self, at: BlockPos) -> Option<Transform> {
        let (_, space_tf) = self.spaces.get(at.space).ok()?;
        let (scale, rotation, translation) = space_tf.to_scale_rotation_translation();
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

    /// World-space point -> block position in the given space. The inverse
    /// of `world_position`, used when translating a physics hit or a
    /// player's feet into grid coordinates.
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
        if old == voxel { return Some(old); }   // no-op write, skip the remesh

        chunk.set_local(address.local, voxel);
        self.commands.entity(address.chunk).insert(NeedsRemeshing);
        Some(old)
    }
}
