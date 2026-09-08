use super::*;
use crate::{
    character::{
        ActiveGesture, BodySlot, GutEntry, IntentTarget, Movement, Patrol, PocketedUnit, RoundEdit,
        TravelIntent,
    },
    checkpoint::{CheckpointBudget, Cohort},
    inventory::{ReservedInput, StockSpec, TransformJob},
    places::PlaceEntry,
};
use serde_json::{Value, json};
fn actor(id: &str) -> Character {
    let sheet=serde_json::from_value(json!({"id":id,"name":"Same","control":"llm","back_story":"private","location_description":"here","voice_key":null,"position_m":{"x":0.0,"y":0.0,"z":0.0},"holds":["historical"],"frontbutt":true})).unwrap();
    let mut c = Character::from_sheet(sheet);
    c.state.holds.clear();
    c
}
fn fixture() -> World {
    let a = ActorId::from_raw("a");
    let b = ActorId::from_raw("b");
    let stock = ItemId::from_raw("stock");
    let mut w = World::new();
    w.characters.insert(a.clone(), actor("a"));
    w.characters.insert(b.clone(), actor("b"));
    w.roster = vec![b.clone(), a.clone()];
    w.items.insert(
        stock.clone(),
        Item {
            id: stock.clone(),
            kind: "spark".into(),
            quantity: 20,
            metadata: BTreeMap::new(),
        },
    );
    w.characters
        .get_mut(&a)
        .unwrap()
        .state
        .holds
        .push(stock.clone());
    w.add_legacy_restock(
        &a,
        "fixture_restock",
        &StockSpec {
            kind: "spark".into(),
            metadata: BTreeMap::new(),
            quantity: 3,
        },
        "restock:1",
    )
    .unwrap();
    for (id, name) in [("p1", "Same's house"), ("p2", "Same's house")] {
        w.places
            .insert(PlaceEntry {
                id: crate::PlaceId::from_raw(id),
                name: name.into(),
                point: Vec3::ZERO,
                ward: None,
                coarse: false,
            })
            .unwrap();
    }
    let home = w.places.add_home(&a, "Same", Vec3::ZERO);
    w.places.add_home(&b, "Same", Vec3::new(1.0, 0.0, 2.0));
    w.household_doors = Arc::new(BTreeMap::from([(a.clone(), Vec3::ZERO)]));
    let c = w.characters.get_mut(&a).unwrap();
    c.state.pockets.push(PocketedUnit {
        slot: BodySlot::Mouth,
        item_id: stock.clone(),
    });
    c.state.gut.push(GutEntry {
        kind: "spark".into(),
        metadata: BTreeMap::new(),
        due_game_days: 2.9,
    });
    c.state.urgency_since_game_days = Some(-0.5);
    c.state.debug_urgency = Some(0.25);
    c.state.statuses.insert(StatusKind::Urgency, 0.25);
    c.state.knows.insert(b.clone());
    c.state.places_known.insert(home);
    c.state.movement = Some(Movement {
        exact_local: false,
        path: vec![Vec3::new(1.0, 0.0, 1.0)],
        speed: 0.3,
        gait_phase: 4.2,
        patrol: Some(Patrol {
            a: "old_a".into(),
            b: "old_b".into(),
            heading_to_b: true,
        }),
        choke_wait: 0.7,
    });
    c.state.intent = Some(TravelIntent {
        receipt: None,
        target: IntentTarget::Person {
            actor_id: b.clone(),
            last_seen: Vec3::ONE,
            visible: false,
        },
        budget_seconds: 30.0,
        deadline: Some(0.5),
    });
    c.state.round_edit = Some(RoundEdit {
        receipt: None,
        presence_epoch: Some(0),
        teach_place_on_commit: false,
        leg: 0,
        place_id: crate::PlaceId::from_raw("disappeared_place"),
    });
    c.state.daily_round = vec!["old committed round prose".into()];
    c.state.active_gesture = Some(ActiveGesture {
        kind: crate::gesture::GestureKind::Dance,
        deadline: None,
    });
    c.state.you_sell = vec![crate::character::VendorListing {
        name: "spark".into(),
        price_sparks: 1,
    }];
    c.state.resident=Some(serde_json::from_value(json!({"phase":"lingering","patch":"saved_patch","patch_description":"private frontage","spot":"saved_spot","destination_spot":null,"dwell_remaining_seconds":0.3,"resting_at_household_frontage":false,"resting_without_home":false,"optional_walk":false,"movement_cause":"other","sheltered":false})).unwrap());
    c.sheet.lore=Some(serde_json::from_value(json!({"significance":"ambient","planning_ward":"wick","age":42,"gender":"f","district":"Wick","children":[],"circumstances":[],"conditions":[],"home":"private home prose","home_point_m":[0.0,0.0],"core_character_description":"core","extended_character_description":"extended","curiosity":0.2,"generated":true,"generated_routine":{"kind":"resident","patch":"saved_patch","spot":"saved_spot"}})).unwrap());
    c.state.presence_epoch = 2;
    c.state.memories = vec!["live memory".into()];
    c.notify_percept("repeated prose");
    c.remember_percept("repeated prose");
    w.characters.get_mut(&b).unwrap().state.presence = Presence::BeyondTheWalls;
    w.needle_claim = Some(NeedleClaim {
        holder: b,
        dir: Vec3::ZERO,
    });
    w.current_time = Some(WorldTime::from_game_days(2.7));
    w.event_sequence = 10;
    w.world_revision = 8;
    w.spatial_sequence = 4;
    w.offers.insert(
        stock.clone(),
        Offer {
            item_id: stock.clone(),
            giver_id: a.clone(),
            target_id: None,
            created_seq: 10,
            quantity: 2,
        },
    );
    w.start_transform_job(TransformJob {
        job_id: "job_1".into(),
        spec_id: "recipe".into(),
        producer: a,
        production_day: 2,
        start_slot: 7,
        inputs: vec![
            ReservedInput {
                item_id: stock.clone(),
                quantity: 2,
            },
            ReservedInput {
                item_id: stock,
                quantity: 3,
            },
        ],
        outputs: vec![
            StockSpec {
                kind: "spark".into(),
                metadata: BTreeMap::new(),
                quantity: 1,
            },
            StockSpec {
                kind: "spark".into(),
                metadata: BTreeMap::new(),
                quantity: 2,
            },
        ],
        progress_work_minutes: 0.125,
    })
    .unwrap();
    w.drain_events();
    w.command_ledger.drain_updates();
    w
}
fn export(w: &World) -> Value {
    let budget = CheckpointBudget::default();
    let dto = w
        .export_backbone_checkpoint(budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap();
    serde_json::to_value(dto.value()).unwrap()
}
fn decode(value: &Value, w: &World) -> Result<Admitted<WorldBackboneDtoV1>> {
    let bytes = serde_json::to_vec(value).unwrap();
    let budget = CheckpointBudget::default();
    WorldBackboneDtoV1::decode(
        &bytes,
        budget
            .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
            .unwrap(),
        &w.item_catalog,
        &w.command_ledger,
    )
}
#[test]
fn private_backbone_round_trip_and_transform_replay_continue() {
    let mut control = fixture();
    let baseline = export(&control);
    let decoded = decode(&baseline, &control).unwrap();
    let candidate = decoded
        .into_candidate(&control.item_catalog, &control.command_ledger)
        .unwrap();
    let mut restored = World::new();
    candidate.install_for_test(&mut restored);
    assert_eq!(control, restored);
    let a = ActorId::from_raw("a");
    let b = ActorId::from_raw("b");
    let stock = ItemId::from_raw("stock");
    for w in [&mut control, &mut restored] {
        w.consume_item_quantity(&a, &stock, 1).unwrap();
        let completed = w.complete_transform_job_by_id(&a, "job_1", 2).unwrap();
        assert_eq!(completed.produced[0].item_id, completed.produced[1].item_id);
        assert_eq!(
            w.complete_transform_job_by_id(&a, "job_1", 2).unwrap(),
            completed
        );
        // Remove every live quantity after completing the job. Its historical
        // receipt and the sheet's seed holdings must still survive another capture.
        w.offers.clear();
        w.characters.get_mut(&a).unwrap().state.pockets.clear();
        let quantity = w.items[&stock].quantity;
        w.transfer_item_quantity(&a, &b, &stock, quantity, "after_completion")
            .unwrap();
        let moved = w.characters[&b].holds()[0].clone();
        w.consume_item_quantity(&b, &moved, quantity).unwrap();
        w.drain_events();
        w.assert_invariants();
    }
    assert_eq!(control, restored);
    assert_eq!(export(&control), export(&restored));
    decode(&export(&control), &control).unwrap();
}
#[test]
fn strict_backbone_corruption_never_constructs_a_candidate() {
    let w = fixture();
    let good = export(&w);
    let cases: [(&str, Value); 12] = [
        ("/roster", json!(["a", "a"])),
        ("/characters/a/sheet/id", json!("b")),
        ("/characters/a/state/needs/thirst", json!(-1)),
        ("/characters/a/state/knows", json!(["missing"])),
        ("/inventory/items/stock/quantity", json!(1)),
        ("/inventory/offers/stock/giver_id", json!("b")),
        (
            "/inventory/transform_jobs/a/inputs/0/quantity",
            json!(u32::MAX),
        ),
        (
            "/inventory/transform_jobs/a/outputs/0/quantity",
            json!(u32::MAX),
        ),
        (
            "/inventory/transform_jobs/a/progress_work_minutes",
            json!(-1),
        ),
        ("/event_sequence", json!(9)),
        ("/current_time/day", json!(3)),
        ("/places/homes/a", json!("missing")),
    ];
    for (path, value) in cases {
        let mut bad = good.clone();
        *bad.pointer_mut(path).unwrap() = value;
        assert!(decode(&bad, &w).is_err(), "accepted {path}");
    }
    let mut bad = good.clone();
    bad["inventory"]["catalog"][0] = json!(255);
    assert!(decode(&bad, &w).is_err());
    let mut bad = good.clone();
    let mut j = bad["inventory"]["transform_jobs"]["a"].clone();
    j["producer"] = json!("b");
    bad["inventory"]["transform_jobs"]["b"] = j;
    assert!(
        decode(&bad, &w)
            .unwrap_err()
            .reason
            .contains("duplicate active/completed transform identity")
    );
    for field in [
        "inventory",
        "characters",
        "places",
        "needle_claim",
        "current_time",
    ] {
        let mut bad = good.clone();
        bad.as_object_mut().unwrap().remove(field);
        assert!(decode(&bad, &w).is_err(), "accepted missing {field}");
    }
}
#[test]
fn duplicate_active_transform_id_refuses_before_mutation() {
    let mut w = fixture();
    let b = ActorId::from_raw("b");
    let input = ItemId::from_raw("second");
    w.items
        .insert(input.clone(), Item::new(input.clone(), "spark"));
    w.characters
        .get_mut(&b)
        .unwrap()
        .state
        .holds
        .push(input.clone());
    let mut job = w.transform_jobs.values().next().unwrap().clone();
    job.producer = b;
    job.inputs = vec![ReservedInput {
        item_id: input,
        quantity: 1,
    }];
    let before = w.clone();
    let error = w.start_transform_job(job).unwrap_err();
    assert_eq!(
        error.code,
        crate::inventory::InventoryErrorCode::DuplicateTransform
    );
    assert!(error.message.contains("already active"));
    assert_eq!(w, before);
}
#[test]
fn component_boundary_requires_flush_and_valid_numeric_export() {
    let mut w = fixture();
    w.event_sequence = 0;
    assert!(
        w.export_backbone_checkpoint(
            CheckpointBudget::default()
                .reserve(Cohort::SavePayload, 4096)
                .unwrap()
        )
        .is_err()
    );
    let mut w = fixture();
    w.characters
        .get_mut(&ActorId::from_raw("a"))
        .unwrap()
        .state
        .movement
        .as_mut()
        .unwrap()
        .speed = f64::NAN;
    assert!(
        w.export_backbone_checkpoint(
            CheckpointBudget::default()
                .reserve(Cohort::SavePayload, 4096)
                .unwrap()
        )
        .is_err()
    );
    let mut w = fixture();
    w.consume_item_quantity(&ActorId::from_raw("a"), &ItemId::from_raw("stock"), 1)
        .unwrap();
    // This inventory path does not necessarily emit a DomainEvent. Force a
    // real action event and retain it rather than manufacturing an empty flush.
    crate::actions::apply_action(
        &mut w,
        &ActorId::from_raw("a"),
        "retract_offer",
        &json!({"item_id":"stock"}),
    )
    .unwrap();
    assert!(
        w.export_backbone_checkpoint(
            CheckpointBudget::default()
                .reserve(Cohort::SavePayload, 4096)
                .unwrap()
        )
        .is_err()
    );
}
#[test]
fn strict_wire_and_aggregate_preflight_reject_malicious_shapes() {
    let w = fixture();
    let good = export(&w);
    let json = serde_json::to_string(&good).unwrap();
    let duplicate = json.replacen("\"quantity\":23", "\"quantity\":23,\"quantity\":23", 1);
    assert_ne!(duplicate, json);
    for (bytes, reason) in [
        (duplicate.into_bytes(), "duplicate field"),
        (
            format!("{{\"unknown\":{}{}}}", "[".repeat(65), "]".repeat(65)).into_bytes(),
            "nesting limit",
        ),
        (
            format!("{{\"unknown\":[{}]}}", vec!["[]"; 270_000].join(",")).into_bytes(),
            "aggregate expanded",
        ),
    ] {
        let budget = CheckpointBudget::default();
        let r = budget
            .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
            .unwrap();
        let error =
            WorldBackboneDtoV1::decode(&bytes, r, &w.item_catalog, &w.command_ledger).unwrap_err();
        assert!(error.reason.contains(reason), "{error}");
        assert_eq!(budget.retained_bytes(), 0);
    }
    let budget = CheckpointBudget::default();
    let running = budget
        .reserve(
            Cohort::Running,
            crate::checkpoint::MAX_RESIDENT_BYTES - 4096,
        )
        .unwrap();
    let r = budget.reserve(Cohort::SavePayload, 4096).unwrap();
    assert!(w.export_backbone_checkpoint(r).is_err());
    drop(running);
    assert_eq!(budget.retained_bytes(), 0);
}
#[test]
fn exact_supported_backbone_fixture() {
    for (w, bytes) in [
        (
            fixture(),
            include_str!("../../../tests/fixtures/checkpoint_v1/backbone.json"),
        ),
        (
            completed_fixture(),
            include_str!("../../../tests/fixtures/checkpoint_v1/backbone_completed.json"),
        ),
        (
            World::new(),
            include_str!("../../../tests/fixtures/checkpoint_v1/backbone_empty.json"),
        ),
    ] {
        let expected: Value = serde_json::from_str(bytes).unwrap();
        assert_eq!(export(&w), expected);
        decode(&expected, &w).unwrap();
    }
}
#[test]
#[ignore = "explicit supported component fixture generation"]
fn regenerate_backbone_fixture() {
    for (name, w) in [
        ("backbone", fixture()),
        ("backbone_completed", completed_fixture()),
        ("backbone_empty", World::new()),
    ] {
        std::fs::write(
            format!(
                "{}/tests/fixtures/checkpoint_v1/{name}.json",
                env!("CARGO_MANIFEST_DIR")
            ),
            serde_json::to_string_pretty(&export(&w)).unwrap() + "\n",
        )
        .unwrap();
    }
}
fn completed_fixture() -> World {
    let mut w = fixture();
    let a = ActorId::from_raw("a");
    let stock = ItemId::from_raw("stock");
    w.complete_transform_job_by_id(&a, "job_1", 2).unwrap();
    w.offers.clear();
    w.characters.get_mut(&a).unwrap().state.pockets.clear();
    let quantity = w.items[&stock].quantity;
    w.consume_item_quantity(&a, &stock, quantity).unwrap();
    w.drain_events();
    w
}

#[test]
fn populated_record_layout_has_admitted_inline_and_nested_headroom() {
    // These are the actual supported container element layouts on the bound
    // build/target. No claim is made for arbitrary DeserializeOwned types.
    use std::mem::size_of;
    let rows = [
        ("Character", size_of::<Character>()),
        ("CharacterSheet", size_of::<crate::CharacterSheet>()),
        ("CharacterState", size_of::<crate::CharacterState>()),
        ("LoreProfile", size_of::<crate::lore::LoreProfile>()),
        ("MovementOption", size_of::<Option<Movement>>()),
        ("IntentOption", size_of::<Option<TravelIntent>>()),
        (
            "ResidentOption",
            size_of::<Option<crate::round::residents::ResidentStatus>>(),
        ),
        (
            "GeneratedRoutineOption",
            size_of::<Option<crate::crowd::GeneratedRoutine>>(),
        ),
        ("Item", size_of::<Item>()),
        ("TransformJob", size_of::<TransformJob>()),
        ("StockSpec", size_of::<StockSpec>()),
        (
            "RestockShare",
            size_of::<crate::inventory::LegacyRestockShare>(),
        ),
        ("PlaceEntry", size_of::<PlaceEntry>()),
        ("String", size_of::<String>()),
        ("VecHeader", size_of::<Vec<String>>()),
    ];
    eprintln!("M2A2_LAYOUTS {}", serde_json::to_string(&rows).unwrap());
    // A minimal Character includes inline storage for its largest Option
    // variants even when null. Its enclosing explicit sheet/state fields supply
    // the unused-key/container allowance; standalone Vec<Option<Movement>> is
    // not a supported wire shape.
    let mut minimal = actor("a");
    minimal.sheet.holds.clear();
    minimal.sheet.name.clear();
    minimal.sheet.back_story.clear();
    minimal.sheet.location_description.clear();
    minimal.sheet.goal.clear();
    minimal.state.goal.clear();
    let budget = CheckpointBudget::default();
    let dto = minimal
        .export_checkpoint(budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap();
    assert!(dto.value().cost().unwrap().expanded_upper_bytes >= 4 * size_of::<Character>());
    #[derive(Serialize)]
    struct MinimalRecord<'a>(#[serde(with = "records::CharacterV1")] &'a Character);
    let record_charge = aggregate::measure(&MinimalRecord(&minimal), "layout_test")
        .unwrap()
        .expanded_upper_bytes;
    let sparse_node_bound = 11 * (size_of::<ActorId>() + size_of::<Character>()) + 128;
    eprintln!(
        "M2A2_SPARSE_CHARACTER_NODE record_charge={record_charge} node_bound={sparse_node_bound}"
    );
    assert!(record_charge >= sparse_node_bound);
    assert!(size_of::<String>() * 2 <= 64);
    for (_, size) in &rows[8..13] {
        assert!(*size * 2 <= 512, "container element exceeds proof");
    }
}

#[test]
fn escaped_real_fields_numeric_and_set_corruption_are_bounded() {
    let w = fixture();
    let good = export(&w);
    for prose in ["é".repeat(32_768), "é".repeat(32_769)] {
        let mut v = good.clone();
        v["characters"]["a"]["state"]["goal"] = json!(prose);
        assert_eq!(decode(&v, &w).is_ok(), prose.len() <= 65_536);
    }
    let mut v = good.clone();
    v["characters"]["a"]["state"]["goal"] = json!("replace_escaped");
    let bytes = serde_json::to_string(&v)
        .unwrap()
        .replace("replace_escaped", &"\\u0061".repeat(65_537))
        .into_bytes();
    let budget = CheckpointBudget::default();
    let result = WorldBackboneDtoV1::decode(
        &bytes,
        budget
            .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
            .unwrap(),
        &w.item_catalog,
        &w.command_ledger,
    );
    assert!(result.unwrap_err().reason.contains("string byte limit"));
    let bytes = serde_json::to_string(&good)
        .unwrap()
        .replacen("\"speed\":0.3", "\"speed\":1e400", 1)
        .into_bytes();
    let budget = CheckpointBudget::default();
    assert!(
        WorldBackboneDtoV1::decode(
            &bytes,
            budget
                .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
                .unwrap(),
            &w.item_catalog,
            &w.command_ledger
        )
        .is_err()
    );
    let mut v = good;
    v["characters"]["a"]["state"]["places_known"] = json!(["p1", "p1"]);
    assert!(decode(&v, &w).is_err());
}

#[test]
fn movement_polyline_patrol_and_gait_continue_from_saved_state() {
    let nav = NavData::from_parts(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/world/navigation.json"
        )),
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/world/navigation.bin"
        )),
    )
    .unwrap();
    let mut control = fixture();
    let a = ActorId::from_raw("a");
    let start = nav.node_point(0);
    let end = nav.node_point(1);
    let c = control.characters.get_mut(&a).unwrap();
    c.state.position_m = start;
    c.state.movement.as_mut().unwrap().path = nav.route_between(start, end).unwrap().points;
    let candidate = decode(&export(&control), &control)
        .unwrap()
        .into_candidate(&control.item_catalog, &control.command_ledger)
        .unwrap();
    let mut restored = World::new();
    candidate.install_for_test(&mut restored);
    for _ in 0..5 {
        assert_eq!(
            control.step_movement(0.05, &nav, None),
            restored.step_movement(0.05, &nav, None)
        );
        assert_eq!(control, restored);
    }
    assert_ne!(control.characters[&a].state.position_m, start);
    assert!(
        control.characters[&a]
            .state
            .movement
            .as_ref()
            .unwrap()
            .gait_phase
            > 4.2
    );
}

#[test]
fn ordinary_engine_digest_after_component_restore_mints_the_same_item_once() {
    use crate::{
        Capabilities, Cognition, CognitionBusy, Engine, EngineConfig, IdleCognitionMode, NullSight,
        NullTranscription, NullTts, Office, PromptEnv, RequestId, TtsBackendKind, WorldClock,
        WorldSeed,
    };
    struct Unavailable;
    impl Cognition for Unavailable {
        fn request(&mut self, _: String) -> std::result::Result<RequestId, CognitionBusy> {
            Err(CognitionBusy)
        }
    }
    fn engine() -> Engine {
        let seed = WorldSeed {
            items: vec![],
            characters: ["a", "b"]
                .map(|id| {
                    let mut sheet = actor(id).sheet;
                    sheet.holds.clear();
                    sheet
                })
                .to_vec(),
        };
        Engine::new(
            EngineConfig {
                player_id: ActorId::from_raw("a"),
                fake_mode: true,
                idle_mode: IdleCognitionMode::Stage,
                clock: WorldClock::new(3600.0, Office::Dayspring, 2, 0.05),
                ..Default::default()
            },
            &seed,
            AreaMap::default(),
            SoundCatalog::empty(),
            PromptEnv::new(
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../assets/prompts/turn.j2"
                )),
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../assets/prompts/night.j2"
                )),
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../assets/prompts/strings.toml"
                )),
            )
            .unwrap(),
            Box::new(Unavailable),
            Box::new(NullTranscription),
            Box::new(NullTts),
            Box::new(NullSight),
            Capabilities::new(false, false, false, false, false, TtsBackendKind::Off),
            (Vec3::ZERO, 0.0),
            0,
            0.0,
        )
        .unwrap()
    }
    // Both engines have independently prepared identical omitted service state.
    // This witnesses only the covered body's gut/item continuation, not a save.
    let mut control = engine();
    let mut restored = engine();
    *control.world_mut() = fixture();
    *restored.world_mut() = fixture();
    control.poll(0.0, vec![]);
    restored.poll(0.0, vec![]);
    let a = ActorId::from_raw("a");
    let due = control.world().current_time.unwrap().game_days() + 0.05 / 3600.0;
    control
        .world_mut()
        .characters
        .get_mut(&a)
        .unwrap()
        .state
        .gut[0]
        .due_game_days = due;
    let saved = export(control.world());
    let candidate = decode(&saved, control.world())
        .unwrap()
        .into_candidate(
            &control.world().item_catalog,
            &control.world().command_ledger,
        )
        .unwrap();
    candidate.install_for_test(restored.world_mut());
    assert_eq!(control.world(), restored.world());
    for now in [0.1, 0.2, 0.3] {
        let left = control.poll(now, vec![]);
        let right = restored.poll(now, vec![]);
        assert_eq!(left, right);
        assert_eq!(control.world(), restored.world());
    }
    assert!(control.world().characters[&a].state.gut.is_empty());
    assert_eq!(
        control
            .world()
            .items
            .values()
            .filter(|item| item
                .metadata
                .get("condition")
                .is_some_and(|v| v == "poopstained"))
            .count(),
        1
    );
}
