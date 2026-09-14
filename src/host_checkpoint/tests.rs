//! Owner-only prepared owners and actual city component cost witness.
use super::*;
use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    time::TimeUpdateStrategy,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use cathedral_sim::checkpoint::{CheckpointBudget, Cohort, Reservation};
use std::{
    io::{Read, Write},
    path::Path,
    time::{Duration, Instant},
};

#[derive(Resource, Default)]
struct Capture {
    requested: bool,
    samples: usize,
    prepared_physics: bool,
    check_numeric: bool,
    fixture_verified: bool,
    scalars: Option<ScalarsV1>,
    bytes: Vec<u8>,
    report: Option<serde_json::Value>,
}

#[derive(Resource)]
struct ExpectedFixture {
    // This Running charge covers only test fixture byte storage, not the World.
    // Field order keeps those bytes alive no longer than their reservation.
    bytes: Vec<u8>,
    budget: CheckpointBudget,
    storage: Reservation,
}
impl ExpectedFixture {
    fn read(path: &Path) -> Self {
        let mut file = std::fs::File::open(path).unwrap();
        let len = usize::try_from(file.metadata().unwrap().len()).unwrap();
        assert!(len <= cathedral_sim::checkpoint::POPULATED_PAYLOAD_BYTES);
        let budget = CheckpointBudget::default();
        let storage = budget.reserve(Cohort::Running, len + 4096).unwrap();
        let mut bytes = vec![0; len];
        file.read_exact(&mut bytes).unwrap();
        assert_eq!(file.read(&mut [0]).unwrap(), 0, "fixture grew during read");
        Self {
            bytes,
            budget,
            storage,
        }
    }

    fn verify(&self, source: &HostObservation) {
        let decoded = HostDtoV1::decode(
            &self.bytes,
            source.context().unwrap(),
            self.budget
                .reserve(Cohort::LoadCandidate, self.bytes.len() + 4096)
                .unwrap(),
        )
        .unwrap();
        let cost = decoded.value().cost().unwrap();
        let save = self
            .budget
            .reserve(Cohort::SavePayload, cost.peak_bytes)
            .unwrap();
        let mut roundtrip = Vec::with_capacity(cost.encoded_bytes);
        serde_json::to_writer(&mut roundtrip, decoded.value()).unwrap();
        assert_eq!(roundtrip, self.bytes, "persisted canonical bytes changed");
        drop(roundtrip);
        drop(save);

        let candidate = decoded.into_candidate(source.context().unwrap()).unwrap();
        let fresh = export_host_checkpoint(
            source,
            source.context().unwrap(),
            self.budget.reserve(Cohort::SavePayload, 4096).unwrap(),
        )
        .unwrap();
        let saved_generation = candidate.value().scalars().boundary.generation;
        let mut detached_scalars = *fresh.value().scalars();
        assert_ne!(detached_scalars.boundary.generation, saved_generation);
        detached_scalars.boundary.generation = saved_generation;
        // Old saved identity and the replacement runtime differ. Only this
        // detached scalar copy is aligned; candidate and live owners stay exact.
        #[derive(serde::Serialize)]
        struct Projection<'a> {
            version: u16,
            scalars: ScalarsV1,
            records: &'a [RecordV1<String>],
        }
        let mut comparable = Vec::with_capacity(fresh.value().cost().unwrap().encoded_bytes);
        serde_json::to_writer(
            &mut comparable,
            &Projection {
                version: 1,
                scalars: detached_scalars,
                records: fresh.value().records(),
            },
        )
        .unwrap();
        assert_eq!(
            comparable, self.bytes,
            "compatible actual host differs beyond its replacement generation"
        );
        comparable.clear();
        serde_json::to_writer(
            &mut comparable,
            &Projection {
                version: 1,
                scalars: *candidate.value().scalars(),
                records: candidate.value().records(),
            },
        )
        .unwrap();
        assert_eq!(comparable, self.bytes, "admitted candidate bytes changed");
        assert_eq!(
            candidate.value().scalars().boundary.generation,
            saved_generation
        );
        assert_eq!(
            source.scalars().unwrap().boundary.generation,
            fresh.value().scalars().boundary.generation
        );
        drop(comparable);
        drop(fresh);
        drop(candidate);
        assert_eq!(self.budget.retained_bytes(), self.storage.bytes());
    }
}

fn fixture(city: bool, extra: u32) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin))
        .init_asset::<Mesh>()
        .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
        .init_asset::<StandardMaterial>()
        .init_asset::<Image>()
        .init_asset::<AudioSource>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<AccumulatedMouseMotion>()
        .init_resource::<AccumulatedMouseScroll>()
        .init_resource::<crate::map::MapState>()
        .init_resource::<crate::city::marks::ChalkHold>()
        .init_resource::<crate::city::marks::ChalkChoice>()
        .init_resource::<Capture>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            17,
        )));
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
        smart_actors::SmartActorsPlugin::new(smart_actors::SmartActorsConfig {
            enabled: true,
            fake_backend: true,
            tts_backend: "off".into(),
            extra_ambient_npcs: extra,
            ..default()
        }),
    ));
    app.world_mut()
        .resource_mut::<smart_actors::interaction::MicrophoneInputState>()
        .enabled = false;
    if city {
        app.add_plugins(crate::city::CityPlugin);
    } else {
        app.world_mut().resource_mut::<CollisionWorld>().add_box(
            Vec3::new(-1500.0, -1.0, -1500.0),
            Vec3::new(1500.0, 0.0, 1500.0),
        );
    }
    app.add_systems(PostUpdate, capture.in_set(HostCaptureSet));
    for _ in 0..16 {
        app.update();
        if app
            .world()
            .resource::<smart_actors::SmartActorRuntime>()
            .ready
        {
            return app;
        }
    }
    panic!("fake host did not reach an ordinary boundary")
}
fn request(app: &mut App, samples: usize) {
    let mut capture = app.world_mut().resource_mut::<Capture>();
    capture.requested = true;
    capture.samples = samples;
    drop(capture);
    app.update();
    assert!(!app.world().resource::<Capture>().bytes.is_empty());
}
fn capture(world: &mut World) {
    if !world.resource::<Capture>().requested {
        return;
    }
    if world.resource::<Capture>().prepared_physics {
        let mut query = world.query::<(&mut PlayerController, &mut PhysicalPosition)>();
        let (mut c, mut p) = query.single_mut(world).unwrap();
        c.velocity = Vec3::new(2.0, 3.0, 1.0);
        c.grounded = false;
        c.coyote_remaining = 0.05;
        c.jump_buffer_remaining = 0.1;
        c.pitch = 0.2;
        p.previous = p.current - Vec3::Y * 0.01;
    }
    let samples = world.resource::<Capture>().samples.max(1);
    let source = HostObservation::new(world).unwrap();
    let stamp = || {
        let local = source.local().unwrap();
        let sim = local.world().unwrap();
        (
            sim.world_revision,
            sim.event_sequence,
            local.accepted_boundary.unwrap().input_watermark,
        )
    };
    let before = stamp();
    let budget = CheckpointBudget::default();
    let mut phases: [Vec<f64>; 6] = std::array::from_fn(|_| Vec::with_capacity(samples));
    let mut shared_peak = 0;
    let mut bytes = Vec::new();
    let mut final_cost = None;
    let mut final_scalars = None;
    for _ in 0..samples {
        let start = Instant::now();
        let cost = checkpoint_host_cost(&source).unwrap();
        phases[0].push(start.elapsed().as_secs_f64() * 1e6);
        let start = Instant::now();
        let dto = export_host_checkpoint(
            &source,
            source.context().unwrap(),
            budget.reserve(Cohort::SavePayload, 4096).unwrap(),
        )
        .unwrap();
        phases[1].push(start.elapsed().as_secs_f64() * 1e6);
        let start = Instant::now();
        let raw = dto.encode().unwrap();
        phases[2].push(start.elapsed().as_secs_f64() * 1e6);
        let start = Instant::now();
        let decoded = HostDtoV1::decode(
            raw.value(),
            source.context().unwrap(),
            budget
                .reserve(Cohort::LoadCandidate, raw.value().len() + 4096)
                .unwrap(),
        )
        .unwrap();
        phases[3].push(start.elapsed().as_secs_f64() * 1e6);
        let start = Instant::now();
        let candidate = decoded.into_candidate(source.context().unwrap()).unwrap();
        phases[4].push(start.elapsed().as_secs_f64() * 1e6);
        shared_peak = shared_peak.max(budget.retained_bytes());
        final_scalars = Some(*candidate.value().scalars());
        if bytes.is_empty() {
            bytes = raw.value().clone();
        } else {
            assert_eq!(
                &bytes,
                raw.value(),
                "repeated read-only capture changed covered host bytes"
            );
        }
        let start = Instant::now();
        drop(candidate);
        drop(raw);
        phases[5].push(start.elapsed().as_secs_f64() * 1e6);
        assert_eq!(budget.retained_bytes(), 0);
        final_cost = Some(cost);
    }
    assert_eq!(before, stamp(), "capture spent an ordinary poll");
    // This must precede ordinary CollectInput: an active fixture still has an
    // unread intentional chat line, whose later forwarding advances issued.
    let fixture_verified = if let Some(expected) = world.get_resource::<ExpectedFixture>() {
        expected.verify(&source);
        true
    } else {
        false
    };
    let collision = source.resource::<CollisionWorld>().unwrap();
    let vermin = source
        .optional_component::<crate::city::vermin::Vermin>()
        .unwrap();
    let actual_sim = source.local().unwrap().world().unwrap();
    let requested = source
        .resource::<smart_actors::SmartActorsConfig>()
        .unwrap()
        .extra_ambient_npcs;
    let placed = actual_sim
        .characters
        .values()
        .filter(|c| c.lore().is_some_and(|l| l.generated))
        .count();
    assert_eq!(placed, requested as usize);
    let rows: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    if world.resource::<Capture>().check_numeric {
        let mut changes = vec![
            (
                "/scalars/controller/velocity/1",
                serde_json::json!(f32::MAX),
            ),
            ("/scalars/controller/pitch", serde_json::json!(2.0)),
            (
                "/scalars/controller/coyote_remaining",
                serde_json::json!(0.11),
            ),
            (
                "/scalars/controller/jump_buffer_remaining",
                serde_json::json!(0.13),
            ),
            ("/scalars/ui/selected_index", serde_json::json!(usize::MAX)),
        ];
        if !rows["scalars"]["gates"].is_null() {
            changes.push(("/scalars/gates", serde_json::Value::Null));
        } else {
            changes.push((
                "/scalars/gates",
                serde_json::to_value(
                    crate::city::gates::GateRuntime::default().checkpoint_scalar(),
                )
                .unwrap(),
            ));
        }
        if !rows["scalars"]["vermin"].is_null() {
            changes.push(("/scalars/vermin", serde_json::Value::Null));
            changes.push(("/scalars/vermin/seed", serde_json::json!(999)));
            changes.push(("/scalars/vermin/density", serde_json::json!(0.5)));
            changes.push(("/scalars/vermin/swarm_percepts", serde_json::json!(false)));
        } else {
            changes.push(("/scalars/vermin",serde_json::json!({"seed":40,"swarm_percepts":true,"density":1.0,"announced_boil_night":null,"last_percept_minutes":null})));
        }
        for (path, value) in changes {
            let mut damaged = rows.clone();
            *damaged.pointer_mut(path).unwrap() = value;
            let raw = serde_json::to_vec(&damaged).unwrap();
            assert!(
                HostDtoV1::decode(
                    &raw,
                    source.context().unwrap(),
                    budget
                        .reserve(Cohort::LoadCandidate, raw.len() + 4096)
                        .unwrap()
                )
                .is_err(),
                "accepted invalid owner field {path}"
            );
            assert_eq!(budget.retained_bytes(), 0);
        }
    }
    let mut readable = std::collections::BTreeMap::<String, usize>::new();
    for row in rows["records"].as_array().unwrap() {
        *readable
            .entry(row["kind"].as_str().unwrap().to_owned())
            .or_default() += 1;
    }
    let cost = final_cost.unwrap();
    let report = serde_json::json!({"schema":1,"scenario":"actual-host-boundary-v1","samples":samples,
        "counts":{"entities":world.entities().len(),"characters":source.local().unwrap().world().unwrap().characters.len(),"collision_boxes":collision.boxes.len(),"collision_prisms":collision.convex_prisms.len(),"dynamic_barriers":world.iter_entities().filter(|e|e.contains::<DynamicBarrier>()).count(),"cut_margin":world.contains_resource::<crate::city::CutMarginProfile>(),"vermin_colonies":vermin.map_or(0,|v|v.colonies.len()),"rats":vermin.map_or(0,|v|v.colonies.iter().map(|c|c.rats.len()+c.boil_rats.len()).sum::<usize>()),"rows":rows["records"].as_array().unwrap().len()},
        "cost":{"encoded_bytes":cost.encoded_bytes,"expanded_upper_bytes":cost.expanded_upper_bytes,"peak_bytes":cost.peak_bytes,"validation_working_bytes":VALIDATION_WORKING_BYTES,"container_stride_bytes":ROW_WORKING_BYTES},
        "placement":{"requested":requested,"placed":placed,"unplaced":0},"readable_counts":readable,
        "shared_reserved_peak_excluding_running_bytes":shared_peak,"readonly":{"world_revision_before":before.0,"world_revision_after":stamp().0,"event_sequence_before":before.1,"event_sequence_after":stamp().1,"input_watermark_before":before.2,"input_watermark_after":stamp().2},
        "preflight_us":phases[0],"export_us":phases[1],"encode_us":phases[2],"decode_validate_us":phases[3],"candidate_validate_us":phases[4],"drop_us":phases[5]});
    let mut result = world.resource_mut::<Capture>();
    result.requested = false;
    result.bytes = bytes;
    result.report = Some(report);
    result.scalars = final_scalars;
    result.fixture_verified = fixture_verified;
}

fn install_prepared_controller(world: &mut World, s: &ControllerV1) {
    let mut query = world.query::<(&mut PlayerController, &mut PhysicalPosition)>();
    let (mut c, mut p) = query.single_mut(world).unwrap();
    c.flying = s.flying;
    c.velocity = Vec3::from_array(s.velocity);
    c.yaw = s.yaw;
    c.pitch = s.pitch;
    c.grounded = s.grounded;
    c.coyote_remaining = s.coyote_remaining;
    c.jump_buffer_remaining = s.jump_buffer_remaining;
    p.previous = Vec3::from_array(s.previous);
    p.current = Vec3::from_array(s.current);
}
fn physical_bits(world: &World) -> Vec<u32> {
    let o = HostObservation::new(world).unwrap();
    let c = o.component::<PlayerController>().unwrap();
    let p = o.component::<PhysicalPosition>().unwrap();
    c.velocity
        .to_array()
        .into_iter()
        .chain(p.previous.to_array())
        .chain(p.current.to_array())
        .chain([c.yaw, c.pitch, c.coyote_remaining, c.jump_buffer_remaining])
        .map(f32::to_bits)
        .chain([c.flying as u32, c.grounded as u32])
        .collect()
}

#[test]
fn host_owner_scrambled_physics_restores_exact_next_fixed_step() {
    let mut app = fixture(false, 0);
    app.world_mut().resource_mut::<Capture>().prepared_physics = true;
    request(&mut app, 1);
    let saved = app
        .world()
        .resource::<Capture>()
        .scalars
        .unwrap()
        .controller;
    // Capture precedes the remaining frame consumers; reinstall this prepared
    // controller before each independent fixed solve, never a live load path.
    install_prepared_controller(app.world_mut(), &saved);
    let expected_before = physical_bits(app.world());
    crate::controller::host_checkpoint_test_fixed_step(app.world_mut());
    let expected = physical_bits(app.world());
    let mut wrong = saved;
    wrong.velocity = [-19.0, -8.0, 4.0];
    wrong.current = [30.0, 30.0, 30.0];
    wrong.previous = [0.0; 3];
    wrong.coyote_remaining = 0.0;
    wrong.jump_buffer_remaining = 0.0;
    wrong.flying = !saved.flying;
    install_prepared_controller(app.world_mut(), &wrong);
    assert_ne!(physical_bits(app.world()), expected);
    install_prepared_controller(app.world_mut(), &saved);
    assert_eq!(
        physical_bits(app.world()),
        expected_before,
        "every physical field matches before the next step"
    );
    crate::controller::host_checkpoint_test_fixed_step(app.world_mut());
    assert_eq!(physical_bits(app.world()), expected);
    assert!(
        app.world()
            .iter_entities()
            .find_map(|e| e.get::<PlayerController>())
            .unwrap()
            .velocity
            .y
            > 0.0,
        "the restored queued jump/coyote caused a real upward solve"
    );
}

#[test]
fn host_owner_rejects_a_controller_and_body_split_between_entities() {
    let mut app = fixture(false, 0);
    app.world_mut().resource_mut::<Capture>().check_numeric = true;
    request(&mut app, 1);
    let entity = app
        .world()
        .iter_entities()
        .find(|e| e.contains::<PlayerController>())
        .unwrap()
        .id();
    let p = app
        .world_mut()
        .entity_mut(entity)
        .take::<PhysicalPosition>()
        .unwrap();
    app.world_mut().spawn(p);
    assert!(
        HostObservation::new(app.world())
            .unwrap()
            .scalars()
            .is_err()
    );
}

#[test]
fn host_owner_keeps_issued_authority_after_sim_refuses_an_explicit_gap() {
    use cathedral_sim::receipts::{HOST_PRODUCER, OperationId, RECENT_CAPACITY};
    let mut app = fixture(false, 0);
    let issued = app
        .world()
        .resource::<BridgeHandle>()
        .checkpoint_issued()
        .unwrap();
    let reserved = issued + RECENT_CAPACITY as u64 + 1;
    // Each explicit enqueue stays within the host window. Identified Hello
    // translates to no sim command, so it advances issued without a ledger
    // acceptance; the following action then exceeds the sim's own window.
    app.world()
        .resource::<BridgeHandle>()
        .try_send(smart_actors::bridge::BridgeCommand::Identified {
            id: OperationId {
                producer: HOST_PRODUCER,
                sequence: reserved - 1,
            }
            .command(0),
            command: Box::new(smart_actors::bridge::BridgeCommand::Hello {
                position_m: smart_actors::model::Position::try_from(
                    HostObservation::new(app.world())
                        .unwrap()
                        .component::<PhysicalPosition>()
                        .unwrap()
                        .current,
                )
                .unwrap(),
                spatial_seq: 0,
            }),
        })
        .unwrap();
    let id = OperationId {
        producer: HOST_PRODUCER,
        sequence: reserved,
    }
    .command(0);
    app.world()
        .resource::<BridgeHandle>()
        .try_send(smart_actors::bridge::BridgeCommand::Identified {
            id,
            command: Box::new(smart_actors::bridge::BridgeCommand::PlayerSound {
                sound_id: "fart".into(),
            }),
        })
        .unwrap();
    assert_eq!(
        app.world()
            .resource::<BridgeHandle>()
            .checkpoint_issued()
            .unwrap(),
        reserved
    );
    request(&mut app, 1);
    assert!(
        app.world()
            .non_send::<LocalEngine>()
            .world()
            .unwrap()
            .command_ledger
            .get(id)
            .is_none(),
        "the sim refused the explicit sequence gap"
    );
    assert_eq!(
        app.world()
            .resource::<Capture>()
            .scalars
            .unwrap()
            .boundary
            .issued,
        reserved,
        "successful enqueue allocator must survive refused sim admission"
    );
}

#[derive(Resource, Default)]
struct ReceiptCaptureRefusal {
    requested: bool,
    reason: Option<String>,
}
fn probe_receipt_capture_refusal(world: &mut World) {
    if !world.resource::<ReceiptCaptureRefusal>().requested {
        return;
    }
    let budget = CheckpointBudget::default();
    let source = HostObservation::new(world).unwrap();
    let result = export_host_checkpoint(
        &source,
        source.context().unwrap(),
        budget.reserve(Cohort::SavePayload, 4096).unwrap(),
    );
    let reason = result.err().map(|e| e.reason);
    assert_eq!(budget.retained_bytes(), 0);
    let mut outcome = world.resource_mut::<ReceiptCaptureRefusal>();
    outcome.requested = false;
    outcome.reason = reason;
}

#[test]
fn host_owner_receipt_limit_declines_capture_until_ordinary_readable_expiry() {
    let mut app = fixture(false, 0);
    let retained = app
        .world()
        .resource::<SpeechPresentationState>()
        .checkpoint_test_fill_receipt_allowance();
    send_player_line(&mut app, "receipt-limit", "What's your name?", true);
    for _ in 0..24 {
        app.update();
        if !app
            .world()
            .resource::<SpeechPresentationState>()
            .subtitles
            .is_empty()
        {
            break;
        }
    }
    let state = app.world().resource::<SpeechPresentationState>();
    assert!(
        !state.subtitles.is_empty(),
        "ordinary NPC words must still display"
    );
    assert!(state.subtitles.front().unwrap().receipt.is_none());
    assert!(
        app.world()
            .resource::<SmartActorHudState>()
            .player_receipt_unavailable
    );
    assert!(
        app.world()
            .resource::<SmartActorHudState>()
            .player_transcript
            .as_ref()
            .unwrap()
            .text
            .contains("What's your name?")
    );
    app.insert_resource(ReceiptCaptureRefusal {
        requested: true,
        ..default()
    });
    app.add_systems(
        PostUpdate,
        probe_receipt_capture_refusal.in_set(HostCaptureSet),
    );
    app.update();
    assert_eq!(
        app.world()
            .resource::<ReceiptCaptureRefusal>()
            .reason
            .as_deref(),
        Some("host: unavailable committed player caption receipt")
    );
    app.world_mut()
        .resource_mut::<SmartActorHudState>()
        .show_player_transcript("Unsent provisional replacement");
    app.world_mut()
        .resource_mut::<ReceiptCaptureRefusal>()
        .requested = true;
    app.update();
    assert_eq!(
        app.world()
            .resource::<ReceiptCaptureRefusal>()
            .reason
            .as_deref(),
        Some("host: subtitle lacks original receipt")
    );
    drop(retained);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        100,
    )));
    for _ in 0..120 {
        app.update();
    }
    assert!(
        !app.world()
            .resource::<SmartActorHudState>()
            .player_receipt_unavailable
    );
    request(&mut app, 1);
}

fn install_prepared_time(world: &mut World, s: &TimeV1) {
    let mut virtual_time = Time::<Virtual>::default();
    virtual_time.advance_by(s.virtual_elapsed - s.virtual_delta);
    virtual_time.advance_by(s.virtual_delta);
    let live = LiveTime {
        continuation: s.accepted.accepted().unwrap(),
        accepted_virtual: virtual_time,
        last_frame: s
            .last_frame
            .0
            .map(|f| cathedral_sim::timeline::AcceptedFrame {
                wall_delta: f.wall_delta,
                accepted_delta: f.accepted_delta,
                elapsed: f.elapsed,
                debt: f.debt,
            }),
    };
    let mut fixed = Time::<Fixed>::from_duration(s.accepted.fixed_step().unwrap());
    fixed.advance_by(s.fixed_elapsed - s.fixed_delta);
    fixed.advance_by(s.fixed_delta);
    fixed.accumulate_overstep(s.accepted.fixed_residual().unwrap());
    world.insert_resource(live);
    world.insert_resource(virtual_time);
    world.insert_resource(virtual_time.as_generic());
    world.insert_resource(fixed);
}
#[derive(Debug, PartialEq)]
struct TimeSignature {
    continuation: cathedral_sim::timeline::AcceptedTime,
    last_frame: Option<cathedral_sim::timeline::AcceptedFrame>,
    fixed: [Duration; 4],
    clocks: [Duration; 6],
}
fn time_signature(world: &World) -> TimeSignature {
    let live = world.resource::<LiveTime>();
    let fixed = world.resource::<Time<Fixed>>();
    let published = world.resource::<Time<Virtual>>();
    let generic = world.resource::<Time>();
    TimeSignature {
        continuation: live.continuation,
        last_frame: live.last_frame,
        fixed: [
            fixed.elapsed(),
            fixed.delta(),
            fixed.overstep(),
            fixed.timestep(),
        ],
        clocks: [
            live.accepted_virtual.elapsed(),
            live.accepted_virtual.delta(),
            published.elapsed(),
            published.delta(),
            generic.elapsed(),
            generic.delta(),
        ],
    }
}
fn prepared_time_step(world: &mut World) {
    world.run_schedule(First);
    world.run_schedule(RunFixedMainLoop);
}

#[test]
fn host_owner_debt_and_fixed_residual_survive_scrambled_prepared_clock() {
    let mut app = fixture(false, 0);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        503,
    )));
    request(&mut app, 1);
    let saved = app.world().resource::<Capture>().scalars.unwrap();
    assert!(saved.time.accepted.accepted().unwrap().debt > Duration::ZERO);
    assert!(saved.time.accepted.fixed_residual().unwrap() > Duration::ZERO);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        17,
    )));
    install_prepared_time(app.world_mut(), &saved.time);
    install_prepared_controller(app.world_mut(), &saved.controller);
    let before = time_signature(app.world());
    let body_before = physical_bits(app.world());
    prepared_time_step(app.world_mut());
    let expected = time_signature(app.world());
    let body_expected = physical_bits(app.world());
    assert!(expected.continuation.elapsed > before.continuation.elapsed);
    assert!(expected.continuation.debt < before.continuation.debt);
    app.world_mut().insert_resource(LiveTime::default());
    app.world_mut()
        .insert_resource(Time::<Fixed>::from_hz(30.0));
    assert_ne!(time_signature(app.world()), before);
    install_prepared_time(app.world_mut(), &saved.time);
    install_prepared_controller(app.world_mut(), &saved.controller);
    assert_eq!(time_signature(app.world()), before);
    assert_eq!(physical_bits(app.world()), body_before);
    prepared_time_step(app.world_mut());
    assert_eq!(time_signature(app.world()), expected);
    assert_eq!(physical_bits(app.world()), body_expected);
}

#[test]
fn host_owner_actual_city_definitions_and_readonly_boundary() {
    let mut app = fixture(true, 0);
    app.world_mut().resource_mut::<Capture>().check_numeric = true;
    request(&mut app, 1);
    let report = app.world().resource::<Capture>().report.as_ref().unwrap();
    // City buildings use convex prisms; both installed physical families must
    // be covered, without mistaking the total city collider count for boxes.
    assert!(report["counts"]["collision_boxes"].as_u64().unwrap() > 0);
    assert!(report["counts"]["collision_prisms"].as_u64().unwrap() > 0);
    assert_eq!(report["counts"]["dynamic_barriers"], 2);
    assert_eq!(report["counts"]["vermin_colonies"], 8);
    assert_eq!(report["counts"]["cut_margin"], true);
    let original = HostObservation::new(app.world())
        .unwrap()
        .definitions()
        .unwrap();
    app.world_mut()
        .resource_mut::<crate::city::CutMarginProfile>()
        .checkpoint_test_flip_feather();
    assert_ne!(
        HostObservation::new(app.world())
            .unwrap()
            .definitions()
            .unwrap()
            .collision,
        original.collision
    );
    app.world_mut()
        .resource_mut::<crate::city::CutMarginProfile>()
        .checkpoint_test_flip_feather();
    assert_eq!(
        HostObservation::new(app.world())
            .unwrap()
            .definitions()
            .unwrap(),
        original
    );
    let barrier = app
        .world()
        .iter_entities()
        .find(|e| e.contains::<DynamicBarrier>())
        .unwrap()
        .id();
    let active = app.world().get::<DynamicBarrier>(barrier).unwrap().active;
    app.world_mut()
        .entity_mut(barrier)
        .get_mut::<DynamicBarrier>()
        .unwrap()
        .active = !active;
    assert!(
        HostObservation::new(app.world())
            .unwrap()
            .definitions()
            .is_err(),
        "scrambled active publication cannot pass"
    );
    app.world_mut()
        .entity_mut(barrier)
        .get_mut::<DynamicBarrier>()
        .unwrap()
        .active = active;
    let kind = app
        .world()
        .get::<crate::city::gates::GateBarrier>(barrier)
        .unwrap()
        .0;
    app.world_mut()
        .entity_mut(barrier)
        .get_mut::<crate::city::gates::GateBarrier>()
        .unwrap()
        .0 = match kind {
        crate::city::gates::GateKind::Stone => crate::city::gates::GateKind::River,
        crate::city::gates::GateKind::River => crate::city::gates::GateKind::Stone,
    };
    assert_ne!(
        HostObservation::new(app.world())
            .unwrap()
            .definitions()
            .unwrap()
            .barriers,
        original.barriers
    );
    app.world_mut()
        .entity_mut(barrier)
        .get_mut::<crate::city::gates::GateBarrier>()
        .unwrap()
        .0 = kind;
    app.world_mut()
        .entity_mut(barrier)
        .remove::<crate::city::gates::GateBarrier>();
    assert!(
        HostObservation::new(app.world())
            .unwrap()
            .definitions()
            .is_err(),
        "unowned physical barriers are unsupported"
    );
}

#[test]
fn persisted_initial_and_active_host_fixtures_decode_at_actual_compatible_boundaries() {
    // Force a different process-global generation even when this is the only
    // selected test. Loading must also work after arbitrary earlier host tests.
    drop(fixture(false, 0));
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("crates/cathedral-sim/tests/fixtures/checkpoint_host");
    for (name, active) in [("initial-v1.json", false), ("active-v1.json", true)] {
        let expected = ExpectedFixture::read(&directory.join(name));
        let mut app = fixture(true, 0);
        app.insert_resource(expected);
        if active {
            prepare_readable_workload(&mut app);
        }
        request(&mut app, 1);
        let result = app.world().resource::<Capture>();
        assert!(
            result.fixture_verified,
            "fixture skipped its actual capture barrier"
        );
        let counts = &result.report.as_ref().unwrap()["readable_counts"];
        if active {
            for (kind, expected) in [
                ("bubble", 1),
                ("subtitle", 1),
                ("player_receipt", 1),
                ("unread_speech", 4),
                ("unread_intent", 1),
                ("pending_command", 1),
                ("cooldown", 1),
                ("well_draw", 1),
            ] {
                assert_eq!(counts[kind], expected, "active fixture family {kind}");
            }
        } else {
            for kind in ["unread_speech", "unread_intent", "subtitle", "bubble"] {
                assert!(counts.get(kind).is_none(), "initial fixture has {kind}");
            }
        }
    }
}

#[test]
#[ignore = "explicit create_new writer for canonical host component fixtures"]
fn m2a15_write_component_fixtures() {
    let directory = std::env::var("ALIBI_HOST_OUTPUT").expect("explicit fresh output directory");
    let directory = Path::new(&directory);
    std::fs::create_dir(directory).unwrap();
    for (name, active) in [("initial-v1.json", false), ("active-v1.json", true)] {
        let mut app = fixture(true, 0);
        if active {
            prepare_readable_workload(&mut app);
        }
        request(&mut app, 1);
        let result = app.world().resource::<Capture>();
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.join(name))
            .unwrap();
        file.write_all(&result.bytes).unwrap();
        println!("host fixture {name}: {} exact bytes", result.bytes.len());
    }
}

#[test]
#[ignore = "serial release component cost probe; no renderer/device"]
fn m2a15_cost_probe() {
    let mode = std::env::var("ALIBI_HOST_MODE").unwrap_or_else(|_| "authored".into());
    let extra = match mode.as_str() {
        "authored" => 0,
        "populated" => 2000,
        _ => panic!("unknown host probe mode"),
    };
    let samples = std::env::var("ALIBI_HOST_SAMPLES")
        .unwrap_or_else(|_| "100".into())
        .parse::<usize>()
        .unwrap();
    assert!((1..=1000).contains(&samples));
    let mut app = fixture(true, extra);
    prepare_readable_workload(&mut app);
    request(&mut app, samples);
    let mut report = app
        .world_mut()
        .resource_mut::<Capture>()
        .report
        .take()
        .unwrap();
    report["mode"] = mode.into();
    report["scope"]="Actual renderer-free CityPlugin geometry and authored or +2000 citizen host; existing host component only, excluding full Save/Load/Running assembly".into();
    let output = std::env::var("ALIBI_HOST_OUTPUT").expect("explicit raw JSON output path");
    std::fs::write(output, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
}

fn send_player_line(app: &mut App, request: &str, text: &str, scripted: bool) {
    use smart_actors::{
        bridge::BridgeCommand,
        model::{ActorId, Position},
    };
    let position = HostObservation::new(app.world())
        .unwrap()
        .component::<PhysicalPosition>()
        .unwrap()
        .current;
    let seq = app
        .world_mut()
        .resource_mut::<PlayerSpatialState>()
        .position_for_action(position);
    let command = if scripted {
        BridgeCommand::DebugPlayerSay {
            request_id: request.into(),
            text: text.into(),
            target_id: Some(ActorId("k0fb1".into())),
            spatial_seq: seq,
            position_m: Position::try_from(position).unwrap(),
        }
    } else {
        BridgeCommand::PlayerSay {
            request_id: request.into(),
            text: text.into(),
            spatial_seq: seq,
            position_m: Position::try_from(position).unwrap(),
        }
    };
    app.world()
        .resource::<BridgeHandle>()
        .try_send(command)
        .unwrap();
}
fn prepare_readable_workload(app: &mut App) {
    send_player_line(app, "owner-warmup", "What's your name?", true);
    for _ in 0..24 {
        app.update();
        if !app
            .world()
            .resource::<SpeechPresentationState>()
            .subtitles
            .is_empty()
        {
            break;
        }
    }
    assert!(
        !app.world()
            .resource::<SpeechPresentationState>()
            .subtitles
            .is_empty(),
        "ordinary fake reply must reach the real subtitle and bubble consumers"
    );
    app.world_mut()
        .write_message(soundscape::SoundscapeCue::MarketMeasurement {
            position: Vec3::new(0.0, 0.91, 95.0),
        });
    app.world_mut()
        .write_message(soundscape::SoundscapeCue::WellDraw {
            source: soundscape::SpecialWell::Ford,
        });
    for index in 0..4 {
        send_player_line(
            app,
            &format!("owner-committed-{index}"),
            &format!("Receipt {index}: Å, the stone remembers our words."),
            false,
        );
    }
    let mut chat = app
        .world_mut()
        .resource_mut::<smart_actors::ChatInputState>();
    chat.open = true;
    chat.buffer = "Å — this intentional line is waiting at the capture boundary.".into();
    chat.cursor = chat.buffer.chars().count();
    drop(chat);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Enter);
}
