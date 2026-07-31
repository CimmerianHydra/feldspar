use bevy::prelude::*;

use crate::content::item::ItemRegistry;
use crate::sim::inventory::storage::{transfer_automated, Inventory, InventoryChangedEvent};
use crate::space::address::BlockPos;
use crate::prelude::VoxelWriteRequest;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ITEM MOVER
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//
// Strip a chute down to what actually moves items and you get: a source, a
// sink, a batch size, a rate, a slot cursor, and a sleep flag. None of that
// is vertical. An extractor needs the same six things pointed somewhere
// else, and so will every loader, pump and bus after it.
//
// So the machinery lives here as one component and one system, and each
// device contributes only the part that is genuinely its own: working out
// *which* two entities are its endpoints. Once it has, it calls
// `set_endpoints` and stops caring.
//
// Everything in this module is device-agnostic. Nothing here knows what a
// chute is.

// ── tuning ───────────────────────────────────────────────────────────────

/// Must match the app's fixed timestep. Rates are authored in batches per
/// second and converted with this, so the simulation stays integer-only.
pub const TICK_HZ: u32 = 60;

/// Credit needed to move one batch. Scaled by 1000 so any whole-number
/// batches-per-second rate is exactly representable — seven per second is
/// not a whole number of ticks, and a plain countdown could not express it
/// at all.
pub const CREDIT_PER_BATCH: u32 = TICK_HZ * 1000;

/// Batches per second -> credit accrued per tick.
pub const fn credit_per_tick(batches_per_second: u32) -> u32 {
    batches_per_second * 1000
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – COMPONENTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Moves items from one inventory to another on a schedule.
///
/// Sits alongside whatever component describes the device's *shape* — a
/// `ChuteRun` for a column, an `ItemExtractor` for a single block. The tick
/// system below queries only this, so adding a new kind of mover costs a
/// resolution system and nothing else.
#[derive(Component, Debug)]
pub struct ItemMover {
    /// Where items come from. `None` means unresolved or not an inventory;
    /// stored unvalidated because the transfer simply no-ops, which is
    /// cheaper than checking here and cannot go stale between checks.
    pub source: Option<Entity>,
    /// Where items go.
    pub sink: Option<Entity>,

    /// Items per batch.
    pub batch: u16,
    pub credit_per_tick: u32,
    /// Token bucket, capped at one batch.
    ///
    /// A bucket rather than a countdown because a countdown can only
    /// express rates that divide the tick evenly — seven batches a second
    /// is simply not sayable at 60 Hz. The cap matters just as much: an
    /// uncapped bucket lets a mover blocked by a full destination bank
    /// credit and burst when it clears, and players would quickly discover
    /// that a deliberately-full buffer is a rate accumulator.
    pub credit: u32,

    /// Round-robin position over the source's slots. Lives on the device,
    /// not the inventory, so two movers drawing from one barrel rotate
    /// independently.
    pub cursor: usize,
}

impl ItemMover {
    pub fn new(batch: u16, batches_per_second: u32) -> Self {
        Self {
            source: None,
            sink: None,
            batch,
            credit_per_tick: credit_per_tick(batches_per_second),
            credit: 0,
            cursor: 0,
        }
    }
}

/// Marker: this mover has work to do.
///
/// Sleep as an archetype rather than a branch. A sleeping mover is not
/// *skipped* by the tick system, it is invisible to it, because it lives in
/// a different archetype and the query never touches it. Ten thousand idle
/// chutes cost nothing.
#[derive(Component)]
pub struct MoverAwake;

/// Placed on an inventory entity: the movers that care when it changes.
///
/// **Both endpoints, not just the source.** A mover whose destination is
/// full moves nothing, concludes it has no work, and sleeps — and if only
/// the source could wake it, draining the destination would leave it asleep
/// forever. A chute that silently stopped an hour ago is the worst bug this
/// system can have, and this list is the whole fix.
///
/// Deliberately tolerant of staleness: entries are added, never pruned, and
/// the observer revalidates each against the mover's own endpoints. A few
/// dead `Entity`s cost less than removal bookkeeping.
#[derive(Component, Default)]
pub struct MoverWatchers(pub Vec<Entity>);

/// Block positions whose surroundings changed and may need re-resolving.
///
/// Double-buffered on purpose. Entries arrive from observers and component
/// hooks at unpredictable points in the frame, so a single list that gets
/// cleared at the end of the tick can silently drop anything queued after
/// the topology systems ran. Instead `pending` accumulates freely and is
/// swapped into `current` once, at the top of the topology set, giving
/// every consumer the same snapshot and losing nothing.
#[derive(Resource, Default)]
pub struct TransportDirty {
    pending: Vec<BlockPos>,
    current: Vec<BlockPos>,
}

impl TransportDirty {
    pub fn mark(&mut self, at: BlockPos) {
        self.pending.push(at);
    }

    /// This tick's snapshot. Read as many times as you like.
    pub fn current(&self) -> &[BlockPos] {
        &self.current
    }

    /// Swap the accumulated list into this tick's snapshot.
    ///
    /// Lives here rather than in the system because `ResMut` reaches its
    /// fields through `DerefMut`, so two field accesses in one expression
    /// are two overlapping mutable borrows of the *guard*. Behind a plain
    /// `&mut self` they're ordinary disjoint field borrows and the problem
    /// disappears.
    ///
    /// Swap rather than `take`, so both buffers keep their allocations
    /// instead of one being freed and the other reallocated every tick.
    pub fn rotate(&mut self) {
        self.current.clear();
        std::mem::swap(&mut self.pending, &mut self.current);
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – ENDPOINT PLUMBING
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Point a mover at its endpoints, register it for wake-ups at both, and
/// wake it.
///
/// The one function every device calls at the end of its own resolution
/// logic. Keeping the watcher registration in here rather than at each call
/// site is what makes it structurally impossible to add a device that
/// resolves endpoints correctly but never wakes up.
pub fn set_endpoints(
    commands: &mut Commands,
    mover_entity: Entity,
    mover: &mut ItemMover,
    source: Option<Entity>,
    sink: Option<Entity>,
) {
    mover.source = source;
    mover.sink = sink;

    for endpoint in [source, sink].into_iter().flatten() {
        watch(commands, endpoint, mover_entity);
    }

    commands.entity(mover_entity).insert(MoverAwake);
}

/// Add `mover` to an endpoint's watcher list.
///
/// Goes through `entry` rather than query-then-insert because two movers
/// can register on the same barrel in the same pass — one chute above it
/// and one below. A deferred `insert` would have the second overwrite the
/// first, and one of those two would never wake again.
fn watch(commands: &mut Commands, endpoint: Entity, mover: Entity) {
    commands
        .entity(endpoint)
        .entry::<MoverWatchers>()
        .or_default()
        .and_modify(move |mut watchers| {
            if !watchers.0.contains(&mover) {
                watchers.0.push(mover);
            }
        });
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – TRANSPORT
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Accrue credit, spend it on one batch when there's enough, sleep when
/// there's nothing to do.
///
/// This is every item mover in the game. It does not know whether it is
/// driving a chute, an extractor, or something that doesn't exist yet.
pub fn tick_movers_sys(
    mut commands: Commands,
    mut movers: Query<(Entity, &mut ItemMover), With<MoverAwake>>,
    mut inventories: Query<&mut Inventory>,
    registry: Res<ItemRegistry>,
) {
    for (entity, mut mover) in movers.iter_mut() {
        mover.credit = (mover.credit + mover.credit_per_tick).min(CREDIT_PER_BATCH);

        if mover.credit < CREDIT_PER_BATCH {
            continue;
        }

        // No endpoints, or endpoints that hold no inventory. Sleep: only a
        // topology change can fix that, and resolution re-inserts
        // `MoverAwake` when it does.
        let (Some(source), Some(sink)) = (mover.source, mover.sink) else {
            commands.entity(entity).remove::<MoverAwake>();
            continue;
        };

        // A well-formed mover always has two distinct endpoints, so the
        // aliasing check cannot fail. A malformed one sleeps, which is the
        // correct outcome anyway.
        let Ok([mut from, mut into]) = inventories.get_many_mut([source, sink]) else {
            commands.entity(entity).remove::<MoverAwake>();
            continue;
        };

        let batch = mover.batch;
        let moved = transfer_automated(
            &mut commands,
            (source, &mut from),
            (sink, &mut into),
            batch,
            &mut mover.cursor,
            &registry,
        );

        if moved > 0 {
            mover.credit -= CREDIT_PER_BATCH;
        } else {
            // Empty source, full sink, or nothing the sink will accept.
            // None of those can change without an `InventoryChangedEvent`
            // on an endpoint, and the watchers cover both.
            commands.entity(entity).remove::<MoverAwake>();
        }
    }
}

/// Wake the movers attached to an inventory whose contents just changed.
///
/// The other half of sleeping, and the reason a chute between two idle
/// barrels is genuinely free rather than merely cheap.
///
/// This also fires for the events a mover's own transfer emits, which is
/// correct rather than circular: a working mover keeps itself awake, and
/// falls asleep the moment it stops moving anything.
pub fn wake_on_inventory_change_obs(
    event: On<InventoryChangedEvent>,
    mut commands: Commands,
    watchers: Query<&MoverWatchers>,
    movers: Query<&ItemMover>,
) {
    let Ok(list) = watchers.get(event.entity) else { return };

    for &mover_entity in &list.0 {
        // Revalidate rather than prune: a stale entry simply fails here.
        let Ok(mover) = movers.get(mover_entity) else { continue };

        if mover.source == Some(event.entity) || mover.sink == Some(event.entity) {
            commands.entity(mover_entity).insert(MoverAwake);
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 4 – DIRTY TRACKING
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Any block write dirties its position.
///
/// Deliberately unfiltered. A device's endpoints are a fact about its
/// *neighbours*, so "a chute changed" is not enough — placing a barrel next
/// to an existing chute has to re-resolve that chute, and the barrel has no
/// idea the chute is there. Filtering by block type here is exactly the bug
/// that made the first chute sit inert.
///
/// Consumers fail fast on positions they don't care about, so the breadth
/// costs almost nothing.
pub fn mark_dirty_on_write_obs(
    event: On<VoxelWriteRequest>,
    mut dirty: ResMut<TransportDirty>,
) {
    dirty.mark(event.at);
}

/// Swap the accumulated list into this tick's snapshot. Runs first in the
/// topology set, before anything reads it.
pub fn rotate_transport_dirty_sys(mut dirty: ResMut<TransportDirty>) {
    dirty.rotate();
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 5 – PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Resolution strictly before transport. Every mover the tick system sees
/// has already been reconciled with the voxel data this tick, so it never
/// has to defend against a stale endpoint.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransportSet {
    /// The dirty list is swapped into this tick's snapshot.
    Refresh,
    /// Devices work out their endpoints. Device plugins add systems here.
    Topology,
    /// Items actually move.
    Transport,
}

/// The shared half. Device plugins (`ChutePlugin`, `ExtractorPlugin`) add
/// themselves to `TransportSet::Topology` and contribute nothing else.
pub struct ItemTransportPlugin;

impl Plugin for ItemTransportPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TransportDirty>()
            .configure_sets(
                FixedUpdate,
                (
                    TransportSet::Refresh,
                    TransportSet::Topology,
                    TransportSet::Transport,
                )
                    .chain(),
            )
            .add_systems(
                FixedUpdate,
                rotate_transport_dirty_sys.in_set(TransportSet::Refresh),
            )
            .add_systems(FixedUpdate, tick_movers_sys.in_set(TransportSet::Transport))
            .add_observer(mark_dirty_on_write_obs)
            .add_observer(wake_on_inventory_change_obs);
    }
}