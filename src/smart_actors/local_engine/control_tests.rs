use super::*;
use crate::{
    checkpoint_controls::{CheckpointAction, CheckpointControls, ControlRefusal},
    installed_recipe::{InstalledRecipe, StagedStartup},
};
use std::collections::BTreeMap;

#[test]
fn control_refusal_preserves_live_commands_and_cannot_roll_back_retirement() {
    let installed = InstalledRecipe::new().unwrap();
    let mut config = installed
        .load_config_from_paths("/nonexistent/alibi-m3c-config", "default_config.ron")
        .unwrap();
    config.smart_actors.fake_backend = true;
    config.smart_actors.tts_backend = "off".into();
    let staged = StagedStartup::prepare_with(installed, config, |c| {
        BackendsConfig::resolve(
            &cathedral_backends::Environment::from_map(BTreeMap::new()),
            &BackendsOptions {
                fake_mode: true,
                dotenv_path: None,
                workers_dir: "/nonexistent/workers".into(),
                uv_binary: c.smart_actors.uv_binary.clone(),
            },
        )
    })
    .unwrap();
    let committed = staged.recipe().clone();
    let mut controls = CheckpointControls::admitted(committed.budget()).unwrap();
    let (handle, inbox, guard, mut engine) = spawn_installed(
        &committed.config().smart_actors,
        &committed.config().weather,
        Some(committed.clone()),
    );
    let generation = handle.generation();
    let position = Position {
        x: 0.0,
        y: 0.91,
        z: 0.0,
    };
    handle
        .try_send(BridgeCommand::Hello {
            position_m: position,
            spatial_seq: 1,
        })
        .unwrap();
    engine.pump(0.0);
    let callback = handle.command_sender();
    let witness = |engine: &LocalEngine| {
        let world = engine.checkpoint_engine().unwrap().world();
        (
            world.world_revision,
            world.event_sequence,
            world.spatial_sequence,
            engine.input_watermark,
            engine.commands.len(),
            engine.events.len(),
        )
    };
    handle
        .try_send(BridgeCommand::PlayerSay {
            request_id: "after-save-refusal".into(),
            text: "The street remains open.".into(),
            position_m: position,
            spatial_seq: 2,
        })
        .unwrap();
    let before = witness(&engine);
    let issued = handle.checkpoint_issued().unwrap();
    let bytes = committed.budget().retained_bytes();
    for action in [CheckpointAction::QuickSave, CheckpointAction::QuickLoad] {
        let receipt = controls
            .request(
                action,
                Some(generation),
                Some(&handle),
                Some(&engine),
                Some(&committed),
            )
            .unwrap();
        assert_eq!(receipt.refusal, ControlRefusal::IncompleteAdmission);
        assert_eq!(witness(&engine), before);
        assert_eq!(handle.checkpoint_issued().unwrap(), issued);
        assert_eq!(committed.budget().retained_bytes(), bytes);
    }
    let stale = cathedral_sim::RuntimeGeneration(generation.0 - 1);
    assert_eq!(
        controls
            .request(
                CheckpointAction::QuickLoad,
                Some(stale),
                Some(&handle),
                Some(&engine),
                Some(&committed)
            )
            .unwrap()
            .refusal,
        ControlRefusal::StaleGeneration
    );
    // A same-number endpoint is not the engine's actual control identity.
    let (tx, _rx) = bounded(1);
    let impostor = BridgeHandle::new_for_generation(tx, "/tmp".into(), generation);
    assert_eq!(
        controls
            .request(
                CheckpointAction::QuickLoad,
                Some(generation),
                Some(&impostor),
                Some(&engine),
                Some(&committed)
            )
            .unwrap()
            .refusal,
        ControlRefusal::WorldOwnerMismatch
    );
    assert_eq!(witness(&engine), before);
    engine.pump(0.017);
    assert_eq!(engine.commands.len(), 0);
    assert_eq!(engine.input_watermark, before.3 + 1);
    assert!(engine.checkpoint_engine().unwrap().world().event_sequence > before.1);
    // Real runtime retirement closes both ECS and worker input. A refused
    // load never reverses that fence, even when its receipt has the same ID.
    engine.retire_runtime();
    assert_eq!(
        controls
            .request(
                CheckpointAction::QuickLoad,
                Some(generation),
                Some(&handle),
                Some(&engine),
                Some(&committed)
            )
            .unwrap()
            .refusal,
        ControlRefusal::NoActiveWorld
    );
    assert!(
        callback
            .try_send(BridgeCommand::SpeechPresented {
                speech_event_id: "old".into()
            })
            .is_err()
    );
    assert!(handle.try_send(BridgeCommand::PlayerStruggling).is_err());
    assert_eq!(
        handle.control_route(),
        super::super::bridge::ControlRoute::Retiring
    );
    drop((
        impostor, callback, handle, inbox, engine, guard, controls, committed, staged,
    ));
}
