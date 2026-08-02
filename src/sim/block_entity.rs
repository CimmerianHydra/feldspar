use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
};

use crate::content::block::components::SpawnsBlockEntities;
use crate::content::block::{BlockRegistry, BlockSpawnContext};
use crate::space::{BlockEvent, BlockPos, ChunkBlockEntities, VoxelAddress, VoxelWorld};
use crate::voxel::{BlockID, Direction};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – THE TAG
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The only component a caller has to insert to wire a block-entity into
/// the world correctly. Its hooks do the rest.
///
/// Stores `VoxelAddress`, never `BlockPos`. Chunk-relative addressing means
/// a barrel welded to a ship needs no updating when the ship moves, and no
/// updating when the ship *splits in half* and its chunks are reassigned to
/// a new grid — the chunk entity and the local coordinate are both
/// unchanged. Call `VoxelWorld::locate` when you need a live `BlockPos`.
///
/// `cells` holds every occupied address. One entry for a normal block; 27
/// for a 3x3x3 machine, all pointing back at this same entity, so a lookup
/// resolves identically whichever cell was hit.
#[derive(Component, Clone, Debug)]
#[component(on_add = block_entity_tag_on_add, on_remove = block_entity_tag_on_remove)]
pub struct BlockEntityTag {
    /// The controller cell — the one that was placed.
    pub origin: VoxelAddress,
    /// Every occupied cell. Always contains `origin`.
    pub cells:  Vec<VoxelAddress>,
}

impl BlockEntityTag {
    /// The common case: one block, one cell.
    ///
    /// `None` if the chunk isn't loaded, in which case there's nothing to
    /// attach to and the caller should skip spawning entirely.
    pub fn single(world: &VoxelWorld, at: BlockPos) -> Option<Self> {
        let origin = world.resolve(at)?;
        Some(Self { origin, cells: vec![origin] })
    }

    /// When the address is already known and no resolution is needed —
    /// chunk hydration, deserialization. Cheaper and, more importantly,
    /// usable before the chunk has been registered in its space's
    /// `ChunkMap`, which matters when hydrating a chunk on the same frame
    /// it was spawned.
    pub fn at_address(address: VoxelAddress) -> Self {
        Self { origin: address, cells: vec![address] }
    }

    /// Multiblock. `cells` are absolute positions in the same space as
    /// `origin`; cells in unloaded chunks are skipped with a warning.
    pub fn multi(
        world: &VoxelWorld,
        origin: BlockPos,
        cells: impl IntoIterator<Item = IVec3>,
    ) -> Option<Self> {
        let origin_address = world.resolve(origin)?;

        let mut resolved = vec![origin_address];
        for pos in cells {
            let at = BlockPos::new(origin.space, pos);
            match world.resolve(at) {
                Some(address) if address != origin_address => resolved.push(address),
                Some(_) => {}   // duplicate of origin, skip
                None => warn!("multiblock cell at {pos} is in an unloaded chunk — skipped"),
            }
        }

        Some(Self { origin: origin_address, cells: resolved })
    }

    /// Live position of this block-entity, or `None` if its chunk is gone.
    pub fn block_pos(&self, world: &VoxelWorld) -> Option<BlockPos> {
        world.locate(self.origin)
    }
}

// ── hooks ────────────────────────────────────────────────────────────────
//
// Both defer into a `commands.queue` closure with `&mut World`. Get-or-
// insert on `ChunkBlockEntities` is a structural change, which
// `DeferredWorld` can't do, and splitting it across two commands would race
// when two block-entities land in the same previously-plain chunk on one
// frame.

fn block_entity_tag_on_add(mut world: DeferredWorld, ctx: HookContext) {
    let entity = ctx.entity;
    let Some(tag) = world.entity(entity).get::<BlockEntityTag>().cloned() else { return };

    world.commands().queue(move |world: &mut World| {
        for address in &tag.cells {
            let Ok(mut chunk_ref) = world.get_entity_mut(address.chunk) else { continue };
            match chunk_ref.get_mut::<ChunkBlockEntities>() {
                Some(mut table) => { table.insert(address.local, entity); }
                None => {
                    let mut table = ChunkBlockEntities::default();
                    table.insert(address.local, entity);
                    chunk_ref.insert(table);
                }
            }
        }

        // Parenting to the chunk buys three things at once: recursive
        // despawn when the chunk unloads, transform propagation so a block
        // on a moving grid physically rides along, and a correct
        // GlobalTransform for spatial audio and particles — all without a
        // single system knowing that grids exist.
        if let Ok(mut e) = world.get_entity_mut(entity) {
            e.insert((
                ChildOf(tag.origin.chunk),
                Transform::from_translation(tag.origin.local.as_vec3() + Vec3::splat(0.5)),
                Visibility::default(),
            ));
        }
    });
}

fn block_entity_tag_on_remove(mut world: DeferredWorld, ctx: HookContext) {
    let entity = ctx.entity;
    // Still readable during on_remove.
    let Some(tag) = world.entity(entity).get::<BlockEntityTag>().cloned() else { return };

    world.commands().queue(move |world: &mut World| {
        for address in &tag.cells {
            let Ok(mut chunk_ref) = world.get_entity_mut(address.chunk) else { continue };
            let Some(mut table) = chunk_ref.get_mut::<ChunkBlockEntities>() else { continue };

            // Only clear if the slot still points at us — guards a
            // replace-in-place where the new entity registered first.
            if table.get(address.local) == Some(entity) {
                table.remove(address.local);
            }

            // Hand the chunk its zero-overhead state back.
            if table.is_empty() {
                chunk_ref.remove::<ChunkBlockEntities>();
            }
        }
    });
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – INTERACTION
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Marker: right-clicking this block-entity invokes it instead of placing
/// the held item's block. A spawner adds this next to whatever observer
/// actually handles the interaction.
#[derive(Component, Default, Debug)]
pub struct Interactable;

/// Sent *at* a block-entity when a player uses it.
///
/// Entity-targeted on purpose: handlers are attached per-entity by the
/// behavior that created it, so the interaction system never learns which
/// block types exist. Works identically for a barrel in a cave and a barrel
/// bolted to a ship.
///
/// Presentation watches this too — `ui::inventory` opens a screen for any
/// entity that turns out to have an `Inventory`, which is why a barrel does
/// not have to know that UI exists.
#[derive(EntityEvent, Clone, Copy, Debug)]
pub struct BlockEntityEvent {
    #[event_target]
    pub entity: Entity,
    /// The player (input context entity) that used the block.
    pub player: Entity,
    /// Which cell was clicked — matters for multiblocks. Space-local.
    pub at:     BlockPos,
    /// The face the ray geometrically struck. Space-local, so a conveyor
    /// keeps feeding the same neighbor when the ship rolls.
    pub face:   Direction,
    /// The face this interaction *addresses*.
    ///
    /// Equal to `face` for every block that does not declare `FaceTargeted`,
    /// so an existing handler that ignores this field keeps behaving
    /// exactly as it did. A pipe reads it to pick which connection to
    /// toggle; a barrel ignores it and opens its screen.
    pub target_face: Direction,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – THE SHARED BUILDER
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
    block_id: BlockID,
) -> Option<Entity> {
    let definition = registry.get(block_id);
    let spawns = definition.components.get::<SpawnsBlockEntities>()?;

    // Inserting the tag *is* indexing and parenting it — see the hooks.
    let root = commands
        .spawn((tag, Name::new(format!("BlockEntity<{}>", definition.name))))
        .id();

    let mut ctx = BlockSpawnContext { commands, root, at, block_id };
    for spawner in spawns.iter() {
        spawner.spawn(&mut ctx);
    }

    Some(root)
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 4 – LIFECYCLE OBSERVERS
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

    build_block_entity(&mut commands, &registry, tag, at, block_id);
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

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct BlockEntityPlugin;

impl Plugin for BlockEntityPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(spawn_block_entities_on_place_obs)
            .add_observer(despawn_block_entities_on_break_obs);
    }
}
