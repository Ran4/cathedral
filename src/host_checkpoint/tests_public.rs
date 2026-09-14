// Coordinator-owned independent tests. Production owner leaves this file alone.
//! Coordinator-owned tests at the real ordinary host capture set.
//! This small host fixture deliberately has one flat collider and no city
//! render geometry. It proves the application boundary, not a full-city cost.

use super::{HostCaptureSet, HostObservation};
use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
    time::TimeUpdateStrategy,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use cathedral_sim::checkpoint::{
    CheckpointBudget, Cohort, MAX_RESIDENT_BYTES,
    host::{
        HostCheckpointSource, HostDtoV1, RecordRef, RecordV1, ScalarsV1, checkpoint_host_cost,
        export_host_checkpoint,
    },
};
use serde_json::{Value, json};
use std::{cell::Cell, sync::Arc, time::Duration};

use crate::smart_actors::{
    SmartActorRuntime, SmartActorsConfig, SmartActorsPlugin,
    bridge::{BridgeCommand, BridgeHandle},
    interaction::{MicrophoneInputState, PlayerSpatialState},
    local_engine::LocalEngine,
    model::Position,
};

#[derive(Clone, Copy)]
enum Case {
    Repeat,
    Fields,
    Aliases,
    Padding,
    WorkerAfterBoundary,
    UnreadInputsAndSpeech,
    MutableSource,
    Binding,
    SavedContext,
}

#[derive(Resource, Default)]
struct CaptureProbe {
    requested: Option<Case>,
    done: bool,
    bytes: Vec<u8>,
}

pub(super) fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin))
        .init_asset::<Mesh>()
        .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
        .init_asset::<StandardMaterial>()
        .init_asset::<Image>()
        .init_asset::<AudioSource>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<AccumulatedMouseScroll>()
        .init_resource::<AccumulatedMouseMotion>()
        .init_resource::<crate::map::MapState>()
        .init_resource::<crate::city::marks::ChalkHold>()
        .init_resource::<crate::city::marks::ChalkChoice>()
        .init_resource::<CaptureProbe>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            17,
        )));
    // No WindowPlugin, renderer or AudioPlugin is installed. This is only the
    // ordinary input owner's cursor resource, never an operating-system window.
    app.world_mut().spawn((
        PrimaryWindow,
        CursorOptions {
            grab_mode: CursorGrabMode::Locked,
            ..default()
        },
    ));
    app.add_plugins((
        crate::controller::ControllerPlugin,
        crate::soundscape::SoundscapePlugin,
        SmartActorsPlugin::new(SmartActorsConfig {
            enabled: true,
            fake_backend: true,
            tts_backend: "off".into(),
            ..SmartActorsConfig::default()
        }),
    ));
    app.world_mut()
        .resource_mut::<MicrophoneInputState>()
        .enabled = false;
    app.world_mut()
        .resource_mut::<crate::controller::CollisionWorld>()
        .add_box(
            Vec3::new(-1500.0, -1.0, -1500.0),
            Vec3::new(1500.0, 0.0, 1500.0),
        );
    app.add_systems(PostUpdate, capture.in_set(HostCaptureSet));
    for _ in 0..16 {
        app.update();
        if app.world().resource::<SmartActorRuntime>().ready {
            return app;
        }
    }
    panic!("ordinary fake host never reached ready");
}

fn run(app: &mut App, case: Case) -> Value {
    app.world_mut().resource_mut::<CaptureProbe>().requested = Some(case);
    app.update();
    let probe = app.world().resource::<CaptureProbe>();
    assert!(probe.done, "the ordinary capture set must execute");
    serde_json::from_slice(&probe.bytes).unwrap()
}

pub(super) fn simulation_stamp(world: &World) -> (i64, i64, i64, usize) {
    let sim = world.non_send::<LocalEngine>().world().unwrap();
    (
        sim.world_revision,
        sim.event_sequence,
        sim.spatial_sequence,
        sim.characters.len(),
    )
}

fn capture(world: &mut World) {
    let Some(case) = world.resource_mut::<CaptureProbe>().requested.take() else {
        return;
    };
    if matches!(case, Case::WorkerAfterBoundary) {
        let sender = world.resource::<BridgeHandle>().command_sender();
        for seq in 0..4 {
            sender
                .try_send(BridgeCommand::PlayerAudioChunk {
                    wav_basename: "later-old-timeline.wav".into(),
                    seq,
                    samples: Arc::from([0_i16; 4]),
                })
                .unwrap();
        }
    }
    let before = simulation_stamp(world);
    let source = HostObservation::new(world).expect("ordinary capture observation");
    let budget = CheckpointBudget::default();
    let cost = checkpoint_host_cost(&source).unwrap();
    let export = || {
        export_host_checkpoint(
            &source,
            source.context().unwrap(),
            budget.reserve(Cohort::SavePayload, 4096).unwrap(),
        )
        .unwrap()
        .encode()
        .unwrap()
    };
    let saved = export();
    let raw = saved.value();
    let value: Value = serde_json::from_slice(raw).unwrap();
    let reject = |candidate: &Value, label: &str| {
        let bytes = serde_json::to_vec(candidate).unwrap();
        let previous = budget.retained_bytes();
        let result = HostDtoV1::decode(
            &bytes,
            source.context().unwrap(),
            budget
                .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
                .unwrap(),
        );
        assert!(result.is_err(), "accepted malformed host field: {label}");
        assert_eq!(
            budget.retained_bytes(),
            previous,
            "failed decode retained its lease"
        );
    };
    match case {
        Case::SavedContext => saved_component_context(&source, raw),
        Case::Repeat | Case::WorkerAfterBoundary | Case::UnreadInputsAndSpeech => {
            let admitted = HostDtoV1::decode(
                raw,
                source.context().unwrap(),
                budget
                    .reserve(Cohort::LoadCandidate, raw.len() + 4096)
                    .unwrap(),
            )
            .unwrap()
            .into_candidate(source.context().unwrap())
            .unwrap();
            assert!(!admitted.value().records().is_empty());
            drop(admitted);
        }
        Case::Fields => {
            let mut paths = Vec::new();
            object_paths(&value, "", &mut paths);
            let mut removed = 0;
            for path in paths {
                let object = value.pointer(&path).unwrap().as_object().unwrap();
                let mut unknown = value.clone();
                unknown
                    .pointer_mut(&path)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .insert("__not_a_host_field".into(), json!(true));
                reject(&unknown, &format!("unknown at {path}"));
                for key in object.keys() {
                    let mut missing = value.clone();
                    missing
                        .pointer_mut(&path)
                        .unwrap()
                        .as_object_mut()
                        .unwrap()
                        .remove(key);
                    reject(&missing, &format!("missing {path}/{key}"));
                    removed += 1;
                }
            }
            assert!(
                removed > 70,
                "exercise the nested actual host record, including nullable fields"
            );
        }
        Case::Aliases => {
            let mut aliases = 0;
            for pointer in [
                "/scalars/clock/office",
                "/scalars/clock/weekday",
                "/scalars/spatial/last_background_send",
            ] {
                if let Some(spelling) = value.pointer(pointer).and_then(Value::as_str) {
                    let mut changed = value.clone();
                    *changed.pointer_mut(pointer).unwrap() = json!({spelling: null});
                    reject(&changed, pointer);
                    aliases += 1;
                }
            }
            for (index, row) in value["records"].as_array().unwrap().iter().enumerate() {
                if let Some(spelling) = row.pointer("/data/slot").and_then(Value::as_str) {
                    let pointer = format!("/records/{index}/data/slot");
                    let mut changed = value.clone();
                    *changed.pointer_mut(&pointer).unwrap() = json!({spelling: null});
                    reject(&changed, &pointer);
                    aliases += 1;
                }
            }
            assert!(aliases >= 2);
            let original = std::str::from_utf8(raw).unwrap();
            for key in ["version", "input_watermark", "chat_open"] {
                let needle = format!("\"{key}\":");
                let replacement = format!("{needle}null,{needle}");
                let duplicate = original.replacen(&needle, &replacement, 1);
                assert_ne!(duplicate, original);
                assert!(
                    HostDtoV1::decode(
                        duplicate.as_bytes(),
                        source.context().unwrap(),
                        budget
                            .reserve(Cohort::LoadCandidate, duplicate.len() + 4096)
                            .unwrap(),
                    )
                    .is_err(),
                    "duplicate {key}"
                );
            }
        }
        Case::Padding => {
            let mut padded = vec![b' '; 1024 * 1024];
            padded.extend_from_slice(raw);
            let candidate = HostDtoV1::decode(
                &padded,
                source.context().unwrap(),
                budget
                    .reserve(Cohort::LoadCandidate, padded.len() + 4096)
                    .unwrap(),
            )
            .unwrap()
            .into_candidate(source.context().unwrap())
            .unwrap();
            assert!(candidate.reserved_bytes() >= padded.len() * 3);
            let retained = budget.retained_bytes();
            assert!(retained >= saved.reserved_bytes() + candidate.reserved_bytes());
            drop(candidate);
            assert_eq!(budget.retained_bytes(), saved.reserved_bytes());
            let initially = padded.len() + 4096;
            let running = budget
                .reserve(
                    Cohort::Running,
                    MAX_RESIDENT_BYTES - budget.retained_bytes() - initially,
                )
                .unwrap();
            let load = budget.reserve(Cohort::LoadCandidate, initially).unwrap();
            let error = HostDtoV1::decode(&padded, source.context().unwrap(), load).unwrap_err();
            assert_eq!(
                error.owner, "admission",
                "raw expansion must be refused before parsing"
            );
            drop(running);
        }
        Case::MutableSource => {
            for stride_attack in [false, true] {
                let unstable = ChangingSource {
                    source: &source,
                    changed: Cell::new(false),
                    text: "x".repeat(512 * 1024),
                    stride_attack,
                };
                let admission = CheckpointBudget::default();
                let error = export_host_checkpoint(
                    &unstable,
                    source.context().unwrap(),
                    admission.reserve(Cohort::SavePayload, 4096).unwrap(),
                )
                .unwrap_err();
                if stride_attack {
                    assert_eq!(
                        error.owner, "admission",
                        "actual row stride must be charged before typed allocation: {error:?}"
                    );
                } else {
                    assert!(
                        error.reason.contains("source changed"),
                        "byte growth must hit the bounded writer: {error:?}"
                    );
                }
                assert_eq!(admission.retained_bytes(), 0);
            }
        }
        Case::Binding => {
            for definition in ["collision", "barriers", "vermin", "installed_catalogs"] {
                let mut changed = value.clone();
                let byte = &mut changed["scalars"]["definitions"][definition][0];
                *byte = json!(byte.as_u64().unwrap() ^ 1);
                reject(&changed, definition);
            }
            for pointer in [
                "/scalars/boundary/input_watermark",
                "/scalars/boundary/physical_sequence",
                "/scalars/boundary/issued",
                "/scalars/spatial/sequence",
            ] {
                let mut changed = value.clone();
                let field = changed.pointer_mut(pointer).unwrap();
                *field = json!(field.as_u64().unwrap().checked_add(1).unwrap());
                reject(&changed, pointer);
            }
            let mut changed = value.clone();
            let x = &mut changed["scalars"]["controller"]["current"][0];
            *x = json!(x.as_f64().unwrap() + 0.5);
            reject(&changed, "host body differs from accepted sim sample");
            let mut changed = value.clone();
            changed["scalars"]["boundary"]["last_speech_sequence"] = json!(u64::MAX);
            reject(
                &changed,
                "speech consumer is ahead of all published messages",
            );
        }
    }
    assert_eq!(
        before,
        simulation_stamp(world),
        "capture or decode spent a simulation effect"
    );
    let bytes = raw.clone();
    drop(saved);
    let repeated = export();
    assert_eq!(
        repeated.value(),
        &bytes,
        "read-only capture drifted at the same boundary"
    );
    assert!(cost.encoded_bytes > 0);
    drop(repeated);
    let mut probe = world.resource_mut::<CaptureProbe>();
    probe.bytes = bytes;
    probe.done = true;
}

/// Each prepared component owns an independent admission in this API test.
/// This checks context coherence only, not the pending full-envelope budget.
fn saved_component_context(source: &HostObservation<'_>, raw: &[u8]) {
    use cathedral_sim::{
        checkpoint::{Reservation, host::HostCheckpointContext},
        dogs::checkpoint::AnimalsCheckpointContext,
        engine::climate_checkpoint::ClimateCheckpointContext,
        knowledge::checkpoint::KnowledgeCheckpointContext,
        marks::checkpoint::MarksCheckpointContext,
        notices::checkpoint::LawCheckpointContext,
        receipts::{Admission, CommandLedger, CommandLedgerDtoV1, Outcome, TURN_PRODUCER},
        timeline::LogicalTime,
    };
    fn save(bytes: usize) -> Reservation {
        CheckpointBudget::default()
            .reserve(Cohort::SavePayload, bytes)
            .unwrap()
    }
    let engine = source.local().unwrap().checkpoint_engine().unwrap();
    let world = engine.world();
    let elapsed = source.context().unwrap().elapsed();
    let now = LogicalTime::new(elapsed.as_secs_f64()).unwrap();
    let scalars = source.scalars().unwrap();
    let backbone = world
        .export_backbone_checkpoint(save(4096))
        .unwrap()
        .into_candidate(&world.item_catalog, &world.command_ledger)
        .unwrap();
    let law = engine
        .export_law_checkpoint(now, save(4096))
        .unwrap()
        .into_candidate(LawCheckpointContext::from_world(world, now))
        .unwrap();
    let knowledge = engine
        .export_knowledge_checkpoint(now, save(4096))
        .unwrap()
        .into_candidate(KnowledgeCheckpointContext::from_world(world, now))
        .unwrap();
    let marks = engine
        .export_marks_checkpoint(now, save(4096))
        .unwrap()
        .into_candidate(MarksCheckpointContext::from_world(world, now))
        .unwrap();
    let climate = engine
        .export_climate_checkpoint(now, save(4096))
        .unwrap()
        .into_candidate(ClimateCheckpointContext::from_world(world, now))
        .unwrap();
    let animals = engine
        .export_animals_checkpoint(now, save(4096))
        .unwrap()
        .into_candidate(
            AnimalsCheckpointContext::from_world(world, now)
                .with_engine_nav(engine.config().nav.as_deref()),
        )
        .unwrap();
    let ledger = world
        .command_ledger
        .checkpoint_v1(now, save(CommandLedgerDtoV1::WORKING_BYTES))
        .unwrap();
    let context = |law, ledger, elapsed, definitions| {
        HostCheckpointContext::from_components(
            backbone.value(),
            law,
            knowledge.value(),
            marks.value(),
            climate.value(),
            animals.value(),
            ledger,
            elapsed,
            definitions,
            scalars.boundary.input_watermark,
            scalars.boundary.issued,
        )
    };
    let matching = context(law.value(), ledger.value(), elapsed, scalars.definitions);
    let budget = CheckpointBudget::default();
    let decode = |context| {
        HostDtoV1::decode(
            raw,
            context,
            budget
                .reserve(Cohort::LoadCandidate, raw.len() + 4096)
                .unwrap(),
        )
    };
    let accepted = decode(matching).unwrap().into_candidate(matching).unwrap();
    assert!(!accepted.value().records().is_empty());
    drop(accepted);
    assert_eq!(budget.retained_bytes(), 0);
    let reject_context = |bad_context, expected: &str| {
        let error = decode(bad_context).unwrap_err().to_string();
        assert!(error.contains(expected), "wrong refusal: {error}");
        assert_eq!(budget.retained_bytes(), 0);
        // A context swap after successful decode must be checked again.
        let error = decode(matching)
            .unwrap()
            .into_candidate(bad_context)
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "wrong candidate refusal: {error}");
        assert_eq!(budget.retained_bytes(), 0);
    };
    let mut definitions = scalars.definitions;
    definitions.collision[0] ^= 1;
    reject_context(
        context(law.value(), ledger.value(), elapsed, definitions),
        "definition",
    );
    reject_context(
        context(
            law.value(),
            ledger.value(),
            elapsed + Duration::from_secs(1),
            scalars.definitions,
        ),
        "calendar",
    );

    // A separately valid component from a later boundary must not be mixed
    // with the captured backbone and the other saved component candidates.
    let later = LogicalTime::new(now.seconds() + 1.0).unwrap();
    let later_law = engine
        .export_law_checkpoint(later, save(4096))
        .unwrap()
        .into_candidate(LawCheckpointContext::from_world(world, later))
        .unwrap();
    reject_context(
        context(
            later_law.value(),
            ledger.value(),
            elapsed,
            scalars.definitions,
        ),
        "component boundaries",
    );

    // This terminal ledger is valid at its own horizon. Supplying it to the
    // earlier host boundary must revalidate receipt time, not trust admission.
    let mut future_ledger = CommandLedger::default();
    let operation = future_ledger.issue(TURN_PRODUCER).unwrap();
    let Admission::New(ticket) =
        future_ledger.begin(operation.command(0), &json!({"test":"future"}))
    else {
        panic!("fresh future command must be admitted");
    };
    future_ledger.finish(
        ticket,
        later.seconds(),
        Outcome::completed("future test receipt"),
        vec![],
    );
    let future_ledger = future_ledger
        .checkpoint_v1(later, save(CommandLedgerDtoV1::WORKING_BYTES))
        .unwrap();
    assert!(future_ledger.value().validate(now).is_err());
    let future_context = context(
        law.value(),
        future_ledger.value(),
        elapsed,
        scalars.definitions,
    );
    assert!(decode(future_context).is_err());
    assert_eq!(budget.retained_bytes(), 0);
    assert!(
        decode(matching)
            .unwrap()
            .into_candidate(future_context)
            .is_err()
    );
    assert_eq!(budget.retained_bytes(), 0);
}

/// The public borrowed-source contract allows interior mutability. Change
/// after measurement but before serialization; neither added bytes nor more
/// inline enum rows may escape the first reservation.
struct ChangingSource<'a, 'w> {
    source: &'a HostObservation<'w>,
    changed: Cell<bool>,
    text: String,
    stride_attack: bool,
}

impl HostCheckpointSource for ChangingSource<'_, '_> {
    fn scalars(&self) -> cathedral_sim::checkpoint::Result<ScalarsV1> {
        self.source.scalars()
    }

    fn records(
        &self,
        visitor: &mut dyn FnMut(RecordRef<'_>) -> cathedral_sim::checkpoint::Result<()>,
    ) -> cathedral_sim::checkpoint::Result<()> {
        if self.stride_attack && self.changed.get() {
            // Smaller encoded input, much larger typed collection allowance.
            for _ in 0..10_000 {
                visitor(RecordV1::ChalkPen { present: true })?;
            }
            Ok(())
        } else {
            let large = self.stride_attack || self.changed.get();
            visitor(RecordV1::Draft {
                text: if large { &self.text } else { "x" },
            })
        }
    }

    fn validate_boundary(&self) -> cathedral_sim::checkpoint::Result<()> {
        self.source.validate_boundary()?;
        self.changed.set(true);
        Ok(())
    }
}

fn object_paths(value: &Value, path: &str, output: &mut Vec<String>) {
    match value {
        Value::Object(fields) => {
            output.push(path.into());
            for (key, child) in fields {
                let key = key.replace('~', "~0").replace('/', "~1");
                object_paths(child, &format!("{path}/{key}"), output);
            }
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                object_paths(child, &format!("{path}/{index}"), output);
            }
        }
        _ => {}
    }
}

#[test]
fn host_checkpoint_public_repeated_actual_capture_spends_no_poll() {
    run(&mut app(), Case::Repeat);
}

#[test]
fn host_checkpoint_public_all_observed_object_fields_are_closed_and_required() {
    run(&mut app(), Case::Fields);
}

#[test]
fn host_checkpoint_public_unit_map_aliases_and_duplicate_fields_fail() {
    run(&mut app(), Case::Aliases);
}

#[test]
fn host_checkpoint_public_raw_padding_and_candidate_keep_their_admission() {
    run(&mut app(), Case::Padding);
}

#[test]
fn host_checkpoint_public_worker_arrivals_after_h_do_not_prevent_capture() {
    run(&mut app(), Case::WorkerAfterBoundary);
}

#[test]
fn host_checkpoint_public_mutable_source_cannot_escape_byte_or_row_admission() {
    run(&mut app(), Case::MutableSource);
}

#[test]
fn host_checkpoint_public_installed_definitions_and_accepted_boundary_are_bound() {
    run(&mut app(), Case::Binding);
}

#[test]
fn host_checkpoint_public_saved_components_must_share_the_actual_boundary() {
    run(&mut app(), Case::SavedContext);
}

#[test]
fn host_checkpoint_public_open_journal_keeps_its_scroll_position() {
    let mut app = app();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyJ);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    assert!(
        app.world()
            .resource::<crate::smart_actors::JournalUiState>()
            .open
    );
    {
        let world = app.world_mut();
        let mut query = world.query_filtered::<
            &mut ScrollPosition,
            With<crate::smart_actors::journal_ui::JournalEntriesRoot>,
        >();
        let mut position = query.single_mut(world).unwrap();
        position.x = 2.25;
        position.y = 97.5;
    }
    let snapshot = run(&mut app, Case::Repeat);
    assert_eq!(snapshot["scalars"]["ui"]["journal_open"], true);
    assert_eq!(
        snapshot["scalars"]["ui"]["journal_scroll"],
        json!([2.25, 97.5])
    );
    let world = app.world_mut();
    let mut query = world.query_filtered::<
        &ScrollPosition,
        With<crate::smart_actors::journal_ui::JournalEntriesRoot>,
    >();
    let position = query.single(world).unwrap();
    assert_eq!([position.x, position.y], [2.25, 97.5]);
}

#[test]
fn host_checkpoint_public_open_unicode_draft_keeps_character_cursor_without_submission() {
    let mut app = app();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Enter);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    let draft = "Å — café, 石 and e\u{301}: unfinished words  ";
    let cursor = "Å — café, 石".chars().count();
    {
        let mut chat = app
            .world_mut()
            .resource_mut::<crate::smart_actors::ChatInputState>();
        assert!(chat.open, "ordinary Enter must open the real editor");
        chat.buffer = draft.into();
        chat.cursor = cursor;
    }
    let snapshot = run(&mut app, Case::Repeat);
    assert_eq!(snapshot["scalars"]["ui"]["chat_open"], true);
    assert_eq!(snapshot["scalars"]["ui"]["chat_cursor"], cursor);
    let rows = snapshot["records"].as_array().unwrap();
    assert!(
        rows.iter()
            .any(|row| row["kind"] == "draft" && row["data"]["text"] == draft)
    );
    assert!(
        !rows
            .iter()
            .any(|row| row["kind"] == "unread_intent"
                && row["data"]["intent"]["say"]["text"] == draft)
    );
    let chat = app
        .world()
        .resource::<crate::smart_actors::ChatInputState>();
    assert_eq!(chat.buffer, draft);
    assert_eq!(chat.cursor, cursor);
}

#[test]
fn host_checkpoint_public_unread_intent_and_committed_speech_survive_together() {
    let mut app = app();
    let position = {
        let world = app.world_mut();
        let mut query = world.query::<&crate::controller::PhysicalPosition>();
        query.single(world).unwrap().current
    };
    let spatial_seq = app
        .world_mut()
        .resource_mut::<PlayerSpatialState>()
        .position_for_action(position);
    let handle = app.world().resource::<BridgeHandle>();
    handle
        .try_send(BridgeCommand::PlayerSay {
            request_id: "root-committed".into(),
            text: "A line already committed before this capture.".into(),
            position_m: Position::try_from(position).unwrap(),
            spatial_seq,
        })
        .unwrap();
    // Use the actual PreUpdate chat submission path. It must survive the
    // intervening ordinary physics/pump before CollectInput forwards it.
    let pending_text = "Å — this separately submitted line is still waiting.";
    {
        let mut chat = app
            .world_mut()
            .resource_mut::<crate::smart_actors::ChatInputState>();
        chat.open = true;
        chat.buffer = pending_text.into();
        chat.cursor = pending_text.chars().count();
    }
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Enter);
    let snapshot = run(&mut app, Case::UnreadInputsAndSpeech);
    let rows = snapshot["records"].as_array().unwrap();
    assert!(
        rows.iter().any(|row| row["kind"] == "unread_speech"
            && row["data"]["speech"]["text"] == "A line already committed before this capture."),
        "committed speech is missing from the completed drain: {snapshot}"
    );
    assert!(
        rows.iter().any(|row| row["kind"] == "unread_intent"
            && row["data"]["intent"]["say"]["text"] == pending_text),
        "ordinary chat submission is missing from unread intents: {snapshot}"
    );
    app.update();
    let hud = app
        .world()
        .resource::<crate::smart_actors::hud::SmartActorHudState>();
    assert!(
        hud.player_transcript
            .as_ref()
            .is_some_and(|caption| caption.text.contains(pending_text)),
        "the next ordinary pump did not commit the waiting typed line: {hud:?}"
    );
}
