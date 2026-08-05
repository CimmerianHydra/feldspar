use std::collections::{HashMap, HashSet};
use serde::Deserialize;

use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::world::DeferredWorld;
use bevy::prelude::*;

use crate::content::block::behaviors::{BlockBehavior, BlockSpawnContext};
use crate::sim::transport::mover::{
    credit_per_tick, resolve_port, set_endpoints, ItemMover, ItemPort, TransportDirty,
    TransportSet,
};
use crate::sim::transport::network::{NetworkIndex, TransportNetwork};
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
//
// The invariant this file exists to maintain:
//
//   for every column, the set of runs is exactly the set of spans.
//
// Note the *exactly*. A `ChuteRun` is an entity with no owner — no voxel,
// no parent, nothing that dies and takes it along — so if the rebuild only
// ever asks "which spans need a run?" then a run whose voxels have all
// gone is never asked about, and never dies. It keeps its endpoints, keeps
// its place in both barrels' watcher lists, and keeps moving items from a
// chute that is no longer there. Place and break the same block twenty
// times and you get twenty invisible movers pumping the same pair of
// barrels in parallel.
//
// So the rebuild reconciles in both directions, and its unit of work is the
// *column*, not the span. A span that has just been deleted cannot be
// found by scanning, which is exactly why it cannot be the thing that
// drives the search.
//
// ## What changed when pipes arrived
//
// The two lines that found the blocks above and below a run. They ask
// `resolve_port` now, so a chute that ends over a pipe drops into the
// network instead of into nothing. Everything else in this file — spans,
// surveys, the claim rules, the sweep — is untouched, which is the test
// that `ItemPort` was cut in the right place.

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
///
/// The rebuild does read `space` and `column` — that's how a run whose
/// voxels are all gone is still found — but only ever as a *hint* about
/// which column to rescan, never as truth. A stale hint costs one wasted
/// scan; the run is also reachable through any segment pointing at it, and
/// whichever route finds it, the geometry is overwritten from voxels.
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
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ChuteBehavior {
    pub batch: u16,
    pub batches_per_second: u32,
}

impl Default for ChuteBehavior {
    fn default() -> Self {
        Self { batch: DEFAULT_BATCH, batches_per_second: DEFAULT_RATE }
    }
}

impl BlockBehavior for ChuteBehavior {
    const NAME: &'static str = "chute";

    fn on_place(&self, ctx: &mut BlockSpawnContext) {
        ctx.insert(ChuteSegment {
            at: ctx.at,
            run: Entity::PLACEHOLDER,
            batch: self.batch,
            credit_per_tick: credit_per_tick(self.batches_per_second),
        });
    }

    // `segment_touched` fires on removal and marks the rebuild, so there is
    // no teardown here and no way for the two paths to drift.

    /// The chute's own systems travel with the chute.
    fn build(app: &mut App) {
        app.add_plugins(ChutePlugin);
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

    /// Voxels shared with a run's stored extent. Zero means the run has no
    /// business continuing as this span.
    fn overlap(&self, run: &ChuteRun) -> i32 {
        (self.top.min(run.top) - self.bottom.max(run.bottom) + 1).max(0)
    }
}

/// The unit of work.
///
/// Deliberately not a span. Half of what the rebuild has to fix is spans
/// that stopped existing, and those are unreachable from a scan — the
/// voxels are gone. A column is reachable whatever happened to it, because
/// it's derived from the dirty *position*, which survives its block.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct ColumnKey {
    space: Entity,
    column: IVec2,
}

impl ColumnKey {
    fn of(at: BlockPos) -> Self {
        Self { space: at.space, column: IVec2::new(at.pos.x, at.pos.z) }
    }

    fn at(&self, y: i32) -> BlockPos {
        BlockPos::new(self.space, IVec3::new(self.column.x, y, self.column.y))
    }
}

/// Everything gathered about one dirty column before anything is written.
#[derive(Default)]
struct ColumnWork {
    /// Heights worth scanning from.
    probes: HashSet<i32>,
    /// Runs that currently claim this column.
    claimants: Vec<Entity>,
    /// Spans that exist now. Filled in phase 1c.
    spans: Vec<Span>,
}

/// One span, read in a single downward pass, with everything the commit
/// needs. Separated from the commit so the whole read happens under an
/// immutable borrow of `segments`.
struct Survey {
    span: Span,
    segments: Vec<Entity>,
    batch: u16,
    credit_per_tick: u32,
    /// Live runs the span's own segments point at, topmost first, deduped.
    referenced: Vec<Entity>,
}

/// Reconcile runs against whatever the voxels currently say.
///
/// One code path covers extend, merge, split and delete, because it doesn't
/// care which happened: it rescans each affected column, works out the
/// spans that exist *now*, and makes the runs match. Four hand-rolled cases
/// is where the bugs would live — and the case that was missing is the one
/// with no span left to hang the logic off.
///
/// Because transfer is instantaneous there's no in-flight state to
/// preserve, so this is pure bookkeeping — no item positions to rebase, and
/// nothing to spill when a column splits.
pub fn rebuild_chutes_sys(
    mut commands: Commands,
    dirty: Res<TransportDirty>,
    voxels: VoxelWorld,
    index: Res<NetworkIndex>,
    networks: Query<&TransportNetwork>,
    mut segments: Query<&mut ChuteSegment>,
    mut runs: Query<(Entity, &mut ChuteRun, &mut ItemMover)>,
) {
    if dirty.current().is_empty() {
        return;
    }

    // ── Phase 1 — read only ──────────────────────────────────────────────
    //
    // Decide everything before touching anything, so the whole decision is
    // derived from one consistent view of the world and the borrow of
    // `segments` stays immutable throughout.

    // 1a. Every dirty position names a column.
    //
    // A break leaves its own position empty, so the spans that changed may
    // be the ones above and below it — hence the ±1 probes. A write to a
    // *barrel* above or below a chute lands in the same column too, which
    // is what makes endpoint re-resolution fall out of the same grouping
    // rather than needing a rule of its own. A rebuilt pipe network
    // re-dirties its node cells for the same reason, and they land here by
    // the same route.
    let mut columns: HashMap<ColumnKey, ColumnWork> = HashMap::new();

    for &at in dirty.current() {
        let work = columns.entry(ColumnKey::of(at)).or_default();
        work.probes.extend([at.pos.y - 1, at.pos.y, at.pos.y + 1]);
    }

    // 1b. Which runs claim a dirty column.
    //
    // One pass over every run rather than one filtered pass per column, so
    // this stays O(runs) no matter how many columns a chunk load dirties.
    // There is one run per chute column in the world, so the scan is small.
    //
    // Seeding probes from the run's own extent is not optional. Without it,
    // a write far up an otherwise-untouched column would find no spans near
    // the write, and the sweep at the end would conclude the run is
    // orphaned and delete a working chute. Probing top and bottom
    // guarantees every claimant gets re-derived from voxels, which is what
    // makes the sweep safe to run unconditionally.
    for (entity, run, _) in runs.iter() {
        let key = ColumnKey { space: run.space, column: run.column };
        let Some(work) = columns.get_mut(&key) else { continue };

        work.claimants.push(entity);
        work.probes.extend([run.bottom, run.top]);
    }

    // 1c. Which spans exist now.
    for (key, work) in columns.iter_mut() {
        let mut tops: HashSet<i32> = HashSet::new();

        for &y in &work.probes {
            let Some(span) = find_span(&voxels, &segments, key.at(y)) else { continue };
            if tops.insert(span.top) {
                work.spans.push(span);
            }
        }

        // Top-down, which makes the order deterministic and preserves the
        // old "topmost run wins" rule when a merge has to pick a survivor.
        work.spans.sort_unstable_by_key(|span| std::cmp::Reverse(span.top));
    }

    // ── Phase 2 — commit, one column at a time ───────────────────────────
    for work in columns.into_values() {
        rebuild_column(
            &mut commands,
            &voxels,
            &index,
            &networks,
            &mut segments,
            &mut runs,
            work,
        );
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

/// Read one span without writing anything.
fn survey_span(
    voxels: &VoxelWorld,
    segments: &Query<&mut ChuteSegment>,
    span: Span,
) -> Option<Survey> {
    let mut entities = Vec::new();
    let mut referenced = Vec::new();

    // A run is as slow as its slowest segment; mixed tiers resolve by
    // taking the minimum of each axis independently.
    let mut batch = u16::MAX;
    let mut rate = u32::MAX;

    for y in (span.bottom..=span.top).rev() {
        let Some(entity) = voxels.block_entity_at(span.at(y)) else { continue };
        let Ok(segment) = segments.get(entity) else { continue };

        entities.push(entity);
        batch = batch.min(segment.batch);
        rate = rate.min(segment.credit_per_tick);

        if segment.run != Entity::PLACEHOLDER && !referenced.contains(&segment.run) {
            referenced.push(segment.run);
        }
    }

    if entities.is_empty() {
        return None;
    }

    Some(Survey { span, segments: entities, batch, credit_per_tick: rate, referenced })
}

/// What a run at this span draws from, and what it delivers into.
///
/// The only place in this file that knows endpoints are not always
/// inventories. A chute ending over a pipe hands items to the network; a
/// chute ending over a barrel behaves exactly as it always did.
///
/// The source is collapsed to `None` when it resolves to a network, for the
/// same reason it is in the extractor: nothing pulls out of a pipe, and a
/// source we will never draw from should not be registering wake-ups on
/// every acceptor that network reaches.
fn endpoints_for(
    voxels: &VoxelWorld,
    index: &NetworkIndex,
    span: Span,
) -> (ItemPort, ItemPort) {
    let source = match resolve_port(voxels, index, span.at(span.top), Direction::Up) {
        ItemPort::Network { .. } => ItemPort::None,
        port => port,
    };

    let sink = resolve_port(voxels, index, span.at(span.bottom), Direction::Down);

    (source, sink)
}

/// Reconcile one column's runs against one column's spans.
///
/// Both directions, which is the whole point. Every span ends up with
/// exactly one run, and — the arm that used to be missing — every run left
/// without a span is despawned. "Column emptied" and "merge absorbed a
/// neighbour" are then the same three lines, rather than one case handled
/// and one case forgotten.
#[allow(clippy::too_many_arguments)]
fn rebuild_column(
    commands: &mut Commands,
    voxels: &VoxelWorld,
    index: &NetworkIndex,
    networks: &Query<&TransportNetwork>,
    segments: &mut Query<&mut ChuteSegment>,
    runs: &mut Query<(Entity, &mut ChuteRun, &mut ItemMover)>,
    work: ColumnWork,
) {
    let ColumnWork { spans, mut claimants, .. } = work;

    // ── survey every span first ──────────────────────────────────────────
    let surveys: Vec<Survey> = spans
        .into_iter()
        .filter_map(|span| survey_span(voxels, segments, span))
        .collect();

    // A run reachable only through a segment — because its own stored
    // address went stale under a grid split — would otherwise escape the
    // sweep. Two independent routes to the same set, unioned, so a run has
    // to be invisible to both to leak.
    for survey in &surveys {
        for &entity in &survey.referenced {
            if !claimants.contains(&entity) {
                claimants.push(entity);
            }
        }
    }

    // ── match spans to runs ──────────────────────────────────────────────
    //
    // Both lists are at most a handful of entries — a column with three
    // separate runs in it is already unusual — so linear membership beats
    // allocating a set.
    let mut taken: Vec<Entity> = Vec::with_capacity(surveys.len());

    for survey in surveys {
        let span = survey.span;
        let (source, sink) = endpoints_for(voxels, index, span);

        let mut reused = None;

        if let Some(entity) = pick_run(runs, &claimants, &taken, &survey) {
            if let Ok((_, mut run, mut mover)) = runs.get_mut(entity) {
                run.space = span.space;
                run.column = span.column;
                run.top = span.top;
                run.bottom = span.bottom;

                mover.batch = survey.batch;
                mover.credit_per_tick = survey.credit_per_tick;
                set_endpoints(commands, entity, &mut mover, source, sink, networks);

                reused = Some(entity);
            }
        }

        let run_entity = match reused {
            Some(entity) => entity,
            None => spawn_run(commands, &survey, source, sink, networks),
        };

        taken.push(run_entity);

        for entity in survey.segments {
            if let Ok(mut segment) = segments.get_mut(entity) {
                segment.run = run_entity;
            }
        }
    }

    // ── sweep ────────────────────────────────────────────────────────────
    //
    // Nothing to unwind first. `MoverWatchers` is documented as tolerant of
    // dead entries and revalidates each against the mover query, so a
    // despawned run simply stops being woken and its endpoints never
    // notice. The `contains` guard is for claimants that arrived through a
    // stale `segment.run` and are already gone.
    for entity in claimants {
        if !taken.contains(&entity) && runs.contains(entity) {
            commands.entity(entity).despawn();
        }
    }
}

/// Which existing run, if any, should this span continue?
///
/// Two rules, in order:
///
/// 1. The topmost live run the span's own segments already point at. This
///    is the old behaviour: extending a column upward continues the run
///    that was there rather than handing the player a free credit reset.
/// 2. Failing that, the unclaimed run whose stored extent overlaps this
///    span most, ties broken upward. Covers a span whose segments are all
///    freshly spawned — a chunk reload — while its run survived.
///
/// A run already handed to a taller span is never handed out twice, and
/// that single fact is what makes a split produce two runs instead of two
/// spans fighting over one and the loser silently going dark.
fn pick_run(
    runs: &Query<(Entity, &mut ChuteRun, &mut ItemMover)>,
    claimants: &[Entity],
    taken: &[Entity],
    survey: &Survey,
) -> Option<Entity> {
    let available = |entity: Entity| runs.contains(entity) && !taken.contains(&entity);

    if let Some(&entity) = survey.referenced.iter().find(|&&e| available(e)) {
        return Some(entity);
    }

    claimants
        .iter()
        .copied()
        .filter(|&entity| available(entity))
        .filter_map(|entity| {
            let (_, run, _) = runs.get(entity).ok()?;
            let overlap = survey.span.overlap(run);
            (overlap > 0).then_some((overlap, run.top, entity))
        })
        .max_by_key(|&(overlap, top, _)| (overlap, top))
        .map(|(_, _, entity)| entity)
}

fn spawn_run(
    commands: &mut Commands,
    survey: &Survey,
    source: ItemPort,
    sink: ItemPort,
    networks: &Query<&TransportNetwork>,
) -> Entity {
    let span = survey.span;

    let mut mover = ItemMover::new(survey.batch, 0);
    mover.credit_per_tick = survey.credit_per_tick;

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

    set_endpoints(commands, entity, &mut mover, source, sink, networks);
    commands.entity(entity).insert(mover);

    entity
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 4 – DEBUG TRIPWIRE
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Assert that every run is still vouched for by its own voxels.
///
/// The failure this guards against is invisible in the world — a leaked run
/// has no blocks — and only shows up as a device running at some integer
/// multiple of its rated speed, which reads as a tuning problem rather than
/// a lifecycle one. Checking the two end voxels is O(runs) and catches it
/// on the very next tick.
///
/// Endpoints only, not the whole extent: a leak always shows there, and
/// walking every voxel of every run would make a long chute expensive to
/// validate.
#[cfg(debug_assertions)]
fn debug_orphaned_runs_sys(
    voxels: VoxelWorld,
    segments: Query<&ChuteSegment>,
    runs: Query<(Entity, &ChuteRun)>,
) {
    for (entity, run) in runs.iter() {
        for y in [run.bottom, run.top] {
            let at = BlockPos::new(run.space, IVec3::new(run.column.x, y, run.column.y));

            let vouched = voxels
                .block_entity_at(at)
                .and_then(|block| segments.get(block).ok())
                .is_some_and(|segment| segment.run == entity);

            if !vouched {
                warn_once!(
                    "chute: run {entity} claims column {:?} y={y}, but that voxel does not \
                     claim it back — a run has outlived its segments",
                    run.column
                );
            }
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 5 – PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// One system. Everything else a chute does is `ItemTransportPlugin`.
pub struct ChutePlugin;

impl Plugin for ChutePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, rebuild_chutes_sys.in_set(TransportSet::Topology));

        // After the whole topology set, so the rebuild's spawns and
        // despawns have been applied before anything is asserted about
        // them.
        #[cfg(debug_assertions)]
        app.add_systems(
            FixedUpdate,
            debug_orphaned_runs_sys.after(TransportSet::Topology),
        );
    }
}