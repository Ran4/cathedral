use super::*;
use cathedral_sim::checkpoint::MAX_RESIDENT_BYTES;

fn small_nav() -> Arc<NavData> {
    Arc::new(
        NavData::from_parts(
            r#"{
      "schema_version":1,
      "grid":{"x0":0,"z0":0,"cell_m":1,"w":2,"h":2,
        "agent_radius_m":0.1,"bitset_file":"n.bin","bitset_bits":4,"bitset_sha256":""},
      "nodes":[[0,0],[1,0]],"edges":[[0,1,1]],
      "places":[],"sites":[],"doors":[],"reference":{"forecourt":0}
    }"#,
            &[15],
        )
        .unwrap(),
    )
}

#[test]
fn recipe_sources_keep_one_charge_until_last_borrowing_handle_dies() {
    let recipe = InstalledRecipe::new().unwrap();
    let budget = recipe.budget().clone();
    let sources = recipe
        .retain_sources(&[b"fixed", b"utf8: \xc3\xa5"])
        .unwrap();
    let bytes = sources.charged_bytes();
    let clone = sources.clone();
    assert_eq!(budget.retained_bytes(), CONTROL_BYTES + bytes);
    drop(sources);
    drop(recipe);
    assert_eq!(budget.retained_bytes(), bytes);
    assert_eq!(clone.source(0), Some(b"fixed".as_slice()));
    assert_eq!(clone.source(1), Some(b"utf8: \xc3\xa5".as_slice()));
    drop(clone);
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn recipe_refuses_caps_and_pressure_without_partial_retention() {
    let recipe = InstalledRecipe::new().unwrap();
    let too_large = vec![0; MAX_RECIPE_SOURCE_BYTES + 1];
    assert!(recipe.retain_sources(&[&too_large]).is_err());
    let too_many = vec![b"".as_slice(); MAX_RECIPE_SOURCES + 1];
    assert!(recipe.retain_sources(&too_many).is_err());
    let at_cap = &too_large[..MAX_RECIPE_SOURCE_BYTES];
    assert!(recipe.retain_sources(&[at_cap; 9]).is_err());
    assert_eq!(recipe.budget().retained_bytes(), CONTROL_BYTES);
    let pressure = recipe
        .budget()
        .reserve(Cohort::LoadCandidate, MAX_RESIDENT_BYTES - CONTROL_BYTES)
        .unwrap();
    assert!(recipe.retain_sources(&[b"refuse before clone"]).is_err());
    assert_eq!(recipe.budget().retained_bytes(), MAX_RESIDENT_BYTES);
    drop(pressure);
    assert!(recipe.retain_sources(&[b"capacity returned"]).is_ok());
}

#[test]
fn navigation_roles_bill_distinct_graphs_and_shared_maximum_cache_once() {
    let nav = small_nav();
    let distinct_graph_shared_cache = Arc::new((*nav).clone());
    let independent = small_nav();
    let one = InstalledCost::navigation([Some(&nav), None]);
    assert_eq!(InstalledCost::navigation([Some(&nav), Some(&nav)]), one);
    let shared = InstalledCost::navigation([Some(&nav), Some(&distinct_graph_shared_cache)]);
    // Vec::clone may compact spare capacity. Bill each actual graph rather
    // than assuming the clone retained its source's allocation capacities.
    assert_eq!(
        shared.navigation_graph_bytes,
        one.navigation_graph_bytes
            + distinct_graph_shared_cache
                .checkpoint_storage_inventory()
                .graph_and_indexes_bytes
            + 64
    );
    assert_eq!(shared.navigation_cache_bytes, one.navigation_cache_bytes);
    let separate = InstalledCost::navigation([Some(&nav), Some(&independent)]);
    assert_eq!(
        separate.navigation_cache_bytes,
        2 * one.navigation_cache_bytes
    );
    assert_eq!(nav.checkpoint_storage_inventory().cache_rows_retained, 0);
    assert_eq!(nav.cached_distance_m(0, 1), Some(1.0));
    assert_eq!(nav.checkpoint_storage_inventory().cache_rows_retained, 1);
    assert_eq!(
        InstalledCost::navigation([Some(&nav), Some(&distinct_graph_shared_cache)]),
        shared
    );
    assert_eq!(InstalledCost::default().total().unwrap(), 0);
    assert!(
        InstalledCost {
            recipe_bytes: usize::MAX,
            navigation_graph_bytes: 1,
            navigation_cache_bytes: 0
        }
        .total()
        .is_err()
    );
}

#[test]
fn installed_city_inventory_includes_full_cache_without_warming() {
    let nav = Arc::new(
        NavData::from_parts(
            include_str!("../../assets/world/navigation.json"),
            include_bytes!("../../assets/world/navigation.bin"),
        )
        .unwrap(),
    );
    let inventory = nav.checkpoint_storage_inventory();
    let cost = InstalledCost::navigation([Some(&nav), Some(&nav)]);
    assert_eq!(inventory.cache_rows_retained, 0);
    assert!(inventory.cache_maximum_bytes > inventory.cache_retained_bytes);
    assert_eq!(
        cost.navigation_graph_bytes,
        inventory.graph_and_indexes_bytes + 64
    );
    assert_eq!(cost.navigation_cache_bytes, inventory.cache_maximum_bytes);
    println!(
        "installed_nav nodes={} graph={} cache_max={} total={}",
        inventory.nodes,
        cost.navigation_graph_bytes,
        cost.navigation_cache_bytes,
        cost.total().unwrap()
    );
}

#[test]
fn startup_config_admission_precedes_read_and_oversize_uses_existing_fallback() {
    let recipe = InstalledRecipe::new().unwrap();
    let path = std::env::temp_dir().join(format!("alibi-m3b2b4-config-{}", std::process::id()));
    std::fs::create_dir(&path).unwrap();
    let override_path = path.join("override.ron");
    let default_path = path.join("default.ron");
    std::fs::write(
        &override_path,
        vec![b' '; crate::config::MAX_CONFIG_SOURCE_BYTES + 1],
    )
    .unwrap();
    std::fs::write(&default_path, "(title: \"bounded fallback\")").unwrap();
    let pressure = recipe
        .budget()
        .reserve(Cohort::LoadCandidate, MAX_RESIDENT_BYTES - CONTROL_BYTES)
        .unwrap();
    assert!(
        recipe
            .load_config_from_paths(&override_path, &default_path)
            .is_err()
    );
    drop(pressure);
    let config = recipe
        .load_config_from_paths(&override_path, &default_path)
        .unwrap();
    assert_eq!(config.title, "bounded fallback");
    assert_eq!(
        recipe.budget().retained_bytes(),
        CONTROL_BYTES + CONFIG_BYTES
    );
    drop(config);
    assert_eq!(recipe.budget().retained_bytes(), CONTROL_BYTES);
    // Exactly at the byte cap remains a supported input, including whitespace.
    let mut at_cap = b"(title: \"exact cap\")".to_vec();
    at_cap.resize(crate::config::MAX_CONFIG_SOURCE_BYTES, b' ');
    std::fs::write(&override_path, &at_cap).unwrap();
    let config = recipe
        .load_config_from_paths(&override_path, &default_path)
        .unwrap();
    assert_eq!(config.title, "exact cap");
    std::fs::remove_dir_all(path).unwrap();
}
