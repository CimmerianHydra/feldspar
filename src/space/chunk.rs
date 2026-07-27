use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
};
use std::collections::HashMap;

use crate::voxel::{chunk_translation, local_from_index, local_index, Voxel, CHUNK_VOLUME};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – VOXEL STORAGE
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Dense 16 × 16 × 16 voxel storage component.
///
/// Shared by both **static world chunks** and **moving grids** — the only
/// difference is which marker component sits alongside it.
///
/// ## Dirtiness
///
/// This component carries no dirty flag, on purpose. Bevy already tracks
/// `Changed<VoxelChunk>` for us, and the mesher and the collider builder
/// each query it independently. A `NeedsRemeshing` marker owned by `space`
/// would be the world model depending on the renderer's bookkeeping; a
/// marker owned by the renderer and inserted here would be worse.
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

    // ---- read ---------------------------------------------------------------

    #[inline] pub fn get(&self, x: usize, y: usize, z: usize) -> Voxel {
        self.voxels[local_index(x, y, z)]
    }

    #[inline] pub fn get_local(&self, p: UVec3) -> Voxel {
        self.get(p.x as usize, p.y as usize, p.z as usize)
    }

    // ---- write --------------------------------------------------------------

    #[inline] pub fn set(&mut self, x: usize, y: usize, z: usize, v: Voxel) {
        self.voxels[local_index(x, y, z)] = v;
    }

    #[inline] pub fn set_local(&mut self, p: UVec3, v: Voxel) {
        self.set(p.x as usize, p.y as usize, p.z as usize, v);
    }

    // ---- iteration ----------------------------------------------------------

    /// Iterate every non-air voxel as `(local_pos, voxel)`.
    pub fn iter_non_air(&self) -> impl Iterator<Item = (UVec3, Voxel)> + '_ {
        self.voxels.iter().enumerate().filter_map(|(i, &v)| {
            if v.is_air() { return None; }
            Some((local_from_index(i), v))
        })
    }

    /// Short-circuiting checker to see if all voxels are air.
    pub fn is_all_air(&self) -> bool {
        !self.raw().iter().any(|v| !v.is_air())
    }

    /// Raw slice access (e.g. for bulk copy into a mesh buffer).
    #[inline] pub fn raw(&self) -> &[Voxel; CHUNK_VOLUME] { &self.voxels }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – THE PER-SPACE INDEX
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

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

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – CHUNK MEMBERSHIP
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
