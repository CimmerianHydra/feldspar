use bevy::prelude::*;

use crate::plugin::block::barrel::BarrelSpawner;
use crate::plugin::block::behavior::*;
use crate::plugin::block::entities::*;
use crate::plugin::block::interaction::BlockEvent;
use crate::plugin::loader::block_registry::BlockRegistry;
use crate::plugin::space::prelude::VoxelWorld;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct BlockPlugin;

impl Plugin for BlockPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<BlockBehaviorRegistry>()

            // ---- behavior registrations -----------------------------------
            // One line per machine, and adding one never touches anything
            // above. These migrate into their own plugins as they grow.
            .register_block_behavior("barrel", BarrelSpawner { slots: 27 })

            // ---- lifecycle ------------------------------------------------
            .add_observer(spawn_block_entities_on_place_obs)
            .add_observer(despawn_block_entities_on_break_obs)
        ;
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// LIFECYCLE OBSERVERS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Promotes a freshly written voxel into an ECS entity, if its definition
/// asks for one.
///
/// Note what this is *not* coupled to: the player, the held item, the input
/// system, or which space the block landed in. It hangs off the
/// authoritative voxel write, so worldgen, machines, explosions,
/// save-loading, and blocks welded to a ship all get correct block-entities
/// through the same path.
fn spawn_block_entities_on_place_obs(
    event: On<BlockEvent>,
    mut commands: Commands,
    registry: Res<BlockRegistry>,
    voxel_world: VoxelWorld,
) {
    let BlockEvent::Place { block_id, at, voxel } = *event else { return };

    let definition = registry.get(block_id);
    let Some(spawns) = definition.components.get::<SpawnsBlockEntities>() else { return };

    // Resolve to a canonical address up front. `None` means the chunk went
    // away between the write and now — nothing to attach to.
    let Some(tag) = BlockEntityTag::single(&voxel_world, at) else {
        warn!("block '{}' placed at {at:?} but its chunk is gone", definition.name);
        return;
    };

    // One root per placed block. Inserting the tag *is* indexing it and
    // parenting it — see the hooks in entities.rs.
    let root = commands
        .spawn((tag, Name::new(format!("BlockEntity<{}>", definition.name))))
        .id();

    // Each spawner decorates the same root; sub-entities become its children.
    let mut ctx = BlockSpawnContext { commands: &mut commands, root, at, voxel, block_id };
    for spawner in spawns.iter() {
        spawner.spawn(&mut ctx);
    }
}

/// Tears the entity down when the voxel goes away.
///
/// `despawn` is recursive over children, so sub-entities spawned through
/// `ctx.spawn_child` go with it, and `BlockEntityTag`'s `on_remove` hook
/// un-indexes every occupied cell.
fn despawn_block_entities_on_break_obs(
    event: On<BlockEvent>,
    mut commands: Commands,
    registry: Res<BlockRegistry>,
    voxel_world: VoxelWorld,
    tags: Query<&BlockEntityTag>,
) {
    let BlockEvent::Break { block_id, at, .. } = *event else { return };

    let Some(root) = voxel_world.block_entity_at(at) else { return };

    // For a multiblock, breaking any cell destroys the whole machine — so
    // resolve through the index rather than assuming `at` is the origin.
    if tags.get(root).is_err() { return; }

    // Let each behavior do non-trivial teardown before the entity vanishes.
    if let Some(spawns) = registry.get(block_id).components.get::<SpawnsBlockEntities>() {
        for spawner in spawns.iter() {
            spawner.despawn(&mut commands, root);
        }
    }

    commands.entity(root).despawn();
}
