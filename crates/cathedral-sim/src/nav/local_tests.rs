use super::residents::*;
use super::*;
use crate::ActorId;
use serde_json::json;

pub(crate) fn surface(blocked: &[(usize, usize)]) -> NavData {
    let (w, h) = (40, 40);
    let mut bits = vec![255; w * h / 8];
    for &(r, c) in blocked {
        let i = r * w + c;
        bits[i / 8] &= !(1 << (7 - i % 8));
    }
    let doc = json!({"schema_version":1,
        "grid":{"x0":0.,"z0":0.,"cell_m":0.25,"w":w,"h":h,"agent_radius_m":0.35,"bitset_file":"","bitset_bits":w*h,"bitset_sha256":""},
        "nodes":[[0.625,0.625]],"edges":[],"places":[],"sites":[],"doors":[],"reference":{"forecourt":0}});
    NavData::from_parts(&doc.to_string(), &bits).unwrap()
}
fn p(x: f64, z: f64) -> Vec3 {
    Vec3::new(x, WALK_Y, z)
}

#[test]
fn measured_lanes_and_corner_connections_are_exactly_walkable() {
    let nav = NavData::from_parts(
        include_str!("../../../../assets/world/navigation.json"),
        include_bytes!("../../../../assets/world/navigation.bin"),
    )
    .unwrap();
    let mut checked = 0;
    let mut widest_spread = 0.0_f64;
    for a in 0..nav.node_count() {
        for edge in &nav.adjacency[a] {
            let b = edge.to;
            if a > b {
                continue;
            }
            assert!(
                nav.segment_walkable_exact(nav.node_point(a), nav.node_point(b)),
                "centreline {a}-{b}"
            );
            let route = nav.route_from_nodes(vec![a, b]);
            let narrow = nav.offset_route(&route, 0.1);
            let wide = nav.offset_route(&route, 0.7);
            widest_spread = widest_spread.max(narrow[0].distance(wide[0]));
            for lane in [-1.0, -0.7, -0.1, 0.0, 0.1, 0.7, 1.0] {
                let path = nav.offset_route(&route, lane);
                assert!(!path.is_empty());
                assert!(nav.segment_walkable_exact(nav.node_point(a), path[0]));
                assert!(
                    path.windows(2)
                        .all(|p| nav.segment_walkable_exact(p[0], p[1]))
                );
                for next in &nav.adjacency[b] {
                    if next.to == a {
                        continue;
                    }
                    let corner = nav.route_from_nodes(vec![a, b, next.to]);
                    let path = nav.offset_route(&corner, lane);
                    assert!(!path.is_empty());
                    assert!(
                        path.windows(2)
                            .all(|p| nav.segment_walkable_exact(p[0], p[1]))
                    );
                }
                checked += 1;
            }
        }
    }
    println!(
        "measured lane cases={checked}, widest 0.1–0.7 same-direction spread={widest_spread:.3}m"
    );
    assert!(
        widest_spread > 1.5,
        "open ground admits visibly different tracks"
    );
}

#[test]
fn initial_correction_is_bounded_clear_and_respects_existing_bodies() {
    let nav = surface(&[(8, 8), (8, 9), (9, 8), (9, 9)]);
    let blocked = p(2.25, 2.25);
    assert!(!nav.segment_walkable_exact(blocked, blocked));
    let first = nav.initial_surface_position(blocked, 2.0, &[]).unwrap();
    let second = nav
        .initial_surface_position(blocked, 2.0, &[first])
        .unwrap();
    assert!(first.distance(second) >= 1.2);
    assert!(second.distance(blocked) <= 2.0);
    assert!(nav.segment_walkable_exact(second, second));
    let target = p(5., 5.);
    let route = nav
        .local_route(second, target, LocalPathBudget::VISIT)
        .unwrap();
    assert!(
        route
            .points
            .windows(2)
            .all(|p| nav.segment_walkable_exact(p[0], p[1]))
    );
    assert!(nav.initial_surface_position(blocked, 0.1, &[]).is_none());
    assert!(nav.initial_surface_position(blocked, 41.0, &[]).is_none());
}
fn check_path(nav: &NavData, route: &LocalRoute, from: Vec3, to: Vec3, limit: f64) {
    assert_eq!(route.points.first(), Some(&from));
    assert_eq!(route.points.last(), Some(&to));
    assert!(route.length_m <= limit);
    for segment in route.points.windows(2) {
        assert!(nav.segment_walkable_exact(segment[0], segment[1]));
    }
}

#[test]
fn local_navigation_exact_open_square_and_off_graph_corner_route() {
    let open = surface(&[]);
    let (from, to) = (p(2.13, 1.13), p(5.71, 1.19));
    let direct = open
        .local_route(from, to, LocalPathBudget::CHANGE_SPOT)
        .unwrap();
    assert_eq!(direct.points, vec![from, to]);
    assert_eq!(direct.expanded_cells, 0);
    let wall: Vec<_> = (0..12).map(|r| (r, 16)).collect();
    let nav = surface(&wall);
    assert!(!nav.segment_walkable_exact(from, to));
    let route = nav
        .local_route(from, to, LocalPathBudget::CHANGE_SPOT)
        .unwrap();
    assert!(route.points.len() > 2);
    assert!(route.points.iter().any(|p| p.z >= 3.));
    assert!(route.expanded_cells > 0 && route.expanded_cells <= 4096);
    check_path(&nav, &route, from, to, 15.);
    assert_eq!(
        route,
        nav.local_route(from, to, LocalPathBudget::CHANGE_SPOT)
            .unwrap()
    );
    assert!(
        nav.local_route(
            from,
            to,
            LocalPathBudget {
                max_distance_m: 5.,
                max_expansions: 4096
            }
        )
        .is_none()
    );
    assert!(
        nav.local_route(
            from,
            to,
            LocalPathBudget {
                max_distance_m: 15.,
                max_expansions: 1
            }
        )
        .is_none()
    );
    let movement = route.into_movement(2.);
    assert!(movement.exact_local);
    assert_eq!(movement.path.last(), Some(&to));
}

#[test]
fn local_navigation_checks_corner_grazes_boundaries_and_short_clips() {
    let nav = surface(&[(1, 1)]);
    // The segment touches a blocked corner without ever entering its interior.
    assert!(!nav.segment_walkable_exact(p(0.125, 0.375), p(0.375, 0.125)));
    assert!(!nav.segment_walkable_exact(p(0.25, 0.125), p(0.25, 0.875)));
    assert!(!nav.segment_walkable_exact(p(0.249, 0.499), p(0.499, 0.249)));
    assert!(!nav.segment_walkable_exact(p(f64::NAN, 1.), p(1., 1.)));
    // Reverse direction must test precisely the same closed supercover.
    assert!(!nav.segment_walkable_exact(p(0.375, 0.125), p(0.125, 0.375)));
    assert!(nav.segment_walkable_exact(p(0.125, 0.125), p(0.125, 0.875)));
}

#[test]
fn local_navigation_rejects_unreachable_and_keeps_narrow_door_approach_safe() {
    let wall: Vec<_> = (0..40).map(|r| (r, 16)).collect();
    let nav = surface(&wall);
    assert!(
        nav.local_route(p(2., 2.), p(6., 2.), LocalPathBudget::VISIT)
            .is_none()
    );
    let passage: Vec<_> = (0..40)
        .flat_map(|r| {
            (0..40)
                .filter(move |c| !(18..=21).contains(c))
                .map(move |c| (r, c))
        })
        .collect();
    let nav = surface(&passage);
    let (from, to) = (p(5.1, 1.2), p(5.1, 8.8));
    let route = nav
        .local_route(from, to, LocalPathBudget::CHANGE_SPOT)
        .unwrap();
    check_path(&nav, &route, from, to, 15.);
    assert!(
        nav.local_route(from, p(4.3, 8.8), LocalPathBudget::CHANGE_SPOT)
            .is_none()
    );
    assert!(
        nav.local_route(
            from,
            to,
            LocalPathBudget {
                max_distance_m: 31.,
                max_expansions: 8192
            }
        )
        .is_none()
    );
}

#[test]
fn resident_bake_capacity_connections_spacing_and_optional_metadata() {
    let nav = NavData::from_parts(
        include_str!("../../../../assets/world/navigation.json"),
        include_bytes!("../../../../assets/world/navigation.bin"),
    )
    .unwrap();
    let places = nav.resident_places();
    assert!(places.capacity() >= 2000, "capacity {}", places.capacity());
    assert!(places.patches.len() >= 500);
    assert!(
        places
            .patches
            .iter()
            .filter(|p| p.home_building.is_some())
            .map(|p| p.capacity)
            .sum::<usize>()
            >= 1500
    );
    let reach = nav.distances_from(nav.forecourt());
    for patch in &places.patches {
        assert!(reach[patch.graph_node].is_some());
        for spot in &patch.spots {
            for door in nav.doors() {
                assert!(
                    spot.position().distance(nav.node_point(door.node)) >= 2.0,
                    "{} obstructs {}",
                    spot.id,
                    door.building
                );
            }
            assert!(
                nav.local_route(
                    spot.position(),
                    patch.spots[0].position(),
                    LocalPathBudget::CHANGE_SPOT
                )
                .is_some()
            );
        }
    }
    assert_eq!(
        surface(&[]).resident_places().capacity(),
        0,
        "old tiny graph schema remains valid"
    );
    // Allocate 1,000 then 2,000 without a single duplicate, and report the real
    // finite capacity rather than reusing occupied coordinates on overflow.
    for count in [1000, 2000] {
        let mut claims = SpotReservations::new(places);
        let slots: Vec<_> = places
            .patches
            .iter()
            .flat_map(|p| &p.spots)
            .take(count)
            .collect();
        for (i, spot) in slots.iter().enumerate() {
            assert!(claims.occupy(
                &ActorId::from_raw(format!("x{i:05}")),
                &spot.id,
                spot.position()
            ));
        }
        assert_eq!(claims.occupied_count(), count);
        assert!(!claims.occupy(
            &ActorId::from_raw("excess"),
            &slots[0].id,
            slots[0].position()
        ));
    }
}

#[test]
fn resident_reservations_are_atomic_bounded_and_release_on_failure_or_departure() {
    let places = ResidentPlaces {
        shelter_spots: vec![],
        patches: vec![ResidentPatch {
            id: "patch".into(),
            building: "house".into(),
            description: "a frontage".into(),
            capacity: 3,
            spots: (0..3)
                .map(|i| StandingSpot {
                    id: format!("s{i}"),
                    xz: [1. + 2. * i as f64, 1.],
                    facing_yaw: 0.,
                })
                .collect(),
            graph_node: 0,
            graph_path: vec![],
            home_building: None,
            home_path: vec![],
        }],
    };
    let mut claims = SpotReservations::new(&places);
    let a = ActorId::from_raw("a");
    let b = ActorId::from_raw("b");
    assert!(claims.occupy(&a, "s0", p(1., 1.)));
    assert!(claims.occupy(&b, "s2", p(5., 1.)));
    assert!(!claims.occupy(&a, "s1", p(3., 1.)));
    assert!(claims.reserve_destination(&a, "s1"));
    assert!(!claims.reserve_destination(&b, "s1"));
    assert!(!claims.reserve_destination(&a, "s2"));
    assert!(!claims.release_departed(&a, p(1.1, 1.)));
    assert!(!claims.settle(&a, p(1.1, 1.)));
    claims.cancel_destination(&a);
    assert!(claims.is_available("s1"));
    assert_eq!(claims.owner("s0"), Some(&a));
    assert!(claims.reserve_destination(&a, "s1"));
    assert!(claims.release_departed(&a, p(2., 1.)));
    assert!(claims.is_available("s0"));
    // Final avoidance displacement does not require correcting back to exact xz.
    assert!(claims.settle(&a, p(3.04, 1.03)));
    assert_eq!(claims.claims(&a).unwrap().occupied.as_deref(), Some("s1"));
    assert_eq!(claims.destination_count(), 0);
    assert!(claims.reserve_destination(&a, "s0"));
    claims.release_actor(&a); // custody, leaving town, or actor removal
    claims.release_actor(&a); // idempotent
    assert!(claims.is_available("s0") && claims.is_available("s1"));
    assert_eq!(claims.owner("s2"), Some(&b));
    assert!(!claims.reserve_destination(&a, "missing"));
    assert!(claims.occupy(&a, "s0", p(1., 1.)));
    assert!(claims.reserve_destination(&a, "s0"));
    assert!(claims.release_departed(&a, p(2., 1.)));
    assert_eq!(
        claims.owner("s0"),
        Some(&a),
        "an origin reused as a destination stays reserved on a detour"
    );
    claims.cancel_destination(&a);
    assert!(claims.is_available("s0"));
}
