use bevy::prelude::*;

use crate::space::access::VoxelWorldMut;
use crate::space::address::BlockPos;
use crate::voxel::{BlockID, BlockRotation, Direction, Voxel};

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
///
/// ## Rotate and StateChange are independent, not exclusive
///
/// A pipe that is both re-aimed and re-connected in one write emits both.
/// The alternative — "Rotate only if state held still" — would silently
/// starve a facing cache on exactly the combined writes that matter most,
/// which is the worst failure mode this enum can have. Listeners filter on
/// the variant they care about and ignore the other.
#[derive(Event, Clone, Copy, Debug)]
pub enum BlockEvent {
    Place { block_id: BlockID, at: BlockPos },
    Break { block_id: BlockID, at: BlockPos },
    /// Only for blocks with no entity — see `InteractsOnSecondary`.
    ///
    /// `face` is the face the ray struck; `target_face` is the face the
    /// interaction addresses, which differs only when the block declares
    /// `FaceTargeted` and the player clicked an off-centre cell.
    Interact {
        block_id: BlockID,
        at: BlockPos,
        player: Entity,
        face: Direction,
        target_face: Direction,
    },
    /// Same block, new orientation. Never decomposed into Break + Place, so
    /// the block entity and everything it owns survives a wrench click.
    Rotate {
        block_id: BlockID,
        at: BlockPos,
        from: BlockRotation,
        to: BlockRotation,
    },
    /// Same block, new state bits — a pipe reconnecting, a lamp lighting.
    StateChange {
        block_id: BlockID,
        at: BlockPos,
        from: u16,
        to: u16,
    },
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – THE AUTHORITATIVE PATH
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The one place in the codebase that decides what a write *was*.
///
/// Every writer — input, worldgen, machines, explosions, save-loading —
/// funnels through here, in any space, so block-entities can never fall out
/// of sync with the voxel grid.
///
/// Classification happens once, here, rather than in every consumer. That
/// is the same argument as putting the face-cell rule in `voxel`: derive it
/// where there is one writer, not where there are twenty readers.
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

    // ── Identity changed: the block is genuinely replaced ────────────────
    //
    // Break first, then place. A replace-in-place therefore tears down the
    // old block-entity before building the new one, which is the only order
    // that leaves the chunk index consistent.
    if !old.is_same_block(new) {
        if !old.is_air() {
            commands.trigger(BlockEvent::Break { block_id: BlockID(old.id()), at });
        }
        if !new.is_air() {
            commands.trigger(BlockEvent::Place { block_id: BlockID(new.id()), at });
        }
        return;
    }

    // ── Same block, different bits ───────────────────────────────────────
    //
    // Air can't reach here: `is_same_block` compares ids, and air's id is 0,
    // so an air→air write was already caught by `old == new` above.
    let block_id = BlockID(new.id());

    if old.rotation() != new.rotation() {
        commands.trigger(BlockEvent::Rotate {
            block_id,
            at,
            from: old.rotation(),
            to:   new.rotation(),
        });
    }

    if old.state() != new.state() {
        commands.trigger(BlockEvent::StateChange {
            block_id,
            at,
            from: old.state(),
            to:   new.state(),
        });
    }
}
