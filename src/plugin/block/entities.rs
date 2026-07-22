use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
};

use crate::plugin::space::prelude::*;
use crate::plugin::voxel::Direction;

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
#[derive(EntityEvent, Clone, Copy, Debug)]
pub struct BlockEntityEvent {
    #[event_target]
    pub entity: Entity,
    /// The player (input context entity) that used the block.
    pub player: Entity,
    /// Which cell was clicked — matters for multiblocks. Space-local.
    pub at:     BlockPos,
    /// Which face was clicked — matters for sided machines. Space-local, so
    /// a conveyor keeps feeding the same neighbor when the ship rolls.
    pub face:   Direction,
}
