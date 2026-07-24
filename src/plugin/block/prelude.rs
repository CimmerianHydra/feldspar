use bevy::prelude::*;

use crate::plugin::block::barrel::BarrelSpawner;
use crate::plugin::block::behavior::*;
use crate::plugin::block::entities::*;
use crate::plugin::block::interaction::BlockEvent;
use crate::plugin::loader::block_registry::{BlockID, BlockRegistry};
use crate::plugin::space::prelude::*;
use crate::plugin::voxel::Direction;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct BlockPlugin;

impl Plugin for BlockPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<BlockBehaviorRegistry>()

            // ---- behavior registrations -----------------------------------
            .register_block_behavior("barrel", BarrelSpawner { slots: 27 })

            // ---- lifecycle ------------------------------------------------
            .add_observer(spawn_block_entities_on_place_obs)
            .add_observer(despawn_block_entities_on_break_obs)
        ;
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – THE SHARED BUILDER
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Builds one block-entity from a definition's spawner list.
///
/// Both entry points funnel through here — the place observer (a player or
/// machine wrote a voxel) and the hydration pass (a chunk arrived with
/// voxels already in it). Keeping it in one function is what guarantees a
/// barrel loaded from disk is indistinguishable from one just placed.
///
/// Returns `None` if the block declares no entity behavior.
pub fn build_block_entity(
    commands: &mut Commands,
    registry: &BlockRegistry,
    tag:      BlockEntityTag,
    at:       BlockPos,
    facing:   Direction,
    block_id: BlockID,
) -> Option<Entity> {
    let definition = registry.get(block_id);
    let spawns = definition.components.get::<SpawnsBlockEntities>()?;

    // Inserting the tag *is* indexing and parenting it — see the hooks.
    let root = commands
        .spawn((tag, Name::new(format!("BlockEntity<{}>", definition.name))))
        .id();

    let mut ctx = BlockSpawnContext { commands, root, at, facing, block_id };
    for spawner in spawns.iter() {
        spawner.spawn(&mut ctx);
    }

    Some(root)
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – LIFECYCLE OBSERVERS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Promotes a freshly written voxel into an ECS entity.
///
/// Note what this is *not* coupled to: the player, the held item, the input
/// system, or which space the block landed in. It hangs off the
/// authoritative voxel write, so machines, explosions, and blocks welded to
/// a flying ship all come through the same path.
fn spawn_block_entities_on_place_obs(
    event: On<BlockEvent>,
    mut commands: Commands,
    registry: Res<BlockRegistry>,
    voxel_world: VoxelWorld,
) {
    let BlockEvent::Place { block_id, at } = *event else { return };

    let Some(tag) = BlockEntityTag::single(&voxel_world, at) else {
        warn!("BlockEvent recorded at {at:?}, but its chunk is nowhere to be found.");
        return;
    };

    let facing = voxel_world.get_voxel(at).facing();

    build_block_entity(&mut commands, &registry, tag, at, facing, block_id);
}

/// Tears the entity down when the voxel goes away. `despawn` is recursive,
/// so sub-entities go with it, and the tag's `on_remove` hook un-indexes
/// every occupied cell.
fn despawn_block_entities_on_break_obs(
    event: On<BlockEvent>,
    mut commands: Commands,
    registry: Res<BlockRegistry>,
    voxel_world: VoxelWorld,
    tags: Query<&BlockEntityTag>,
) {
    let BlockEvent::Break { block_id, at } = *event else { return };

    let Some(root) = voxel_world.block_entity_at(at) else { return };
    if tags.get(root).is_err() { return; }

    if let Some(spawns) = registry.get(block_id).components.get::<SpawnsBlockEntities>() {
        for spawner in spawns.iter() {
            spawner.despawn(&mut commands, root);
        }
    }

    commands.entity(root).despawn();
}