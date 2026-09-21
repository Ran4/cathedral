use super::*;

fn nav() -> NavData {
    let doc = serde_json::json!({"schema_version":1,
        "grid":{"x0":0.,"z0":0.,"cell_m":0.25,"w":40,"h":40,"agent_radius_m":0.35,"bitset_file":"","bitset_bits":1600,"bitset_sha256":""},
        "nodes":[[1.,1.],[5.,1.],[1.,5.],[5.,5.]],"edges":[[0,1,1.],[0,2,1.],[2,3,1.],[3,1,1.]],"places":[],"sites":[],"doors":[],"reference":{"forecourt":0}});
    NavData::from_parts(&doc.to_string(), &[255; 200]).unwrap()
}

#[test]
fn temporary_closure_reroutes_then_reopens_without_changing_baked_graph() {
    let nav = nav();
    let ordinary = nav.route_nodes(0, 1).unwrap();
    let open = StreetQuery::new(&nav, &[], 4).unwrap();
    assert_eq!(
        open.route_nodes(Surface::Street, 0, 1)
            .unwrap()
            .route(&open)
            .unwrap(),
        &ordinary
    );
    let closures = [Closure {
        id: 7,
        min: [2., 0.],
        max: [3., 2.],
    }];
    let closed = StreetQuery::new(&nav, &closures, 5).unwrap();
    let detour = closed.route_nodes(Surface::Street, 0, 1).unwrap();
    assert_eq!(detour.route(&closed).unwrap().nodes, [0, 2, 3, 1]);
    assert!(detour.route(&closed).unwrap().points.windows(2).all(|w| {
        closed
            .segment_available(Surface::Street, w[0], w[1])
            .is_ok()
    }));
    assert_eq!(detour.route(&open), Err(QueryError::Stale));
    assert_eq!(nav.route_nodes(0, 1).unwrap(), ordinary);
    let reopened = StreetQuery::new(&nav, &[], 6).unwrap();
    assert_eq!(
        reopened
            .route_nodes(Surface::Street, 0, 1)
            .unwrap()
            .route(&reopened)
            .unwrap(),
        &ordinary
    );
}

#[test]
fn route_receipt_checks_graph_revision_and_actual_closure_contents() {
    let nav = nav();
    let closures = [Closure {
        id: 1,
        min: [7., 7.],
        max: [8., 8.],
    }];
    let query = StreetQuery::new(&nav, &closures, 1).unwrap();
    let route = query.route_nodes(Surface::Street, 0, 1).unwrap();
    let changed = [Closure {
        id: 1,
        min: [2., 0.],
        max: [3., 2.],
    }];
    assert_eq!(
        route.route(&StreetQuery::new(&nav, &changed, 1).unwrap()),
        Err(QueryError::Stale)
    );
    assert_eq!(
        route.route(&StreetQuery::new(&nav, &closures, 2).unwrap()),
        Err(QueryError::Stale)
    );
    let other = super::super::local_tests::surface(&[]);
    assert_eq!(
        route.route(&StreetQuery::new(&other, &closures, 1).unwrap()),
        Err(QueryError::Stale)
    );
}

#[test]
fn surface_capacity_and_endpoint_refusals_are_explicit() {
    let nav = nav();
    let query = StreetQuery::new(&nav, &[], 0).unwrap();
    assert!(matches!(
        query.route_nodes(Surface::Interior(2), 0, 1),
        Err(QueryError::UnsupportedSurface)
    ));
    assert!(matches!(
        query.route_between(Surface::Street, Vec3::new(1., 4., 1.), nav.node_point(1)),
        Err(QueryError::InvalidPoint)
    ));
    assert!(matches!(
        query.route_nodes(Surface::Street, usize::MAX, 1),
        Err(QueryError::InvalidPoint)
    ));
    let all = [Closure {
        id: 1,
        min: [0., 0.],
        max: [10., 10.],
    }];
    assert!(matches!(
        StreetQuery::new(&nav, &all, 0)
            .unwrap()
            .route_nodes(Surface::Street, 0, 1),
        Err(QueryError::Unavailable)
    ));
    let duplicate = [all[0]; 2];
    assert!(matches!(
        StreetQuery::new(&nav, &duplicate, 0),
        Err(QueryError::InvalidClosures)
    ));
    let too_many = [all[0]; 33];
    assert!(matches!(
        StreetQuery::new(&nav, &too_many, 0),
        Err(QueryError::Capacity)
    ));
    assert!(matches!(
        nav.route_nodes_filtered(0, 1, None, Some(1), |_, _| true),
        Err(QueryError::Capacity)
    ));
}

#[test]
fn closure_edges_and_zero_length_segments_do_not_leak_through() {
    let closure = Closure {
        id: 1,
        min: [2., 2.],
        max: [3., 3.],
    };
    let p = |x, z| Vec3::new(x, WALK_Y, z);
    assert!(closure.intersects(p(1., 2.), p(4., 2.)));
    assert!(closure.intersects(p(2., 2.), p(2., 2.)));
    assert!(!closure.intersects(p(1., 1.), p(1., 4.)));
    assert!(closure.intersects(p(2.5, 2.5), p(2.5, 2.5)));
}

#[test]
fn unsupported_graph_envelopes_refuse_before_search_or_segment_scratch() {
    let mut nav = nav();
    nav.grid.w = MAX_GRID_SPAN;
    assert!(matches!(
        StreetQuery::new(&nav, &[], 0),
        Err(QueryError::Capacity)
    ));
    nav.grid.w = 40;
    let edge = nav.adjacency[0][0].clone();
    nav.adjacency[0].resize(MAX_QUERY_ADJACENCY + 1, edge);
    assert!(matches!(
        StreetQuery::new(&nav, &[], 0),
        Err(QueryError::Capacity)
    ));
}

#[test]
fn ordinary_city_routes_preserve_baked_ties_and_costs() {
    let nav = NavData::from_parts(
        include_str!("../../../../../assets/world/navigation.json"),
        include_bytes!("../../../../../assets/world/navigation.bin"),
    )
    .unwrap();
    let query = StreetQuery::new(&nav, &[], 0).unwrap();
    for destination in nav.places() {
        let plain = nav.route_nodes(nav.forecourt(), destination.node).unwrap();
        let checked = query
            .route_nodes(Surface::Street, nav.forecourt(), destination.node)
            .unwrap();
        assert_eq!(
            checked.route(&query).unwrap(),
            &plain,
            "{}",
            destination.name
        );
    }
}
