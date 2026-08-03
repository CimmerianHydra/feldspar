use std::collections::HashSet;

use bevy::prelude::*;
use serde::Deserialize;

use crate::content::block::behaviors::{BlockBehavior, BlockSpawnContext};
use crate::sim::BlockEntityEvent;
use crate::sim::transport::mover::{set_endpoints, ItemMover, TransportDirty, TransportSet};
use crate::space::access::VoxelWorld;
use crate::space::address::BlockPos;
use crate::voxel::rotation::BlockRotation;
use crate::voxel::{Direction, Voxel, ALL_DIRECTIONS};
use crate::prelude::VoxelWriteRequest;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ITEM EXTRACTOR
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//
// An omnidirectional chute. It draws from the block on one face and pushes
// into the block on the opposite one, and the player picks which face is
// which by right-clicking it.
//
// All the actual item-moving is `ItemMover`; this file is geometry and one
// interaction. That is the whole point of the split — a new mover costs a
// resolution system, and nothing else.
//
// Facing lives in the voxel, not in a component.
//
// The extractor's configuration *is* its `BlockRotation`. Right-clicking
// rewrites the voxel; the mesh rotates for free, the setting survives save
// and load for free, it travels with a moving grid for free, and the write
// dirties the position so endpoints re-resolve through the path that
// already exists.


// ── tuning ───────────────────────────────────────────────────────────────

/// Which way an *unrotated* extractor points.
///
/// Down, so a freshly-placed extractor behaves exactly like a chute. Good
/// for teaching the block: it does the familiar thing until you turn it.
const CANONICAL_OUTPUT: Direction = Direction::Down;

const DEFAULT_BATCH: u16 = 1;
const DEFAULT_RATE: u32 = 3;

/// The face a rotated extractor pushes out of.
#[inline]
fn output_of(rotation: BlockRotation) -> Direction {
    rotation.apply_dir(CANONICAL_OUTPUT)
}

/// The rotation that makes an extractor push out of `output`.
#[inline]
fn rotation_for_output(output: Direction) -> BlockRotation {
    BlockRotation::from_parts(output, 0)
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – COMPONENTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// One extractor block.
///
/// Carries its own position so resolution never has to consult an address
/// table, and a cached output face so the tick path never has to read a
/// voxel.
#[derive(Component, Debug)]
pub struct ItemExtractor {
    pub at: BlockPos,
    /// Cached from the voxel's rotation, which is authoritative. Refreshed
    /// by `resolve_extractors_sys`.
    pub output: Direction,
}

impl ItemExtractor {
    /// The face items are drawn from — always opposite the output.
    #[inline]
    pub fn input(&self) -> Direction {
        self.output.opposite()
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – BLOCK BEHAVIOR
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Declares three components and stops. `Interactable` is what routes a
/// right-click here instead of placing the held block; the observer in
/// section 4 is what it routes to.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExtractorBehavior {
    pub batch: u16,
    pub batches_per_second: u32,
}

impl Default for ExtractorBehavior {
    fn default() -> Self {
        Self { batch: DEFAULT_BATCH, batches_per_second: DEFAULT_RATE }
    }
}

impl BlockBehavior for ExtractorBehavior {
    const NAME: &'static str = "item_extractor";   // whatever the JSON says today

    fn on_place(&self, ctx: &mut BlockSpawnContext) {
        ctx.insert((
            // `output` is a placeholder; the first resolution pass reads
            // the real value off the voxel. The write that placed this
            // block already dirtied the position, so that pass is the very
            // next tick.
            ItemExtractor { at: ctx.at, output: CANONICAL_OUTPUT },
            ItemMover::new(self.batch, self.batches_per_second),
        ));
    }

    fn build(app: &mut App) {
        app.add_plugins(ExtractorPlugin);
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – RESOLUTION
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Re-point every extractor whose surroundings changed.
///
/// An extractor cares about a write at its own position — it might be the
/// block that was placed, or its rotation might have changed — and about
/// writes at any of its six neighbours, since one of those may have just
/// become or stopped being its endpoint. Checking the position plus its
/// neighbours covers both without the extractor and the barrel needing to
/// know about each other.
pub fn resolve_extractors_sys(
    mut commands: Commands,
    dirty: Res<TransportDirty>,
    voxels: VoxelWorld,
    mut extractors: Query<(&mut ItemExtractor, &mut ItemMover)>,
) {
    if dirty.current().is_empty() {
        return;
    }

    // Phase 1 — read only. Which extractors are affected?
    let mut affected: HashSet<Entity> = HashSet::new();

    for &at in dirty.current() {
        let candidates = std::iter::once(at).chain(ALL_DIRECTIONS.map(|d| at.neighbor(d)));

        for pos in candidates {
            let Some(entity) = voxels.block_entity_at(pos) else { continue };
            if extractors.contains(entity) {
                affected.insert(entity);
            }
        }
    }

    // Phase 2 — commit.
    for entity in affected {
        let Ok((mut extractor, mut mover)) = extractors.get_mut(entity) else { continue };

        let at = extractor.at;
        let output = output_of(voxels.get_voxel(at).rotation());
        extractor.output = output;

        let source = voxels.block_entity_at(at.neighbor(output.opposite()));
        let sink = voxels.block_entity_at(at.neighbor(output));

        set_endpoints(&mut commands, entity, &mut mover, source, sink);
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 5 – PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct ExtractorPlugin;

impl Plugin for ExtractorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            resolve_extractors_sys.in_set(TransportSet::Topology),
        )
        ;
    }
}