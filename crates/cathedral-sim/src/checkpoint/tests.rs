use super::*;
use crate::{
    ActorId, Character, Office, Vec3, World, WorldClock, WorldSeed,
    operations::{
        self, FixtureDeclaration, OperationConfig, OperationKernel, OperationKernelDtoV1, Request,
    },
    receipts::{
        self, Admission, AffectedRef, CommandId, CommandLedger, CommandLedgerDtoV1, OperationId,
        Outcome, ReceiptState,
    },
    round::Round,
    timeline::{AcceptedTime, LogicalTime},
};
use serde_json::{Value, json};
use std::{collections::BTreeSet, time::Duration};

fn now() -> LogicalTime {
    LogicalTime::new(1.0).unwrap()
}
fn charge(budget: &CheckpointBudget, cohort: Cohort) -> Reservation {
    budget.reserve(cohort, 40 * 1024 * 1024).unwrap()
}
fn id(n: u64) -> CommandId {
    OperationId {
        producer: receipts::HOST_PRODUCER,
        sequence: n,
    }
    .command(0)
}
fn commit(ledger: &mut CommandLedger, id: CommandId, outcome: Outcome, refs: Vec<AffectedRef>) {
    let Admission::New(ticket) = ledger.begin(id, &json!({"x": 1})) else {
        panic!("new ticket");
    };
    ledger.finish(ticket, now().seconds(), outcome, refs);
}
fn ledger_wire(ledger: &CommandLedger, budget: &CheckpointBudget) -> Admitted<Vec<u8>> {
    let dto = ledger
        .checkpoint_v1(now(), charge(budget, Cohort::SavePayload))
        .unwrap();
    CommandLedgerDtoV1::encode_json(dto, now()).unwrap()
}
fn decode_ledger(v: &Value, budget: &CheckpointBudget) -> Result<Admitted<CommandLedgerDtoV1>> {
    CommandLedgerDtoV1::decode_json(
        &serde_json::to_vec(v).unwrap(),
        now(),
        charge(budget, Cohort::LoadCandidate),
    )
}

#[test]
fn admission_tracks_all_cohorts_and_attached_values_across_threads() {
    fn assert_send<T: Send>() {}
    assert_send::<Admitted<Vec<u8>>>();
    let budget = CheckpointBudget::default();
    let running = budget
        .reserve(Cohort::Running, MAX_RESIDENT_BYTES - 300)
        .unwrap();
    let save = budget.reserve(Cohort::SavePayload, 100).unwrap();
    assert!(budget.reserve(Cohort::SavePayload, 1).is_err());
    let load = budget.reserve(Cohort::LoadCandidate, 100).unwrap();
    let mut old = budget.reserve(Cohort::RetiringGeneration, 100).unwrap();
    assert_eq!(budget.retained_bytes(), MAX_RESIDENT_BYTES);
    assert!(old.resize(101).is_err());
    let retained = Admitted::new(vec![0u8; 100], save);
    drop(load);
    old.resize(200).unwrap();
    assert!(budget.reserve(Cohort::SavePayload, 1).is_err());
    std::thread::spawn(move || {
        assert_eq!(retained.value().len(), 100);
        drop(retained);
    })
    .join()
    .unwrap();
    assert!(budget.reserve(Cohort::SavePayload, 100).is_ok());
    drop((old, running));
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn malformed_decode_and_failed_export_release_admission_without_allocating_owner() {
    let budget = CheckpointBudget::default();
    let insufficient = budget
        .reserve(Cohort::LoadCandidate, CommandLedgerDtoV1::WORKING_BYTES)
        .unwrap();
    assert!(
        CommandLedgerDtoV1::decode_json(b"{}", now(), insufficient)
            .unwrap_err()
            .reason
            .contains("not reserved")
    );
    assert_eq!(budget.retained_bytes(), 0);
    let too_large = vec![b' '; CommandLedgerDtoV1::MAX_ENCODED_BYTES + 1];
    assert!(
        CommandLedgerDtoV1::decode_json(&too_large, now(), charge(&budget, Cohort::LoadCandidate))
            .is_err()
    );
    let mut ledger = CommandLedger::default();
    assert!(matches!(ledger.begin(id(1), &json!({})), Admission::New(_)));
    assert!(
        ledger
            .checkpoint_v1(now(), charge(&budget, Cohort::SavePayload))
            .is_err()
    );
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn accepted_time_preserves_debt_and_both_residuals_without_offline_work() {
    let time = AcceptedTime {
        elapsed: Duration::from_secs(10),
        debt: Duration::from_millis(400),
        wall: Duration::from_millis(10400),
    };
    let dto = HostTimeV1::from_accepted(
        time,
        Duration::from_millis(20),
        Duration::from_millis(15),
        0.025,
    )
    .unwrap();
    let wire = serde_json::to_vec(&dto).unwrap();
    let restored: HostTimeV1 = serde_json::from_slice(&wire).unwrap();
    assert_eq!(dto, restored);
    assert_eq!(restored.accepted().unwrap(), time);
    assert_eq!(
        restored.fixed_residual().unwrap(),
        Duration::from_millis(15)
    );
    assert_eq!(restored.movement_residual().unwrap(), 0.025);
    let mut continued = restored.accepted().unwrap();
    assert_eq!(
        continued.admit(Duration::ZERO).accepted_delta,
        Duration::from_millis(100)
    );
    assert_eq!(continued.debt, Duration::from_millis(300));
    for (field, value) in [
        ("ordinary_debt", json!({"seconds": 0, "nanos": 0})),
        ("fixed_residual", json!({"seconds": 0, "nanos": 20_000_000})),
        ("elapsed", json!({"seconds": 0, "nanos": 1_000_000_000})),
    ] {
        let mut bad = serde_json::to_value(dto).unwrap();
        bad[field] = value;
        assert!(
            serde_json::from_value::<HostTimeV1>(bad)
                .unwrap()
                .validate()
                .is_err()
        );
    }
}

#[test]
fn only_explicit_negative_infinity_becomes_never_and_negative_calendar_is_valid() {
    assert_eq!(
        serde_json::to_string(&LogicalAnchorV1::from_legacy(f64::NEG_INFINITY).unwrap()).unwrap(),
        "\"never\""
    );
    assert_eq!(
        serde_json::to_string(&CalendarAnchorV1::from_legacy(f64::NEG_INFINITY).unwrap()).unwrap(),
        "\"never\""
    );
    assert!(CalendarAnchorV1::from_legacy(-1.5).is_ok());
    assert!(LogicalAnchorV1::from_legacy(-1.5).is_err());
    for invalid in [f64::INFINITY, f64::NAN, MAX_LOGICAL_SECONDS * 2.0] {
        assert!(LogicalAnchorV1::from_legacy(invalid).is_err());
    }
    for invalid in [f64::INFINITY, f64::NAN, MAX_CALENDAR_DAYS * 2.0] {
        assert!(CalendarAnchorV1::from_legacy(invalid).is_err());
    }
    assert!(serde_json::from_str::<LogicalAnchorV1>("{\"at\":null}").is_err());
}

fn manifest() -> CompatibilityManifestV1 {
    CompatibilityManifestV1::new(
        [1; 32],
        [2; 32],
        [3; 32],
        VersionedAlgorithmV1::new("resident_generator", 1, [4; 32]).unwrap(),
        VersionedAlgorithmV1::new("std_default_hasher", 1, [5; 32]).unwrap(),
    )
    .unwrap()
}
#[test]
fn manifest_requires_exact_content_geometry_behavior_generator_and_hash_build() {
    let original = manifest();
    original.require_exact(&original).unwrap();
    for field in ["content_sha256", "geometry_sha256", "behavior_sha256"] {
        let mut changed = serde_json::to_value(&original).unwrap();
        changed[field][0] = json!(99);
        let candidate: CompatibilityManifestV1 = serde_json::from_value(changed).unwrap();
        assert!(candidate.require_exact(&original).is_err());
    }
    for field in ["generator", "procedural_hash"] {
        let mut changed = serde_json::to_value(&original).unwrap();
        changed[field]["implementation_sha256"][0] = json!(99);
        assert!(
            serde_json::from_value::<CompatibilityManifestV1>(changed)
                .unwrap()
                .require_exact(&original)
                .is_err()
        );
    }
    for field in [
        "schema_version",
        "command_payload_version",
        "provider_reply_version",
    ] {
        let mut changed = serde_json::to_value(&original).unwrap();
        changed[field] = json!(999);
        assert!(
            serde_json::from_value::<CompatibilityManifestV1>(changed)
                .unwrap()
                .validate()
                .is_err()
        );
    }
    let mut changed = serde_json::to_value(&original).unwrap();
    changed["extra"] = json!(true);
    assert!(serde_json::from_value::<CompatibilityManifestV1>(changed).is_err());
}

#[test]
fn clock_rate_segment_and_calendar_bits_roundtrip_without_reconstruction() {
    let original = WorldClock::new(300.0, Office::Snuffing, -2, 0.05).with_scale(0.1, 60.0);
    let dto = original.checkpoint_v1(now()).unwrap();
    let restored: crate::clock::WorldClockDtoV1 =
        serde_json::from_slice(&serde_json::to_vec(&dto).unwrap()).unwrap();
    assert_eq!(dto, restored);
    restored
        .validate_position(now(), original.game_days(1.0))
        .unwrap();
    assert!(
        restored
            .validate_position(now(), original.game_days(1.0) + 1e-10)
            .is_err()
    );
    for (field, value) in [
        ("elapsed_origin", json!(2.0)),
        ("scale", json!(0)),
        ("epoch_days", json!(MAX_CALENDAR_DAYS * 2.0)),
        ("night_brightness", json!(1.1)),
    ] {
        let mut bad = serde_json::to_value(dto).unwrap();
        bad[field] = value;
        assert!(
            serde_json::from_value::<crate::clock::WorldClockDtoV1>(bad)
                .unwrap()
                .validate(now())
                .is_err()
        );
    }
    // Known non-dyadic floats plus a deterministic exponent/mantissa sample
    // catch accidental loss of serde_json's correctly rounded parse feature.
    for n in 1..10_000u64 {
        let t = f64::from_bits(
            0x3fe0_0000_0000_0000 | n.wrapping_mul(0x12345_6789ab) & 0x000f_ffff_ffff_ffff,
        );
        let anchor = LogicalAnchorV1::from_legacy(t).unwrap();
        let decoded: LogicalAnchorV1 =
            serde_json::from_slice(&serde_json::to_vec(&anchor).unwrap()).unwrap();
        assert_eq!(anchor.legacy().to_bits(), decoded.legacy().to_bits());
    }
}

#[test]
fn ledger_empty_and_saturated_replay_keep_holes_floors_and_unfinished_roots() {
    let budget = CheckpointBudget::default();
    let mut ledger = CommandLedger::default();
    let empty = ledger_wire(&ledger, &budget);
    let decoded = CommandLedgerDtoV1::decode_json(
        empty.value(),
        now(),
        charge(&budget, Cohort::LoadCandidate),
    )
    .unwrap();
    assert_eq!(decoded.value().candidate(now()).unwrap(), ledger);
    drop((empty, decoded));
    commit(&mut ledger, id(10), Outcome::completed("first"), vec![]);
    ledger.protect(id(10).operation).unwrap();
    commit(
        &mut ledger,
        id(2),
        Outcome::completed("out of order"),
        vec![],
    );
    let unfinished = ledger.reserve_operation(receipts::TURN_PRODUCER).unwrap();
    for n in 11..=4105 {
        commit(&mut ledger, id(n), Outcome::completed("filled"), vec![]);
    }
    assert_eq!(ledger.recent_len(), receipts::RECENT_CAPACITY);
    assert_eq!(ledger.retained_len(), 1);
    assert_eq!(ledger.producers[1].compacted_floor, 10);
    assert_eq!(ledger.producers[1].issued, 0);
    let wire = ledger_wire(&ledger, &budget);
    let loaded = CommandLedgerDtoV1::decode_json(
        wire.value(),
        now(),
        charge(&budget, Cohort::LoadCandidate),
    )
    .unwrap();
    loaded
        .value()
        .validate_owner_roots(&BTreeSet::from([id(10).operation, unfinished]))
        .unwrap();
    assert!(
        loaded
            .value()
            .validate_owner_roots(&BTreeSet::new())
            .is_err()
    );
    let mut restored = loaded.value().candidate(now()).unwrap();
    assert_eq!(ledger, restored);
    for retry in [id(10), id(2), id(4105)] {
        assert!(matches!(
            restored.begin(retry, &json!({"x": 1})),
            Admission::Replay(_)
        ));
    }
    assert!(
        matches!(restored.begin(id(3), &json!({"x": 1})), Admission::Refused(o) if o.code == "command_expired")
    );
    assert!(
        matches!(restored.begin(id(2), &json!({"x": 2})), Admission::Refused(o) if o.code == "payload_conflict")
    );
    assert!(restored.is_protected(unfinished));
    assert!(restored.get(unfinished.command(0)).is_none());
}

#[test]
fn receipt_principals_preserve_historical_and_rejected_absent_targets() {
    let budget = CheckpointBudget::default();
    let mut ledger = CommandLedger::default();
    commit(
        &mut ledger,
        id(1),
        Outcome::completed("eaten"),
        vec![AffectedRef::new("item", "consumed_item").unwrap()],
    );
    commit(
        &mut ledger,
        id(2),
        Outcome::rejected("unknown_actor", "absent"),
        vec![
            AffectedRef::new("actor", "never_existed").unwrap(),
            AffectedRef::new("fixture", "absent_workbench").unwrap(),
        ],
    );
    let wire = ledger_wire(&ledger, &budget);
    let decoded = CommandLedgerDtoV1::decode_json(
        wire.value(),
        now(),
        charge(&budget, Cohort::LoadCandidate),
    )
    .unwrap();
    assert_eq!(decoded.value().candidate(now()).unwrap(), ledger);
}

#[test]
fn ledger_rejects_duplicate_ids_ordinals_roots_and_invalid_numeric_or_string_fields() {
    let budget = CheckpointBudget::default();
    let mut ledger = CommandLedger::default();
    commit(&mut ledger, id(1), Outcome::completed("first"), vec![]);
    commit(&mut ledger, id(2), Outcome::completed("second"), vec![]);
    ledger.protect(id(1).operation).unwrap();
    let wire = ledger_wire(&ledger, &budget);
    let base: Value = serde_json::from_slice(wire.value()).unwrap();
    let changes = [
        ("/version", json!(2)),
        (
            "/recent/1/receipt/id",
            base["recent"][0]["receipt"]["id"].clone(),
        ),
        ("/recent/1/receipt/ordinal", json!(1)),
        ("/recent/0/receipt/at", json!(-1)),
        ("/recent/0/receipt/at", json!(2.0)),
        ("/recent/0/receipt/id/operation/producer", json!(32)),
        ("/recent/0/receipt/id/operation/sequence", json!(0)),
        ("/recent/0/receipt/id/step", json!(257)),
        ("/recent/0/payload_version", json!(2)),
        ("/recent/0/receipt/outcome/code", json!("dispatch_pending")),
        ("/recent/0/receipt/outcome/message", json!("a".repeat(193))),
        ("/recent/0/receipt/outcome/message", json!("\n")),
        ("/producers/1/compacted_floor", json!(3)),
        (
            "/protected",
            json!([base["protected"][0], base["protected"][0]]),
        ),
        ("/retained", json!([base["recent"][1]])),
    ];
    for (path, value) in changes {
        let mut bad = base.clone();
        *bad.pointer_mut(path).unwrap() = value;
        assert!(
            decode_ledger(&bad, &budget).is_err(),
            "accepted {path}: {bad}"
        );
    }
    let mut unknown = base.clone();
    unknown["recent"][0]["receipt"]["id"]["extra"] = json!(1);
    assert!(decode_ledger(&unknown, &budget).is_err());
    let text = String::from_utf8(wire.value().clone()).unwrap().replacen(
        "\"version\":1",
        "\"version\":1,\"version\":1",
        1,
    );
    assert!(
        CommandLedgerDtoV1::decode_json(
            text.as_bytes(),
            now(),
            charge(&budget, Cohort::LoadCandidate)
        )
        .is_err()
    );
}

#[test]
fn wire_rejects_extra_records_huge_escaped_strings_and_deep_json_before_owner_use() {
    let budget = CheckpointBudget::default();
    let mut ledger = CommandLedger::default();
    commit(
        &mut ledger,
        id(1),
        Outcome::completed("message_marker"),
        vec![],
    );
    let wire = ledger_wire(&ledger, &budget);
    let base = String::from_utf8(wire.value().clone()).unwrap();
    let over = base.replace(
        "\"protected\":[]",
        &format!(
            "\"protected\":[{}]",
            vec!["{\"producer\":1,\"sequence\":1}"; 257].join(",")
        ),
    );
    assert!(
        CommandLedgerDtoV1::decode_json(
            over.as_bytes(),
            now(),
            charge(&budget, Cohort::LoadCandidate)
        )
        .is_err()
    );
    let huge = base.replace("message_marker", &"\\u0061".repeat(20_000));
    assert!(
        CommandLedgerDtoV1::decode_json(
            huge.as_bytes(),
            now(),
            charge(&budget, Cohort::LoadCandidate)
        )
        .unwrap_err()
        .reason
        .contains("string byte limit")
    );
    let valid = base.replace("message_marker", &"é".repeat(96));
    assert!(
        CommandLedgerDtoV1::decode_json(
            valid.as_bytes(),
            now(),
            charge(&budget, Cohort::LoadCandidate)
        )
        .is_ok()
    );
    let invalid = base.replace("message_marker", &"é".repeat(97));
    assert!(
        CommandLedgerDtoV1::decode_json(
            invalid.as_bytes(),
            now(),
            charge(&budget, Cohort::LoadCandidate)
        )
        .unwrap_err()
        .reason
        .contains("string byte limit")
    );
    let mut too_many: Value = serde_json::from_str(&base).unwrap();
    too_many["recent"] = json!(vec![
        too_many["recent"][0].clone();
        receipts::RECENT_CAPACITY + 1
    ]);
    assert!(
        decode_ledger(&too_many, &budget)
            .unwrap_err()
            .reason
            .contains("count limit")
    );
    let deep = format!("{}0{}", "[".repeat(200), "]".repeat(200));
    assert!(
        CommandLedgerDtoV1::decode_json(
            deep.as_bytes(),
            now(),
            charge(&budget, Cohort::LoadCandidate)
        )
        .is_err()
    );
    let empty: BoundedVec<u8, 0> = serde_json::from_str("[]").unwrap();
    assert!(empty.0.is_empty());
    assert!(serde_json::from_str::<BoundedVec<u8, 0>>("[0]").is_err());
}

#[test]
fn exhausted_command_counters_remain_exhausted_after_component_roundtrip() {
    let budget = CheckpointBudget::default();
    let wire = ledger_wire(&CommandLedger::default(), &budget);
    let mut value: Value = serde_json::from_slice(wire.value()).unwrap();
    value["producers"][1]["issued"] = json!(u64::MAX);
    value["next_ordinal"] = json!(u64::MAX);
    let decoded = decode_ledger(&value, &budget).unwrap();
    let mut ledger = decoded.value().candidate(now()).unwrap();
    assert!(ledger.issue(1).is_err());
    assert!(
        matches!(ledger.begin(id(1), &json!({})), Admission::Refused(o) if o.code == "identity_exhausted")
    );
}

fn operations_world(count: usize) -> (World, Round, WorldClock, OperationConfig) {
    let mut world = World::new();
    let mut round = Round::new();
    let clock = WorldClock::new(3600.0, Office::Dayspring, 0, 0.05);
    let template = WorldSeed::from_json_str(include_str!("../../tests/fixtures/demo_seed.json"))
        .unwrap()
        .characters[0]
        .clone();
    let mut config = OperationConfig::default();
    for n in 0..count {
        let mut sheet = template.clone();
        sheet.id = ActorId::from_raw(format!("actor_{n}"));
        sheet.position_m = Vec3::ZERO;
        world.add_character(Character::from_sheet(sheet));
        config.fixtures.push(FixtureDeclaration {
            id: format!("bench_{n}"),
            adapter: Default::default(),
            position: [0.0; 3],
        });
    }
    world.operations = OperationKernel::from_config(&config).unwrap();
    for n in 0..count {
        let request = Request::Start {
            actor: ActorId::from_raw(format!("actor_{n}")),
            resource: format!("bench_{n}"),
            adapter: Default::default(),
            work_seconds: 2.0,
            recovery_seconds: 5.0,
            retries: 2,
        };
        let Admission::New(ticket) = world
            .command_ledger
            .begin(id(n as u64 + 1), &json!({"request": request}))
        else {
            panic!("admission");
        };
        let outcome = operations::command(
            &mut world,
            &mut round,
            &clock,
            now(),
            id(n as u64 + 1),
            request,
        );
        assert_eq!(outcome.state, ReceiptState::Accepted);
        world.command_ledger.finish(ticket, 1.0, outcome, vec![]);
    }
    (world, round, clock, config)
}

fn operation_wire(
    world: &World,
    config: &OperationConfig,
    budget: &CheckpointBudget,
) -> Admitted<Vec<u8>> {
    let dto = world
        .operations
        .checkpoint_v1(config, world, now(), charge(budget, Cohort::SavePayload))
        .unwrap();
    OperationKernelDtoV1::encode_json(dto, config, world, now()).unwrap()
}

#[test]
fn operation_owner_roundtrip_rebuilds_claims_and_preserves_exact_once_completion() {
    let budget = CheckpointBudget::default();
    let (mut world, round, clock, config) = operations_world(2);
    let wire = operation_wire(&world, &config, &budget);
    let decoded = OperationKernelDtoV1::decode_json(
        wire.value(),
        &config,
        &world,
        now(),
        charge(&budget, Cohort::LoadCandidate),
    )
    .unwrap();
    let candidate = decoded.value().candidate(&config, &world, now()).unwrap();
    assert_eq!(candidate, world.operations);
    world.operations = candidate;
    for _ in 0..2 {
        operations::poll(&mut world, &round, &clock, LogicalTime::new(3.0).unwrap());
    }
    for n in 0..2 {
        assert_eq!(
            world
                .operations
                .fixture(&format!("bench_{n}"))
                .unwrap()
                .completed_units,
            1
        );
        assert_eq!(
            world.command_ledger.get(id(n + 1)).unwrap().outcome.state,
            ReceiptState::Completed
        );
    }
    assert_eq!(world.operations.active_count(), 0);
    assert!(
        world
            .command_ledger
            .checkpoint_v1(
                LogicalTime::new(3.0).unwrap(),
                charge(&CheckpointBudget::default(), Cohort::SavePayload)
            )
            .is_err()
    );
    world.command_ledger.drain_updates();
    let budget2 = CheckpointBudget::default();
    assert!(
        world
            .command_ledger
            .checkpoint_v1(
                LogicalTime::new(3.0).unwrap(),
                charge(&budget2, Cohort::SavePayload)
            )
            .is_ok()
    );
}

#[test]
fn maximum_operation_owner_and_empty_owner_fit_and_roundtrip() {
    for count in [0, operations::MAX_INSTANCES] {
        let budget = CheckpointBudget::default();
        let (world, _, _, config) = operations_world(count);
        let wire = operation_wire(&world, &config, &budget);
        let decoded = OperationKernelDtoV1::decode_json(
            wire.value(),
            &config,
            &world,
            now(),
            charge(&budget, Cohort::LoadCandidate),
        )
        .unwrap();
        assert_eq!(
            decoded.value().candidate(&config, &world, now()).unwrap(),
            world.operations
        );
        assert!(wire.value().len() <= OperationKernelDtoV1::MAX_ENCODED_BYTES);
    }
}

#[test]
fn operation_wire_rejects_duplicates_impossible_claims_deadlines_and_manifest_mismatch() {
    let budget = CheckpointBudget::default();
    let (world, _, _, config) = operations_world(2);
    let wire = operation_wire(&world, &config, &budget);
    let base: Value = serde_json::from_slice(wire.value()).unwrap();
    let changes = [
        ("/version", json!(9)),
        ("/active/0/step_index", json!(1)),
        ("/active/1/instance", base["active"][0]["instance"].clone()),
        ("/active/1/actor", base["active"][0]["actor"].clone()),
        ("/active/1/resource", base["active"][0]["resource"].clone()),
        ("/fixtures/1/id", base["fixtures"][0]["id"].clone()),
        ("/active/0/actor", json!("absent")),
        ("/active/0/presence_epoch", json!(99)),
        ("/active/0/adapter/version", json!(99)),
        ("/fixtures/0/position/0", json!(1)),
        ("/active/0/last_observed_at", json!(2)),
        ("/active/0/last_progress_at", json!(0)),
        ("/active/0/recovery_deadline", json!(1)),
        ("/active/0/recovery_deadline", json!(3)),
        ("/active/0/completed_work", json!(2)),
        ("/active/0/retries_spent", json!(3)),
        ("/active/0/plan_revision", json!(1)),
        ("/active/0/running", json!(true)),
        ("/active/0/accepted_at", json!(MAX_LOGICAL_SECONDS * 2.0)),
    ];
    for (path, value) in changes {
        let mut bad = base.clone();
        *bad.pointer_mut(path).unwrap() = value;
        let bytes = serde_json::to_vec(&bad).unwrap();
        assert!(
            OperationKernelDtoV1::decode_json(
                &bytes,
                &config,
                &world,
                now(),
                charge(&budget, Cohort::LoadCandidate)
            )
            .is_err(),
            "accepted {path}: {bad}"
        );
    }
    let mut no_root = world.clone();
    no_root.command_ledger.unprotect(id(1).operation);
    assert!(
        OperationKernelDtoV1::decode_json(
            wire.value(),
            &config,
            &no_root,
            now(),
            charge(&budget, Cohort::LoadCandidate)
        )
        .is_err()
    );
    let mut completed = world.clone();
    completed
        .command_ledger
        .advance(id(1), 1.0, Outcome::completed("done"))
        .unwrap();
    assert!(
        OperationKernelDtoV1::decode_json(
            wire.value(),
            &config,
            &completed,
            now(),
            charge(&budget, Cohort::LoadCandidate)
        )
        .is_err()
    );
    let mut wrong = config.clone();
    wrong.fixtures[0].adapter.version = 99;
    assert!(
        OperationKernelDtoV1::decode_json(
            wire.value(),
            &wrong,
            &world,
            now(),
            charge(&budget, Cohort::LoadCandidate)
        )
        .is_err()
    );
    let mut absent = world.clone();
    absent.characters.remove(&ActorId::from_raw("actor_0"));
    assert!(
        OperationKernelDtoV1::decode_json(
            wire.value(),
            &config,
            &absent,
            now(),
            charge(&budget, Cohort::LoadCandidate)
        )
        .is_err()
    );
}

#[test]
fn operation_credit_cannot_exceed_elapsed_work_and_keeps_obstruction_gaps() {
    let budget = CheckpointBudget::default();
    let (mut world, mut round, clock, config) = operations_world(1);
    let mut logical_now = 1.0;
    for n in 1..=300 {
        logical_now += 0.001;
        operations::poll(
            &mut world,
            &round,
            &clock,
            LogicalTime::new(logical_now).unwrap(),
        );
        if n == 100 || n == 200 {
            let outcome = operations::command(
                &mut world,
                &mut round,
                &clock,
                LogicalTime::new(logical_now).unwrap(),
                id(2 + n),
                Request::SetObstructed {
                    instance: operations::InstanceId(id(1)),
                    obstructed: n == 100,
                },
            );
            assert!(outcome.succeeded());
        }
    }
    world.command_ledger.drain_updates();
    let at = LogicalTime::new(logical_now).unwrap();
    let dto = world
        .operations
        .checkpoint_v1(&config, &world, at, charge(&budget, Cohort::SavePayload))
        .unwrap();
    let op = world
        .operations
        .active(operations::InstanceId(id(1)))
        .unwrap();
    assert!((op.completed_work - 0.2).abs() < 1e-10);
    let wire = OperationKernelDtoV1::encode_json(dto, &config, &world, at).unwrap();
    let loaded = OperationKernelDtoV1::decode_json(
        wire.value(),
        &config,
        &world,
        at,
        charge(&budget, Cohort::LoadCandidate),
    )
    .unwrap();
    assert_eq!(
        loaded.value().candidate(&config, &world, at).unwrap(),
        world.operations
    );
    drop(loaded);
    let mut forged: Value = serde_json::from_slice(wire.value()).unwrap();
    forged["active"][0]["completed_work"] = json!(1.5);
    assert!(
        OperationKernelDtoV1::decode_json(
            &serde_json::to_vec(&forged).unwrap(),
            &config,
            &world,
            at,
            charge(&budget, Cohort::LoadCandidate)
        )
        .is_err()
    );
    // A false zero-credit anchor cannot claim that positive work occurred.
    forged["active"][0]["completed_work"] = json!(0);
    forged["active"][0]["running"] = json!(false);
    assert!(
        OperationKernelDtoV1::decode_json(
            &serde_json::to_vec(&forged).unwrap(),
            &config,
            &world,
            at,
            charge(&budget, Cohort::LoadCandidate)
        )
        .is_err()
    );
}

fn fixture_payloads() -> Vec<(&'static str, Vec<u8>)> {
    let (world, _, clock, config) = operations_world(1);
    let budget = CheckpointBudget::default();
    let ledger = ledger_wire(&world.command_ledger, &budget).value().clone();
    let operations = operation_wire(&world, &config, &budget).value().clone();
    let host_time = HostTimeV1::from_accepted(
        AcceptedTime {
            elapsed: Duration::from_secs(10),
            debt: Duration::from_millis(400),
            wall: Duration::from_millis(10400),
        },
        Duration::from_millis(20),
        Duration::from_millis(15),
        0.025,
    )
    .unwrap();
    vec![
        ("ledger", ledger),
        ("operations", operations),
        ("manifest", serde_json::to_vec(&manifest()).unwrap()),
        ("host_time", serde_json::to_vec(&host_time).unwrap()),
        (
            "clock",
            serde_json::to_vec(&clock.checkpoint_v1(now()).unwrap()).unwrap(),
        ),
    ]
}

#[test]
fn supported_component_fixtures_pin_owner_wire_without_claiming_a_complete_save() {
    let supported: [&[u8]; 5] = [
        include_bytes!("../../tests/fixtures/checkpoint_v1/ledger.json"),
        include_bytes!("../../tests/fixtures/checkpoint_v1/operations.json"),
        include_bytes!("../../tests/fixtures/checkpoint_v1/manifest.json"),
        include_bytes!("../../tests/fixtures/checkpoint_v1/host_time.json"),
        include_bytes!("../../tests/fixtures/checkpoint_v1/clock.json"),
    ];
    for ((name, expected), actual) in fixture_payloads().into_iter().zip(supported) {
        assert_eq!(actual, expected, "supported v1 component {name}");
    }
    let budget = CheckpointBudget::default();
    let (mut world, _, _, config) = operations_world(1);
    let loaded = CommandLedgerDtoV1::decode_json(
        supported[0],
        now(),
        charge(&budget, Cohort::LoadCandidate),
    )
    .unwrap();
    world.command_ledger = loaded.value().candidate(now()).unwrap();
    drop(loaded);
    let loaded = OperationKernelDtoV1::decode_json(
        supported[1],
        &config,
        &world,
        now(),
        charge(&budget, Cohort::LoadCandidate),
    )
    .unwrap();
    assert_eq!(
        loaded.value().candidate(&config, &world, now()).unwrap(),
        world.operations
    );
}

#[test]
#[ignore = "explicit supported-component fixture regeneration only"]
fn regenerate_checkpoint_component_fixtures() {
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/checkpoint_v1");
    for (name, bytes) in fixture_payloads() {
        std::fs::write(directory.join(format!("{name}.json")), bytes).unwrap();
    }
}
