use std::collections::HashSet;

use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::world::DeferredWorld;
use bevy::prelude::*;

use crate::content::block::components::{BlockEntitySpawner, BlockSpawnContext};
use crate::sim::transport::mover::{
    credit_per_tick, set_endpoints, ItemMover, TransportDirty, TransportSet,
};
use crate::space::access::VoxelWorld;
use crate::space::address::BlockPos;
use crate::voxel::Direction;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// CHUTE
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//
// Gravity is free; horizontal costs power. A chute therefore runs straight
// down, for any distance, unpowered, and accepts input through its top face
// and nowhere else. That last clause is load-bearing: because a chute can
// never be fed from the side, no arrangement of chutes can form a
// horizontal chain, which is what keeps the pipe unlock meaningful.
//
// Since the extractor arrived, everything that actually *moves* items lives
// in `mover.rs`. What's left here is the one thing genuinely peculiar to a
// chute: a column of voxels collapses into a single run, and that run's
// endpoints are whatever sits above its top and below its bottom.
//
//   ChuteSegment  one per voxel. Pure index, plus the tuning read off its
//                 block definition. Never iterated per tick.
//   ChuteRun      one per contiguous column. Geometry only — the item
//                 moving is an `ItemMover` on the same entity.

// ── tuning ───────────────────────────────────────────────────────────────

/// Items per batch, and batches per second, for a basic chute.
///
/// Two axes rather than one because they *feel* different: a small batch on
/// a short period is smooth, a large batch on a long period is bursty and
/// needs buffer space downstream. That's a real choice for an upgrade tree,
/// not two spellings of the same number.
const DEFAULT_BATCH: u16 = 1;
const DEFAULT_RATE: u32 = 3;

/// Sanity bound on column scanning. A legitimate chute this long is
/// possible; a runaway scan is a bug, and should say so rather than
/// silently walk to the world floor.
const MAX_RUN_BLOCKS: i32 = 4096;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – COMPONENTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A single chute voxel's block entity.
///
/// Carries its own position so the lifecycle hook below is self-contained:
/// it fires however the entity dies — block broken, chunk unloaded, grid
/// split — without having to consult an address table.
///
/// It also carries this block's tuning, which is what lets a column of
/// mixed tiers resolve sensibly: the rebuild takes the *minimum* of each
/// axis across the span, so a run is as slow as its slowest segment.
/// Splicing a wooden chute into a steel column throttles the whole column,
/// which is both intuitive and free to compute.
///
/// `run` is `Entity::PLACEHOLDER` between placement and the next topology
/// pass. Nothing reads it in that window except the rebuild itself.
#[derive(Component, Debug)]
#[component(on_add = segment_touched, on_remove = segment_touched)]
pub struct ChuteSegment {
    pub at: BlockPos,
    pub run: Entity,
    pub batch: u16,
    pub credit_per_tick: u32,
}

/// One contiguous vertical column of chute. Geometry only.
///
/// `space + column + top/bottom` is a `BlockPos`-shaped address, which means
/// it goes stale if a grid splits underneath it — see the note in
/// `space::address`. That is exactly why topology is rebuilt from a dirty
/// list rather than trusted: a `SpaceSplit` observer only has to mark the
/// affected positions, and the rebuild re-derives everything.
#[derive(Component, Debug)]
pub struct ChuteRun {
    pub space: Entity,
    /// (x, z). The column this run occupies.
    pub column: IVec2,
    /// Highest chute voxel — draws from the block above this.
    pub top: i32,
    /// Lowest chute voxel — delivers into the block below this.
    pub bottom: i32,
}

/// Placing or breaking a chute changes which runs exist.
///
/// A hook rather than logic in the spawner's `despawn`, so that every route
/// out of the world — broken, unloaded, grid-split — goes through one path.
fn segment_touched(mut world: DeferredWorld, ctx: HookContext) {
    let Some(segment) = world.get::<ChuteSegment>(ctx.entity) else { return };
    let at = segment.at;
    world.resource_mut::<TransportDirty>().mark(at);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – BLOCK BEHAVIOR
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Same modularity contract as the barrel: declare a component and stop.
/// The hook on `ChuteSegment` handles both enrolment and withdrawal, so
/// there is no despawn logic here and no way for the two paths to drift.
#[derive(Clone, Copy, Debug)]
pub struct ChuteSpawner {
    pub batch: u16,
    pub batches_per_second: u32,
}

impl Default for ChuteSpawner {
    fn default() -> Self {
        Self { batch: DEFAULT_BATCH, batches_per_second: DEFAULT_RATE }
    }
}

impl BlockEntitySpawner for ChuteSpawner {
    fn spawn(&self, ctx: &mut BlockSpawnContext) {
        ctx.insert(ChuteSegment {
            at: ctx.at,
            run: Entity::PLACEHOLDER,
            batch: self.batch,
            credit_per_tick: credit_per_tick(self.batches_per_second),
        });
    }

    fn despawn(&self, _commands: &mut Commands, _root: Entity) {
        // `segment_touched` fires on removal and marks the rebuild.
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – TOPOLOGY
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A contiguous run of chute voxels, as found by scanning.
#[derive(Clone, Copy, Debug)]
struct Span {
    space: Entity,
    column: IVec2,
    top: i32,
    bottom: i32,
}

impl Span {
    fn at(&self, y: i32) -> BlockPos {
        BlockPos::new(self.space, IVec3::new(self.column.x, y, self.column.y))
    }
}

/// Reconcile runs against whatever the voxels currently say.
///
/// One code path covers extend-up, extend-down, merge and split, because it
/// doesn't care which happened: it rescans the affected column, finds the
/// spans that exist *now*, and reconciles runs against them. Four
/// hand-rolled cases is where the bugs would live.
///
/// Because transfer is instantaneous there's no in-flight state to
/// preserve, so this is pure bookkeeping — no item positions to rebase, and
/// nothing to spill when a column splits.
pub fn rebuild_chutes_sys(
    mut commands: Commands,
    dirty: Res<TransportDirty>,
    voxels: VoxelWorld,
    mut segments: Query<&mut ChuteSegment>,
    mut runs: Query<(&mut ChuteRun, &mut ItemMover)>,
) {
    if dirty.current().is_empty() {
        return;
    }

    // Phase 1 — read only. Decide which spans exist before touching
    // anything, so the set is derived from one consistent view of the world
    // and the borrow of `segments` stays immutable throughout the scan.
    let mut spans: Vec<Span> = Vec::new();
    let mut seen: HashSet<(Entity, IVec2, i32)> = HashSet::new();

    for &at in dirty.current() {
        // A break leaves `at` empty, so the spans that changed may be the
        // ones above and below it rather than one containing it. A write to
        // a *barrel* next to a chute lands here too, and finds the chute.
        let seeds = [at, at.neighbor(Direction::Up), at.neighbor(Direction::Down)];

        for seed in seeds {
            let Some(span) = find_span(&voxels, &segments, seed) else { continue };
            if seen.insert((span.space, span.column, span.top)) {
                spans.push(span);
            }
        }
    }

    // Phase 2 — commit.
    for span in spans {
        rebuild_span(&mut commands, &voxels, &mut segments, &mut runs, span);
    }
}

fn is_chute(voxels: &VoxelWorld, segments: &Query<&mut ChuteSegment>, at: BlockPos) -> bool {
    voxels
        .block_entity_at(at)
        .is_some_and(|entity| segments.get(entity).is_ok())
}

/// Walk up and down from `at` to find the contiguous run containing it.
///
/// Testing chute-ness through the block entity rather than the voxel id
/// keeps this tier-agnostic: a steel chute and a wooden one are both just
/// entities carrying `ChuteSegment`.
fn find_span(
    voxels: &VoxelWorld,
    segments: &Query<&mut ChuteSegment>,
    at: BlockPos,
) -> Option<Span> {
    if !is_chute(voxels, segments, at) {
        return None;
    }

    let column = IVec2::new(at.pos.x, at.pos.z);
    let probe = |y: i32| BlockPos::new(at.space, IVec3::new(column.x, y, column.y));

    let mut top = at.pos.y;
    while is_chute(voxels, segments, probe(top + 1)) {
        top += 1;
        if top - at.pos.y > MAX_RUN_BLOCKS {
            warn!("chute: runaway upward scan at {:?}, clamping", at.pos);
            break;
        }
    }

    let mut bottom = at.pos.y;
    while is_chute(voxels, segments, probe(bottom - 1)) {
        bottom -= 1;
        if at.pos.y - bottom > MAX_RUN_BLOCKS {
            warn!("chute: runaway downward scan at {:?}, clamping", at.pos);
            break;
        }
    }

    Some(Span { space: at.space, column, top, bottom })
}

fn rebuild_span(
    commands: &mut Commands,
    voxels: &VoxelWorld,
    segments: &mut Query<&mut ChuteSegment>,
    runs: &mut Query<(&mut ChuteRun, &mut ItemMover)>,
    span: Span,
) {
    // ── survey the column ────────────────────────────────────────────────
    let mut old: Vec<Entity> = Vec::new();
    let mut segment_entities: Vec<Entity> = Vec::new();

    // A run is as slow as its slowest segment; mixed tiers resolve by
    // taking the minimum of each axis independently.
    let mut batch = u16::MAX;
    let mut credit_rate = u32::MAX;

    for y in (span.bottom..=span.top).rev() {
        let Some(entity) = voxels.block_entity_at(span.at(y)) else { continue };
        let Ok(segment) = segments.get(entity) else { continue };

        segment_entities.push(entity);
        batch = batch.min(segment.batch);
        credit_rate = credit_rate.min(segment.credit_per_tick);

        if segment.run != Entity::PLACEHOLDER && !old.contains(&segment.run) {
            old.push(segment.run);
        }
    }

    if segment_entities.is_empty() {
        return;
    }

    let source = voxels.block_entity_at(span.at(span.top + 1));
    let sink = voxels.block_entity_at(span.at(span.bottom - 1));

    // ── settle on one run entity ─────────────────────────────────────────
    //
    // Reuse the topmost existing run where there is one, so extending a
    // column preserves accrued credit rather than handing the player a free
    // reset every time they place a block.
    let run_entity = match old.first().copied() {
        Some(entity) if runs.contains(entity) => {
            let (mut run, mut mover) = runs.get_mut(entity).unwrap();

            run.space = span.space;
            run.column = span.column;
            run.top = span.top;
            run.bottom = span.bottom;

            mover.batch = batch;
            mover.credit_per_tick = credit_rate;
            set_endpoints(commands, entity, &mut mover, source, sink);

            entity
        }
        _ => {
            let mut mover = ItemMover::new(batch, 0);
            mover.credit_per_tick = credit_rate;

            let entity = commands
                .spawn((
                    Name::new(format!("ChuteRun({}, {})", span.column.x, span.column.y)),
                    ChuteRun {
                        space: span.space,
                        column: span.column,
                        top: span.top,
                        bottom: span.bottom,
                    },
                ))
                .id();

            set_endpoints(commands, entity, &mut mover, source, sink);
            commands.entity(entity).insert(mover);

            entity
        }
    };

    // Absorbed runs are now unreferenced.
    for &stale in old.iter().skip(1) {
        commands.entity(stale).despawn();
    }

    // ── point every segment at it ────────────────────────────────────────
    for entity in segment_entities {
        if let Ok(mut segment) = segments.get_mut(entity) {
            segment.run = run_entity;
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 4 – PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// One system. Everything else a chute does is `ItemTransportPlugin`.
pub struct ChutePlugin;

impl Plugin for ChutePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, rebuild_chutes_sys.in_set(TransportSet::Topology));
    }
}