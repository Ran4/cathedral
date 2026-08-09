//! The walkable surface and the street graph — L1, "the Body".
//!
//! `assets/world/navigation.json` and its companion `navigation.bin` are baked
//! from the authoritative cadastral plan by `scripts/bake_navigation.py` and
//! read by the host, which hands them here as `&str` / `&[u8]` exactly like
//! `areas.json`. This module has no filesystem, clock or Bevy dependency: where
//! an NPC can walk, and the route between two places, are authoritative
//! simulation facts and must be identical in the game, in `cathedral-headless`
//! and in `cargo test -p cathedral-sim`.
//!
//! Two artifacts, one surface:
//!
//! * the **bitset** — a 0.25 m grid over the plan's bounding box, eroded by the
//!   0.35 m agent radius and reduced to its single connected component. One
//!   array index answers *is this point walkable*.
//! * the **graph** — a few thousand nodes welded from the 49 road centrelines,
//!   split at their crossings and re-routed around anything they clip, with the
//!   69 named places, 23 sites and ~2,560 building doors hung off it as leaves.
//!   A* over it is a few microseconds.
//!
//! Design: `features/implemented/movement/02_navigation.md`.

use std::{
    cell::RefCell,
    collections::{BinaryHeap, HashMap},
};

use serde::Deserialize;

use crate::math::Vec3;

/// The height every NPC walks at: `PLAYER_SPAWN.y` in `controller.rs`. Baked
/// navigation is 2D on this plane — the city has no second storey you stand on.
pub const WALK_Y: f64 = 0.91;

/// How many times [`NavData::offset_route`]'s segment sweep may back a lane off
/// before it leaves that stretch on the centreline. Every failing segment halves
/// both its ends on every pass, so the widest corridor in the city (the Cut's
/// 10 m half-width) reaches the give-up width in seven; the eighth is the pass
/// that finds nothing left to do.
const LANE_BACKOFF_PASSES: usize = 8;

/// Navigation data that cannot be loaded without leaving the graph ambiguous.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavError {
    pub message: String,
}

impl NavError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for NavError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for NavError {}

// --------------------------------------------------------------------------- //
// The baked document, straight off the JSON
// --------------------------------------------------------------------------- //
#[derive(Debug, Deserialize)]
struct NavDoc {
    schema_version: u32,
    grid: GridDoc,
    nodes: Vec<[f64; 2]>,
    edges: Vec<(usize, usize, f64)>,
    places: Vec<PlaceDoc>,
    sites: Vec<SiteDoc>,
    doors: Vec<DoorDoc>,
    reference: ReferenceDoc,
}

#[derive(Debug, Deserialize)]
struct GridDoc {
    x0: f64,
    z0: f64,
    cell_m: f64,
    w: usize,
    h: usize,
    agent_radius_m: f64,
    #[allow(dead_code)]
    bitset_file: String,
    bitset_bits: usize,
    #[allow(dead_code)]
    bitset_sha256: String,
}

#[derive(Debug, Deserialize)]
struct PlaceDoc {
    name: String,
    node: usize,
    kind: String,
}

#[derive(Debug, Deserialize)]
struct SiteDoc {
    id: String,
    name: String,
    node: usize,
}

#[derive(Debug, Deserialize)]
struct DoorDoc {
    building: String,
    edge: usize,
    node: usize,
}

#[derive(Debug, Deserialize)]
struct ReferenceDoc {
    forecourt: usize,
}

// --------------------------------------------------------------------------- //
// The public model
// --------------------------------------------------------------------------- //
/// The quarter-metre grid the bitset is indexed on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NavGrid {
    pub x0: f64,
    pub z0: f64,
    pub cell_m: f64,
    pub w: usize,
    pub h: usize,
    pub agent_radius_m: f64,
}

impl NavGrid {
    /// World XZ -> `(row, col)`, flooring — the cell containing the point. Row
    /// indexes z, column indexes x, matching the Python bake.
    pub fn cell(&self, x: f64, z: f64) -> Option<(usize, usize)> {
        if x < self.x0 || z < self.z0 {
            return None;
        }
        let col = ((x - self.x0) / self.cell_m) as usize;
        let row = ((z - self.z0) / self.cell_m) as usize;
        if row < self.h && col < self.w {
            Some((row, col))
        } else {
            None
        }
    }

    /// The world XZ of a cell centre.
    pub fn centre(&self, row: usize, col: usize) -> (f64, f64) {
        (
            self.x0 + (col as f64 + 0.5) * self.cell_m,
            self.z0 + (row as f64 + 0.5) * self.cell_m,
        )
    }
}

/// One named place, hung off the graph at a walkable node.
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    pub name: String,
    pub node: usize,
    pub kind: String,
}

/// One open site (square, yard, court) — its polygon interior is free movement
/// in a later milestone; here it is a labelled leaf.
#[derive(Debug, Clone, PartialEq)]
pub struct Site {
    pub id: String,
    pub name: String,
    pub node: usize,
}

/// One building door: the render's `stable_hash`-chosen edge (so the visible
/// door and the walked-to door are the same one) and the walkable node a metre
/// outside the threshold.
#[derive(Debug, Clone, PartialEq)]
pub struct Door {
    pub building: String,
    pub edge: usize,
    pub node: usize,
}

/// One graph edge from a node's adjacency list.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edge {
    pub to: usize,
    pub cost: f64,
    pub half_width_m: f64,
}

/// A found route: the node path, the world polyline it traces at [`WALK_Y`], the
/// corridor half-width alongside each point, and the total length.
#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    pub nodes: Vec<usize>,
    pub points: Vec<Vec3>,
    pub half_width_m: Vec<f64>,
    pub length_m: f64,
}

/// The validated walkable surface and street graph.
///
/// `PartialEq` (all fields already are) so it can sit behind an `Arc` inside a
/// `#[derive(PartialEq)]` [`crate::EngineConfig`]; not `Eq`, because the node
/// coordinates are `f64`.
#[derive(Debug, Clone, PartialEq)]
pub struct NavData {
    grid: NavGrid,
    bitset: Vec<u8>,
    nodes: Vec<[f64; 2]>,
    adjacency: Vec<Vec<Edge>>,
    places: Vec<Place>,
    sites: Vec<Site>,
    doors: Vec<Door>,
    place_by_name: HashMap<String, usize>,
    door_by_building: HashMap<String, usize>,
    forecourt: usize,
    /// Derived from `nodes` alone, so two `NavData` built from the same graph
    /// still compare equal.
    node_index: Option<NodeIndex>,
}

impl NavData {
    /// Parse and fully validate the baked graph JSON and its walkable bitset.
    pub fn from_parts(json: &str, bitset: &[u8]) -> Result<Self, NavError> {
        let doc: NavDoc = serde_json::from_str(json)
            .map_err(|error| NavError::new(format!("invalid navigation.json: {error}")))?;
        if doc.schema_version != 1 {
            return Err(NavError::new(format!(
                "unsupported navigation schema {}; expected 1",
                doc.schema_version
            )));
        }
        let grid = NavGrid {
            x0: doc.grid.x0,
            z0: doc.grid.z0,
            cell_m: doc.grid.cell_m,
            w: doc.grid.w,
            h: doc.grid.h,
            agent_radius_m: doc.grid.agent_radius_m,
        };
        if grid.cell_m <= 0.0 || grid.w == 0 || grid.h == 0 {
            return Err(NavError::new("navigation grid has no extent"));
        }
        if doc.grid.bitset_bits != grid.w * grid.h {
            return Err(NavError::new(format!(
                "bitset_bits {} does not match {}x{} grid",
                doc.grid.bitset_bits, grid.w, grid.h
            )));
        }
        let expected_bytes = (grid.w * grid.h).div_ceil(8);
        if bitset.len() != expected_bytes {
            return Err(NavError::new(format!(
                "navigation.bin is {} bytes; expected {} for a {}x{} grid",
                bitset.len(),
                expected_bytes,
                grid.w,
                grid.h
            )));
        }

        let n = doc.nodes.len();
        let check_node = |node: usize, what: &str| -> Result<(), NavError> {
            if node >= n {
                Err(NavError::new(format!(
                    "{what} refers to node {node} but there are only {n}"
                )))
            } else {
                Ok(())
            }
        };

        let mut adjacency: Vec<Vec<Edge>> = vec![Vec::new(); n];
        for &(a, b, half_width) in &doc.edges {
            check_node(a, "edge")?;
            check_node(b, "edge")?;
            let cost = distance(doc.nodes[a], doc.nodes[b]);
            adjacency[a].push(Edge {
                to: b,
                cost,
                half_width_m: half_width,
            });
            adjacency[b].push(Edge {
                to: a,
                cost,
                half_width_m: half_width,
            });
        }

        let mut place_by_name = HashMap::new();
        let mut places = Vec::with_capacity(doc.places.len());
        for p in doc.places {
            check_node(p.node, "place")?;
            place_by_name.insert(p.name.clone(), places.len());
            places.push(Place {
                name: p.name,
                node: p.node,
                kind: p.kind,
            });
        }

        let mut sites = Vec::with_capacity(doc.sites.len());
        for s in doc.sites {
            check_node(s.node, "site")?;
            sites.push(Site {
                id: s.id,
                name: s.name,
                node: s.node,
            });
        }

        let mut door_by_building = HashMap::new();
        let mut doors = Vec::with_capacity(doc.doors.len());
        for d in doc.doors {
            check_node(d.node, "door")?;
            door_by_building.insert(d.building.clone(), doors.len());
            doors.push(Door {
                building: d.building,
                edge: d.edge,
                node: d.node,
            });
        }

        check_node(doc.reference.forecourt, "reference.forecourt")?;

        let node_index = NodeIndex::build(&doc.nodes);
        Ok(Self {
            grid,
            bitset: bitset.to_vec(),
            nodes: doc.nodes,
            adjacency,
            places,
            sites,
            doors,
            place_by_name,
            door_by_building,
            forecourt: doc.reference.forecourt,
            node_index,
        })
    }

    pub fn grid(&self) -> NavGrid {
        self.grid
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// The world XZ of a node.
    pub fn node_xz(&self, node: usize) -> [f64; 2] {
        self.nodes[node]
    }

    /// The node's position as a [`Vec3`] on the walk plane.
    pub fn node_point(&self, node: usize) -> Vec3 {
        let [x, z] = self.nodes[node];
        Vec3::new(x, WALK_Y, z)
    }

    pub fn places(&self) -> &[Place] {
        &self.places
    }

    pub fn sites(&self) -> &[Site] {
        &self.sites
    }

    pub fn doors(&self) -> &[Door] {
        &self.doors
    }

    pub fn adjacency(&self) -> &[Vec<Edge>] {
        &self.adjacency
    }

    pub fn place(&self, name: &str) -> Option<&Place> {
        self.place_by_name.get(name).map(|&i| &self.places[i])
    }

    pub fn door(&self, building: &str) -> Option<&Door> {
        self.door_by_building.get(building).map(|&i| &self.doors[i])
    }

    /// The reference start for reachability — the cathedral forecourt.
    pub fn forecourt(&self) -> usize {
        self.forecourt
    }

    /// Is this world point on walkable ground? A single bitset lookup.
    pub fn is_walkable(&self, x: f64, z: f64) -> bool {
        match self.grid.cell(x, z) {
            Some((row, col)) => self.cell_walkable(row, col),
            None => false,
        }
    }

    fn cell_walkable(&self, row: usize, col: usize) -> bool {
        let idx = row * self.grid.w + col;
        // Row-major, MSB-first within each byte — matches numpy `packbits`.
        (self.bitset[idx >> 3] >> (7 - (idx & 7))) & 1 == 1
    }

    /// The graph node nearest a world point, by straight-line distance.
    ///
    /// Answered through the uniform-grid [`NodeIndex`] the movement layer was
    /// owed: `route_between` snaps both its endpoints this way, and a sweep of
    /// all ~4,000 nodes twice per route was a real share of the burst every
    /// office crossing sets off (the whole enrolled cast re-routes at a bell).
    /// The index falls back to the sweep for a point outside the node bounding
    /// box, or for a graph the index cannot be built over.
    ///
    /// Both paths answer with the *same* node, ties included: the winner is the
    /// smallest `(distance², index)` pair, which is exactly what a sweep in
    /// index order with a strict `<` picks.
    pub fn nearest_node(&self, x: f64, z: f64) -> Option<usize> {
        if let Some(index) = &self.node_index
            && let Some(node) = index.nearest(&self.nodes, x, z)
        {
            return Some(node);
        }
        self.nearest_node_by_sweep(x, z)
    }

    fn nearest_node_by_sweep(&self, x: f64, z: f64) -> Option<usize> {
        let mut best = None;
        let mut best_d2 = f64::INFINITY;
        for (i, &[nx, nz]) in self.nodes.iter().enumerate() {
            let d2 = (nx - x) * (nx - x) + (nz - z) * (nz - z);
            if d2 < best_d2 {
                best_d2 = d2;
                best = Some(i);
            }
        }
        best
    }

    /// Shortest-path distance from `start` to every node it can reach. `None`
    /// marks the unreachable ones. One Dijkstra sweep answers a whole batch of
    /// reachability questions (every place, every door) at once.
    pub fn distances_from(&self, start: usize) -> Vec<Option<f64>> {
        let mut dist = vec![None; self.nodes.len()];
        let mut heap = BinaryHeap::new();
        dist[start] = Some(0.0);
        heap.push(HeapEntry {
            cost: 0.0,
            node: start,
        });
        while let Some(HeapEntry { cost, node }) = heap.pop() {
            if dist[node].is_some_and(|d| cost > d) {
                continue;
            }
            for edge in &self.adjacency[node] {
                let next = cost + edge.cost;
                if dist[edge.to].is_none_or(|d| next < d) {
                    dist[edge.to] = Some(next);
                    heap.push(HeapEntry {
                        cost: next,
                        node: edge.to,
                    });
                }
            }
        }
        dist
    }

    /// Can you get from `start` to `goal` at all?
    pub fn reachable(&self, start: usize, goal: usize) -> bool {
        self.route_nodes(start, goal).is_some()
    }

    /// A* over the street graph between two nodes.
    pub fn route_nodes(&self, start: usize, goal: usize) -> Option<Route> {
        self.route_nodes_avoiding(start, goal, None)
    }

    /// [`route_nodes`], never expanding through `avoid` — the "take Cinder Row
    /// instead" route around an occupied one-person choke
    /// (`features/implemented/movement/02_navigation.md` §5). `None` when every way there
    /// leads through the avoided node (or the goal *is* it).
    ///
    /// [`route_nodes`]: NavData::route_nodes
    pub fn route_nodes_avoiding(
        &self,
        start: usize,
        goal: usize,
        avoid: Option<usize>,
    ) -> Option<Route> {
        if start >= self.nodes.len() || goal >= self.nodes.len() {
            return None;
        }
        if avoid == Some(goal) || avoid == Some(start) {
            return None;
        }
        if start == goal {
            return Some(self.route_from_nodes(vec![start]));
        }
        let goal_xz = self.nodes[goal];
        let heuristic = |node: usize| distance(self.nodes[node], goal_xz);

        // The working set is reused across searches rather than freshly
        // allocated and filled: two `Vec`s the length of the node set is 64 KB
        // of malloc and memset before A* expands a single edge, and the whole
        // enrolled cast re-routes together whenever the office bell changes
        // which leg of their round they are on. A generation stamp stands in
        // for the refill — an entry not stamped with *this* search's number
        // reads as infinity — so a route costs only the nodes it touches.
        ASTAR_SCRATCH.with_borrow_mut(|scratch| {
            let generation = scratch.begin(self.nodes.len());
            scratch.g[start] = 0.0;
            scratch.stamp[start] = generation;
            scratch.heap.push(HeapEntry {
                cost: heuristic(start),
                node: start,
            });
            while let Some(HeapEntry { node, .. }) = scratch.heap.pop() {
                if node == goal {
                    let mut path = vec![goal];
                    let mut cur = goal;
                    while cur != start {
                        cur = scratch.came[cur];
                        path.push(cur);
                    }
                    path.reverse();
                    return Some(self.route_from_nodes(path));
                }
                let base = scratch.g[node];
                for edge in &self.adjacency[node] {
                    if avoid == Some(edge.to) {
                        continue;
                    }
                    let tentative = base + edge.cost;
                    let known = if scratch.stamp[edge.to] == generation {
                        scratch.g[edge.to]
                    } else {
                        f64::INFINITY
                    };
                    if tentative < known {
                        scratch.stamp[edge.to] = generation;
                        scratch.g[edge.to] = tentative;
                        scratch.came[edge.to] = node;
                        scratch.heap.push(HeapEntry {
                            cost: tentative + heuristic(edge.to),
                            node: edge.to,
                        });
                    }
                }
            }
            None
        })
    }

    /// Route between two world points, snapping each to the nearest graph node.
    pub fn route_between(&self, from: Vec3, to: Vec3) -> Option<Route> {
        let start = self.nearest_node(from.x, from.z)?;
        let goal = self.nearest_node(to.x, to.z)?;
        self.route_nodes(start, goal)
    }

    /// The traversed edge's corridor half-width between two adjacent nodes —
    /// tighter than [`Route::half_width_m`]'s per-node "widest corridor meeting
    /// it", which overstates the squeeze at an alley mouth. Falls back to the
    /// graph's 0.6 m minimum for a non-adjacent pair.
    fn segment_half_width(&self, a: usize, b: usize) -> f64 {
        self.adjacency[a]
            .iter()
            .find(|edge| edge.to == b)
            .map_or(0.6, |edge| edge.half_width_m)
    }

    /// The route's polyline, shifted sideways into a walking lane (M7:
    /// "lane offsets, or you get a conga line" — 02_navigation.md §5).
    ///
    /// `lane` is a signed fraction of the usable corridor: positive is the
    /// walker's **right** (so two streams meeting pass right shoulder to right
    /// shoulder), and the usable corridor at each vertex is the *traversed*
    /// segments' half-width minus the agent radius — nothing in a 1.2 m alley,
    /// a couple of metres on the Cut. Every shifted vertex *and the whole
    /// stretch between two of them* is validated against the walkable bitset
    /// (halving the shift until it lands on ground), so the lane can never put a
    /// body inside a wall. The final vertex is left exact: arrival semantics
    /// (the curb, the doorstep) stay point-precise.
    pub fn offset_route(&self, route: &Route, lane: f64) -> Vec<Vec3> {
        let points = &route.points;
        let n = points.len();
        if n < 2 || lane == 0.0 {
            return points.clone();
        }
        // Per-segment corridor half-widths, from the traversed edges.
        let seg_hw: Vec<f64> = (0..n - 1)
            .map(|i| match (route.nodes.get(i), route.nodes.get(i + 1)) {
                (Some(&a), Some(&b)) => self.segment_half_width(a, b),
                _ => 0.6,
            })
            .collect();

        // Each vertex's sideways displacement, held apart from the vertex itself
        // so the segment sweep below can back one off without having to work out
        // afresh which way it was shifted.
        let mut shift: Vec<Vec3> = Vec::with_capacity(n);
        for i in 0..n {
            if i == n - 1 {
                shift.push(Vec3::ZERO);
                break;
            }
            // Averaged travel direction at the vertex (miter), in XZ.
            let dir_out = planar_dir(points[i], points[i + 1]);
            let dir = if i == 0 {
                dir_out
            } else {
                match (planar_dir(points[i - 1], points[i]), dir_out) {
                    (Some(dir_in), Some(out)) => {
                        let sum = dir_in + out;
                        // A U-turn's miter is degenerate; keep the outgoing leg.
                        if sum.length() < 1e-6 { Some(out) } else { Some(sum.normalize()) }
                    }
                    (dir_in, out) => out.or(dir_in),
                }
            };
            let Some(dir) = dir else {
                shift.push(Vec3::ZERO);
                continue;
            };
            // Right of travel: yaw 0 faces -Z, and looking down -Z with +Y up,
            // right is +X — i.e. `dir × up`.
            let right = Vec3::new(-dir.z, 0.0, dir.x);
            let hw = if i == 0 {
                seg_hw[0]
            } else {
                seg_hw[i - 1].min(seg_hw[i])
            };
            let usable = (hw - self.grid.agent_radius_m).max(0.0);
            let mut offset = lane.clamp(-1.0, 1.0) * usable;
            // Halve the shift until it lands on walkable ground; give up on the
            // centreline vertex, which is on the graph and always walkable.
            let mut accepted = Vec3::ZERO;
            for _ in 0..3 {
                let candidate = right * offset;
                if self.is_walkable(points[i].x + candidate.x, points[i].z + candidate.z) {
                    accepted = candidate;
                    break;
                }
                offset *= 0.5;
            }
            shift.push(accepted);
        }

        // Two validated ends say nothing about the stretch between them. A graph
        // edge can run tens of metres while its `half_width_m` describes only its
        // widest part, so a corridor that pinches in the middle takes the whole
        // shifted segment through a building while both vertices sit happily on
        // paving. Walk each segment across the bitset and back both its ends off
        // until it clears: the centreline is on the graph, so the retreat always
        // has ground to land on.
        let give_up_m = self.grid.cell_m * 0.5;
        for _ in 0..LANE_BACKOFF_PASSES {
            let mut settled = true;
            for i in 0..n - 1 {
                if shift[i] == Vec3::ZERO && shift[i + 1] == Vec3::ZERO {
                    continue; // already the centreline; there is nowhere left to go
                }
                if self.segment_walkable(points[i] + shift[i], points[i + 1] + shift[i + 1]) {
                    continue;
                }
                // Both ends give ground, so a pinch in a long leg costs the lane
                // only around itself instead of flattening the whole route.
                settled = false;
                shift[i] = back_off(shift[i], give_up_m);
                shift[i + 1] = back_off(shift[i + 1], give_up_m);
            }
            if settled {
                break;
            }
        }

        (0..n).map(|i| points[i] + shift[i]).collect()
    }

    /// Is every point of the stretch `a` → `b` on walkable ground? Sampled a
    /// grid cell apart, which is the resolution the bitset is baked at — and it
    /// is baked eroded by the agent radius, so nothing a body would collide with
    /// hides between two samples. Crate-visible for the off-graph strides other
    /// movers take (the dogs' drift): two walkable endpoints say nothing about
    /// the ground between them.
    pub(crate) fn segment_walkable(&self, a: Vec3, b: Vec3) -> bool {
        let span = Vec3::new(b.x - a.x, 0.0, b.z - a.z);
        let length = span.length();
        if length < 1e-9 {
            return self.is_walkable(a.x, a.z);
        }
        let steps = (length / self.grid.cell_m).ceil() as usize;
        (0..=steps).all(|s| {
            let t = s as f64 / steps as f64;
            self.is_walkable(a.x + span.x * t, a.z + span.z * t)
        })
    }

    fn route_from_nodes(&self, nodes: Vec<usize>) -> Route {
        let points: Vec<Vec3> = nodes.iter().map(|&n| self.node_point(n)).collect();
        let mut half_width_m = Vec::with_capacity(nodes.len());
        let mut length_m = 0.0;
        for (i, &node) in nodes.iter().enumerate() {
            // half-width at a node is the widest corridor meeting it
            let hw = self.adjacency[node]
                .iter()
                .map(|e| e.half_width_m)
                .fold(0.6_f64, f64::max);
            half_width_m.push(hw);
            if i > 0 {
                length_m += distance(self.nodes[nodes[i - 1]], self.nodes[node]);
            }
        }
        Route {
            nodes,
            points,
            half_width_m,
            length_m,
        }
    }
}

/// Parse only the building → door-edge map from the baked graph, without the
/// bitset. The city renderer uses this so the visible door (`add_facade_openings`)
/// is drawn on the same polygon edge the sim walks to — a building absent from
/// the map has no reachable door and gets none (02_navigation.md §1).
pub fn door_edges_from_json(json: &str) -> Result<HashMap<String, usize>, NavError> {
    #[derive(Deserialize)]
    struct DoorsOnly {
        doors: Vec<DoorDoc>,
    }
    let doc: DoorsOnly = serde_json::from_str(json)
        .map_err(|error| NavError::new(format!("invalid navigation.json: {error}")))?;
    Ok(doc.doors.into_iter().map(|d| (d.building, d.edge)).collect())
}

fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    let dx = a[0] - b[0];
    let dz = a[1] - b[1];
    (dx * dx + dz * dz).sqrt()
}

/// Halve a lane shift, or surrender it entirely once halving it again could no
/// longer change which grid cell the body stands in — a shift that small is not
/// a lane, and pretending otherwise would only cost more passes.
fn back_off(shift: Vec3, give_up_m: f64) -> Vec3 {
    let halved = shift * 0.5;
    if halved.length() < give_up_m {
        Vec3::ZERO
    } else {
        halved
    }
}

/// Unit direction `from` → `to` on the walk plane, or `None` for coincident
/// points.
fn planar_dir(from: Vec3, to: Vec3) -> Option<Vec3> {
    let d = Vec3::new(to.x - from.x, 0.0, to.z - from.z);
    let length = d.length();
    if length < 1e-9 { None } else { Some(d / length) }
}

// --------------------------------------------------------------------------- //
// The A* working set, reused between searches
// --------------------------------------------------------------------------- //
thread_local! {
    /// One working set per thread. The sim is pumped from a single thread, so
    /// in the game this is one allocation for the whole run; a test harness
    /// that routes from several threads simply gets one each.
    static ASTAR_SCRATCH: RefCell<AStarScratch> = RefCell::new(AStarScratch::new());
}

/// Reusable `g` / `came` arrays for [`NavData::route_nodes_avoiding`], kept
/// current by a generation stamp instead of being refilled.
struct AStarScratch {
    /// Which search last wrote `g[node]` and `came[node]`. Anything not equal
    /// to the current generation has not been reached by *this* search.
    stamp: Vec<u32>,
    generation: u32,
    g: Vec<f64>,
    came: Vec<usize>,
    heap: BinaryHeap<HeapEntry>,
}

impl AStarScratch {
    fn new() -> Self {
        Self {
            stamp: Vec::new(),
            generation: 0,
            g: Vec::new(),
            came: Vec::new(),
            heap: BinaryHeap::new(),
        }
    }

    /// Ready the buffers for a search over `nodes` nodes and hand back the
    /// stamp that identifies it. The heap is emptied here rather than on the
    /// way out, because a search that finds its goal returns from the middle of
    /// the loop and leaves the rest of the frontier behind it.
    fn begin(&mut self, nodes: usize) -> u32 {
        if self.stamp.len() != nodes {
            self.stamp = vec![0; nodes];
            self.g = vec![f64::INFINITY; nodes];
            self.came = vec![usize::MAX; nodes];
            self.generation = 0;
        }
        self.heap.clear();
        // Generation 0 is "written by no search", so wrapping past it has to
        // clear the stamps that would otherwise read as current.
        self.generation = self.generation.wrapping_add(1);
        if self.generation == 0 {
            self.stamp.fill(0);
            self.generation = 1;
        }
        self.generation
    }
}

// --------------------------------------------------------------------------- //
// The node index behind `nearest_node`
// --------------------------------------------------------------------------- //
/// A uniform grid over the graph's nodes, in compressed-row form: the cells in
/// `starts` window into `members`, which holds node indices.
///
/// Deliberately its own grid and not [`NavGrid`]: the walkable bitset is a
/// 0.25 m raster of the whole city (millions of cells), where this wants a few
/// hundred buckets of ~8 nodes each.
#[derive(Debug, Clone, PartialEq)]
struct NodeIndex {
    x0: f64,
    z0: f64,
    cell_m: f64,
    cols: usize,
    rows: usize,
    starts: Vec<u32>,
    members: Vec<u32>,
}

impl NodeIndex {
    /// `None` for a graph the ring search below could not answer over: no
    /// nodes at all, or a coordinate that is not finite (where the "everything
    /// unscanned is at least this far away" bound stops meaning anything).
    fn build(points: &[[f64; 2]]) -> Option<Self> {
        if points.is_empty() || points.len() > u32::MAX as usize {
            return None;
        }
        let (mut x0, mut x1) = (f64::INFINITY, f64::NEG_INFINITY);
        let (mut z0, mut z1) = (f64::INFINITY, f64::NEG_INFINITY);
        for &[x, z] in points {
            if !x.is_finite() || !z.is_finite() {
                return None;
            }
            x0 = x0.min(x);
            x1 = x1.max(x);
            z0 = z0.min(z);
            z1 = z1.max(z);
        }
        // About one node a cell: the ring search then reads a couple of dozen
        // candidates for a query anywhere in the city, and the table itself is
        // the same order of size as the node array.
        let span = (x1 - x0).max(z1 - z0).max(1.0);
        let cell_m = (span / (points.len() as f64).sqrt()).max(1.0);
        let cols = ((x1 - x0) / cell_m).floor() as usize + 1;
        let rows = ((z1 - z0) / cell_m).floor() as usize + 1;
        if cols == 0 || rows == 0 || cols.checked_mul(rows).is_none() {
            return None;
        }

        let cell_of = |x: f64, z: f64| -> usize {
            let col = (((x - x0) / cell_m).floor() as usize).min(cols - 1);
            let row = (((z - z0) / cell_m).floor() as usize).min(rows - 1);
            row * cols + col
        };
        let mut starts = vec![0u32; cols * rows + 1];
        for &[x, z] in points {
            starts[cell_of(x, z) + 1] += 1;
        }
        for cell in 1..starts.len() {
            starts[cell] += starts[cell - 1];
        }
        let mut cursor = starts.clone();
        let mut members = vec![0u32; points.len()];
        for (node, &[x, z]) in points.iter().enumerate() {
            let cell = cell_of(x, z);
            members[cursor[cell] as usize] = node as u32;
            cursor[cell] += 1;
        }
        Some(Self {
            x0,
            z0,
            cell_m,
            cols,
            rows,
            starts,
            members,
        })
    }

    /// The nearest node to `(x, z)`, or `None` when the point lies outside the
    /// indexed box and the caller must sweep instead.
    ///
    /// Rings of cells are scanned outward from the query's own cell. Having
    /// scanned every cell within `ring` cells, nothing left can be closer than
    /// `ring * cell_m` — the query point sits inside its own cell, so each of
    /// the block's four sides is at least that far off — and the search stops
    /// as soon as the best hit is *strictly* inside that bound, which also
    /// rules out an unscanned node tying with it.
    fn nearest(&self, points: &[[f64; 2]], x: f64, z: f64) -> Option<usize> {
        if !x.is_finite() || !z.is_finite() {
            return None;
        }
        let col = (x - self.x0) / self.cell_m;
        let row = (z - self.z0) / self.cell_m;
        if !(col >= 0.0 && row >= 0.0) {
            return None;
        }
        let (col, row) = (col.floor() as usize, row.floor() as usize);
        if col >= self.cols || row >= self.rows {
            return None;
        }

        let mut best: Option<(f64, usize)> = None;
        let visit = |cell_row: usize, cell_col: usize, best: &mut Option<(f64, usize)>| {
            let cell = cell_row * self.cols + cell_col;
            for &member in &self.members[self.starts[cell] as usize..self.starts[cell + 1] as usize]
            {
                let node = member as usize;
                let [nx, nz] = points[node];
                let d2 = (nx - x) * (nx - x) + (nz - z) * (nz - z);
                // Smallest `(d², index)` — the pair a sweep in index order with
                // a strict `<` settles on.
                let better = match *best {
                    None => true,
                    Some((best_d2, best_node)) => {
                        d2 < best_d2 || (d2 == best_d2 && node < best_node)
                    }
                };
                if better {
                    *best = Some((d2, node));
                }
            }
        };

        for ring in 0..=self.cols.max(self.rows) {
            let low_col = col.saturating_sub(ring);
            let high_col = (col + ring).min(self.cols - 1);
            let low_row = row.saturating_sub(ring);
            let high_row = (row + ring).min(self.rows - 1);
            if ring == 0 {
                visit(row, col, &mut best);
            } else {
                // The ring's two full rows (each present only when it is on the
                // grid at all), then its two columns over the rows between them.
                if row >= ring {
                    for cell_col in low_col..=high_col {
                        visit(low_row, cell_col, &mut best);
                    }
                }
                if row + ring < self.rows {
                    for cell_col in low_col..=high_col {
                        visit(high_row, cell_col, &mut best);
                    }
                }
                let inner_low = (row + 1).saturating_sub(ring);
                let inner_high = (row + ring - 1).min(self.rows - 1);
                for cell_row in inner_low..=inner_high {
                    if col >= ring {
                        visit(cell_row, low_col, &mut best);
                    }
                    if col + ring < self.cols {
                        visit(cell_row, high_col, &mut best);
                    }
                }
            }
            if let Some((best_d2, _)) = best {
                // Shaved by a hair before it is believed. The "nothing left is
                // closer than `ring * cell_m`" argument is exact arithmetic,
                // but the cell a node was filed under and the cell the query
                // lands in are both a rounded `floor(quotient)`, which can put
                // a node a couple of hundred attometres inside the bound. A
                // relative shave a million times that is still far too small to
                // cost a ring in practice, and it makes the disagreement with
                // the sweep impossible rather than merely improbable — the
                // sweep is what walkers' streets are chosen by.
                let covered = ring as f64 * self.cell_m * (1.0 - 1e-9);
                if best_d2 < covered * covered {
                    break;
                }
            }
        }
        best.map(|(_, node)| node)
    }
}

/// Min-heap entry for Dijkstra / A* — ordered by ascending cost.
struct HeapEntry {
    cost: f64,
    node: usize,
}

impl PartialEq for HeapEntry {
    fn eq(&self, other: &Self) -> bool {
        self.cost == other.cost
    }
}
impl Eq for HeapEntry {}
impl PartialOrd for HeapEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for HeapEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Reversed so `BinaryHeap` (a max-heap) pops the smallest cost first.
        other
            .cost
            .partial_cmp(&self.cost)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| self.node.cmp(&other.node))
    }
}

#[cfg(test)]
mod navigation_tests;
