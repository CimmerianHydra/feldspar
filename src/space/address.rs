use bevy::prelude::*;

use crate::voxel::Direction;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ADDRESSING
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
