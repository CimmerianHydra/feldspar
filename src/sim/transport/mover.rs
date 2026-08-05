use bevy::prelude::*;

use crate::content::item::ItemRegistry;
use crate::prelude::VoxelWriteRequest;
use crate::sim::inventory::storage::{transfer_automated, Inventory, InventoryChangedEvent};
use crate::sim::transport::network::{NetworkIndex, TransportNetwork, NO_NODE};
use crate::space::access::VoxelWorld;
use crate::space::address::BlockPos;
use crate::voxel::Direction;

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
// *which* two endpoints are its own. Once it has, it calls `set_endpoints`
// and stops caring.
//
// ## What changed when pipes arrived
//
// An endpoint used to be `Option<Entity>`, which quietly assumed every
// destination was one inventory. A pipe network is a destination too, and
// so — later — is a priority splitter's ordered candidate list. So the
// endpoint became [`ItemPort`], and the tick system grew exactly one new
// step: resolve the sink to a concrete entity *for this batch*, then run
// the same transfer it always ran.
//
// That is the whole generalisation. "Push into an inventory or a network"
// is now one line at every call site, chutes and extractors included, and
// nothing here knows what a pipe is — `TransportNetwork` is a routing
// graph, and this file only asks it for an entity.

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
// SECTION 1 – PORTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Where one end of a mover leads.
///
/// Deliberately not a trait. There are exactly two kinds of destination in
/// the game and there will be three; an enum keeps the tick system's hot
/// path a match with no indirection, and adding the third is one arm here
/// plus one arm in [`resolve_delivery`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ItemPort {
    /// Unresolved, or pointing at something that holds no items. The
    /// transfer no-ops either way, which is cheaper than validating here
    /// and cannot go stale between checks.
    #[default]
    None,
    /// One inventory. A barrel, a machine, another device's buffer.
    Inventory(Entity),
    /// A routing graph, entered at `node`.
    ///
    /// The node is where the *device* touches the network, not where the
    /// items end up. Which acceptor they end up in is decided per batch, by
    /// the network's own round-robin.
    Network { network: Entity, node: u32 },
}

impl ItemPort {
    /// The entity this port names, whatever kind it is. Used for watcher
    /// bookkeeping and for the aliasing check.
    #[inline]
    pub fn entity(self) -> Option<Entity> {
        match self {
            ItemPort::None => None,
            ItemPort::Inventory(e) => Some(e),
            ItemPort::Network { network, .. } => Some(network),
        }
    }

    #[inline]
    pub fn is_none(self) -> bool {
        matches!(self, ItemPort::None)
    }
}

/// What lies on the `dir` face of `at`.
///
/// The one function every device calls to work out an endpoint, and the
/// reason a chute gained pipe output without a line of chute-specific code.
/// Order matters: a network cell wins over a plain block entity, because a
/// pipe that has a block entity for some other reason is still a pipe.
///
/// A network cell whose arm does *not* point back at us is a wall, not an
/// endpoint. That is what makes a one-sided connection — the player toggled
/// one end and not the other — behave the way it looks.
pub fn resolve_port(
    voxels: &VoxelWorld,
    index: &NetworkIndex,
    at: BlockPos,
    dir: Direction,
) -> ItemPort {
    let neighbor = at.neighbor(dir);

    if let Some(cell) = index.get(neighbor) {
        return if cell.node != NO_NODE && cell.open.has(dir.opposite()) {
            ItemPort::Network { network: cell.network, node: cell.node }
        } else {
            ItemPort::None
        };
    }

    match voxels.block_entity_at(neighbor) {
        Some(entity) => ItemPort::Inventory(entity),
        None => ItemPort::None,
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – COMPONENTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Moves items from one endpoint to another on a schedule.
///
/// Sits alongside whatever component describes the device's *shape* — a
/// `ChuteRun` for a column, an `ItemExtractor` for a single block. The tick
/// system below queries only this, so adding a new kind of mover costs a
/// resolution system and nothing else.
#[derive(Component, Debug)]
pub struct ItemMover {
    /// Where items come from. Only `Inventory` is meaningful today; a
    /// network that *pulls* would be the priority merger, and it lands as
    /// an arm in `resolve_extraction` rather than a new component.
    pub source: ItemPort,
    /// Where items go.
    pub sink: ItemPort,

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

    /// Every entity this mover asked to be woken by.
    ///
    /// Needed because a network sink means the wake-up arrives from an
    /// acceptor the mover has never heard of — the old check ("is this one
    /// of my two endpoints?") would reject it and the device would sleep
    /// through the one event that mattered. Rewritten by `set_endpoints`,
    /// so it cannot drift from what was actually registered.
    pub watching: Vec<Entity>,
}

impl ItemMover {
    pub fn new(batch: u16, batches_per_second: u32) -> Self {
        Self {
            source: ItemPort::None,
            sink: ItemPort::None,
            batch,
            credit_per_tick: credit_per_tick(batches_per_second),
            credit: 0,
            cursor: 0,
            watching: Vec::new(),
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
/// **Every endpoint, not just the source.** A mover whose destination is
/// full moves nothing, concludes it has no work, and sleeps — and if only
/// the source could wake it, draining the destination would leave it asleep
/// forever. A chute that silently stopped an hour ago is the worst bug this
/// system can have, and this list is the whole fix.
///
/// For a network sink the list holds the mover once per *acceptor*, which
/// is O(acceptors) per injector rather than O(acceptors x injectors) — the
/// price of not needing a second wake-up mechanism.
///
/// Deliberately tolerant of staleness: entries are added, never pruned, and
/// the observer revalidates each against the mover's own `watching` list. A
/// few dead `Entity`s cost less than removal bookkeeping.
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

    /// Add to the snapshot **this** tick is already reading.
    ///
    /// Only legal from a system that runs before every consumer, which in
    /// practice means [`TransportSet::Network`]. A rebuilt network needs
    /// the devices around it to re-resolve in the same tick, because their
    /// cached network entity may have just been recycled; deferring that to
    /// `mark` would leave them pointing at a stale graph for a frame.
    pub fn mark_now(&mut self, at: BlockPos) {
        self.current.push(at);
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
// SECTION 3 – ENDPOINT PLUMBING
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Point a mover at its endpoints, register it for wake-ups everywhere that
/// matters, and wake it.
///
/// The one function every device calls at the end of its own resolution
/// logic. Keeping the watcher registration in here rather than at each call
/// site is what makes it structurally impossible to add a device that
/// resolves endpoints correctly but never wakes up.
///
/// `networks` is read-only and used for exactly one thing: expanding a
/// network sink into the acceptors that should be able to wake this mover.
pub fn set_endpoints(
    commands: &mut Commands,
    mover_entity: Entity,
    mover: &mut ItemMover,
    source: ItemPort,
    sink: ItemPort,
    networks: &Query<&TransportNetwork>,
) {
    mover.source = source;
    mover.sink = sink;

    mover.watching.clear();
    for port in [source, sink] {
        match port {
            ItemPort::None => {}
            ItemPort::Inventory(entity) => mover.watching.push(entity),
            ItemPort::Network { network, .. } => {
                if let Ok(net) = networks.get(network) {
                    mover.watching.extend(net.acceptors());
                }
            }
        }
    }

    // A network with many acceptors can list the same barrel twice if two
    // arms reach it. Harmless for waking, wasteful for the watcher lists.
    mover.watching.sort_unstable();
    mover.watching.dedup();

    for endpoint in &mover.watching {
        watch(commands, *endpoint, mover_entity);
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
// SECTION 4 – TRANSPORT
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
    mut networks: Query<&mut TransportNetwork>,
    registry: Res<ItemRegistry>,
) {
    for (entity, mut mover) in movers.iter_mut() {
        mover.credit = (mover.credit + mover.credit_per_tick).min(CREDIT_PER_BATCH);

        if mover.credit < CREDIT_PER_BATCH {
            continue;
        }

        // Only inventories can be drawn from today. A network source is the
        // priority merger, and it lands as a second arm here.
        let ItemPort::Inventory(source) = mover.source else {
            commands.entity(entity).remove::<MoverAwake>();
            continue;
        };

        // Which single entity is this batch going to? For an inventory sink
        // that is the sink; for a network it is whatever the graph's
        // round-robin picks, given what the source is actually holding.
        let sink = match mover.sink {
            ItemPort::None => None,
            ItemPort::Inventory(entity) => Some(entity),
            ItemPort::Network { network, node } => {
                resolve_delivery(network, node, source, mover.cursor, &mut networks, &inventories, &registry)
            }
        };

        // No destination this tick. For an inventory sink only a topology
        // change can fix that, and resolution re-inserts `MoverAwake` when
        // it does; for a network sink only an `InventoryChangedEvent` on an
        // acceptor can, and `watching` covers every one of them.
        let Some(sink) = sink else {
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
            commands.entity(entity).remove::<MoverAwake>();
        }
    }
}

/// Ask a network where this batch should go.
///
/// The acceptance test is "would this inventory take *anything* the source
/// is offering", phrased exactly the way `transfer_automated` will phrase it
/// a moment later. Agreeing by construction matters: a route chosen against
/// a different question would occasionally pick a destination the transfer
/// then refuses, and the mover would sleep with work still to do.
fn resolve_delivery(
    network: Entity,
    node: u32,
    source: Entity,
    cursor: usize,
    networks: &mut Query<&mut TransportNetwork>,
    inventories: &Query<&mut Inventory>,
    registry: &ItemRegistry,
) -> Option<Entity> {
    let mut net = networks.get_mut(network).ok()?;
    let from = inventories.get(source).ok()?;

    // Cheap rejection before touching the graph at all: an empty source has
    // nothing to route.
    if !from.has_extractable() {
        return None;
    }

    net.route(node, |target| {
        // Never deliver into the very inventory we are drawing from. A pipe
        // loop back to its own source would otherwise cycle forever, doing
        // nothing but burning credit.
        if target == source {
            return false;
        }
        let Ok(to) = inventories.get(target) else { return false };

        from.automation_offers(cursor)
            .any(|offer| to.automation_insertable(offer.item, registry).is_some())
    })
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

        if mover.watching.contains(&event.entity) {
            commands.entity(mover_entity).insert(MoverAwake);
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 5 – DIRTY TRACKING
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
pub fn mark_dirty_on_write_obs(event: On<VoxelWriteRequest>, mut dirty: ResMut<TransportDirty>) {
    dirty.mark(event.at);
}

/// Swap the accumulated list into this tick's snapshot. Runs first in the
/// topology set, before anything reads it.
pub fn rotate_transport_dirty_sys(mut dirty: ResMut<TransportDirty>) {
    dirty.rotate();
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 6 – PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Networks before devices before transport.
///
/// The middle boundary is the new one and it is load-bearing. A rebuild can
/// recycle a network entity, so any device holding an `ItemPort::Network`
/// must re-resolve *after* the rebuild and *before* anything routes through
/// it. Networks re-dirty their own node cells to make that happen, which is
/// why `Network` must also run after `Refresh`.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransportSet {
    /// The dirty list is swapped into this tick's snapshot.
    Refresh,
    /// Routing graphs are rebuilt. Network plugins add systems here.
    Network,
    /// Devices work out their endpoints. Device plugins add systems here.
    Topology,
    /// Items actually move.
    Transport,
}

/// The shared half. Device plugins (`ChutePlugin`, `ExtractorPlugin`) and
/// network plugins (`ItemPipePlugin`) add themselves to the sets above and
/// contribute nothing else.
pub struct ItemTransportPlugin;

impl Plugin for ItemTransportPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TransportDirty>()
            .init_resource::<NetworkIndex>()
            .configure_sets(
                FixedUpdate,
                (
                    TransportSet::Refresh,
                    TransportSet::Network,
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