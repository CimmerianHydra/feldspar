use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use serde::Deserialize;

use crate::content::block::behaviors::{BlockBehavior, BlockBehaviors};
use crate::content::block::BlockRegistry;
use crate::sim::behaviors::blocks::face_targeted::FaceTargeted;
use crate::sim::behaviors::blocks::interacts::InteractsOnSecondary;
use crate::sim::inventory::storage::Inventory;
use crate::sim::transport::mover::{TransportDirty, TransportSet};
use crate::sim::transport::network::{
    walk_corridor, Edge, NetworkCell, NetworkIndex, Node, Port, TransportNetwork, NO_NODE,
};
use crate::space::access::VoxelWorld;
use crate::space::address::BlockPos;
use crate::space::chunk::{ChunkMap, ChunkSlot, VoxelChunk};
use crate::space::write::{BlockEvent, VoxelWriteRequest};
use crate::voxel::{
    to_space_pos, BlockID, ConnectionMask, Direction, Voxel, ALL_DIRECTIONS,
};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PIPE
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//
// A pipe is thirty-two bits and nothing else. No entity, no components, no
// per-tick work — that one commitment is where every performance property
// in this file comes from, and everything below is arranged to preserve it.
//
//   the voxel        six state bits say which faces have arms. Authored,
//                    seeded from neighbours at placement, and the sole
//                    input to both the model table and the graph.
//   the network      one entity per connected component, holding a reduced
//                    graph. Rebuilt on writes, never ticked.
//   the injector     an extractor or a chute. The only thing that ticks,
//                    and it sleeps.
//
// ## What the rebuild has to get right
//
// The same thing `chute.rs` spends forty lines on: reconcile by identity,
// never by rediscovery. A network whose cells have all been broken cannot
// be found by scanning the world, so it has to be reachable from the *old*
// index — which is why `TransportNetwork::cells` exists and why the unit of
// work is the network, not the cell.
//
// ## Where directionality went
//
// Faces are connected or not; there is no per-face in/out. Four states per
// face is 4^6 = 4096 configurations and needs twelve state bits, and the
// voxel has eleven. Rather than reinterpret the rotation field to buy the
// twelfth, direction became its own block — a one-way pipe, which is a wall
// in this flood fill plus a directed arc between the components it
// separates. It is not implemented yet, and the seam it needs is marked.

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – BLOCK BEHAVIOR
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Declares that this block is a routable segment.
///
/// Note the absence of `on_place`. Touching the spawn context is what
/// promotes a voxel to an entity, and a pipe must never be promoted — a
/// warehouse floor of ten thousand pipes would otherwise be ten thousand
/// entities and ten thousand `ChunkBlockEntities` map entries, before a
/// single item had moved.
///
/// There is also no tuning here. A pipe has no rate: it does not move
/// items, injectors do. If pipe tiers ever matter they will constrain
/// throughput, which is a property of the *network*, not of a cell.
#[derive(Clone, Copy, Default, Debug, Deserialize)]
pub struct ItemPipeBehavior;

impl BlockBehavior for ItemPipeBehavior {
    const NAME: &'static str = "item_pipe";

    /// Two implications, for the same reason `Orientable` implies
    /// `FaceTargeted`: resolving them here means no consumer has to
    /// remember an `||`.
    ///
    /// - `FaceTargeted` — a corner click addresses the face on the far
    ///   side, which is how you reach the connection you cannot see.
    /// - `InteractsOnSecondary` — a right-click has to reach this block
    ///   even though it has no entity to route through.
    fn apply(self, into: &mut BlockBehaviors) {
        into.push(self);
        into.push(FaceTargeted);
        into.push(InteractsOnSecondary);
    }

    fn build(app: &mut App) {
        app.add_plugins(ItemPipePlugin);
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – READING THE WORLD
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Is this voxel a pipe?
///
/// Asked through the behaviour bag rather than a block id, so a steel pipe,
/// a bronze pipe and a modded one are all the same thing to the rebuild and
/// join the same networks.
#[inline]
fn is_pipe(registry: &BlockRegistry, voxel: Voxel) -> bool {
    !voxel.is_air() && registry.get(BlockID(voxel.id())).behaviors.has::<ItemPipeBehavior>()
}

#[inline]
fn pipe_mask_at(
    voxels: &VoxelWorld,
    registry: &BlockRegistry,
    at: BlockPos,
) -> Option<ConnectionMask> {
    let voxel = voxels.get_voxel(at);
    is_pipe(registry, voxel).then(|| ConnectionMask::from_state(voxel.state()))
}

/// A link exists only when both sides claim it.
///
/// One-sided arms are legal to author — the player toggled one end and not
/// the other — and they render as a visible stub, which is exactly the
/// feedback wanted. What they must not do is carry items.
#[inline]
fn linked(
    voxels: &VoxelWorld,
    registry: &BlockRegistry,
    at: BlockPos,
    mask: ConnectionMask,
    dir: Direction,
) -> bool {
    mask.has(dir)
        && pipe_mask_at(voxels, registry, at.neighbor(dir))
            .is_some_and(|other| other.has(dir.opposite()))
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – AUTHORING CONNECTIONS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A freshly placed pipe reaches for whatever is already around it.
///
/// This bends the "authored, never derived" rule on purpose, and only once:
/// a pipe that placed as a lone cube and needed six clicks to become a pipe
/// would be miserable to build with. Seeding happens at placement time and
/// writes real voxels, so geometry stays a pure function of the voxel and
/// there is still no live dependency on neighbours.
///
/// Hangs off `BlockEvent::Place`, which fires for a write and not for chunk
/// hydration — a loaded chunk must keep the connections it was saved with,
/// including the ones the player deliberately turned off.
fn seed_connections_on_place_obs(
    event: On<BlockEvent>,
    mut commands: Commands,
    registry: Res<BlockRegistry>,
    voxels: VoxelWorld,
    inventories: Query<(), With<Inventory>>,
) {
    let BlockEvent::Place { block_id, at } = *event else { return };
    if !registry.get(block_id).behaviors.has::<ItemPipeBehavior>() {
        return;
    }

    let voxel = voxels.get_voxel(at);
    let mut mask = ConnectionMask::from_state(voxel.state());

    for dir in ALL_DIRECTIONS {
        let neighbor = at.neighbor(dir);

        // Another pipe: connect, and reach back so the link is mutual from
        // the first tick rather than the second.
        if let Some(other) = pipe_mask_at(&voxels, &registry, neighbor) {
            mask = mask.with(dir, true);
            if !other.has(dir.opposite()) {
                let nv = voxels.get_voxel(neighbor);
                commands.trigger(VoxelWriteRequest {
                    at: neighbor,
                    voxel: nv.with_state(other.with(dir.opposite(), true).into_state(nv.state())),
                });
            }
            continue;
        }

        // Something that holds items. Not *any* block entity: a furnace's
        // back wall is a block entity too, and an arm sprouting into it
        // would be a promise the network cannot keep.
        if voxels
            .block_entity_at(neighbor)
            .is_some_and(|e| inventories.contains(e))
        {
            mask = mask.with(dir, true);
        }
    }

    if mask != ConnectionMask::from_state(voxel.state()) {
        commands.trigger(VoxelWriteRequest {
            at,
            voxel: voxel.with_state(mask.into_state(voxel.state())),
        });
    }
}

/// Breaking a pipe retracts the arms that were reaching for it.
///
/// Cosmetic in one direction and load-bearing in the other: a stub pointing
/// at air is ugly, but a stub pointing at air that the player then builds a
/// barrel behind would silently connect to it.
fn prune_connections_on_break_obs(
    event: On<BlockEvent>,
    mut commands: Commands,
    registry: Res<BlockRegistry>,
    voxels: VoxelWorld,
) {
    let BlockEvent::Break { block_id, at } = *event else { return };
    if !registry.get(block_id).behaviors.has::<ItemPipeBehavior>() {
        return;
    }

    for dir in ALL_DIRECTIONS {
        let neighbor = at.neighbor(dir);
        let Some(mask) = pipe_mask_at(&voxels, &registry, neighbor) else { continue };
        if !mask.has(dir.opposite()) {
            continue;
        }

        let nv = voxels.get_voxel(neighbor);
        commands.trigger(VoxelWriteRequest {
            at: neighbor,
            voxel: nv.with_state(mask.with(dir.opposite(), false).into_state(nv.state())),
        });
    }
}

/// Right-click toggles one connection, on both sides at once.
///
/// The face is `target_face`, so the 3x3 grid the highlight already draws
/// lets a corner click address the face pointing away from you. Writing
/// both voxels is what turns "set A's east, then walk around and set B's
/// west" into one click — the data stays per-side, the interaction does not.
fn toggle_connection_obs(
    event: On<BlockEvent>,
    mut commands: Commands,
    registry: Res<BlockRegistry>,
    voxels: VoxelWorld,
) {
    let BlockEvent::Interact { block_id, at, target_face, .. } = *event else { return };
    if !registry.get(block_id).behaviors.has::<ItemPipeBehavior>() {
        return;
    }

    let voxel = voxels.get_voxel(at);
    let mask = ConnectionMask::from_state(voxel.state());
    let on = !mask.has(target_face);

    commands.trigger(VoxelWriteRequest {
        at,
        voxel: voxel.with_state(mask.with(target_face, on).into_state(voxel.state())),
    });

    let neighbor = at.neighbor(target_face);
    if let Some(other) = pipe_mask_at(&voxels, &registry, neighbor) {
        let nv = voxels.get_voxel(neighbor);
        commands.trigger(VoxelWriteRequest {
            at: neighbor,
            voxel: nv.with_state(other.with(target_face.opposite(), on).into_state(nv.state())),
        });
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 4 – THE REBUILD
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// One connected component, mid-fill.
struct Component {
    cells: Vec<BlockPos>,
    masks: HashMap<BlockPos, ConnectionMask>,
}

/// Re-derive every network the dirty list could have disturbed.
///
/// Three phases, in an order that matters:
///
/// 1. **Retire.** Every network claiming a dirty cell or one of its
///    neighbours is pulled out of the index, and its old cell list becomes
///    fill seeds. This is the half that catches *splits* — cells that are
///    about to stop belonging can only be found from the network that used
///    to own them.
/// 2. **Fill.** Flood outward from the seeds over mutual links. A fill that
///    walks into a network nobody retired has just *merged* with it, so
///    that network is retired mid-flight and its cells re-homed.
/// 3. **Install.** Build the reduced graph, reuse a retired entity where
///    one is going spare, despawn the rest.
///
/// Cost is proportional to the cells of the affected networks, paid on
/// writes only. Placing one pipe in the middle of a ten-thousand-cell
/// network does re-walk all ten thousand; the incremental version (adopt
/// into a single neighbouring network in O(1), merge smaller into larger,
/// full fill only on removal) is a strict optimisation of this, and worth
/// doing when a profile says so rather than before.
#[allow(clippy::too_many_arguments)]
fn rebuild_networks_sys(
    mut commands: Commands,
    mut dirty: ResMut<TransportDirty>,
    mut index: ResMut<NetworkIndex>,
    mut networks: Query<&mut TransportNetwork>,
    registry: Res<BlockRegistry>,
    voxels: VoxelWorld,
    inventories: Query<(), With<Inventory>>,
) {
    // ── Phase 1 — retire ─────────────────────────────────────────────────
    let mut seeds: Vec<BlockPos> = Vec::new();
    let mut retired: Vec<Entity> = Vec::new();
    let mut retired_set: HashSet<Entity> = HashSet::new();
    let mut old_cells: Vec<BlockPos> = Vec::new();

    for &at in dirty.current() {
        for pos in std::iter::once(at).chain(ALL_DIRECTIONS.map(|d| at.neighbor(d))) {
            if pipe_mask_at(&voxels, &registry, pos).is_some() {
                seeds.push(pos);
            }

            let Some(cell) = index.get(pos) else { continue };
            if !retired_set.insert(cell.network) {
                continue;
            }
            retired.push(cell.network);
            if let Ok(mut net) = networks.get_mut(cell.network) {
                old_cells.append(&mut net.cells);
            }
        }
    }

    if seeds.is_empty() && retired.is_empty() {
        return;
    }

    // Old cells are seeds too — that is the split case — but only the ones
    // that are still pipes. The rest simply vanish, which is the delete
    // case handling itself.
    for pos in &old_cells {
        if pipe_mask_at(&voxels, &registry, *pos).is_some() {
            seeds.push(*pos);
        }
    }

    // ── Phase 2 & 3 — fill and install ───────────────────────────────────
    let mut claimed: HashSet<BlockPos> = HashSet::new();
    let mut pool: Vec<Entity> = retired.clone();
    let mut fresh: Vec<Entity> = Vec::new();

    let mut i = 0;
    while i < seeds.len() {
        let seed = seeds[i];
        i += 1;

        if claimed.contains(&seed) || pipe_mask_at(&voxels, &registry, seed).is_none() {
            continue;
        }

        let component = fill(
            seed,
            &voxels,
            &registry,
            &index,
            &mut claimed,
            &mut networks,
            &mut retired,
            &mut retired_set,
            &mut pool,
            &mut seeds,
        );

        let entity = pool.pop().unwrap_or_else(|| commands.spawn_empty().id());
        install(
            &mut commands,
            &mut index,
            &mut dirty,
            &voxels,
            &inventories,
            entity,
            component,
        );
        fresh.push(entity);
    }

    // ── Cleanup ──────────────────────────────────────────────────────────
    //
    // Whatever the fill did not need. Purge index entries still pointing at
    // them first: a cell that moved to a live network already overwrote its
    // entry, and `remove_if_owned` is what keeps this from unplugging it.
    for entity in pool {
        for pos in &old_cells {
            index.remove_if_owned(*pos, entity);
        }
        commands.entity(entity).despawn();
    }

    // Any old cell nobody claimed and nobody owns is gone for good.
    for pos in &old_cells {
        if !claimed.contains(pos) {
            for entity in &retired {
                index.remove_if_owned(*pos, *entity);
            }
        }
    }

    debug!(
        "pipe: rebuilt {} network(s), {} cell(s) indexed",
        fresh.len(),
        index.len()
    );
}

/// Flood outward from `seed` over mutual links.
///
/// Absorbs any network it walks into, which is exactly the merge case: two
/// networks joined by a new pipe are one component now, and the fill finds
/// all of both because they are connected.
#[allow(clippy::too_many_arguments)]
fn fill(
    seed: BlockPos,
    voxels: &VoxelWorld,
    registry: &BlockRegistry,
    index: &NetworkIndex,
    claimed: &mut HashSet<BlockPos>,
    networks: &mut Query<&mut TransportNetwork>,
    retired: &mut Vec<Entity>,
    retired_set: &mut HashSet<Entity>,
    pool: &mut Vec<Entity>,
    seeds: &mut Vec<BlockPos>,
) -> Component {
    let mut cells = Vec::new();
    let mut masks = HashMap::new();
    let mut queue = vec![seed];
    claimed.insert(seed);

    while let Some(at) = queue.pop() {
        let Some(mask) = pipe_mask_at(voxels, registry, at) else { continue };
        masks.insert(at, mask);
        cells.push(at);

        // Merged into someone we hadn't retired. Take their entity for the
        // pool and let their cells be re-homed by this same fill.
        if let Some(cell) = index.get(at) {
            if retired_set.insert(cell.network) {
                retired.push(cell.network);
                pool.push(cell.network);
                if let Ok(mut net) = networks.get_mut(cell.network) {
                    seeds.append(&mut net.cells);
                }
            }
        }

        for dir in mask.directions() {
            let next = at.neighbor(dir);
            if claimed.contains(&next) || !linked(voxels, registry, at, mask, dir) {
                continue;
            }
            claimed.insert(next);
            queue.push(next);
        }
    }

    Component { cells, masks }
}

/// Collapse a component into a reduced graph and hand it to an entity.
///
/// A cell becomes a node when a decision exists there:
///
///   - it is not a plain two-link corridor cell, or
///   - it has an *open* face — a face with an arm that is not a mutual pipe
///     link, meaning a machine, an injector, or a stub.
///
/// The second clause is what guarantees a device always finds a real node
/// on the pipe it faces: the pipe's arm pointing at the device is open by
/// definition.
fn install(
    commands: &mut Commands,
    index: &mut NetworkIndex,
    dirty: &mut TransportDirty,
    voxels: &VoxelWorld,
    inventories: &Query<(), With<Inventory>>,
    entity: Entity,
    component: Component,
) {
    let Component { cells, masks } = component;
    let space = cells[0].space;

    // ── which cells are nodes ────────────────────────────────────────────
    let link_dirs = |at: BlockPos, mask: ConnectionMask| -> Vec<Direction> {
        mask.directions()
            .filter(|d| {
                masks
                    .get(&at.neighbor(*d))
                    .is_some_and(|other| other.has(d.opposite()))
            })
            .collect()
    };

    let mut node_of: HashMap<BlockPos, u32> = HashMap::new();
    let mut node_cells: Vec<BlockPos> = Vec::new();

    for &at in &cells {
        let mask = masks[&at];
        let links = link_dirs(at, mask);
        let has_open_face = links.len() < mask.count() as usize;

        if links.len() != 2 || has_open_face {
            node_of.insert(at, node_cells.len() as u32);
            node_cells.push(at);
        }
    }

    // ── edges and ports ──────────────────────────────────────────────────
    let mut nodes: Vec<Node> = Vec::with_capacity(node_cells.len());
    let mut edges: Vec<Edge> = Vec::new();
    let mut ports: Vec<Port> = Vec::new();

    for &at in &node_cells {
        let mask = masks[&at];
        let start = edges.len() as u32;

        // Canonical direction order, so a node's rotation is spatially
        // stable and a rebuild does not reshuffle where items go.
        for dir in ALL_DIRECTIONS {
            if !mask.has(dir) {
                continue;
            }

            let neighbor = at.neighbor(dir);

            // Mutual pipe link: collapse the corridor beyond it.
            if masks.get(&neighbor).is_some_and(|m| m.has(dir.opposite())) {
                let far = walk_corridor(
                    at,
                    dir,
                    |pos| masks.get(&pos).copied(),
                    |pos| node_of.contains_key(&pos),
                );
                if let Some(far) = far.and_then(|pos| node_of.get(&pos)) {
                    edges.push(Edge::Link(*far));
                }
                continue;
            }

            // Open face. A port only if something on the other side holds
            // items — an injector's arm is open too, and it is what makes
            // this cell a node, but there is nothing to deliver into it.
            //
            // This is also where a one-way pipe will be recognised: it is
            // neither a network cell nor an inventory, and it contributes a
            // directed arc to the component on its far side.
            if let Some(target) = voxels.block_entity_at(neighbor) {
                if inventories.contains(target) {
                    edges.push(Edge::Port(ports.len() as u32));
                    ports.push(Port { at, dir, target });
                }
            }
        }

        let len = (edges.len() as u32 - start).min(u8::MAX as u32) as u8;
        nodes.push(Node { start, len });
    }

    // ── index every cell ─────────────────────────────────────────────────
    let mut chunks: Vec<Entity> = Vec::new();
    for &at in &cells {
        index.insert(
            at,
            NetworkCell {
                network: entity,
                node: node_of.get(&at).copied().unwrap_or(NO_NODE),
                open: masks[&at],
            },
        );
        if let Some(address) = voxels.resolve(at) {
            if !chunks.contains(&address.chunk) {
                chunks.push(address.chunk);
            }
        }
    }

    // Devices cache the network entity and the node index, and this rebuild
    // may have just recycled both. Re-dirtying every node cell — a handful,
    // not the whole component — makes each adjacent device re-resolve in
    // `TransportSet::Topology`, which runs after this.
    for &at in &node_cells {
        dirty.mark_now(at);
    }

    let mut network = TransportNetwork::new(space);
    network.cells = cells;
    network.chunks = chunks;
    network.install(nodes, edges, ports);

    commands
        .entity(entity)
        .insert((Name::new("PipeNetwork"), network));
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 5 – WORLD LIFECYCLE
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A chunk that just arrived may be full of pipes nobody has ever seen.
///
/// Chunk hydration does not go through `VoxelWriteRequest`, so without this
/// a saved factory would load looking correct and moving nothing — the
/// quietest possible failure. One scan per chunk, at load, over voxels that
/// are already in cache.
fn seed_loaded_chunks_sys(
    chunks: Query<(&ChunkSlot, &VoxelChunk), Added<VoxelChunk>>,
    registry: Res<BlockRegistry>,
    mut dirty: ResMut<TransportDirty>,
) {
    for (slot, chunk) in chunks.iter() {
        for (local, voxel) in chunk.iter_non_air() {
            if is_pipe(&registry, voxel) {
                dirty.mark(BlockPos::new(slot.space, to_space_pos(slot.coord, local)));
            }
        }
    }
}

/// Retire networks whose world went away underneath them.
///
/// Two cases, both cheap because there are few networks:
///
/// - **The space is gone.** A dimension unloaded or a grid was destroyed.
///   Despawn, and purge the index.
/// - **A chunk is gone.** The network is now partly imaginary. Marking one
///   of its cells dirty is enough to retire the whole thing next tick,
///   because the rebuild reaches it through the index.
///
/// Unloading needs no eager path beyond this. An unloaded cell reads as
/// air, so a stale network's ports resolve to entities that no longer exist
/// and simply fail; the rebuild that eventually reaches it prunes it. The
/// only thing that would actually leak is the index entry, and that is what
/// the second case is for.
fn prune_networks_sys(
    mut commands: Commands,
    mut index: ResMut<NetworkIndex>,
    mut dirty: ResMut<TransportDirty>,
    networks: Query<(Entity, &TransportNetwork)>,
    spaces: Query<(), With<ChunkMap>>,
    chunks: Query<(), With<VoxelChunk>>,
) {
    for (entity, network) in networks.iter() {
        if !spaces.contains(network.space) {
            for cell in &network.cells {
                index.remove_if_owned(*cell, entity);
            }
            commands.entity(entity).despawn();
            continue;
        }

        if network.chunks.iter().any(|c| !chunks.contains(*c)) {
            if let Some(first) = network.cells.first() {
                dirty.mark(*first);
            }
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 6 – PLUGIN
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct ItemPipePlugin;

impl Plugin for ItemPipePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (seed_loaded_chunks_sys, prune_networks_sys).in_set(TransportSet::Refresh),
        )
        .add_systems(
            FixedUpdate,
            rebuild_networks_sys.in_set(TransportSet::Network),
        )
        .add_observer(seed_connections_on_place_obs)
        .add_observer(prune_connections_on_break_obs)
        .add_observer(toggle_connection_obs);
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TESTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;

    /// Corridor collapsing is the property the reduced graph rests on, and
    /// it is pure map arithmetic, so it can be tested without a world.
    #[test]
    fn a_straight_run_collapses_to_its_endpoints() {
        let space = Entity::PLACEHOLDER;
        let straight = ConnectionMask::NONE
            .with(Direction::East, true)
            .with(Direction::West, true);

        // A west stub, four corridor cells, an east stub.
        let mut masks = HashMap::new();
        for x in 0..6 {
            let at = BlockPos::new(space, IVec3::new(x, 0, 0));
            masks.insert(at, straight);
        }
        // The ends only connect inward, which makes them non-corridor.
        masks.insert(
            BlockPos::new(space, IVec3::new(0, 0, 0)),
            ConnectionMask::NONE.with(Direction::East, true).with(Direction::Up, true),
        );
        masks.insert(
            BlockPos::new(space, IVec3::new(5, 0, 0)),
            ConnectionMask::NONE.with(Direction::West, true).with(Direction::Up, true),
        );

        let is_node = |pos: BlockPos| {
            pos.pos.x == 0 || pos.pos.x == 5
        };

        let far = walk_corridor(
            BlockPos::new(space, IVec3::ZERO),
            Direction::East,
            |pos| masks.get(&pos).copied(),
            is_node,
        );

        assert_eq!(far, Some(BlockPos::new(space, IVec3::new(5, 0, 0))));
    }

    /// A one-sided arm is a stub, not a link. If this ever passes the other
    /// way, toggling one end of a connection would leave it live while
    /// looking broken.
    #[test]
    fn a_one_sided_arm_does_not_carry() {
        let space = Entity::PLACEHOLDER;
        let a = BlockPos::new(space, IVec3::ZERO);
        let b = BlockPos::new(space, IVec3::new(1, 0, 0));

        let mut masks = HashMap::new();
        masks.insert(a, ConnectionMask::NONE.with(Direction::East, true));
        masks.insert(b, ConnectionMask::NONE.with(Direction::East, true));

        let far = walk_corridor(
            a,
            Direction::East,
            |pos| masks.get(&pos).copied(),
            |_| true,
        );

        assert_eq!(far, None);
    }
}