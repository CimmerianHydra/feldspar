use std::collections::HashSet;

use bevy::prelude::*;
use serde::Deserialize;

use crate::content::block::behaviors::{BlockBehavior, BlockSpawnContext};
use crate::sim::transport::mover::{
    resolve_port, set_endpoints, ItemMover, ItemPort, TransportDirty, TransportSet,
};
use crate::sim::transport::network::{NetworkIndex, TransportNetwork};
use crate::space::access::VoxelWorld;
use crate::space::address::BlockPos;
use crate::voxel::rotation::BlockRotation;
use crate::voxel::{Direction, ALL_DIRECTIONS};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ITEM EXTRACTOR
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//
// An omnidirectional chute. It draws from the block on one face and pushes
// into whatever is on the opposite one, and the player picks which face is
// which by pointing it with a configuring tool.
//
// All the actual item-moving is `ItemMover`; this file is geometry. That is
// the whole point of the split — a new mover costs a resolution system, and
// nothing else.
//
// Facing lives in the voxel, not in a component.
//
// The extractor's configuration *is* its `BlockRotation`. Wrenching it
// rewrites the voxel; the mesh rotates for free, the setting survives save
// and load for free, it travels with a moving grid for free, and the write
// dirties the position so endpoints re-resolve through the path that
// already exists. The dispatch for that lives in `player::interaction`,
// which reads `Orientable` off this block's definition — so this file owns
// no interaction at all.
//
// ## What changed when pipes arrived
//
// One function call. `block_entity_at` became `resolve_port`, and the
// endpoints became `ItemPort` instead of `Option<Entity>`. An extractor
// pointed at a pipe now feeds a network; an extractor pointed at a barrel
// behaves exactly as it did. Nothing here knows what a pipe is.

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

/// Declares two components and stops.
///
/// Being pointable is not declared here — it comes from `orientable` in the
/// block's JSON, which is also what earns it the 3x3 face grid. That means
/// this file has no interaction code and no `Interactable`: the extractor
/// is not entity-interactive, it is *configurable*, and those are different
/// rungs of `handle_secondary_fire_obs`.
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
    const NAME: &'static str = "item_extractor";

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

/// Re-point every extractor whose surroundings changed.
///
/// An extractor cares about a write at its own position — it might be the
/// block that was placed, or its rotation might have changed — and about
/// writes at any of its six neighbours, since one of those may have just
/// become or stopped being its endpoint. Checking the position plus its
/// neighbours covers both without the extractor and the barrel needing to
/// know about each other.
///
/// A rebuilt pipe network re-dirties its own node cells for exactly this
/// reason: the network entity may have been recycled and the node index may
/// have moved, so any extractor touching it has to come back through here.
/// That is why this runs in `Topology`, strictly after `Network`.
pub fn resolve_extractors_sys(
    mut commands: Commands,
    dirty: Res<TransportDirty>,
    voxels: VoxelWorld,
    index: Res<NetworkIndex>,
    networks: Query<&TransportNetwork>,
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

        // Pipes push, they never pull. Collapsing a network on the input
        // face to `None` here rather than leaving the tick system to
        // decline it keeps the watcher lists honest — a source we will
        // never draw from has no business registering wake-ups on every
        // acceptor that network happens to reach.
        //
        // When the priority merger lands, "a network you may draw from" is
        // a real thing and this collapse becomes a check on which kind.
        let source = match resolve_port(&voxels, &index, at, output.opposite()) {
            ItemPort::Network { .. } => ItemPort::None,
            port => port,
        };

        let sink = resolve_port(&voxels, &index, at, output);

        set_endpoints(&mut commands, entity, &mut mover, source, sink, &networks);
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 4 – PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct ExtractorPlugin;

impl Plugin for ExtractorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            resolve_extractors_sys.in_set(TransportSet::Topology),
        );
    }
}