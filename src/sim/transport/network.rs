use std::collections::HashMap;

use bevy::prelude::*;

use crate::space::access::VoxelWorld;
use crate::space::address::BlockPos;
use crate::voxel::{ConnectionMask, Direction};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TRANSPORT NETWORK
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//
// A network is a *connectivity relation*, not a transport medium. Transfer
// is instant and pipes hold nothing, so the only questions anyone ever asks
// a network are "who can I reach from here" and "which of those has room".
// Interior topology matters exactly once: when it changes.
//
// Nothing in this file knows what a pipe is. It is handed nodes, edges and
// ports by whoever owns the blocks — `sim::behaviors::blocks::pipe` today,
// a cable or a belt tomorrow — and it answers routing questions. That split
// is why the priority splitter will cost a candidate-ordering policy rather
// than a second copy of all of this.
//
// ## The reduced graph
//
// Storing a node per pipe cell would put a cursor on every cell, which
// means per-cell mutable state, which means persistence. Instead only cells
// where a *decision* exists become nodes:
//
//   - anything that is not a plain two-connection corridor cell, and
//   - anything with an open face (a machine, an injector, or a stub).
//
// Everything between two nodes collapses into one edge. A thousand-cell
// network reduces to a few dozen nodes, so the cursor array is a few dozen
// bytes — and because cursors only affect *fairness*, they never need
// serialising. Losing them on a rebuild costs nothing.
//
// ## Round-robin is per node
//
// Each node keeps one cursor over its combined list of edges — links and
// ports interleaved, so a junction with two arms onward and one arm into a
// barrel rotates over all three. Routing is a depth-first walk that starts
// at each node's cursor, and cursors advance *only along a path that
// succeeded*. A branch that is full therefore does not burn its turn, which
// is what "as long as that route is available" has to mean.
//
// The distribution this produces is the branch-weighted one: a split that
// splits again gives 50/25/25, exactly as per-cell item queues would. What
// queues would add on top is latency, throughput ceilings and buffering —
// none of which are distribution properties, which is why this decomposition
// survives if in-flight items ever land.

/// A node index that means "no node here". Corridor cells carry it.
pub const NO_NODE: u32 = u32::MAX;

/// Bound on corridor walking, so a malformed mask cannot spin forever.
const MAX_CORRIDOR: usize = 4096;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 1 – GRAPH PIECES
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// One choice available at a node.
///
/// Links and ports share one list on purpose: they compete for the same
/// cursor, so a barrel hanging off a junction gets its fair share against
/// the branches rather than being served before or after all of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    /// Continue to another node. The corridor between them is not stored —
    /// with instant transfer it has no observable effect.
    Link(u32),
    /// Deliver into an inventory. Index into `ports`.
    Port(u32),
}

/// A decision point. `start`/`len` slice into the network's flat edge list;
/// one `Vec` per node would be one allocation per node.
#[derive(Clone, Copy, Debug, Default)]
pub struct Node {
    pub start: u32,
    pub len: u8,
}

/// Where the network touches something that holds items.
#[derive(Clone, Copy, Debug)]
pub struct Port {
    /// The network cell this port hangs off. Kept for debugging and for
    /// re-dirtying devices after a rebuild.
    pub at: BlockPos,
    /// The face it points through.
    pub dir: Direction,
    /// The block entity on the other side.
    pub target: Entity,
}

/// One frame of the routing walk. Never escapes this module.
#[derive(Default, Debug, Clone, Copy)]
struct Frame {
    node: u32,
    /// The node's cursor when we arrived — where its rotation starts.
    start: u8,
    /// How many of its edges we have taken so far.
    tried: u8,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 2 – THE NETWORK
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// One connected component, as an entity.
///
/// Parented to its space, so a dimension or a grid that goes away takes its
/// networks with it rather than leaving orphans pumping items between
/// barrels that no longer exist — the failure mode `ChuteRun` documents at
/// length, and the reason `cells` is stored rather than rediscovered.
#[derive(Component, Debug)]
pub struct TransportNetwork {
    /// Which space this lives in. A network never spans two, because
    /// `BlockPos::neighbor` never leaves one.
    pub space: Entity,

    /// Every cell, in fill order. The rebuild's reconciliation set: a cell
    /// that has stopped being part of this network can only be found from
    /// here, never by scanning the world.
    pub cells: Vec<BlockPos>,
    /// Every chunk `cells` touches, deduplicated. Lets the plugin notice
    /// when one unloads without walking the cell list.
    pub chunks: Vec<Entity>,

    nodes: Vec<Node>,
    edges: Vec<Edge>,
    ports: Vec<Port>,
    /// One per node, indexing into that node's edge slice.
    cursors: Vec<u8>,

    // ── routing scratch ──────────────────────────────────────────────────
    //
    // Owned rather than allocated per call. Routing happens once per batch
    // per injector, which is often enough that a `Vec::new()` in there
    // would be the most expensive thing in the transport tick.
    visited: Vec<u32>,
    stamp: u32,
    stack: Vec<Frame>,
}

impl TransportNetwork {
    pub fn new(space: Entity) -> Self {
        Self {
            space,
            cells: Vec::new(),
            chunks: Vec::new(),
            nodes: Vec::new(),
            edges: Vec::new(),
            ports: Vec::new(),
            cursors: Vec::new(),
            visited: Vec::new(),
            stamp: 0,
            stack: Vec::new(),
        }
    }

    /// Replace the graph wholesale. Called by the rebuild, never
    /// incrementally — a half-updated graph is worse than a rebuilt one.
    ///
    /// Cursors are reset, which is the whole reason they are allowed to be
    /// this cheap: they carry fairness, not state, so a rebuild is free to
    /// forget them.
    pub fn install(&mut self, nodes: Vec<Node>, edges: Vec<Edge>, ports: Vec<Port>) {
        self.cursors.clear();
        self.cursors.resize(nodes.len(), 0);
        self.visited.clear();
        self.visited.resize(nodes.len(), 0);
        self.stamp = 0;
        self.nodes = nodes;
        self.edges = edges;
        self.ports = ports;
    }

    #[inline]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Every inventory this network can deliver into, in no particular
    /// order. What a device registers itself against for wake-ups.
    ///
    /// Deliberately not filtered by reachability: over-waking costs a tick,
    /// under-waking costs a factory that silently stopped an hour ago.
    pub fn acceptors(&self) -> impl Iterator<Item = Entity> + '_ {
        self.ports.iter().map(|p| p.target)
    }

    /// The cells that carry a node. After a rebuild these get re-dirtied so
    /// that adjacent devices re-resolve — a device's endpoint is a fact
    /// about its neighbour, and the neighbour just changed identity.
    pub fn node_cells(&self) -> impl Iterator<Item = BlockPos> + '_ {
        self.ports.iter().map(|p| p.at)
    }

    // ── routing ──────────────────────────────────────────────────────────

    /// Walk outward from `from` and return the first acceptor `accepts`
    /// approves of, advancing the cursors along the path that worked.
    ///
    /// `accepts` is a closure rather than an inventory query so that this
    /// file never learns what an `Inventory` is. It is called at most once
    /// per port and short-circuits on the first success.
    ///
    /// Typical cost is a handful of node hops. The worst case — a network
    /// where nothing has room — is O(nodes), paid once before the injector
    /// goes back to sleep, not once per tick.
    pub fn route(&mut self, from: u32, mut accepts: impl FnMut(Entity) -> bool) -> Option<Entity> {
        if from == NO_NODE || from as usize >= self.nodes.len() {
            return None;
        }

        // Scratch out, scratch back. Borrowing `self.stack` mutably while
        // reading `self.nodes` is two borrows of one struct; taking the
        // buffers makes them independent locals and costs a pointer swap.
        let mut stack = std::mem::take(&mut self.stack);
        let mut visited = std::mem::take(&mut self.visited);
        visited.resize(self.nodes.len(), 0);

        // Wrapping stamp, with one reset when it laps. Cheaper than
        // clearing a visited array on every route.
        self.stamp = self.stamp.wrapping_add(1);
        if self.stamp == 0 {
            visited.fill(0);
            self.stamp = 1;
        }

        stack.clear();
        visited[from as usize] = self.stamp;
        stack.push(Frame { node: from, start: self.cursors[from as usize], tried: 0 });

        let mut found: Option<Entity> = None;

        while let Some(frame) = stack.last_mut() {
            let node = self.nodes[frame.node as usize];

            if node.len == 0 || frame.tried >= node.len {
                stack.pop();
                continue;
            }

            let slot = (frame.start + frame.tried) % node.len;
            frame.tried += 1;
            let edge = self.edges[node.start as usize + slot as usize];
            // `frame` is not touched again in this iteration, so its borrow
            // ends here and the push below is legal.

            match edge {
                Edge::Port(p) => {
                    let target = self.ports[p as usize].target;
                    if accepts(target) {
                        found = Some(target);
                        break;
                    }
                }
                Edge::Link(to) => {
                    // The visited set is also what stops us walking back
                    // the way we came — the previous node is always marked.
                    if visited[to as usize] != self.stamp {
                        visited[to as usize] = self.stamp;
                        stack.push(Frame {
                            node: to,
                            start: self.cursors[to as usize],
                            tried: 0,
                        });
                    }
                }
            }
        }

        // Advance cursors only along the successful path. Every frame still
        // on the stack is sitting on the edge it descended (or delivered)
        // through, so `tried - 1` is the slot that worked.
        if found.is_some() {
            for frame in stack.iter() {
                let node = self.nodes[frame.node as usize];
                if node.len == 0 || frame.tried == 0 {
                    continue;
                }
                let used = (frame.start + frame.tried - 1) % node.len;
                self.cursors[frame.node as usize] = (used + 1) % node.len;
            }
        }

        self.stack = stack;
        self.visited = visited;
        found
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 3 – THE CELL INDEX
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// What a network cell looks like from outside.
#[derive(Clone, Copy, Debug)]
pub struct NetworkCell {
    pub network: Entity,
    /// The node this cell carries, or [`NO_NODE`] for a corridor cell.
    ///
    /// Any cell with an open face is a node, and a cell facing a device is
    /// by definition open — so a device resolving its own port always finds
    /// a real node here.
    pub node: u32,
    /// The cell's connection mask, cached so port resolution never reads a
    /// voxel.
    pub open: ConnectionMask,
}

/// Every network cell in the game, by position.
///
/// Touched on rebuilds and on device resolution, never per tick. A cell in
/// an unloaded chunk is harmless: it reads as air, so the next rebuild that
/// reaches it drops it, and until then its ports resolve to entities that
/// no longer exist and simply fail. Self-healing rather than eagerly
/// maintained, which is what lets chunk unload need no special path.
#[derive(Resource, Default)]
pub struct NetworkIndex {
    cells: HashMap<BlockPos, NetworkCell>,
}

impl NetworkIndex {
    #[inline]
    pub fn get(&self, at: BlockPos) -> Option<NetworkCell> {
        self.cells.get(&at).copied()
    }

    #[inline]
    pub fn contains(&self, at: BlockPos) -> bool {
        self.cells.contains_key(&at)
    }

    #[inline]
    pub fn insert(&mut self, at: BlockPos, cell: NetworkCell) {
        self.cells.insert(at, cell);
    }

    /// Drop an entry only if it still points at `network`. Guards the case
    /// where a rebuild has already re-homed the cell — removing it then
    /// would silently unplug a live network.
    pub fn remove_if_owned(&mut self, at: BlockPos, network: Entity) {
        if self.cells.get(&at).is_some_and(|c| c.network == network) {
            self.cells.remove(&at);
        }
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECTION 4 – CORRIDOR WALKING
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Follow a corridor of two-connection cells from `at` in direction `dir`
/// until something that is a node.
///
/// `mask_of` returns a cell's connection mask, or `None` if it is not part
/// of this network at all. `is_node` decides where the corridor ends. Both
/// are closures so this stays independent of how the caller stores its
/// component — the pipe rebuild hands it two `HashMap` lookups.
///
/// Returns the node cell reached, or `None` if the corridor runs into
/// something that is not a network cell (a broken link, an unloaded chunk).
pub fn walk_corridor(
    at: BlockPos,
    dir: Direction,
    mask_of: impl Fn(BlockPos) -> Option<ConnectionMask>,
    is_node: impl Fn(BlockPos) -> bool,
) -> Option<BlockPos> {
    let mut back = dir.opposite();
    let mut cur = at.neighbor(dir);

    for _ in 0..MAX_CORRIDOR {
        let mask = mask_of(cur)?;

        // A link is only real if both sides claim it. One-sided arms are
        // legal to author — the player just toggled one end — and they
        // render as a visible stub, which is exactly the feedback wanted.
        if !mask.has(back) {
            return None;
        }

        if is_node(cur) {
            return Some(cur);
        }

        // Two connections, no ports: exactly one way onward.
        let onward = mask.directions().find(|d| *d != back)?;
        back = onward.opposite();
        cur = cur.neighbor(onward);
    }

    warn!("transport: corridor from {at:?} exceeded {MAX_CORRIDOR} cells — truncated");
    None
}

/// Does the block on `dir` side of `at` hold a block entity we could
/// deliver into?
///
/// Kept here so the pipe rebuild and any future network builder agree on
/// what "touches something" means.
#[inline]
pub fn neighbor_entity(voxels: &VoxelWorld, at: BlockPos, dir: Direction) -> Option<Entity> {
    voxels.block_entity_at(at.neighbor(dir))
}