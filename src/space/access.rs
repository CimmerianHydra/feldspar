use bevy::{ecs::system::SystemParam, prelude::*};

use crate::space::address::{BlockPos, VoxelAddress};
use crate::space::chunk::{ChunkMap, ChunkSlot, VoxelChunk};
use crate::space::entities::ChunkBlockEntities;
use crate::voxel::{to_chunk_local, to_space_pos, Direction, Voxel};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – READ ACCESS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Space-agnostic read access to voxels, block-entities, and world-space
/// geometry. A call site passes a `BlockPos` and never learns whether it
/// landed on terrain or on a ship.
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
// SECTION 2 – WRITE ACCESS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(SystemParam)]
pub struct VoxelWorldMut<'w, 's> {
    spaces: Query<'w, 's, &'static ChunkMap>,
    chunks: Query<'w, 's, &'static mut VoxelChunk>,
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
    ///
    /// No dirty marker is inserted. A real write goes through
    /// `VoxelChunk`'s `DerefMut`, which is exactly what `Changed<VoxelChunk>`
    /// keys off; a no-op write returns before that happens, so the renderer
    /// and the physics builder are never woken for nothing.
    pub fn set_voxel(&mut self, at: BlockPos, voxel: Voxel) -> Option<Voxel> {
        let address = self.resolve(at)?;
        let mut chunk = self.chunks.get_mut(address.chunk).ok()?;

        let old = chunk.get_local(address.local);
        if old == voxel { return Some(old); }   // no-op, skip the remesh

        chunk.set_local(address.local, voxel);
        Some(old)
    }
}
