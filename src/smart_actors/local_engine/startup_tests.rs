use super::*;
use crate::installed_recipe::{InstalledRecipe, StagedStartup};
use std::collections::BTreeMap;

#[test]
fn committed_startup_reuses_navigation_and_backend_recipe_through_hello() {
    let installed = InstalledRecipe::new().unwrap();
    let mut config = installed
        .load_config_from_paths("/nonexistent/alibi-startup-config", "default_config.ron")
        .unwrap();
    config.smart_actors.fake_backend = true;
    config.smart_actors.tts_backend = "off".into();
    let staged = StagedStartup::prepare_with(installed, config, |c| {
        BackendsConfig::resolve(
            &cathedral_backends::Environment::from_map(BTreeMap::new()),
            &BackendsOptions {
                fake_mode: c.smart_actors.fake_backend,
                dotenv_path: None,
                workers_dir: "/nonexistent/workers".into(),
                uv_binary: c.smart_actors.uv_binary.clone(),
            },
        )
    })
    .unwrap();
    let committed = staged.recipe().clone();
    let (handle, inbox, guard, mut engine) = spawn_installed(
        &committed.config().smart_actors,
        &committed.config().weather,
        Some(committed.clone()),
    );
    assert!(!engine.dead);
    assert!(Arc::ptr_eq(
        engine.seed.as_ref().unwrap().config.nav.as_ref().unwrap(),
        committed.nav()
    ));
    let other = committed.backends(None, cathedral_sim::RuntimeGeneration(900));
    assert!(std::ptr::eq(
        guard._backends.as_ref().unwrap().config(),
        other.config()
    ));
    assert!(Arc::ptr_eq(
        guard._backends.as_ref().unwrap().runtime(),
        other.runtime()
    ));
    handle
        .try_send(BridgeCommand::Hello {
            position_m: Position {
                x: 0.0,
                y: 0.91,
                z: 0.0,
            },
            spatial_seq: 1,
        })
        .unwrap();
    engine.pump(0.0);
    let domain = engine.checkpoint_engine().unwrap();
    assert!(Arc::ptr_eq(
        domain.config().nav.as_ref().unwrap(),
        committed.nav()
    ));
    assert!(Arc::ptr_eq(
        domain.world().nav.as_ref().unwrap(),
        committed.nav()
    ));
    assert_eq!(
        committed.require_complete_admission(),
        Err(crate::installed_recipe::StartupRefusal::UnprovedWholeAppAccounting)
    );
    drop(other);
    drop(handle);
    drop(inbox);
    drop(engine);
    drop(guard);
    drop(committed);
    // staged still owns the shared definitions; disposal is off-frame.
    drop(staged);
}

#[test]
fn installed_engine_build_uses_captured_sources_after_original_files_disappear() {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "cathedral-installed-source-routing-{}-{}", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
    )));
    let assets = fixture.0.join("assets");
    let lore = fixture.0.join("lore");
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    for relative in [
        "world/seed.json",
        "world/areas.json",
        "sounds/catalog.toml",
        "prompts/turn.j2",
        "prompts/night.j2",
        "prompts/strings.toml",
    ] {
        let target = assets.join(relative);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::copy(repository.join("assets").join(relative), target).unwrap();
    }
    std::fs::create_dir_all(lore.join("core_lore")).unwrap();
    std::fs::copy(
        repository.join("lore/core_lore/occupations.json"),
        lore.join("core_lore/occupations.json"),
    )
    .unwrap();
    for (relative, text) in
        cathedral_backends::world_data::character_sources(&repository.join("lore")).unwrap()
    {
        let target = lore.join("characters").join(relative);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, text).unwrap();
    }
    let seed_path = assets.join("world/seed.json");
    let mut seed: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&seed_path).unwrap()).unwrap();
    seed["characters"][0]["name"] = serde_json::json!("Frozen source routing witness");
    std::fs::write(&seed_path, serde_json::to_vec(&seed).unwrap()).unwrap();
    let installed = InstalledRecipe::new().unwrap();
    let mut config = installed
        .load_config_from_paths(
            "/nonexistent/cathedral-source-routing-config",
            "default_config.ron",
        )
        .unwrap();
    config.smart_actors.fake_backend = true;
    config.smart_actors.tts_backend = "off".into();
    let staged = StagedStartup::prepare_with_source_roots(
        installed,
        config,
        |c| {
            BackendsConfig::resolve(
                &cathedral_backends::Environment::from_map(BTreeMap::new()),
                &BackendsOptions {
                    fake_mode: c.smart_actors.fake_backend,
                    dotenv_path: None,
                    workers_dir: "/nonexistent/workers".into(),
                    uv_binary: c.smart_actors.uv_binary.clone(),
                },
            )
        },
        &assets,
        &lore,
    )
    .unwrap();
    let committed = staged.recipe().clone();
    std::fs::remove_dir_all(&fixture.0).unwrap();
    let built = build_installed(
        &committed.config().smart_actors,
        &committed.config().weather,
        None,
        cathedral_sim::RuntimeGeneration(901),
        Some(&committed),
    )
    .unwrap();
    let player = built
        .0
        .seed
        .characters
        .iter()
        .find(|c| c.id.as_str() == PLAYER_ID)
        .unwrap();
    assert_eq!(player.name, "Frozen source routing witness");
    assert_eq!(
        committed.require_complete_admission(),
        Err(crate::installed_recipe::StartupRefusal::UnprovedWholeAppAccounting)
    );
    drop(built);
    drop(committed);
    drop(staged);
}
