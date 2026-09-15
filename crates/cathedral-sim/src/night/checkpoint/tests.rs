use super::*;
use crate::{
    character::{Character, CharacterSheet},
    checkpoint::{CheckpointBudget, Cohort},
    lore::LoreProfile,
    math::Vec3,
    places::{PlaceEntry, PlaceRegistry},
    traits::{CognitionBusy, CognitionError},
};
use serde_json::json;
use std::collections::BTreeSet;
mod cognition_inputs;
pub(crate) fn env() -> PromptEnv {
    PromptEnv::new(
        include_str!("../../../../../assets/prompts/turn.j2"),
        include_str!("../../../../../assets/prompts/night.j2"),
        include_str!("../../../../../assets/prompts/strings.toml"),
    )
    .expect("the shipped prompt assets load")
}

/// One game day per real hour, opening at the Waning — an hour before the
/// earliest shipped bedtime, so a test can cross Lamplight and the Snuffing
/// without a wrap.
pub(crate) fn clock() -> WorldClock {
    WorldClock::new(3600.0, Office::Waning, 0, 0.05)
}

fn character(id: &str, name: &str, significance: Significance, ward: PlanningWard) -> Character {
    Character::from_sheet(CharacterSheet {
        pockets: Vec::new(),
        frontbutt: None,
        id: ActorId::from_raw(id),
        name: name.to_string(),
        control: Control::Llm,
        back_story: "A life.".into(),
        location_description: "The Tallage".into(),
        appearance: Default::default(),
        voice_key: None,
        position_m: Vec3::new(0.0, 0.0, 0.0),
        facing_yaw: 0.0,
        holds: Vec::new(),
        goal: "None".into(),
        memories: Vec::new(),
        knows: BTreeSet::new(),
        lore: Some(LoreProfile {
            significance,
            planning_ward: ward,
            age: 30,
            gender: "f".into(),
            occupation_id: Some("baker".into()),
            occupation_display: Some("Baker".into()),
            title: None,
            rank: None,
            faction_role: None,
            illegal_activity: None,
            district: "The Tallage".into(),
            father: None,
            mother: None,
            children: Vec::new(),
            circumstances: Vec::new(),
            conditions: Vec::new(),
            home: None,
            home_point_m: None,
            core_character_description: "You bake.".into(),
            extended_character_description: String::new(),
            curiosity: None,
            generated: false,
            generated_routine: None,
        }),
        presence: crate::Presence::InCity,
        presence_epoch: 0,
        economic_class: crate::EconomicClass::Resident,
    })
}

fn player() -> Character {
    let mut sheet = character("player", "You", Significance::Ambient, PlanningWard::Weigh).sheet;
    sheet.control = Control::Player;
    sheet.lore = None;
    // Far from everyone, so the stage is empty unless a test moves somebody.
    sheet.position_m = Vec3::new(500.0, 0.0, 500.0);
    Character::from_sheet(sheet)
}

/// A Major with a two-leg round, a Minor and an Ambient in the Weigh ward, and
/// two place handles the Major holds.
pub(crate) fn world_with_cast() -> World {
    let mut world = World::new();
    world.add_character(player());

    let mut major = character(
        "mjr01",
        "Corin Copp",
        Significance::Major,
        PlanningWard::Weigh,
    );
    major.state.daily_round = vec![
        "at Dayspring: work at The Tallage".to_string(),
        "at Lamplight: home to sleep".to_string(),
    ];
    major.state.places_known = [PlaceId::from_raw("pl_aaaa"), PlaceId::from_raw("pl_bbbb")]
        .into_iter()
        .collect();
    world.add_character(major);

    let mut minor = character("mnr01", "Tam Rud", Significance::Minor, PlanningWard::Weigh);
    minor.state.daily_round = vec!["at Dayspring: work at The Tallage".to_string()];
    minor.state.goal = "Find work for the winter".into();
    world.add_character(minor);

    world.add_character(character(
        "amb01",
        "Nan Skell",
        Significance::Ambient,
        PlanningWard::Weigh,
    ));
    // A Minor in another ward, so a ward batch's reach can be tested.
    let mut elsewhere = character("mnr02", "Ede Pell", Significance::Minor, PlanningWard::Reed);
    elsewhere.state.daily_round = vec!["at Dayspring: work at Cinder Row".to_string()];
    world.add_character(elsewhere);

    let mut places = PlaceRegistry::default();
    for (id, name) in [("pl_aaaa", "The Tallage"), ("pl_bbbb", "The Hungry Ox")] {
        places
            .insert(PlaceEntry {
                id: PlaceId::from_raw(id),
                name: name.to_string(),
                point: Vec3::new(10.0, 0.0, 10.0),
                ward: Some("weigh".into()),
                coarse: true,
            })
            .expect("distinct ids");
    }
    world.places = places;
    world
}

fn office(enabled_tiers: NightOfficeConfig, world: &World, now: f64) -> NightOffice {
    let mut night = NightOffice::new(enabled_tiers, now, &clock());
    night.seed(world, &Round::new());
    night
}

pub(crate) fn all_tiers() -> NightOfficeConfig {
    NightOfficeConfig {
        enabled: true,
        ..NightOfficeConfig::default()
    }
}

/// The gate wide open: nobody near the player, no floor, no reaction owed.
pub(crate) fn open() -> NightGate {
    NightGate {
        floor_busy: false,
        player_composing: false,
        stage_occupied: false,
        player_reaction: false,
    }
}

pub(crate) struct Recorded {
    pub busy: bool,
    pub prompts: Vec<String>,
    pub next: u64,
}
impl Recorded {
    pub(crate) fn new() -> Self {
        Self {
            busy: false,
            prompts: vec![],
            next: 0,
        }
    }
}
impl Cognition for Recorded {
    fn request(&mut self, _: String) -> std::result::Result<RequestId, CognitionBusy> {
        Err(CognitionBusy)
    }
    fn request_night(
        &mut self,
        p: String,
        _: Option<u32>,
    ) -> std::result::Result<RequestId, CognitionBusy> {
        if self.busy {
            return Err(CognitionBusy);
        }
        self.prompts.push(p);
        self.next += 1;
        Ok(RequestId(self.next))
    }
}
fn time(n: f64) -> LogicalTime {
    LogicalTime::new(n).unwrap()
}
fn bytes(n: &NightOffice, w: &World, c: &WorldClock, now: f64) -> Vec<u8> {
    let b = CheckpointBudget::default();
    n.export_checkpoint(
        NightCheckpointContext::from_world(w, time(now), c),
        b.reserve(Cohort::SavePayload, 4096).unwrap(),
    )
    .unwrap()
    .encode()
    .unwrap()
    .value()
    .clone()
}
fn restore(n: &NightOffice, w: &World, c: &WorldClock, now: f64) -> NightOffice {
    let raw = bytes(n, w, c, now);
    let b = CheckpointBudget::default();
    let cx = NightCheckpointContext::from_world(w, time(now), c);
    let d = NightOfficeDtoV1::decode(
        &raw,
        b.reserve(Cohort::LoadCandidate, raw.len() + 4096).unwrap(),
        cx,
    )
    .unwrap()
    .into_candidate(cx)
    .unwrap();
    // Independent constructor deliberately has wrong config, empty history,
    // fresh pacing/office/seed state and counters. Exact field install is local.
    let mut other = NightOffice::new(NightOfficeConfig::default(), 0.0, &clock());
    other.seeded = !n.seeded;
    assert_ne!(other.seeded, n.seeded);
    other = copy(d.value().night());
    assert_eq!(
        bytes(&other, w, c, now),
        raw,
        "immediate equality before poll/ring"
    );
    other
}
pub(crate) fn saturate(w: &mut World, now: f64) -> OperationId {
    let mut first = None;
    for _ in 0..17 {
        let op = w
            .command_ledger
            .reserve_operation(receipts::HOST_PRODUCER)
            .unwrap();
        first.get_or_insert(op);
        for step in 0..256 {
            let receipts::Admission::New(ticket) = w
                .command_ledger
                .begin(op.command(step), &json!({"fixture":"protected"}))
            else {
                panic!("capacity setup")
            };
            w.command_ledger
                .finish(ticket, now, Outcome::completed("fixture"), vec![]);
            w.command_ledger.drain_updates();
        }
    }
    assert_eq!(w.command_ledger.recent_len(), 4096);
    assert_eq!(w.command_ledger.retained_len(), 256);
    first.unwrap()
}
fn staged(subject: Subject) -> (NightOffice, World, WorldClock, Recorded) {
    let w = world_with_cast();
    let c = clock();
    let mut n = office(all_tiers(), &w, 0.0);
    n.enqueue(subject, 0);
    (n, w, c, Recorded::new())
}
fn poll(
    n: &mut NightOffice,
    w: &mut World,
    c: &WorldClock,
    now: f64,
    r: &mut Recorded,
    done: Option<Completion>,
) -> Vec<SchedulerEvent> {
    let out = n.poll(
        now,
        w,
        c,
        &mut done.into_iter().collect(),
        NightGate {
            floor_busy: true,
            ..open()
        },
        r,
        &env(),
    );
    w.command_ledger.drain_updates();
    out
}
#[test]
fn checkpoint_night_held_capacity_release_applies_once_with_exact_prompt_and_receipts() {
    let (mut a, mut wa, c, mut ra) = staged(Subject::Person(ActorId::from_raw("mjr01")));
    let (mut unused, mut wb, _, mut rb) = staged(Subject::Person(ActorId::from_raw("mjr01")));
    for (n, w, r) in [(&mut a, &mut wa, &mut ra), (&mut unused, &mut wb, &mut rb)] {
        n.poll(1.0, w, &c, &mut vec![], open(), r, &env());
    }
    let oa = saturate(&mut wa, 1.0);
    let ob = saturate(&mut wb, 1.0);
    assert_eq!(oa, ob);
    let done=Completion{request_id:RequestId(1),result:Ok("remember {\"memory\":\"The river stayed quiet.\"}\nset_goal {\"goal\":\"Ask at dawn\"}".into()),duration_seconds:0.123456789012345};
    let out = poll(&mut a, &mut wa, &c, 2.0, &mut ra, Some(done.clone()));
    assert!(out.iter().any(|e| matches!(
        e,
        SchedulerEvent::CommandAdmissionRefused {
            retryable: true,
            ..
        }
    )));
    assert_eq!(a.held_result, Some(done));
    let mut b = restore(&a, &wb, &c, 2.0);
    assert_eq!(a.in_flight.as_ref().unwrap().prompt, ra.prompts[0]);
    wa.command_ledger.unprotect(oa);
    wb.command_ledger.unprotect(ob);
    let before = wa.characters[&ActorId::from_raw("mjr01")]
        .state
        .memories
        .len();
    let x = poll(&mut a, &mut wa, &c, 3.0, &mut ra, None);
    let y = poll(&mut b, &mut wb, &c, 3.0, &mut rb, None);
    assert_eq!(x, y);
    assert_eq!(a.reflected, 1);
    assert!(a.held_result.is_none());
    assert_eq!(
        wa.characters[&ActorId::from_raw("mjr01")]
            .state
            .memories
            .len(),
        before + 1
    );
    assert_eq!(
        wa.characters[&ActorId::from_raw("mjr01")].state.memories,
        wb.characters[&ActorId::from_raw("mjr01")].state.memories
    );
    assert_eq!(bytes(&a, &wa, &c, 3.0), bytes(&b, &wb, &c, 3.0));
    assert!(poll(&mut a, &mut wa, &c, 4.0, &mut ra, None).is_empty());
    assert!(poll(&mut b, &mut wb, &c, 4.0, &mut rb, None).is_empty());
}
#[test]
fn checkpoint_night_exact_error_kind_detail_and_stale_incarnation_finish_once() {
    for stale in [false, true] {
        let (mut a, mut wa, c, mut ra) = staged(Subject::Person(ActorId::from_raw("mjr01")));
        let (mut unused, mut wb, _, mut rb) = staged(Subject::Person(ActorId::from_raw("mjr01")));
        for (n, w, r) in [(&mut a, &mut wa, &mut ra), (&mut unused, &mut wb, &mut rb)] {
            n.poll(1.0, w, &c, &mut vec![], open(), r, &env());
        }
        if stale {
            for w in [&mut wa, &mut wb] {
                w.characters
                    .get_mut(&ActorId::from_raw("mjr01"))
                    .unwrap()
                    .state
                    .presence_epoch = 7;
            }
        }
        // Errors are normally consumed immediately. This exact owner-level
        // terminal fixture anticipates M2c completed-at-capture retention.
        a.held_result = Some(Completion {
            request_id: RequestId(1),
            result: if stale {
                Ok("set_goal {\"goal\":\"must not land\"}".into())
            } else {
                Err(CognitionError::detailed(
                    "RateLimited",
                    "429: retry after midnight\nprovider says wait",
                ))
            },
            duration_seconds: 0.25,
        });
        let mut b = restore(&a, &wb, &c, 2.0);
        let x = poll(&mut a, &mut wa, &c, 2.0, &mut ra, None);
        let y = poll(&mut b, &mut wb, &c, 2.0, &mut rb, None);
        assert_eq!(x, y);
        assert_eq!(a.dropped, 1);
        assert_eq!(a.reflected, 0);
        if !stale {
            assert!(x.iter().any(|e|matches!(e,SchedulerEvent::PromptExchange{error:Some(s),..} if s=="429: retry after midnight\nprovider says wait")));
        }
        assert_ne!(
            wa.characters[&ActorId::from_raw("mjr01")].goal(),
            "must not land"
        );
        assert!(poll(&mut b, &mut wb, &c, 3.0, &mut rb, None).is_empty());
    }
}
#[test]
fn checkpoint_night_obsolete_queued_busy_and_daily_stamp_are_not_cleaned_on_decode() {
    let (mut a, mut wa, c, mut r) = staged(Subject::Person(ActorId::from_raw("mjr01")));
    r.busy = true;
    a.poll(1.0, &mut wa, &c, &mut vec![], open(), &mut r, &env());
    assert!(a.queue[0].semantic.is_some());
    a.enqueue(Subject::Ward(PlanningWard::Weigh), 0);
    let mut wb = world_with_cast();
    let mut nb = office(all_tiers(), &wb, 0.0);
    nb.enqueue(Subject::Person(ActorId::from_raw("mjr01")), 0);
    let mut rb = Recorded::new();
    rb.busy = true;
    nb.poll(1.0, &mut wb, &c, &mut vec![], open(), &mut rb, &env());
    let mut b = restore(&a, &wb, &c, 2.0);
    assert_eq!(b.queue.len(), 2);
    for n in [&mut a, &mut b] {
        n.enqueue(Subject::Ward(PlanningWard::Weigh), 0);
        assert_eq!(n.queue.len(), 2);
    }
    for (n, w) in [(&mut a, &mut wa), (&mut b, &mut wb)] {
        n.poll(3601.0, w, &c, &mut vec![], open(), &mut r, &env());
        assert_eq!(n.dropped, 2);
        assert!(n.queue.is_empty());
    }
    assert_eq!(bytes(&a, &wa, &c, 3601.0), bytes(&b, &wb, &c, 3601.0));
}
#[test]
fn checkpoint_night_closed_records_limits_and_raw_charge() {
    let (mut n, mut w, c, mut r) = staged(Subject::Person(ActorId::from_raw("mjr01")));
    n.poll(1.0, &mut w, &c, &mut vec![], open(), &mut r, &env());
    n.held_result = Some(Completion {
        request_id: RequestId(1),
        result: Err(CognitionError::detailed("kind", "detail")),
        duration_seconds: 0.5,
    });
    n.enqueue(Subject::Ward(PlanningWard::Weigh), 0);
    let raw = bytes(&n, &w, &c, 2.0);
    let original: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    let refuse = |bad: Vec<u8>| {
        let b = CheckpointBudget::default();
        assert!(
            NightOfficeDtoV1::decode(
                &bad,
                b.reserve(Cohort::LoadCandidate, bad.len() + 4096).unwrap(),
                NightCheckpointContext::from_world(&w, time(2.0), &c)
            )
            .is_err()
        );
        assert_eq!(b.retained_bytes(), 0);
    };
    for record in [
        "",
        "/night",
        "/night/config",
        "/night/queue/0",
        "/night/in_flight",
        "/night/held_result",
        "/night/held_result/result/err",
    ] {
        for k in original
            .pointer(record)
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
        {
            let mut bad = original.clone();
            bad.pointer_mut(record)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(k);
            refuse(serde_json::to_vec(&bad).unwrap());
        }
        let mut bad = original.clone();
        bad.pointer_mut(record).unwrap()["unknown"] = json!(1);
        refuse(serde_json::to_vec(&bad).unwrap());
    }
    for (path, value) in [
        ("/night/reflected", json!(u64::MAX)),
        ("/night/dropped", json!(u64::MAX)),
        ("/night/held_result/request_id", json!(2)),
        ("/night/held_result/duration_seconds", json!(-1)),
        ("/night/in_flight/semantic/producer", json!(2)),
        ("/night/in_flight/presence_epoch", json!(null)),
        (
            "/night/in_flight/prompt",
            json!("x".repeat(MAX_PROMPT_BYTES + 1)),
        ),
    ] {
        let mut bad = original.clone();
        *bad.pointer_mut(path).unwrap() = value;
        refuse(serde_json::to_vec(&bad).unwrap());
    }
    for (path, value) in [
        ("/night/queue/0/subject", json!({"person":"mjr01"})),
        ("/night/in_flight/owed_day", json!(1)),
        (
            "/night/queue/0/semantic",
            original["night"]["in_flight"]["semantic"].clone(),
        ),
    ] {
        let mut bad = original.clone();
        *bad.pointer_mut(path).unwrap() = value;
        refuse(serde_json::to_vec(&bad).unwrap());
    }
    for path in ["/night/queue", "/night/last_reflected"] {
        let mut bad = original.clone();
        let rows = bad.pointer_mut(path).unwrap().as_array_mut().unwrap();
        rows.push(rows[0].clone());
        refuse(serde_json::to_vec(&bad).unwrap());
    }
    refuse(
        String::from_utf8(raw.clone())
            .unwrap()
            .replacen("\"bedtimes\":{", "\"bedtimes\":{\"mjr01\":\"snuffing\",", 1)
            .into_bytes(),
    );
    let padded = format!(
        "{}{}",
        " ".repeat(16384),
        String::from_utf8(raw)
            .unwrap()
            .replace("detail", "\\u0064etail")
    );
    let b = CheckpointBudget::default();
    let cx = NightCheckpointContext::from_world(&w, time(2.0), &c);
    let d = NightOfficeDtoV1::decode(
        padded.as_bytes(),
        b.reserve(Cohort::LoadCandidate, padded.len() + 4096)
            .unwrap(),
        cx,
    )
    .unwrap();
    let charge = b.retained_bytes();
    assert!(charge > d.value().cost().unwrap().peak_bytes);
    let candidate = d.into_candidate(cx).unwrap();
    assert_eq!(charge, b.retained_bytes());
    drop(candidate);
    assert_eq!(b.retained_bytes(), 0);
}
#[test]
fn checkpoint_night_max_shapes_and_sparse_private_maps() {
    let mut w = world_with_cast();
    let c = clock();
    let mut n = office(all_tiers(), &w, 0.0);
    for i in 0..MAX_PERSONS {
        let id = ActorId::from_raw(format!("historical person {i}"));
        n.bedtimes.insert(id.clone(), Office::Snuffing);
        n.last_reflected.insert(Subject::Person(id.clone()), -1);
        n.queue.push_back(Due {
            queued_presence_epoch: None,
            semantic: None,
            presence_epoch: None,
            subject: Subject::Person(id),
            day: -1,
        });
    }
    // Existing seeded Major would make 25,001; historical map bound is exact.
    n.bedtimes.remove(&ActorId::from_raw("mjr01"));
    for ward in PlanningWard::ALL {
        n.enqueue(Subject::Ward(ward), -1);
    }
    n.dropped = u64::MAX - COUNTER_DROP_HEADROOM;
    n.reflected = u64::MAX - 1;
    let b = restore(&n, &w, &c, 0.0);
    assert_eq!(b.queue.len(), MAX_SUBJECTS);
    n.enqueue(Subject::Person(ActorId::from_raw("one too many")), -1);
    let budget = CheckpointBudget::default();
    assert!(
        n.export_checkpoint(
            NightCheckpointContext::from_world(&w, time(0.0), &c),
            budget.reserve(Cohort::SavePayload, 4096).unwrap()
        )
        .is_err()
    );
    assert_eq!(budget.retained_bytes(), 0);
    w.ward_moods.insert(PlanningWard::Weigh, "é".repeat(2000));
}
#[test]
#[ignore = "closed Night layouts"]
fn checkpoint_night_layout() {
    println!(
        "NightOffice={} Due={} Flight={} Subject={} Completion={} CognitionError={} Config={} WorldDto={} NightDto={} EngineDto={} SubjectRef={} OperationId={}",
        std::mem::size_of::<NightOffice>(),
        std::mem::size_of::<Due>(),
        std::mem::size_of::<Flight>(),
        std::mem::size_of::<Subject>(),
        std::mem::size_of::<Completion>(),
        std::mem::size_of::<CognitionError>(),
        std::mem::size_of::<NightOfficeConfig>(),
        std::mem::size_of::<WorldNightDtoV1>(),
        std::mem::size_of::<NightOfficeDtoV1>(),
        std::mem::size_of::<crate::engine::night_checkpoint::EngineNightDtoV1>(),
        std::mem::size_of::<&Subject>(),
        std::mem::size_of::<OperationId>()
    );
}

#[test]
fn checkpoint_night_ward_mood_mark_round_edit_and_receipt_continuation() {
    let (mut a, mut wa, c, mut ra) = staged(Subject::Ward(PlanningWard::Weigh));
    let (mut n, mut wb, _, mut rb) = staged(Subject::Ward(PlanningWard::Weigh));
    let resort = wa.mark_catalog.ward_sign_place("weigh").unwrap().to_owned();
    for (n, w, r) in [(&mut a, &mut wa, &mut ra), (&mut n, &mut wb, &mut rb)] {
        w.places
            .insert(PlaceEntry {
                id: PlaceId::from_raw("pl_resort"),
                name: resort.clone(),
                point: Vec3::new(3.0, crate::WALK_Y, 4.0),
                ward: Some("weigh".into()),
                coarse: true,
            })
            .unwrap();
        n.poll(1.0, w, &c, &mut vec![], open(), r, &env());
    }
    let reply = format!(
        "ward_mood {{\"mood\":\"  The scales wait for morning.  \"}}\nchalk_ward_sign {{\"place\":{}}}\nset_round {{\"person\":\"mnr01\",\"leg\":1,\"place_id\":\"pl_bbbb\"}}",
        serde_json::to_string(&resort).unwrap()
    );
    a.held_result = Some(Completion {
        request_id: RequestId(1),
        result: Ok(reply),
        duration_seconds: 1.25,
    });
    let mut b = restore(&a, &wb, &c, 2.0);
    let x = poll(&mut a, &mut wa, &c, 2.0, &mut ra, None);
    let y = poll(&mut b, &mut wb, &c, 2.0, &mut rb, None);
    assert_eq!(x, y);
    assert_eq!(
        wa.ward_moods[&PlanningWard::Weigh],
        "The scales wait for morning."
    );
    assert_eq!(wa.ward_moods, wb.ward_moods);
    assert_eq!(wa.marks.len(), 1);
    assert_eq!(
        wa.marks.iter().collect::<Vec<_>>(),
        wb.marks.iter().collect::<Vec<_>>()
    );
    let actor = ActorId::from_raw("mnr01");
    assert!(wa.characters[&actor].state.round_edit.is_some());
    assert_eq!(
        wa.characters[&actor].state.round_edit,
        wb.characters[&actor].state.round_edit
    );
    assert_eq!(wa.round_actions, wb.round_actions);
    assert_eq!(
        x.iter()
            .filter(|e| matches!(e, SchedulerEvent::ActionReceipt(_)))
            .count(),
        4
    );
    assert!(poll(&mut b, &mut wb, &c, 3.0, &mut rb, None).is_empty());
    assert_eq!(wb.marks.len(), 1);
}
#[test]
fn checkpoint_night_busy_retry_at_edge_and_newer_stamp_while_flight_out() {
    let (mut a, mut wa, mut c, mut ra) = staged(Subject::Person(ActorId::from_raw("mjr01")));
    ra.busy = true;
    a.poll(1.0, &mut wa, &c, &mut vec![], open(), &mut ra, &env());
    let op = a.queue[0].semantic.unwrap();
    let mut b = restore(&a, &wa, &c, 2.0);
    let mut rb = Recorded::new();
    let mut wb = world_with_cast();
    let saved = wa
        .command_ledger
        .checkpoint_v1(
            time(2.0),
            CheckpointBudget::default()
                .reserve(
                    Cohort::SavePayload,
                    crate::receipts::CommandLedgerDtoV1::WORKING_BYTES,
                )
                .unwrap(),
        )
        .unwrap();
    wb.command_ledger = saved.value().candidate(time(2.0)).unwrap();
    c = c.with_scale(2.0, 3.0);
    ra.busy = false;
    for at in [5.999, 6.0] {
        for (n, w, r) in [(&mut a, &mut wa, &mut ra), (&mut b, &mut wb, &mut rb)] {
            n.poll(at, w, &c, &mut vec![], open(), r, &env());
        }
        assert_eq!(bytes(&a, &wa, &c, at), bytes(&b, &wb, &c, at));
    }
    assert_eq!(a.in_flight.as_ref().unwrap().semantic, op);
    assert_eq!(ra.prompts, rb.prompts);
    assert_eq!(a.next_attempt_at, 6.0 + pace_seconds(&c));
    let s = a.in_flight.as_ref().unwrap().subject.clone();
    a.enqueue(s.clone(), 1);
    let b = restore(&a, &wb, &c, 6.0);
    assert_eq!(b.in_flight.as_ref().unwrap().owed_day, 0);
    assert_eq!(b.last_reflected[&s], 1);
    assert_eq!(b.queue[0].day, 1);
}

#[test]
fn checkpoint_night_unadopted_backbone_and_ledger_borrow_without_owner_install() {
    let (mut n, mut w, c, mut r) = staged(Subject::Person(ActorId::from_raw("mjr01")));
    n.poll(1.0, &mut w, &c, &mut vec![], open(), &mut r, &env());
    let raw = bytes(&n, &w, &c, 2.0);
    let bb = CheckpointBudget::default();
    let body = w
        .export_backbone_checkpoint(bb.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap();
    let backbone = crate::world::WorldBackboneDtoV1::decode(
        body.value(),
        bb.reserve(Cohort::LoadCandidate, body.value().len() + 4096)
            .unwrap(),
        &w.item_catalog,
        &w.command_ledger,
    )
    .unwrap()
    .into_candidate(&w.item_catalog, &w.command_ledger)
    .unwrap();
    let lb = CheckpointBudget::default();
    let ledger = w
        .command_ledger
        .checkpoint_v1(
            time(2.0),
            lb.reserve(
                Cohort::SavePayload,
                crate::receipts::CommandLedgerDtoV1::WORKING_BYTES,
            )
            .unwrap(),
        )
        .unwrap();
    let cx = NightCheckpointContext::from_backbone(backbone.value(), time(2.0), &c, ledger.value());
    let b = CheckpointBudget::default();
    let d = NightOfficeDtoV1::decode(
        &raw,
        b.reserve(Cohort::LoadCandidate, raw.len() + 4096).unwrap(),
        cx,
    )
    .unwrap()
    .into_candidate(cx)
    .unwrap();
    assert_eq!(d.value().night().in_flight_subject(), n.in_flight_subject());
    let empty = World::new()
        .command_ledger
        .checkpoint_v1(
            time(2.0),
            CheckpointBudget::default()
                .reserve(
                    Cohort::SavePayload,
                    crate::receipts::CommandLedgerDtoV1::WORKING_BYTES,
                )
                .unwrap(),
        )
        .unwrap();
    let wrong =
        NightCheckpointContext::from_backbone(backbone.value(), time(2.0), &c, empty.value());
    drop(d);
    assert!(
        NightOfficeDtoV1::decode(
            &raw,
            b.reserve(Cohort::LoadCandidate, raw.len() + 4096).unwrap(),
            wrong
        )
        .is_err()
    );
    assert_eq!(b.retained_bytes(), 0);
}

#[test]
fn checkpoint_night_clock_binding_distinguishes_signed_zero_and_pacing_keeps_it() {
    let w = world_with_cast();
    let c = WorldClock::new(3600.0, Office::Waning, 0, -0.0).with_scale(-0.0, 1.0);
    let mut n = office(all_tiers(), &w, 0.0);
    n.next_attempt_at = -0.0;
    let raw = bytes(&n, &w, &c, 0.0);
    let b = restore(&n, &w, &c, 0.0);
    assert_eq!(b.next_attempt_at.to_bits(), (-0.0f64).to_bits());
    for wrong in [
        WorldClock::new(3600.0, Office::Waning, 0, 0.0).with_scale(-0.0, 1.0),
        WorldClock::new(3600.0, Office::Waning, 0, -0.0).with_scale(0.0, 1.0),
    ] {
        let b = CheckpointBudget::default();
        assert!(
            NightOfficeDtoV1::decode(
                &raw,
                b.reserve(Cohort::LoadCandidate, raw.len() + 4096).unwrap(),
                NightCheckpointContext::from_world(&w, time(0.0), &wrong)
            )
            .is_err()
        );
        assert_eq!(b.retained_bytes(), 0);
    }
}

const HOUSED_AMBIENT_IDS: [&str; 40] = [
    "a2gpk", "a3crk", "a4anh", "a5sbp", "a6avh", "a7pcr", "a8ewf", "a9rnh", "ar5tl", "b0nll",
    "b1sbb", "b3glc", "b5ewk", "b6clm", "b9stt", "ba8hf", "bc6tf", "bd7hb", "bn1id", "bn2hm",
    "bn3sg", "bn4cp", "bn5jk", "bn6gb", "bn7an", "bn8jm", "bn9et", "bnawr", "bnbcr", "bndhk",
    "bnpro", "bnrse", "br2sk", "brn5o", "bt4hb", "c2nsl", "c3wnk", "c5tbo", "c6pkl", "c7kbd",
];
#[test]
fn checkpoint_night_saved_ambient_guard_suppresses_a_real_seeded_roll_then_allows_next_day() {
    let nav = crate::NavData::from_parts(
        include_str!("../../../../../assets/world/navigation.json"),
        include_bytes!("../../../../../assets/world/navigation.bin"),
    )
    .unwrap();
    let c = clock();
    let setup =
        || {
            let mut w = World::new();
            w.sound_catalog = crate::SoundCatalog::from_toml_str(include_str!(
                "../../../../../assets/sounds/catalog.toml"
            ))
            .unwrap();
            for (i, id) in HOUSED_AMBIENT_IDS.iter().enumerate() {
                let mut actor = character(id, id, Significance::Ambient, PlanningWard::Fabric);
                actor.sheet.lore.as_mut().unwrap().occupation_id = Some("mason".into());
                actor.state.position_m = Vec3::new(i as f64, crate::WALK_Y, 95.0);
                w.add_character(actor);
            }
            let mut round = Round::new();
            round.seed(&mut w, &nav, 0.0, &c);
            let mut n = NightOffice::new(
                NightOfficeConfig {
                    enabled: true,
                    majors: false,
                    wards: false,
                    ambients: true,
                },
                0.0,
                &c,
            );
            n.seed(&w, &round);
            let mut events = vec![];
            n.ring(901.0, &mut w, &mut round, &c, &mut events);
            assert!(events.iter().any(
                |e| matches!(e,SchedulerEvent::Diagnostic(t) if t.contains("ambient evenings"))
            ));
            w.command_ledger.drain_updates();
            // A historical rewind of the processed cursor does not erase the
            // separate day guard. Save the guard before installing anything.
            n.last_office_days = Office::Waning.start_fraction();
            (n, w, round)
        };
    let (mut a, mut wa, mut ra) = setup();
    let (_, mut wb, mut rb) = setup();
    let (mut positive, mut wp, mut rp) = setup();
    let mut b = restore(&a, &wb, &c, 901.0);
    assert_eq!(b.last_ambient_reroll_day, Some(0));
    positive.last_ambient_reroll_day = None;
    let mut events = vec![];
    positive.ring(901.0, &mut wp, &mut rp, &c, &mut events);
    assert!(
        events
            .iter()
            .any(|e| matches!(e,SchedulerEvent::Diagnostic(t) if t.contains("ambient evenings"))),
        "positive control actually rerolls housed ambients"
    );
    let rounds = |w: &World| {
        w.roster
            .iter()
            .map(|id| w.characters[id].state.daily_round.clone())
            .collect::<Vec<_>>()
    };
    let before = rounds(&wa);
    for (n, w, r) in [(&mut a, &mut wa, &mut ra), (&mut b, &mut wb, &mut rb)] {
        let mut events = vec![];
        n.ring(901.0, w, r, &c, &mut events);
        assert!(events.is_empty());
    }
    assert_eq!(rounds(&wa), before);
    assert_eq!(rounds(&wa), rounds(&wb));
    assert_eq!(bytes(&a, &wa, &c, 901.0), bytes(&b, &wb, &c, 901.0));
    for (n, w, r) in [(&mut a, &mut wa, &mut ra), (&mut b, &mut wb, &mut rb)] {
        let mut events = vec![];
        n.ring(4501.0, w, r, &c, &mut events);
        assert_eq!(n.last_ambient_reroll_day, Some(1));
        assert!(
            events.iter().any(
                |e| matches!(e,SchedulerEvent::Diagnostic(t) if t.contains("ambient evenings"))
            )
        );
    }
    assert_ne!(rounds(&wa), before);
    assert_eq!(rounds(&wa), rounds(&wb));
    assert_eq!(bytes(&a, &wa, &c, 4501.0), bytes(&b, &wb, &c, 4501.0));
}

#[test]
fn checkpoint_night_disabled_or_unseeded_owner_keeps_owed_history() {
    for disabled in [false, true] {
        let (mut n, w, c, _) = staged(Subject::Person(ActorId::from_raw("mjr01")));
        if disabled {
            n.config.enabled = false;
        } else {
            n.seeded = false;
        }
        n.last_ambient_reroll_day = Some(-1);
        n.reflected = 17;
        n.dropped = 23;
        let other = restore(&n, &w, &c, 1.0);
        assert_eq!(other.owed(), 1);
        assert!(!other.enabled());
        assert_eq!(other.totals(), (17, 23));
        assert_eq!(other.last_ambient_reroll_day, Some(-1));
    }
}

#[test]
fn checkpoint_night_rejects_nonfront_or_multiple_admitted_queue_rows() {
    let (mut n, mut w, c, mut r) = staged(Subject::Person(ActorId::from_raw("mjr01")));
    r.busy = true;
    n.poll(1.0, &mut w, &c, &mut vec![], open(), &mut r, &env());
    n.enqueue(Subject::Ward(PlanningWard::Weigh), 0);
    let original: serde_json::Value = serde_json::from_slice(&bytes(&n, &w, &c, 2.0)).unwrap();
    for multiple in [false, true] {
        let mut bad = original.clone();
        if multiple {
            bad["night"]["queue"][1]["semantic"] = bad["night"]["queue"][0]["semantic"].clone();
        } else {
            bad["night"]["queue"].as_array_mut().unwrap().swap(0, 1);
        }
        let raw = serde_json::to_vec(&bad).unwrap();
        let b = CheckpointBudget::default();
        assert!(
            NightOfficeDtoV1::decode(
                &raw,
                b.reserve(Cohort::LoadCandidate, raw.len() + 4096).unwrap(),
                NightCheckpointContext::from_world(&w, time(2.0), &c)
            )
            .is_err()
        );
        assert_eq!(b.retained_bytes(), 0);
    }
}
