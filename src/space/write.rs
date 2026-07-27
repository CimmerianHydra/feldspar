use bevy::prelude::*;

use crate::space::access::VoxelWorldMut;
use crate::space::address::BlockPos;
use crate::voxel::{BlockID, Voxel};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – THE REQUEST
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Ask for a voxel to be written. The only sanctioned way to change the
/// world, in any space.
#[derive(Event, Clone, Copy, Debug)]
pub struct VoxelWriteRequest {
    pub at:    BlockPos,
    pub voxel: Voxel,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – WHAT ACTUALLY HAPPENED
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Carries a `BlockPos`, not an `IVec3`. Consumers that need world-space
/// geometry (audio, particles) call `VoxelWorld::world_position`.
///
/// This is the seam every layer above `space` listens on: block entities
/// spawn from it, audio plays from it, and the renderer ignores it entirely
/// because `Changed<VoxelChunk>` already told it what it needed.
#[derive(Event, Clone, Copy, Debug)]
pub enum BlockEvent {
    Place    { block_id: BlockID, at: BlockPos },
    Break    { block_id: BlockID, at: BlockPos },
    /// Only for blocks with no entity — see `InteractsOnSecondary`.
    Interact { block_id: BlockID, at: BlockPos, player: Entity },
    StateChange { block_id: BlockID, at: BlockPos, old: Voxel, new: Voxel },
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – THE AUTHORITATIVE PATH
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The one place in the codebase that decides a block was placed or broken.
///
/// Every writer — input, worldgen, machines, explosions, save-loading —
/// funnels through here, in any space, so block-entities can never fall out
/// of sync with the voxel grid.
pub fn voxel_write_obs(
    event: On<VoxelWriteRequest>,
    mut commands: Commands,
    mut voxel_world: VoxelWorldMut,
) {
    let at  = event.at;
    let new = event.voxel;

    // `None` means the chunk isn't loaded: nothing written, so emit nothing.
    let Some(old) = voxel_world.set_voxel(at, new) else { return };
    if old == new { return; }

    // Break first, then place. A replace-in-place therefore tears down the
    // old block-entity before building the new one, which is the only order
    // that leaves the chunk index consistent.
    if !old.is_air() {
        commands.trigger(BlockEvent::Break { block_id: BlockID(old.id()), at });
    }
    if !new.is_air() {
        commands.trigger(BlockEvent::Place { block_id: BlockID(new.id()), at });
    }
}
