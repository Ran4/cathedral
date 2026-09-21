use super::*;
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicUsize, Ordering},
};

fn config(recipe: &InstalledRecipe) -> InstalledConfig {
    let mut config = recipe
        .load_config_from_paths("/nonexistent/alibi-c-config.ron", "default_config.ron")
        .unwrap();
    config.smart_actors.fake_backend = true;
    config
}
fn backend(config: &crate::config::AppConfig) -> BackendsConfig {
    BackendsConfig::resolve(
        &cathedral_backends::Environment::from_map(BTreeMap::new()),
        &BackendsOptions {
            fake_mode: config.smart_actors.fake_backend,
            dotenv_path: None,
            workers_dir: "/nonexistent/alibi-workers".into(),
            uv_binary: config.smart_actors.uv_binary.clone(),
        },
    )
}

#[test]
fn embedded_navigation_constructor_and_preflight_fit_scoped_admission() {
    let (nodes, preflight_bytes) = crate::host_checkpoint::measure_installed_allocations(|| {
        serde_json::from_str::<NavNodes>(NAV_JSON).unwrap()
    });
    assert!(preflight_bytes <= CONTROL_BYTES);
    let (nav, allocated) = crate::host_checkpoint::measure_installed_allocations(|| {
        NavData::from_parts(NAV_JSON, NAV_BIN).unwrap()
    });
    assert_eq!(nodes.nodes.0, nav.node_count());
    assert!(
        allocated <= NAV_PARSE_BYTES,
        "all synchronous requested allocation bytes: {allocated}"
    );
    assert_eq!(nav.checkpoint_storage_inventory().cache_rows_retained, 0);
    println!(
        "installed_navigation constructor_requested={allocated} preflight_requested={preflight_bytes} nodes={}",
        nav.node_count()
    );
}

#[test]
fn staging_admission_failure_does_not_resolve_backends_or_change_app() {
    #[derive(Resource)]
    struct Sentinel(u64);
    let mut app = App::new();
    app.insert_resource(Sentinel(73));
    let recipe = InstalledRecipe::new().unwrap();
    let config = config(&recipe);
    let budget = recipe.budget().clone();
    let pressure = budget
        .reserve(
            Cohort::LoadCandidate,
            checkpoint::MAX_RESIDENT_BYTES - budget.retained_bytes(),
        )
        .unwrap();
    let called = AtomicUsize::new(0);
    let result = StagedStartup::prepare_with(recipe, config, |c| {
        called.fetch_add(1, Ordering::SeqCst);
        backend(c)
    });
    assert!(matches!(result, Err(StartupRefusal::Admission)));
    assert_eq!(called.load(Ordering::SeqCst), 0);
    assert_eq!(app.world().resource::<Sentinel>().0, 73);
    assert!(!app.world().contains_resource::<CommittedStartup>());
    assert_eq!(budget.retained_bytes(), pressure.bytes());
    drop(pressure);
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn committed_recipe_is_shared_across_consumers_and_duplicate_install_rolls_back() {
    let recipe = InstalledRecipe::new().unwrap();
    let config = config(&recipe);
    let budget = recipe.budget().clone();
    let called = AtomicUsize::new(0);
    let staged = StagedStartup::prepare_with(recipe, config, |c| {
        called.fetch_add(1, Ordering::SeqCst);
        backend(c)
    })
    .unwrap();
    let committed = staged.recipe().clone();
    assert!(Arc::ptr_eq(committed.budget(), &budget));
    let retained = budget.retained_bytes();
    let first = committed.backends(None, cathedral_sim::RuntimeGeneration(10));
    let second = first
        .next_generation(cathedral_sim::RuntimeGeneration(11), None)
        .unwrap();
    assert!(std::ptr::eq(first.config(), second.config()));
    assert!(Arc::ptr_eq(first.runtime(), second.runtime()));
    assert!(std::ptr::eq(
        first.config(),
        committed.0.backend_config.as_ref()
    ));
    assert_eq!(called.load(Ordering::SeqCst), 1);
    assert_eq!(
        committed.require_complete_admission(),
        Err(StartupRefusal::UnprovedWholeAppAccounting)
    );
    assert_eq!(
        committed.navigation_bytes(),
        InstalledCost::navigation([Some(committed.nav()), None])
            .total()
            .unwrap()
    );
    let mut app = App::new();
    assert!(staged.install(&mut app).is_ok());
    app.add_plugins(crate::nav_overlay::NavDebugPlugin);
    assert!(Arc::ptr_eq(
        &app.world()
            .resource::<crate::nav_overlay::Navigation>()
            .data,
        committed.nav()
    ));
    assert!(Arc::ptr_eq(
        &app.world().resource::<CommittedStartup>().0,
        &committed.0
    ));
    assert_eq!(budget.retained_bytes(), retained);
    #[cfg(target_os = "linux")]
    {
        let duplicate = StagedStartup {
            recipe: committed.clone(),
            preparation: cathedral_backends::checkpoint_preparation::CheckpointPreparation::start(
                budget.clone(),
            )
            .unwrap(),
        };
        let Err((reason, duplicate)) = duplicate.install(&mut app) else {
            panic!("duplicate installed")
        };
        assert_eq!(reason, StartupRefusal::AlreadyInstalled);
        assert!(Arc::ptr_eq(
            &app.world().resource::<CommittedStartup>().0,
            &committed.0
        ));
        duplicate
            .preparation
            .join()
            .ok()
            .expect("empty preparation joins");
    }
    // No App update/window/audio/provider. The actual consumer constructors
    // share the recipe; complete runtime replacement remains refused.
    drop(second);
    drop(first);
    #[cfg(target_os = "linux")]
    app.world_mut()
        .remove_resource::<InstalledPreparation>()
        .unwrap()
        ._worker
        .join()
        .ok()
        .expect("empty preparation joins");
    drop(app);
    assert!(budget.retained_bytes() >= committed.navigation_bytes());
    drop(committed);
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn committed_prompt_session_forks_keep_same_second_archive_order() {
    let directory =
        std::env::temp_dir().join(format!("alibi-m3b2c-prompts-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let installed = InstalledRecipe::new().unwrap();
    let config = config(&installed);
    let staged =
        StagedStartup::prepare_with_directory(installed, config, backend, Some(directory.clone()))
            .unwrap();
    let first = staged.recipe().prompt_log();
    let second = staged.recipe().prompt_log();
    let clock =
        || Box::new(|| LocalTime::from_unix_seconds(42)) as Box<dyn FnMut() -> LocalTime + Send>;
    let mut first = first.fork_with_clock(clock());
    let mut second = second.fork_with_clock(clock());
    for (log, text) in [(&mut first, "first"), (&mut second, "second")] {
        log.record(cathedral_backends::PromptExchange {
            actor_id: "m3b2c".into(),
            actor_name: "Actor".into(),
            prompt: text.into(),
            answer: Some("answer".into()),
            duration_seconds: 0.0,
            error: None,
        })
        .unwrap();
    }
    first.flush();
    second.flush();
    let mut paths: Vec<_> = std::fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();
    assert_eq!(
        paths.len(),
        2,
        "successor must not overwrite prior same-second exchange"
    );
    assert!(
        paths[0]
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .contains("__00__")
    );
    assert!(
        paths[1]
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .contains("__01__")
    );
    for (path, text) in paths.iter().zip(["first", "second"]) {
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(value["prompt"], text);
    }
    drop(first);
    drop(second);
    #[cfg(target_os = "linux")]
    staged.preparation.join().ok().expect("empty worker joins");
    std::fs::remove_dir_all(directory).unwrap();
}
