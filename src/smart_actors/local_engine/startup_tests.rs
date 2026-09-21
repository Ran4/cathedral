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
