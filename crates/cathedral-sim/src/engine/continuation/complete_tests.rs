//! Full-envelope joint validation with two simultaneous accepted obligations.
//! The explicit synthetic Host owner isolates simulation completeness; actual
//! Bevy Host extraction/admission is covered independently in the binary tests.
//! Diagnostic JSON copies serve behavior/joint validation, not heap measurement.
use super::*;
use crate::checkpoint::{
    self, Admitted, CheckpointBudget, Cohort, HostTimeV1,
    complete::{
        self, CheckpointProfile as Profile, CompleteCheckpointCandidate, CompleteCheckpointInput,
        HydrationAssets, HydrationWorldAssets, InstalledCheckpointDefinitions,
        PreparedContinuation, WorldIdentity,
    },
    host::{HostCheckpointSource, HudSlot, Nullable, RecordRef, RecordV1, ScalarsV1},
};
use std::time::Duration;
struct Recorded(u64);
impl Cognition for Recorded {
    fn request(
        &mut self,
        _: String,
    ) -> std::result::Result<crate::RequestId, crate::CognitionBusy> {
        self.0 += 1;
        Ok(crate::RequestId(self.0))
    }
}
struct Host(ScalarsV1);
impl HostCheckpointSource for Host {
    fn scalars(&self) -> checkpoint::Result<ScalarsV1> {
        Ok(self.0)
    }
    fn records(
        &self,
        f: &mut dyn FnMut(RecordRef<'_>) -> checkpoint::Result<()>,
    ) -> checkpoint::Result<()> {
        self.complete_records(f)
    }
    fn complete_records<'a>(
        &'a self,
        f: &mut dyn FnMut(RecordRef<'a>) -> checkpoint::Result<()>,
    ) -> checkpoint::Result<()> {
        for r in [
            RecordV1::Draft { text: "" },
            RecordV1::SelectedItem {
                item: Nullable(None),
            },
            RecordV1::Hud {
                slot: HudSlot::Subtitle,
                text: "",
                remaining: Nullable(None),
            },
            RecordV1::Hud {
                slot: HudSlot::Inventory,
                text: "",
                remaining: Nullable(None),
            },
            RecordV1::Hud {
                slot: HudSlot::OfferCard,
                text: "",
                remaining: Nullable(None),
            },
            RecordV1::Hud {
                slot: HudSlot::LawStanding,
                text: "",
                remaining: Nullable(None),
            },
            RecordV1::Hud {
                slot: HudSlot::JournalStanding,
                text: "",
                remaining: Nullable(None),
            },
            RecordV1::Hud {
                slot: HudSlot::FocusHint,
                text: "",
                remaining: Nullable(None),
            },
            RecordV1::ChalkHold {
                intent: Nullable(None),
            },
            RecordV1::ChalkPen { present: false },
        ] {
            f(r)?;
        }
        Ok(())
    }
    fn validate_boundary(&self) -> checkpoint::Result<()> {
        Ok(())
    }
}
fn host(engine: &Engine) -> Host {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checkpoint_host/initial-v1.json"
    )))
    .unwrap();
    let mut s: ScalarsV1 = serde_json::from_value(fixture["scalars"].clone()).unwrap();
    let p = engine.world.characters[&engine.config.player_id].position_m();
    s.controller.current = [p.x as f32, p.y as f32, p.z as f32];
    s.controller.previous = s.controller.current;
    s.controller.yaw = 0.0;
    s.controller.velocity = [0.0; 3];
    s.spatial.sequence = engine.world.spatial_sequence as u64;
    s.spatial.last_position = Nullable(Some(s.controller.current));
    s.spatial.last_yaw = Nullable(Some(0.0));
    s.boundary.generation = 1;
    s.boundary.physical_sequence = s.spatial.sequence;
    s.boundary.input_watermark = 0;
    s.boundary.issued = 0;
    s.time.accepted = HostTimeV1::from_accepted(
        crate::timeline::AcceptedTime::default(),
        Duration::from_nanos(8_333_333),
        Duration::ZERO,
        0.0,
    )
    .unwrap();
    s.time.virtual_elapsed = Duration::ZERO;
    s.time.virtual_delta = Duration::ZERO;
    s.time.fixed_elapsed = Duration::ZERO;
    s.time.fixed_delta = Duration::ZERO;
    s.time.last_frame = Nullable(None);
    let time = engine.clock.at(0.0);
    s.clock.day = time.day;
    s.clock.fraction = time.fraction;
    s.clock.office = time.office.into();
    s.clock.weekday = time.weekday.into();
    s.clock.brightness = engine.clock.brightness(0.0);
    s.clock.scale = engine.clock.scale();
    Host(s)
}
fn assets(e: &Engine) -> checkpoint::Result<HydrationAssets> {
    let seed = crate::WorldSeed::from_json_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/demo_seed.json"
    )))
    .unwrap();
    let env = crate::night::checkpoint::tests::env();
    let w = &e.world;
    HydrationAssets::new(
        &seed,
        e.config.clone(),
        env,
        HydrationWorldAssets {
            areas: w.area_map.clone(),
            sounds: w.sound_catalog.clone(),
            items: w.item_catalog.clone(),
            nav: w.nav.clone(),
            shelters: w.shelters.clone(),
            marks: w.mark_catalog.clone(),
            facts: w.fact_catalog.clone(),
            salience: w.salience.clone(),
            area_adjacency: w.area_adjacency.clone(),
        },
    )
}
fn reload(
    saved: Admitted<CompleteCheckpointCandidate>,
    e: &Engine,
    h: &Host,
    b: &CheckpointBudget,
    g: u64,
) -> Admitted<PreparedContinuation> {
    let input = CompleteCheckpointInput::copy_from(
        saved.value().bytes(),
        b.reserve(Cohort::LoadCandidate, 4096).unwrap(),
    )
    .unwrap();
    drop(saved);
    let d = InstalledCheckpointDefinitions::from_engine(
        e,
        h.0.definitions,
        crate::timeline::LogicalTime::new(0.0).unwrap(),
    )
    .unwrap();
    let validated = complete::validate(input, &d).unwrap();
    drop(d);
    validated
        .prepare_hydration(64 * 1024 * 1024)
        .unwrap()
        .hydrate(|| assets(e), h.0.definitions, crate::RuntimeGeneration(g))
        .unwrap()
        .prepare_continuation()
        .unwrap()
}
fn add_context(e: &mut Engine, actor: &ActorId, subject: &ActorId, text: &str) -> crate::FactKey {
    let day = e.clock.at(0.0).game_days();
    let key = crate::knowledge::mint::mint_claim(
        &mut e.world,
        actor,
        crate::knowledge::Topic::Talk,
        text.into(),
        vec![subject.clone()],
        None,
        Some(day),
    )
    .unwrap();
    let question = format!("Tell me about {}", e.world.characters[subject].name());
    e.world
        .characters
        .get_mut(actor)
        .unwrap()
        .state
        .inbox
        .push(question);
    e.world
        .knowledge
        .note_occasion(actor, Some(subject.clone()), None, day);
    key
}
#[test]
fn continuation_complete_mixed_obligations_resave_reload_and_joint_knowledge_roots() {
    let a = ActorId::from_raw("sv3n1");
    let old_subject = ActorId::from_raw("cb947");
    let new_subject = ActorId::from_raw("k0fb1");
    let mut e = demo_engine(
        EngineConfig {
            fake_mode: true,
            knowledge_enabled: true,
            turn_delay_seconds: 0.0,
            checkpoint_host_image: Some([7; 32]),
            checkpoint_world_identity: Some(WorldIdentity::from_bytes([9; 16]).unwrap()),
            ..Default::default()
        },
        Box::new(Recorded(0)),
    );
    e.scheduler.close();
    // Ordinary accepted spatial sample makes the synthetic f32 Host body exact.
    let p = e.world.characters[&e.config.player_id].position_m();
    e.poll(
        0.0,
        vec![EngineCommand::SpatialUpdate {
            spatial_seq: 1,
            updates: vec![crate::SpatialActorUpdate::new(
                e.config.player_id.clone(),
                Vec3::new(
                    f64::from(p.x as f32),
                    f64::from(p.y as f32),
                    f64::from(p.z as f32),
                ),
                Some(0.0),
            )],
        }],
    );
    // Start one explicit ordinary turn after installing the physical sample.
    e.scheduler = NpcScheduler::new(vec![a.clone()], 0.0, 60.0, 0.0);
    e.scheduler.start(0.0);
    let old_key = add_context(&mut e, &a, &old_subject, "original complete context");
    e.scheduler.poll(
        0.0,
        &mut e.world,
        &mut e.transcript,
        &mut vec![],
        false,
        crate::attention::IdleGate::All,
        e.cognition.as_mut(),
        &e.env,
    );
    e.scheduler.take_submitted();
    e.world.command_ledger.drain_updates();
    let h = host(&e);
    let b = CheckpointBudget::default();
    let running = b
        .reserve(Cohort::Running, complete::RUNNING_AUTHORITY_ALLOWANCE_BYTES)
        .unwrap();
    let saved = complete::capture(
        &e,
        &h,
        Profile::Authored,
        b.reserve(Cohort::SavePayload, 4096).unwrap(),
    )
    .unwrap();
    let prepared = reload(saved, &e, &h, &b, 2);
    let first = prepared
        .value()
        .capture(
            Profile::Authored,
            b.reserve(Cohort::SavePayload, 4096).unwrap(),
        )
        .unwrap();
    let first_wire: serde_json::Value = serde_json::from_slice(first.value().bytes()).unwrap();
    drop(first);
    let original = first_wire["scheduler"]["continuation"]["load_retries"][0].clone();
    assert!(
        original["context"]["seated"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!(old_key.0))
    );
    // This private, cfg(test)-only stand-in changes the same prepared Engine at
    // the same accepted time; production still exposes no Engine or poll.
    let mut new_key = None;
    let prepared = prepared
        .try_map(|mut p, _| {
            p.engine.cognition = Box::new(Recorded(20));
            new_key = Some(add_context(
                &mut p.engine,
                &a,
                &new_subject,
                "newer complete context",
            ));
            p.engine
                .scheduler
                .prioritize_player_reaction(&p.engine.world, &a, 0.0);
            p.engine.scheduler.poll(
                0.0,
                &mut p.engine.world,
                &mut p.engine.transcript,
                &mut vec![],
                false,
                crate::attention::IdleGate::All,
                p.engine.cognition.as_mut(),
                &p.engine.env,
            );
            p.engine.scheduler.take_submitted();
            p.engine.world.command_ledger.drain_updates();
            Ok(p)
        })
        .unwrap();
    let mixed = prepared
        .value()
        .capture(
            Profile::Authored,
            b.reserve(Cohort::SavePayload, 4096).unwrap(),
        )
        .unwrap();
    let wire: serde_json::Value = serde_json::from_slice(mixed.value().bytes()).unwrap();
    assert_eq!(
        wire["scheduler"]["continuation"]["load_retries"][0],
        original
    );
    assert!(!wire["scheduler"]["base"]["scheduler"]["in_flight"].is_null());
    assert!(!wire["cognition_inputs"]["scheduler"].is_null());
    assert_ne!(new_key.unwrap(), old_key);
    drop(prepared);
    let prepared = reload(mixed, &e, &h, &b, 3);
    assert_eq!(prepared.value().report().scheduler_retries, 2);
    let saved = prepared
        .value()
        .capture(
            Profile::Authored,
            b.reserve(Cohort::SavePayload, 4096).unwrap(),
        )
        .unwrap();
    let both: serde_json::Value = serde_json::from_slice(saved.value().bytes()).unwrap();
    assert!(
        both["scheduler"]["continuation"]["load_retries"][1]["context"]["seated"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!(new_key.unwrap().0))
    );
    let raw = saved.value().bytes().to_vec();
    drop(prepared);
    let prepared = reload(saved, &e, &h, &b, 4);
    let again = prepared
        .value()
        .capture(
            Profile::Authored,
            b.reserve(Cohort::SavePayload, 4096).unwrap(),
        )
        .unwrap();
    assert_eq!(again.value().bytes(), raw);
    drop(again);
    drop(prepared);
    let d = InstalledCheckpointDefinitions::from_engine(
        &e,
        h.0.definitions,
        crate::timeline::LogicalTime::new(0.0).unwrap(),
    )
    .unwrap();
    let mut invalid: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    invalid["scheduler"]["continuation"]["load_retries"][0]["context"]["seated"] =
        serde_json::json!([u32::MAX]);
    let invalid = serde_json::to_vec(&invalid).unwrap();
    let error = complete::validate(
        CompleteCheckpointInput::copy_from(
            &invalid,
            b.reserve(Cohort::LoadCandidate, 4096).unwrap(),
        )
        .unwrap(),
        &d,
    )
    .unwrap_err();
    assert_eq!(error.owner, "scheduler");
    assert!(error.reason.contains("allocation"));
    let mut invalid: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    let roots = invalid["ledger"]["protected"].as_array_mut().unwrap();
    let position = roots
        .iter()
        .position(|root| *root == original["flight"]["semantic"])
        .unwrap();
    roots.remove(position);
    let invalid = serde_json::to_vec(&invalid).unwrap();
    let error = complete::validate(
        CompleteCheckpointInput::copy_from(
            &invalid,
            b.reserve(Cohort::LoadCandidate, 4096).unwrap(),
        )
        .unwrap(),
        &d,
    )
    .unwrap_err();
    assert_eq!(error.owner, "scheduler");
    assert!(error.reason.contains("root disagreement"));
    assert_eq!(b.retained_bytes(), running.bytes());
}
