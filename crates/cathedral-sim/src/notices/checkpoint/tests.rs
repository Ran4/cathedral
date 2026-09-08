use super::*;
use crate::{
    Character, CharacterSheet, Control, GOAL_NONE, Significance, Vec3,
    checkpoint::{CheckpointBudget, Cohort},
    custody::Station,
    lore::{LoreProfile, PlanningWard},
};
use serde_json::{Value, json};
pub(crate) fn actor(s: &str) -> ActorId {
    ActorId::from_raw(s)
}
pub(crate) fn person(id: &str, x: f64, occupation: Option<&str>) -> Character {
    let lore = occupation.map(|occupation_id| LoreProfile {
        significance: Significance::Ambient,
        planning_ward: PlanningWard::Fabric,
        age: 30,
        gender: "m".into(),
        occupation_id: Some(occupation_id.into()),
        occupation_display: None,
        title: None,
        rank: None,
        faction_role: None,
        illegal_activity: None,
        district: "Fabric".into(),
        father: None,
        mother: None,
        children: Vec::new(),
        circumstances: Vec::new(),
        conditions: Vec::new(),
        home: None,
        home_point_m: None,
        core_character_description: String::new(),
        extended_character_description: String::new(),
        curiosity: None,
        generated: false,
        generated_routine: None,
    });
    Character::from_sheet(CharacterSheet {
        pockets: Vec::new(),
        frontbutt: None,
        id: ActorId::from_raw(id),
        name: id.to_uppercase(),
        control: Control::Llm,
        back_story: "test".into(),
        location_description: "test".into(),
        appearance: Default::default(),
        voice_key: None,
        position_m: Vec3::new(x, 0.0, 0.0),
        facing_yaw: 0.0,
        holds: Vec::new(),
        goal: GOAL_NONE.into(),
        memories: Vec::new(),
        knows: BTreeSet::new(),
        lore,
        presence: crate::Presence::InCity,
        presence_epoch: 0,
        economic_class: crate::EconomicClass::Resident,
    })
}

pub(crate) fn station(point: Vec3, stone_house: bool) -> Station {
    Station {
        place_id: crate::PlaceId::from_raw("frozen_missing_station"),
        name: "The old station".into(),
        point,
        stone_house,
    }
}
pub(crate) fn raise(n: &mut Notices, accused: Option<&str>, raised: Option<f64>) -> u64 {
    n.raise(
        "a familiar stranger".into(),
        "a missing stack".into(),
        Some("old quay".into()),
        Some("before dawn".into()),
        raised,
        actor("departed_raiser"),
        accused.map(actor),
        Some(actor("historical_wronged")),
        Some(ItemId::from_raw("historical_item")),
    )
    .unwrap()
}
pub(crate) fn active() -> World {
    let mut w = World::new();
    for (id, x, o) in [
        ("player", 0.0, None),
        ("srgnt", 1.0, Some("bailiff_and_gaoler")),
        ("second", 2.0, Some("watchman_and_keeper")),
        ("committed", 0.0, None),
        ("inmate", 0.0, None),
        ("closing", 12.0, None),
    ] {
        w.add_character(person(id, x, o));
    }
    w.current_time = Some(WorldTime::from_game_days(2.25));
    let old = raise(&mut w.notices, Some("committed"), Some(-30.0));
    w.notices.expire(2.25);
    raise(&mut w.notices, Some("player"), Some(2.0));
    let hearsay = w
        .notices
        .raise_hearsay(
            "someone long gone".into(),
            "an unproven wrong".into(),
            None,
            None,
            None,
            actor("departed_raiser"),
            Some(actor("absent_accused")),
            None,
            None,
        )
        .unwrap();
    w.notices
        .summon(hearsay, actor("departed_summoner"), Office::Watch, None);
    let future = raise(&mut w.notices, Some("player"), Some(2.0));
    w.notices
        .summon(future, actor("srgnt"), Office::Lamplight, Some(3.0));
    let warrant = raise(&mut w.notices, Some("player"), None);
    w.notices
        .summon(warrant, actor("srgnt"), Office::Dayspring, Some(2.0));
    w.notices.issue_warrants(2.25);
    confront(&mut w);
    w.custody.seize(
        actor("player"),
        actor("srgnt"),
        Some(future),
        station(Vec3::new(100.0, 0.0, 0.0), false),
        1.0,
    );
    w.custody.grab(&actor("player"), actor("second"));
    w.custody.grab(&actor("player"), actor("srgnt"));
    for _ in 0..7 {
        w.custody.get_mut(&actor("player")).unwrap().note_struggle();
    }
    w.custody
        .get_mut(&actor("player"))
        .unwrap()
        .officer_last_turn = Some(2.0);
    w.custody.seize(
        actor("committed"),
        actor("departed_officer"),
        Some(old),
        station(Vec3::ZERO, true),
        1.0,
    );
    w.custody.commit(&actor("committed"), 2.0);
    w.custody.forget(&actor("departed_officer"));
    let r = w.custody.get_mut(&actor("committed")).unwrap();
    r.sentence_office = Some(Office::Dayspring);
    r.sentence_due_game_days = Some(3.0);
    r.closing = true;
    w.custody
        .seed_inmate(actor("inmate"), station(Vec3::ZERO, true));
    w.custody.seize(
        actor("closing"),
        actor("srgnt"),
        None,
        station(Vec3::ZERO, false),
        1.0,
    );
    w.custody.get_mut(&actor("closing")).unwrap().closing = true;
    w
}
fn context(w: &World) -> LawCheckpointContext<'_> {
    LawCheckpointContext::from_world(w, LogicalTime::new(10.0).unwrap())
}
fn bytes(w: &World) -> Admitted<Vec<u8>> {
    w.export_law_checkpoint(
        LogicalTime::new(10.0).unwrap(),
        CheckpointBudget::default()
            .reserve(Cohort::SavePayload, 4096)
            .unwrap(),
    )
    .unwrap()
    .encode()
    .unwrap()
}
fn decode(w: &World, b: &[u8]) -> Admitted<WorldLawCandidate> {
    let budget = CheckpointBudget::default();
    WorldLawDtoV1::decode(
        b,
        budget
            .reserve(Cohort::LoadCandidate, b.len() + 4096)
            .unwrap(),
        context(w),
    )
    .unwrap()
    .into_candidate(context(w))
    .unwrap()
}
#[test]
fn supported_law_fixture_and_standalone_owners_round_trip() {
    let w = active();
    let original = bytes(&w);
    let restored = decode(&w, original.value());
    assert_eq!(
        serde_json::to_vec(&restored.value().data).unwrap(),
        *original.value()
    );
    let expected = include_bytes!("../../../tests/fixtures/checkpoint_v1/law.json");
    assert!(
        original.value().as_slice() == expected.as_slice(),
        "supported law fixture differs"
    );
    let b = CheckpointBudget::default();
    let n = w
        .notices
        .export_checkpoint(context(&w), b.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap();
    let d = NoticesDtoV1::decode(
        n.value(),
        b.reserve(Cohort::LoadCandidate, 4096 + n.value().len())
            .unwrap(),
        context(&w),
    )
    .unwrap()
    .into_candidate(context(&w))
    .unwrap();
    assert_eq!(
        format!("{:?}", d.value().notices()),
        format!("{:?}", w.notices)
    );
    drop(d);
    drop(n);
    let cu = w
        .custody
        .export_checkpoint(context(&w), b.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap();
    let d = crate::custody::checkpoint::CustodyDtoV1::decode(
        cu.value(),
        b.reserve(Cohort::LoadCandidate, 4096 + cu.value().len())
            .unwrap(),
        context(&w),
    )
    .unwrap()
    .into_candidate(context(&w))
    .unwrap();
    assert_eq!(
        format!("{:?}", d.value().custody()),
        format!("{:?}", w.custody)
    );
    assert_eq!(
        restored.value().history_counts(context(&w)),
        LawHistoryCounts {
            historical_notice_links: 1,
            historical_officers: 1
        }
    );
}
#[test]
fn restored_grip_order_idempotence_release_and_hash_progression() {
    let w = active();
    let saved = decode(&w, bytes(&w).value());
    let mut control = w.custody.clone();
    let mut resumed = saved.value().custody().clone();
    let p = actor("player");
    for c in [&mut control, &mut resumed] {
        c.grab(&p, actor("second"));
        assert_eq!(
            c.get(&p).unwrap().holders,
            [actor("second"), actor("srgnt")]
        );
        c.let_go(&p, &actor("second"));
        assert_eq!(c.get(&p).unwrap().holders, [actor("srgnt")]);
    }
    for _ in 0..100 {
        let a = control.get_mut(&p).unwrap().note_struggle();
        let b = resumed.get_mut(&p).unwrap().note_struggle();
        assert_eq!(a, b);
        assert_eq!(
            crate::custody::struggle_roll(&p, &control.get(&p).unwrap().holders, a, 0.31),
            crate::custody::struggle_roll(&p, &resumed.get(&p).unwrap().holders, b, 0.31)
        );
    }
    assert_eq!(control.commit(&p, 10.0), resumed.commit(&p, 10.0));
    assert!(resumed.get(&p).unwrap().holders.is_empty());
    assert!(resumed.commit(&p, 11.0).is_none());
    assert!(resumed.release(&p).is_some());
    assert!(resumed.release(&p).is_none());
}
#[test]
fn saturated_warrants_allocator_holes_and_later_expiry_continue() {
    let mut w = active();
    w.notices = Notices::default();
    for _ in 0..NOTICES_MAX_LIVE {
        let n = raise(&mut w.notices, Some("player"), Some(2.0));
        w.notices
            .summon(n, actor("srgnt"), Office::Watch, Some(2.0));
    }
    w.notices.issue_warrants(2.25);
    let d = decode(&w, bytes(&w).value());
    let mut restored = d.value().notices().clone();
    let before = restored.next_id;
    assert!(
        restored
            .raise(
                "x".into(),
                "y".into(),
                None,
                None,
                None,
                actor("old"),
                None,
                None,
                None
            )
            .is_none()
    );
    assert_eq!(restored.next_id, before);
    restored.expire(22.0);
    assert!(restored.live.is_empty());
    assert_eq!(raise(&mut restored, None, None), before + 1);
}
#[test]
fn malformed_owner_fields_and_missing_nullables_refuse_before_adoption() {
    let w = active();
    let v: Value = serde_json::from_slice(bytes(&w).value()).unwrap();
    let mut cases = Vec::new();
    for (path, value) in [
        ("/notices/next_id", json!(u64::MAX)),
        ("/notices/live/0/id", json!(0)),
        ("/custody/held/player/struggles", json!(u64::MAX)),
        ("/custody/held/player/seized_at", json!(11.0)),
        ("/custody/held/player/officer", json!("missing_active")),
        ("/custody/held/player/station/point", json!([1e7, 0, 0])),
        (
            "/custody/held/committed/sentence_due_game_days",
            json!(null),
        ),
    ] {
        let mut x = v.clone();
        *x.pointer_mut(path).unwrap() = value;
        cases.push(x);
    }
    for path in ["/notices/live/0", "/custody/held/player"] {
        let row = v.pointer(path).unwrap().as_object().unwrap();
        for (key, value) in row {
            if value.is_null() {
                let mut x = v.clone();
                x.pointer_mut(path)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .remove(key);
                cases.push(x);
            }
        }
    }
    let mut x = v.clone();
    let dup = x["notices"]["live"][0].clone();
    x["notices"]["live"].as_array_mut().unwrap().push(dup);
    cases.push(x);
    for x in cases {
        let b = CheckpointBudget::default();
        let wire = serde_json::to_vec(&x).unwrap();
        assert!(
            WorldLawDtoV1::decode(
                &wire,
                b.reserve(Cohort::LoadCandidate, wire.len() + 4096).unwrap(),
                context(&w)
            )
            .is_err()
        );
        assert_eq!(b.retained_bytes(), 0);
    }
}
#[test]
fn aggregate_shape_and_real_escaped_text_limits_admit_before_parsing() {
    let w = active();
    let base: Value = serde_json::from_slice(bytes(&w).value()).unwrap();
    let b = CheckpointBudget::default();
    for (wire, reason) in [
        (format!("{}0{}", "[".repeat(80), "]".repeat(80)), "nesting"),
        (
            format!("{{\"unknown\":[{}]}}", vec!["[]"; 300_000].join(",")),
            "aggregate expanded",
        ),
    ] {
        let e = WorldLawDtoV1::decode(
            wire.as_bytes(),
            b.reserve(Cohort::LoadCandidate, wire.len() + 4096).unwrap(),
            context(&w),
        )
        .unwrap_err();
        assert!(e.to_string().contains(reason), "{e}");
        assert_eq!(b.retained_bytes(), 0);
    }
    let mut v = base;
    v["notices"]["live"][0]["about"] =
        json!("é".repeat(crate::checkpoint::records::MAX_TEXT_BYTES / 2));
    let wire = serde_json::to_vec(&v).unwrap();
    drop(
        WorldLawDtoV1::decode(
            &wire,
            b.reserve(Cohort::LoadCandidate, wire.len() + 4096).unwrap(),
            context(&w),
        )
        .unwrap(),
    );
    let wire = String::from_utf8(wire).unwrap().replace(
        &"é".repeat(crate::checkpoint::records::MAX_TEXT_BYTES / 2),
        &"\\u00e9".repeat(crate::checkpoint::records::MAX_TEXT_BYTES / 2 + 1),
    );
    let err = WorldLawDtoV1::decode(
        wire.as_bytes(),
        b.reserve(Cohort::LoadCandidate, wire.len() + 4096).unwrap(),
        context(&w),
    )
    .unwrap_err();
    assert!(err.to_string().contains("string byte limit"));
    assert_eq!(b.retained_bytes(), 0);
}
#[test]
#[ignore = "regenerate only the new supported M2a6 fixture"]
fn regenerate_law_fixture() {
    std::fs::write(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/checkpoint_v1/law.json"
        ),
        bytes(&active()).value(),
    )
    .unwrap();
}

#[test]
fn explicit_handle_horizon_preserves_last_supported_advances() {
    let mut w = active();
    w.notices = Notices::default();
    w.notices.next_id = u64::MAX - HANDLE_HEADROOM;
    w.custody.get_mut(&actor("player")).unwrap().struggles = u64::MAX - HANDLE_HEADROOM;
    let d = decode(&w, bytes(&w).value());
    let mut n = d.value().notices().clone();
    let mut c = d.value().custody().clone();
    for step in 1..=HANDLE_HEADROOM {
        assert_eq!(raise(&mut n, None, None), u64::MAX - HANDLE_HEADROOM + step);
        assert_eq!(
            c.get_mut(&actor("player")).unwrap().note_struggle(),
            u64::MAX - HANDLE_HEADROOM + step
        );
    }
    w.notices = n;
    w.custody = c;
    let budget = CheckpointBudget::default();
    assert!(
        w.export_law_checkpoint(
            LogicalTime::new(10.0).unwrap(),
            budget.reserve(Cohort::SavePayload, 4096).unwrap()
        )
        .is_err()
    );
    assert_eq!(budget.retained_bytes(), 0);
}
#[test]
#[ignore = "closed law layout and sparse-node diagnostic"]
fn checkpoint_law_layout() {
    use crate::custody::CustodyRecord;
    use std::mem::size_of;
    let mut w = active();
    let r = w.custody.get_mut(&actor("player")).unwrap();
    r.officer = Some(actor("a"));
    r.holders.clear();
    r.notice_id = None;
    r.station.place_id = crate::PlaceId::from_raw("a");
    r.station.name.clear();
    r.station.point = Vec3::ZERO;
    r.seized_at = 0.0;
    r.officer_last_turn = None;
    r.struggles = 0;
    struct Record<'a>(&'a CustodyRecord);
    impl Serialize for Record<'_> {
        fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
            crate::custody::checkpoint::records::CustodyRecordV1::serialize(self.0, s)
        }
    }
    let record = aggregate::measure(&Record(r), OWNER).unwrap();
    let sparse_node = 11 * size_of::<(ActorId, CustodyRecord)>() + 168;
    assert!(record.expanded_upper_bytes >= sparse_node);
    let layouts = json!({"Notices":size_of::<Notices>(),"WardNotice":size_of::<WardNotice>(),"Summons":size_of::<Summons>(),"Custody":size_of::<crate::custody::Custody>(),"CustodyRecord":size_of::<CustodyRecord>(),"Station":size_of::<Station>(),"ActorId":size_of::<ActorId>(),"OptionActorId":size_of::<Option<ActorId>>(),"OptionF64":size_of::<Option<f64>>(),"OptionOffice":size_of::<Option<Office>>(),"WorldLawDto":size_of::<WorldLawDtoV1>(),"WorldLawCandidate":size_of::<WorldLawCandidate>(),"EngineLawDto":size_of::<crate::engine::law_checkpoint::EngineLawDtoV1>(),"EngineMessage":size_of::<crate::EngineMessage>(),"PlayerNotice":size_of::<crate::engine::PlayerNotice>(),"PlayerCustody":size_of::<crate::engine::PlayerCustody>(),"registry_name_tuple":size_of::<(&String,usize)>(),"minimal_record_expanded_bytes":record.expanded_upper_bytes,"sparse_custody_node_bound_bytes":sparse_node,"maximum_registry_index_nodes":1+(MAX_NAMES-1)/5,"registry_index_bound_bytes":(1+(MAX_NAMES-1)/5)*(11*size_of::<(&String,usize)>()+168),"validation_working_bytes":VALIDATION_WORKING_BYTES});
    println!("M2A6_LAW_LAYOUT={layouts}");
    assert!(size_of::<WardNotice>() * 2 <= 2048);
    assert!(size_of::<crate::engine::PlayerCustody>() <= 512);
    assert!(size_of::<crate::EngineMessage>() <= 1024);
    assert!(
        (1 + (MAX_NAMES - 1) / 5) * (11 * size_of::<(&String, usize)>() + 168)
            < VALIDATION_WORKING_BYTES
    );
}

#[test]
fn served_escalation_round_trip_tells_each_officer_once_per_rung() {
    let mut control = active();
    let mut resumed = active();
    let saved = decode(&control, bytes(&control).value());
    resumed.notices = saved.value().notices().clone();
    let inbox = |w: &World| w.characters[&actor("srgnt")].inbox().len();
    let initial = inbox(&control);
    for w in [&mut control, &mut resumed] {
        confront(w);
        assert_eq!(inbox(w), initial);
        assert!(
            w.notices
                .summon(2, actor("srgnt"), Office::Watch, Some(3.0))
        );
        confront(w);
        assert_eq!(inbox(w), initial + 1);
        confront(w);
        assert_eq!(inbox(w), initial + 1);
        assert_eq!(w.notices.issue_warrants(3.0), [2, 4]);
        confront(w);
        assert_eq!(inbox(w), initial + 3);
        assert!(w.notices.issue_warrants(3.0).is_empty());
        confront(w);
        assert_eq!(inbox(w), initial + 3);
    }
    assert!(bytes(&control).value() == bytes(&resumed).value());
}
#[test]
fn duplicate_owner_keys_served_sets_and_raw_context_charge_are_strict() {
    let mut w = active();
    let encoded = bytes(&w);
    let raw = String::from_utf8(encoded.value().clone()).unwrap();
    let original: Value = serde_json::from_str(&raw).unwrap();
    let record = serde_json::to_string(&original["custody"]["held"]["player"]).unwrap();
    let duplicate = raw.replace("\"held\":{", &format!("\"held\":{{\"player\":{record},"));
    let served = raw.replace(
        "\"served\":[\"second\",\"srgnt\"]",
        "\"served\":[\"second\",\"second\"]",
    );
    let b = CheckpointBudget::default();
    for (wire, reason) in [
        (&duplicate, "duplicate owner key"),
        (&served, "duplicate set entry"),
    ] {
        let err = WorldLawDtoV1::decode(
            wire.as_bytes(),
            b.reserve(Cohort::LoadCandidate, wire.len() + 4096).unwrap(),
            context(&w),
        )
        .unwrap_err();
        assert!(err.to_string().contains(reason), "{err}");
        assert_eq!(b.retained_bytes(), 0);
    }
    let canonical = decode(&w, encoded.value())
        .value()
        .data
        .cost()
        .unwrap()
        .peak_bytes;
    let padding = 1024 * 1024;
    let padded = format!("{raw}{}", " ".repeat(padding));
    let d = WorldLawDtoV1::decode(
        padded.as_bytes(),
        b.reserve(Cohort::LoadCandidate, padded.len() + 4096)
            .unwrap(),
        context(&w),
    )
    .unwrap();
    assert!(d.reserved_bytes() >= canonical + 3 * padding);
    let candidate = d.into_candidate(context(&w)).unwrap();
    assert!(candidate.reserved_bytes() >= canonical + 3 * padding);
    drop(candidate);
    assert_eq!(b.retained_bytes(), 0);
    w.places.add_home(
        &actor("inmate"),
        "new registry home",
        Vec3::new(3.0, 0.0, 0.0),
    );
    let err = WorldLawDtoV1::decode(
        encoded.value(),
        b.reserve(Cohort::LoadCandidate, encoded.value().len() + 4096)
            .unwrap(),
        context(&w),
    )
    .unwrap_err();
    assert!(err.to_string().contains("context disagreement"));
    assert_eq!(b.retained_bytes(), 0);
    let updated = decode(&w, bytes(&w).value());
    assert_eq!(
        updated
            .value()
            .custody()
            .get(&actor("committed"))
            .unwrap()
            .station
            .point,
        Vec3::ZERO
    );
}
