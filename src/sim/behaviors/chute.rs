use std::collections::HashSet;

use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::world::DeferredWorld;
use bevy::prelude::*;

use crate::content::block::components::{BlockEntitySpawner, BlockSpawnContext};
use crate::content::item::ItemRegistry;
use crate::sim::inventory::storage::{transfer_automated, Inventory, InventoryChangedEvent};
use crate::space::access::VoxelWorld;
use crate::space::address::BlockPos;

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
// Transfer is instantaneous — items leave the source and arrive at the
// destination within one tick, with nothing in between. Throughput, not
// latency, is what's being metered.
//
// Three layers live here, and only one of them ticks:
//
//   ChuteSegment  one per voxel. Pure index, plus the tuning read off its
//                 block definition. Never iterated per tick.
//   ChuteRun      one per contiguous column. The simulated unit: a
//                 hundred-block chute is one entity doing one transfer, not
//                 a hundred hops.
//   ChuteWatchers on the inventories at either end, so a sleeping run can
//                 be woken by whoever changes its circumstances.

// ── tuning ───────────────────────────────────────────────────────────────

/// Must match the app's fixed timestep. Rates are authored in batches per
/// second and converted with this, so the simulation stays integer-only.
const TICK_HZ: u32 = 60;

/// Credit needed to move one batch. Scaled by 1000 so that any whole-number
/// batches-per-second rate is exactly representable — seven per second is
/// not a whole number of ticks, and a plain countdown could not express it
/// at all.
const CREDIT_PER_BATCH: u32 = TICK_HZ * 1000;

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

/// Batches per second -> credit accrued per tick.
const fn credit_per_tick(batches_per_second: u32) -> u32 {
    batches_per_second * 1000
}

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

/// One contiguous vertical column of chute.
///
/// `space + column + top/bottom` is a `BlockPos`-shaped address, which means
/// it goes stale if a grid splits underneath it — see the note in
/// `space::address`. That is exactly why topology is rebuilt from a queue
/// rather than trusted: a `SpaceSplit` observer only has to enqueue the
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

    /// Block entity directly above `top`. Stored unvalidated: the transfer
    /// simply fails to find an `Inventory` and no-ops, which is cheaper
    /// than checking here and cannot go stale between checks.
    pub source: Option<Entity>,
    /// Block entity directly below `bottom`, same deal.
    pub sink: Option<Entity>,

    pub batch: u16,
    pub credit_per_tick: u32,
    /// Token bucket. Capped at one batch, so a chute blocked by a full
    /// destination can't bank credit and burst when it clears — otherwise
    /// players would discover that a deliberately-full buffer is a rate
    /// accumulator.
    pub credit: u32,
    /// Round-robin position over the source's slots. Lives here rather than
    /// on the inventory so that two chutes drawing from one barrel rotate
    /// independently.
    pub cursor: usize,
}

/// Marker: this run has work to do.
///
/// Sleep as an archetype rather than a branch. A sleeping run is not
/// *skipped* by the transport system, it is invisible to it, because it
/// lives in a different archetype and the query never touches it. Ten
/// thousand idle chutes cost nothing.
#[derive(Component)]
pub struct ChuteAwake;

/// Placed on an inventory entity: the runs that care when it changes.
///
/// **Both endpoints, not just the source.** With instant transfer, a run
/// whose destination is full moves nothing, concludes it has no work, and
/// sleeps — and if only the source could wake it, draining the destination
/// would leave it asleep forever. A chute that silently stopped an hour ago
/// is the worst bug this system can have, and this list is the whole fix.
///
/// Deliberately tolerant of staleness: entries are added, never pruned, and
/// the observer revalidates each one against the run's own endpoints.
/// A few dead `Entity`s cost less than removal bookkeeping.
#[derive(Component, Default)]
pub struct ChuteWatchers(pub Vec<Entity>);

/// Positions whose topology changed and hasn't been resolved yet.
///
/// Deferring graph work off the write path is the same pattern AE2 uses for
/// repathing: a placement sets a flag, and one pass at the top of the next
/// tick does all of it at once. A player holding the place key down gets
/// one rebuild per tick, not one per block.
#[derive(Resource, Default)]
pub struct ChuteTopologyQueue(pub Vec<BlockPos>);

fn segment_touched(mut world: DeferredWorld, ctx: HookContext) {
    let Some(segment) = world.get::<ChuteSegment>(ctx.entity) else { return };
    let at = segment.at;
    world.resource_mut::<ChuteTopologyQueue>().0.push(at);
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
        // `segment_touched` fires on removal and enqueues the rebuild.
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

/// Resolve every queued topology change.
///
/// One code path covers extend-up, extend-down, merge and split, because it
/// doesn't care which happened: it rescans the affected column, finds the
/// spans that exist *now*, and reconciles runs against them.
///
/// Because transfer is instantaneous there's no in-flight state to
/// preserve, so this is pure bookkeeping — no item positions to rebase, and
/// nothing to spill when a column splits.
pub fn rebuild_topology_sys(
    mut commands: Commands,
    mut queue: ResMut<ChuteTopologyQueue>,
    voxels: VoxelWorld,
    mut segments: Query<&mut ChuteSegment>,
    mut runs: Query<&mut ChuteRun>,
) {
    if queue.0.is_empty() {
        return;
    }

    // Phase 1 — read only. Decide which spans exist before touching
    // anything, so the set is derived from one consistent view of the world
    // and the borrow of `segments` stays immutable throughout the scan.
    let mut spans: Vec<Span> = Vec::new();
    let mut seen: HashSet<(Entity, IVec2, i32)> = HashSet::new();

    for at in std::mem::take(&mut queue.0) {
        // A break leaves `at` empty, so the spans that changed may be the
        // ones above and below it rather than one containing it.
        for seed in [at, at.offset(IVec3::Y), at.offset(IVec3::NEG_Y)] {
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
    runs: &mut Query<&mut ChuteRun>,
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
        Some(entity) if runs.get(entity).is_ok() => {
            let mut run = runs.get_mut(entity).unwrap();
            run.space = span.space;
            run.column = span.column;
            run.top = span.top;
            run.bottom = span.bottom;
            run.source = source;
            run.sink = sink;
            run.batch = batch;
            run.credit_per_tick = credit_rate;
            run.credit = run.credit.min(CREDIT_PER_BATCH);
            entity
        }
        _ => commands
            .spawn((
                Name::new(format!("ChuteRun({}, {})", span.column.x, span.column.y)),
                ChuteRun {
                    space: span.space,
                    column: span.column,
                    top: span.top,
                    bottom: span.bottom,
                    source,
                    sink,
                    batch,
                    credit_per_tick: credit_rate,
                    credit: 0,
                    cursor: 0,
                },
            ))
            .id(),
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

    // ── register for wake-ups at both ends, and start awake ─────────────
    for endpoint in [source, sink].into_iter().flatten() {
        watch(commands, endpoint, run_entity);
    }

    commands.entity(run_entity).insert(ChuteAwake);
}

/// Add `run` to an endpoint's watcher list.
///
/// Goes through `entry` rather than query-then-insert because two runs can
/// register on the same barrel in the same rebuild pass — one chute above
/// it and one below. A deferred `insert` would have the second overwrite
/// the first, and one of those two chutes would never wake again.
fn watch(commands: &mut Commands, endpoint: Entity, run: Entity) {
    commands
        .entity(endpoint)
        .entry::<ChuteWatchers>()
        .or_default()
        .and_modify(move |mut watchers| {
            if !watchers.0.contains(&run) {
                watchers.0.push(run);
            }
        });
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 4 – TRANSPORT
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Accrue credit, spend it on one batch when there's enough, sleep when
/// there's nothing to do.
pub fn tick_chutes_sys(
    mut commands: Commands,
    mut runs: Query<(Entity, &mut ChuteRun), With<ChuteAwake>>,
    mut inventories: Query<&mut Inventory>,
    registry: Res<ItemRegistry>,
) {
    for (entity, mut run) in runs.iter_mut() {
        run.credit = (run.credit + run.credit_per_tick).min(CREDIT_PER_BATCH);

        if run.credit < CREDIT_PER_BATCH {
            continue;
        }

        // No endpoints, or endpoints that hold no inventory. Sleep: only a
        // topology change can fix that, and it re-inserts `ChuteAwake`.
        let (Some(source), Some(sink)) = (run.source, run.sink) else {
            commands.entity(entity).remove::<ChuteAwake>();
            continue;
        };

        // Source and sink are always different blocks, so the aliasing
        // check here cannot fail for a well-formed run.
        let Ok([mut from, mut into]) = inventories.get_many_mut([source, sink]) else {
            commands.entity(entity).remove::<ChuteAwake>();
            continue;
        };

        let batch = run.batch;
        let moved = transfer_automated(
            &mut commands,
            (source, &mut from),
            (sink, &mut into),
            batch,
            &mut run.cursor,
            &registry,
        );

        if moved > 0 {
            run.credit -= CREDIT_PER_BATCH;
        } else {
            // Empty source, full sink, or nothing the sink will accept.
            // None of those can change without an `InventoryChangedEvent`
            // on an endpoint, and the watchers cover both.
            commands.entity(entity).remove::<ChuteAwake>();
        }
    }
}

/// Wake the runs attached to an inventory whose contents just changed.
///
/// The other half of sleeping, and the reason a chute between two idle
/// barrels is genuinely free rather than merely cheap.
///
/// This also fires for the events a chute's own transfer emits, which is
/// correct rather than circular: a working chute keeps itself awake, and
/// falls asleep the moment it stops moving anything.
pub fn wake_on_inventory_change_obs(
    event: On<InventoryChangedEvent>,
    mut commands: Commands,
    watchers: Query<&ChuteWatchers>,
    runs: Query<&ChuteRun>,
) {
    let Ok(list) = watchers.get(event.entity) else { return };

    for &run_entity in &list.0 {
        // Revalidate rather than prune: a stale entry simply fails here.
        let Ok(run) = runs.get(run_entity) else { continue };

        if run.source == Some(event.entity) || run.sink == Some(event.entity) {
            commands.entity(run_entity).insert(ChuteAwake);
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 5 – PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Topology strictly before transport. Every run the transport system sees
/// has already been reconciled with the voxel data this tick, so it never
/// has to defend against a stale endpoint or a despawned run.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChuteSet {
    Topology,
    Transport,
}

pub struct ChutePlugin;

impl Plugin for ChutePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChuteTopologyQueue>()
            .configure_sets(FixedUpdate, (ChuteSet::Topology, ChuteSet::Transport).chain())
            .add_systems(FixedUpdate, rebuild_topology_sys.in_set(ChuteSet::Topology))
            .add_systems(FixedUpdate, tick_chutes_sys.in_set(ChuteSet::Transport))
            .add_observer(wake_on_inventory_change_obs);
    }
}