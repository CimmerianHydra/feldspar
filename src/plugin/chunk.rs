use bevy::prelude::*;

use crate::plugin::voxel::Voxel;

// Contains chunk logic.

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – VOXEL CHUNK
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

#[derive(Component)]
pub struct NeedsRemeshing;