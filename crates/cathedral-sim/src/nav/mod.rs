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
//! Design: `features/movement/02_navigation.md`.

use std::collections::{BinaryHeap, HashMap};

use serde::Deserialize;

use crate::math::Vec3;

/// The height every NPC walks at: `PLAYER_SPAWN.y` in `controller.rs`. Baked
/// navigation is 2D on this plane — the city has no second storey you stand on.
pub const WALK_Y: f64 = 0.91;

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
#[derive(Debug, Clone)]
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

    /// The graph node nearest a world point, by straight-line distance. Brute
    /// force over the node set; the movement layer will add a spatial index, but
    /// reachability queries do not need one.
    pub fn nearest_node(&self, x: f64, z: f64) -> Option<usize> {
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
        if start >= self.nodes.len() || goal >= self.nodes.len() {
            return None;
        }
        if start == goal {
            return Some(self.route_from_nodes(vec![start]));
        }
        let goal_xz = self.nodes[goal];
        let heuristic = |node: usize| distance(self.nodes[node], goal_xz);

        let mut g: Vec<f64> = vec![f64::INFINITY; self.nodes.len()];
        let mut came: Vec<usize> = vec![usize::MAX; self.nodes.len()];
        g[start] = 0.0;
        let mut heap = BinaryHeap::new();
        heap.push(HeapEntry {
            cost: heuristic(start),
            node: start,
        });
        while let Some(HeapEntry { node, .. }) = heap.pop() {
            if node == goal {
                let mut path = vec![goal];
                let mut cur = goal;
                while cur != start {
                    cur = came[cur];
                    path.push(cur);
                }
                path.reverse();
                return Some(self.route_from_nodes(path));
            }
            let base = g[node];
            for edge in &self.adjacency[node] {
                let tentative = base + edge.cost;
                if tentative < g[edge.to] {
                    g[edge.to] = tentative;
                    came[edge.to] = node;
                    heap.push(HeapEntry {
                        cost: tentative + heuristic(edge.to),
                        node: edge.to,
                    });
                }
            }
        }
        None
    }

    /// Route between two world points, snapping each to the nearest graph node.
    pub fn route_between(&self, from: Vec3, to: Vec3) -> Option<Route> {
        let start = self.nearest_node(from.x, from.z)?;
        let goal = self.nearest_node(to.x, to.z)?;
        self.route_nodes(start, goal)
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
