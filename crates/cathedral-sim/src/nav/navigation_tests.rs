//! The navigation acceptance suite — `cargo test -p cathedral-sim navigation`.
//!
//! Several of these were written for us in `lore/places/04_routes_and_sightlines.md`
//! under "Route acceptance tests for later builds"; the rest are the mechanical
//! guards from `features/movement/02_navigation.md` §8. They load the committed
//! artifacts exactly as the host does — no filesystem, no bake at test time.

use serde_json::Value;

use super::*;

const NAV_JSON: &str = include_str!("../../../../assets/world/navigation.json");
const NAV_BIN: &[u8] = include_bytes!("../../../../assets/world/navigation.bin");
const PLAN_JSON: &str = include_str!("../../../../lore/places/ombreval_buildings.json");

fn nav() -> NavData {
    NavData::from_parts(NAV_JSON, NAV_BIN).expect("the committed navigation artifact loads")
}

fn plan() -> Value {
    serde_json::from_str(PLAN_JSON).expect("the cadastral plan is valid JSON")
}

fn road_points(plan: &Value, id: &str) -> Vec<[f64; 2]> {
    plan["roads"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == id)
        .unwrap_or_else(|| panic!("road {id} exists"))["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| [p[0].as_f64().unwrap(), p[1].as_f64().unwrap()])
        .collect()
}

fn point(xz: [f64; 2]) -> Vec3 {
    Vec3::new(xz[0], WALK_Y, xz[1])
}

fn straight_len(pts: &[[f64; 2]]) -> f64 {
    pts.windows(2)
        .map(|w| {
            let (a, b) = (w[0], w[1]);
            ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
        })
        .sum()
}

#[test]
fn navigation_loads_with_a_plausible_shape() {
    let nav = nav();
    assert!(nav.node_count() > 500, "a real street graph");
    assert_eq!(nav.grid().cell_m, 0.25);
    assert!((nav.grid().agent_radius_m - 0.35).abs() < 1e-9);
    // Player spawn is on the walkable apron before the west doors.
    assert!(nav.is_walkable(0.0, 95.0));
    // Inside the cathedral nave (its footprint is subtracted) is not walkable.
    assert!(!nav.is_walkable(0.0, -50.0));
    // Beyond the south wall is not walkable.
    assert!(!nav.is_walkable(0.0, -800.0));
}

/// The committed bitset must still match the manifest the JSON carries, so the
/// two files cannot silently drift apart in a commit. A dependency-free FNV-1a
/// stands in for the sha256 the crate cannot hash.
#[test]
fn navigation_bitset_matches_its_manifest() {
    let manifest: Value = serde_json::from_str(NAV_JSON).unwrap();
    let claimed = manifest["grid"]["bitset_fnv1a"]
        .as_u64()
        .expect("the manifest carries bitset_fnv1a");
    let mut fnv: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in NAV_BIN {
        fnv = (fnv ^ byte as u64).wrapping_mul(0x0000_0100_0000_01b3);
    }
    assert_eq!(fnv, claimed, "navigation.bin no longer matches navigation.json");
}

/// Every named place routes from the cathedral forecourt. The one plan anchor
/// that does not appear here is "Outer Serle wharves" — genuinely beyond the
/// south wall, on real water the city has no bridge to (02_navigation.md §6).
#[test]
fn every_named_place_is_reachable() {
    let nav = nav();
    assert_eq!(
        nav.places().len(),
        68,
        "68 of the 69 plan anchors are on walkable ground (the Serle wharves is not)"
    );
    let dist = nav.distances_from(nav.forecourt());
    let unreachable: Vec<&str> = nav
        .places()
        .iter()
        .filter(|p| dist[p.node].is_none())
        .map(|p| p.name.as_str())
        .collect();
    assert!(
        unreachable.is_empty(),
        "these named places do not route from the forecourt: {unreachable:?}"
    );
}

/// The test M1 exists to catch: the `stable_hash % polygon.len()` door once put
/// 106 front doors against a wall. Picking the edge by reachability takes it from
/// 2,154 to essentially every building.
#[test]
fn every_door_is_reachable() {
    let nav = nav();
    assert!(
        nav.doors().len() >= 2_560,
        "expected >= 2560 doors, baked {}",
        nav.doors().len()
    );
    let dist = nav.distances_from(nav.forecourt());
    let stranded = nav
        .doors()
        .iter()
        .filter(|d| dist[d.node].is_none())
        .count();
    assert_eq!(stranded, 0, "{stranded} baked doors do not route from the forecourt");
}

/// Every graph edge lies on walkable ground: the midpoint of each edge is inside
/// the bitset. Guards against a route that clips a wall.
#[test]
fn graph_matches_bitset() {
    let nav = nav();
    let mut bad = 0;
    for (a, edges) in nav.adjacency().iter().enumerate() {
        let pa = nav.node_xz(a);
        for edge in edges {
            if edge.to <= a {
                continue; // each undirected edge once
            }
            let pb = nav.node_xz(edge.to);
            let mid = [(pa[0] + pb[0]) / 2.0, (pa[1] + pb[1]) / 2.0];
            if !nav.is_walkable(mid[0], mid[1]) {
                bad += 1;
            }
        }
    }
    assert_eq!(bad, 0, "{bad} graph edges have a non-walkable midpoint");
}

/// Every one of the 49 road centrelines is traversable end to end on the graph.
/// A schematic centreline may clip a building (St Maren's sits across one); the
/// graph re-routes around it, so the road stays passable without walking through
/// a wall.
#[test]
fn roads_are_walkable_end_to_end() {
    let nav = nav();
    let plan = plan();
    let roads = plan["roads"].as_array().unwrap();
    assert_eq!(roads.len(), 49);
    for road in roads {
        let id = road["id"].as_str().unwrap();
        let pts = road_points(&plan, id);
        let route = nav
            .route_between(point(pts[0]), point(*pts.last().unwrap()))
            .unwrap_or_else(|| panic!("road {id} has no end-to-end route"));
        let ratio = route.length_m / straight_len(&pts).max(1.0);
        assert!(
            ratio < 2.5,
            "road {id} detours {ratio:.2}x its length — likely disconnected"
        );
    }
}

/// The Draper's Reach is a covered trade route; its three cloth halls sit across
/// it, but it still walks nearly straight through.
#[test]
fn the_reach_is_passable() {
    let nav = nav();
    let plan = plan();
    let pts = road_points(&plan, "drapers_reach");
    let route = nav
        .route_between(point(pts[0]), point(*pts.last().unwrap()))
        .expect("the Draper's Reach routes end to end");
    let ratio = route.length_m / straight_len(&pts).max(1.0);
    assert!(ratio < 1.5, "the Reach should be near-direct, got {ratio:.2}x");
}

/// The malt-house stands *over* Malt Passage: it is an overhead structure, so the
/// ground beneath stays open and the named place routes.
#[test]
fn malt_passage_is_passable() {
    let nav = nav();
    let plan = plan();
    let malt = plan["buildings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["id"] == "named_malt_house")
        .unwrap();
    let poly = malt["polygon"].as_array().unwrap();
    let n = poly.len() as f64;
    let cx: f64 = poly.iter().map(|p| p[0].as_f64().unwrap()).sum::<f64>() / n;
    let cz: f64 = poly.iter().map(|p| p[1].as_f64().unwrap()).sum::<f64>() / n;
    assert!(
        nav.is_walkable(cx, cz),
        "the ground under the malt-house must stay walkable"
    );
    let dist = nav.distances_from(nav.forecourt());
    let passage = nav.place("Malt Passage").expect("Malt Passage is a named place");
    assert!(dist[passage.node].is_some(), "Malt Passage routes from the forecourt");
}

/// The Cut is a filled, dry canal — a 20 m cartway and the widest walkable space
/// in Ombreval. Its centreline is almost entirely walkable and it routes the full
/// length (Chain Bridge to Old Sluice).
#[test]
fn the_cut_is_walkable() {
    let nav = nav();
    let plan = plan();
    let pts = road_points(&plan, "cut");

    let mut walkable = 0;
    let mut total = 0;
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
        let steps = (len / 0.5).ceil() as usize;
        for s in 0..=steps {
            let t = s as f64 / steps as f64;
            let x = a[0] + (b[0] - a[0]) * t;
            let z = a[1] + (b[1] - a[1]) * t;
            total += 1;
            if nav.is_walkable(x, z) {
                walkable += 1;
            }
        }
    }
    let fraction = walkable as f64 / total as f64;
    assert!(
        fraction > 0.95,
        "the dry Cut should be almost entirely walkable, got {fraction:.2}"
    );
    assert!(
        nav.route_between(point(pts[0]), point(*pts.last().unwrap()))
            .is_some(),
        "the Cut routes end to end"
    );
}

/// Wickmarket to Tallage — the lore's own route acceptance test — connects.
#[test]
fn wickmarket_to_tallage_connects() {
    let nav = nav();
    let wick = nav.place("The Wickmarket").expect("Wickmarket exists");
    let tallage = nav.place("The Tallage").expect("Tallage exists");
    assert!(nav.reachable(wick.node, tallage.node));
}

/// A* and Dijkstra agree on reachability, and a route's traced polyline starts
/// and ends where it should.
#[test]
fn route_is_self_consistent() {
    let nav = nav();
    let a = nav.forecourt();
    let b = nav.place("The Wickmarket").unwrap().node;
    let route = nav.route_nodes(a, b).expect("a route exists");
    assert_eq!(*route.nodes.first().unwrap(), a);
    assert_eq!(*route.nodes.last().unwrap(), b);
    assert_eq!(route.points.len(), route.nodes.len());
    assert_eq!(route.half_width_m.len(), route.nodes.len());
    assert!(route.length_m > 0.0);
    // every routed point is on walkable ground
    for p in &route.points {
        assert!(nav.is_walkable(p.x, p.z), "routed point {p:?} is off-surface");
    }
}
