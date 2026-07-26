//! Tests for the Night Office (movement M6).
//!
//! The lane's three rules are the thing under test: one in flight, yields
//! absolutely, drops silently. The rest is what a reflection is allowed to
//! change, and what it is not.

use super::*;
use crate::{
    character::{Character, CharacterSheet},
    fake::FakeCognition,
    lore::LoreProfile,
    math::Vec3,
    places::{PlaceEntry, PlaceRegistry},
    traits::{CognitionBusy, CognitionError},
};
use std::collections::BTreeSet;

fn env() -> PromptEnv {
    PromptEnv::new(
        include_str!("../../../../assets/prompts/turn.j2"),
        include_str!("../../../../assets/prompts/night.j2"),
        include_str!("../../../../assets/prompts/strings.toml"),
    )
    .expect("the shipped prompt assets load")
}

/// One game day per real hour, opening at the Waning — an hour before the
/// earliest shipped bedtime, so a test can cross Lamplight and the Snuffing
/// without a wrap.
fn clock() -> WorldClock {
    WorldClock::new(3600.0, Office::Waning, 0, 0.05)
}

/// Real seconds from the clock's epoch to `office` on day 0.
fn at_office(clock: &WorldClock, office: Office) -> f64 {
    let target = office.start_fraction();
    let start = Office::Waning.start_fraction();
    (target - start) * clock.seconds_per_day()
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
            core_character_description: "You bake.".into(),
            extended_character_description: String::new(),
            curiosity: None,
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
fn world_with_cast() -> World {
    let mut world = World::new();
    world.add_character(player());

    let mut major = character("mjr01", "Corin Copp", Significance::Major, PlanningWard::Weigh);
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
    let mut night = NightOffice::new(enabled_tiers, now);
    night.seed(world, &Round::new());
    night
}

fn all_tiers() -> NightOfficeConfig {
    NightOfficeConfig {
        enabled: true,
        ..NightOfficeConfig::default()
    }
}

/// The gate wide open: nobody near the player, no floor, no reaction owed.
fn open() -> NightGate {
    NightGate {
        floor_busy: false,
        player_composing: false,
        stage_occupied: false,
        player_reaction: false,
    }
}

/// Ring, then poll, in the order the engine does.
#[allow(clippy::too_many_arguments)]
fn beat(
    night: &mut NightOffice,
    world: &mut World,
    round: &mut Round,
    clock: &WorldClock,
    now: f64,
    gate: NightGate,
    cognition: &mut FakeCognition,
    env: &PromptEnv,
) -> Vec<SchedulerEvent> {
    let mut events = Vec::new();
    night.ring(now, world, round, clock, &mut events);
    let mut completions = cognition.drain_completions();
    events.extend(night.poll(
        now,
        world,
        clock,
        &mut completions,
        gate,
        cognition,
        env,
    ));
    events
}

fn diagnostics(events: &[SchedulerEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event {
            SchedulerEvent::Diagnostic(line) => Some(line.clone()),
            _ => None,
        })
        .collect()
}

// --------------------------------------------------------------------------- //
// The lane
// --------------------------------------------------------------------------- //

/// Nobody's bedtime has come, so the lane costs nothing at all.
#[test]
fn a_lane_with_no_bedtime_crossed_never_asks_for_the_slot() {
    let world = world_with_cast();
    let night = office(all_tiers(), &world, 0.0);
    assert!(night.enabled());
    assert_eq!(night.owed(), 0);
    assert!(!night.wants_slot(0.0), "no bell has rung yet");
}

/// The Snuffing owes one reflection per Major plus one per populated ward, and
/// exactly one request goes out for them.
#[test]
fn the_curfew_owes_every_major_and_every_populated_ward_one_reflection() {
    let mut world = world_with_cast();
    let mut round = Round::new();
    let clock = clock();
    let mut night = office(all_tiers(), &world, 0.0);
    let mut cognition = FakeCognition::new();
    let env = env();

    let now = at_office(&clock, Office::Snuffing) + 1.0;
    beat(
        &mut night,
        &mut world,
        &mut round,
        &clock,
        now,
        open(),
        &mut cognition,
        &env,
    );
    // One Major (bedtime defaults to the curfew without a seeded round), and
    // the two wards that hold a Minor — Weigh and Reed. One is already out.
    assert_eq!(night.owed(), 2, "one spent, two still owed");
    assert!(night.in_flight_subject().is_some());
    assert_eq!(cognition.prompts().len(), 1, "one in flight, ever");
}

/// Rule 2, four ways. Each closed gate alone keeps the lane standing down, and
/// none of them loses the reflection — it is still owed when the gate opens.
#[test]
fn the_lane_yields_to_the_floor_the_microphone_the_stage_and_the_players_lane() {
    for closed in [
        NightGate {
            floor_busy: true,
            ..open()
        },
        NightGate {
            player_composing: true,
            ..open()
        },
        NightGate {
            stage_occupied: true,
            ..open()
        },
        NightGate {
            player_reaction: true,
            ..open()
        },
    ] {
        assert!(closed.yields());
        let mut world = world_with_cast();
        let mut round = Round::new();
        let clock = clock();
        let mut night = office(all_tiers(), &world, 0.0);
        let mut cognition = FakeCognition::new();
        let env = env();

        let now = at_office(&clock, Office::Snuffing) + 1.0;
        beat(
            &mut night,
            &mut world,
            &mut round,
            &clock,
            now,
            closed,
            &mut cognition,
            &env,
        );
        assert!(
            cognition.prompts().is_empty(),
            "the lane submitted through a closed gate: {closed:?}"
        );
        assert_eq!(night.owed(), 3, "nothing was lost, only deferred");

        // …and the moment the gate opens, it spends.
        beat(
            &mut night,
            &mut world,
            &mut round,
            &clock,
            now + 0.1,
            open(),
            &mut cognition,
            &env,
        );
        assert_eq!(cognition.prompts().len(), 1);
    }
}

/// Rule 3: the night ends and the rest keep yesterday's Round. No error, no
/// retry, no percept — the queue simply empties.
#[test]
fn reflections_the_night_outran_drop_silently_when_the_day_rolls_over() {
    let mut world = world_with_cast();
    let mut round = Round::new();
    let clock = clock();
    let mut night = office(all_tiers(), &world, 0.0);
    let mut cognition = FakeCognition::new();
    let env = env();

    let dusk = at_office(&clock, Office::Snuffing) + 1.0;
    // The gate is shut all night, so nothing is spent.
    beat(
        &mut night,
        &mut world,
        &mut round,
        &clock,
        dusk,
        NightGate {
            stage_occupied: true,
            ..open()
        },
        &mut cognition,
        &env,
    );
    assert_eq!(night.owed(), 3);

    // Morning, one game day on: the queue is stale and the first free poll
    // clears it instead of spending it.
    let morning = dusk + clock.seconds_per_day();
    beat(
        &mut night,
        &mut world,
        &mut round,
        &clock,
        morning,
        open(),
        &mut cognition,
        &env,
    );
    let (_, dropped) = night.totals();
    assert_eq!(dropped, 3, "the whole stale night dropped");
    // The bells crossed on the way to morning owed a *fresh* night, and that
    // one is spent normally.
    assert!(cognition.prompts().len() <= 1, "still one in flight, ever");
}

/// A subject already reflected tonight is not queued twice, however many times
/// the bell is crossed — a debug time-scale must not buy anyone two nights.
#[test]
fn nobody_reflects_twice_in_one_night() {
    let mut world = world_with_cast();
    let mut round = Round::new();
    let clock = clock();
    let mut night = office(all_tiers(), &world, 0.0);
    let mut cognition = FakeCognition::new();
    let env = env();

    let dusk = at_office(&clock, Office::Snuffing) + 1.0;
    for step in 0..4 {
        beat(
            &mut night,
            &mut world,
            &mut round,
            &clock,
            dusk + step as f64 * 0.5,
            NightGate {
                stage_occupied: true,
                ..open()
            },
            &mut cognition,
            &env,
        );
    }
    assert_eq!(night.owed(), 3, "one Major and two wards, once each");
}

/// A backend with no second slot refuses, and the reflection waits for its own
/// lane rather than reaching for the player's.
#[test]
fn a_backend_without_a_second_slot_never_gets_night_work() {
    /// The default [`Cognition::request_night`]: refuse.
    #[derive(Default)]
    struct OneLane {
        foreground: Vec<String>,
    }
    impl Cognition for OneLane {
        fn request(&mut self, prompt: String) -> Result<RequestId, CognitionBusy> {
            self.foreground.push(prompt);
            Ok(RequestId(0))
        }
    }

    let mut world = world_with_cast();
    let mut round = Round::new();
    let clock = clock();
    let mut night = office(all_tiers(), &world, 0.0);
    let mut cognition = OneLane::default();
    let env = env();

    let now = at_office(&clock, Office::Snuffing) + 1.0;
    let mut events = Vec::new();
    night.ring(now, &mut world, &mut round, &clock, &mut events);
    night.poll(
        now,
        &mut world,
        &clock,
        &mut Vec::new(),
        open(),
        &mut cognition,
        &env,
    );
    assert!(
        cognition.foreground.is_empty(),
        "a refused night request must never fall back to the turn stream"
    );
    assert_eq!(night.owed(), 3, "put back at the front, not lost");
    assert!(
        !night.wants_slot(now),
        "and not retried on the very next frame"
    );
    assert!(night.wants_slot(now + RETRY_SECONDS));
}

/// A provider failure is archived and forgotten. No backoff, no retry, and —
/// above all — no `system:` line waiting in the morning's inbox.
#[test]
fn a_failed_reflection_is_archived_and_never_reaches_an_inbox() {
    let mut world = world_with_cast();
    let mut round = Round::new();
    let clock = clock();
    let mut night = office(all_tiers(), &world, 0.0);
    let mut cognition = FakeCognition::new();
    let env = env();

    let now = at_office(&clock, Office::Snuffing) + 1.0;
    beat(
        &mut night,
        &mut world,
        &mut round,
        &clock,
        now,
        open(),
        &mut cognition,
        &env,
    );
    // Answer the outstanding request with a failure instead of the fake's reply.
    let mut completions = cognition.drain_completions();
    completions[0].result = Err(CognitionError::new("TimeoutError"));
    let events = night.poll(
        now + 0.1,
        &mut world,
        &clock,
        &mut completions,
        open(),
        &mut cognition,
        &env,
    );

    let archived = events
        .iter()
        .filter(|event| matches!(event, SchedulerEvent::PromptExchange { .. }))
        .count();
    assert_eq!(archived, 1, "a failed exchange still happened");
    assert!(
        diagnostics(&events)
            .iter()
            .any(|line| line.contains("reflection failed")),
    );
    for actor in world.characters.values() {
        assert!(
            actor.inbox().is_empty(),
            "{} was told about a night that did not happen",
            actor.name()
        );
    }
}

/// The lane takes its own completion out of the queue and leaves everybody
/// else's — otherwise the scheduler would log it as a stale result and lose it.
#[test]
fn the_night_harvests_only_its_own_completion() {
    let mut world = world_with_cast();
    let mut round = Round::new();
    let clock = clock();
    let mut night = office(all_tiers(), &world, 0.0);
    let mut cognition = FakeCognition::new();
    let env = env();

    let now = at_office(&clock, Office::Snuffing) + 1.0;
    beat(
        &mut night,
        &mut world,
        &mut round,
        &clock,
        now,
        open(),
        &mut cognition,
        &env,
    );
    let mut completions = cognition.drain_completions();
    // A foreground turn's completion, riding the same vec.
    completions.push(Completion {
        request_id: RequestId(9_999),
        result: Ok("wait {}".to_string()),
        duration_seconds: 0.0,
    });
    night.poll(
        now + 0.1,
        &mut world,
        &clock,
        &mut completions,
        open(),
        &mut cognition,
        &env,
    );
    assert_eq!(completions.len(), 1, "the scheduler's is left alone");
    assert_eq!(completions[0].request_id, RequestId(9_999));
}

/// The night is trickled, not fired off in one burst — and the pace is a slice
/// of the *game* day, so the debug time-scale speeds the night up with the sun
/// instead of dropping it unspent.
#[test]
fn reflections_are_paced_by_the_game_clock_not_by_the_frame() {
    let mut world = world_with_cast();
    let mut round = Round::new();
    let clock = clock();
    let mut night = office(all_tiers(), &world, 0.0);
    let mut cognition = FakeCognition::new();
    let env = env();

    let now = at_office(&clock, Office::Snuffing) + 1.0;
    beat(
        &mut night,
        &mut world,
        &mut round,
        &clock,
        now,
        open(),
        &mut cognition,
        &env,
    );
    assert_eq!(cognition.prompts().len(), 1);

    // The answer lands, but the next reflection still waits out its slice.
    let pace = super::pace_seconds(&clock);
    assert!(pace > 0.0, "a shipped clock paces the night");
    beat(
        &mut night,
        &mut world,
        &mut round,
        &clock,
        now + pace * 0.5,
        open(),
        &mut cognition,
        &env,
    );
    assert_eq!(cognition.prompts().len(), 1, "still inside the slice");
    beat(
        &mut night,
        &mut world,
        &mut round,
        &clock,
        now + pace * 1.1,
        open(),
        &mut cognition,
        &env,
    );
    assert_eq!(cognition.prompts().len(), 2, "and out the other side");

    // Sixty times the clock is sixty times the trickle: the whole night still
    // fits inside a night.
    let fast = clock.with_scale(0.0, 60.0);
    assert!(
        (super::pace_seconds(&fast) - pace / 60.0).abs() < 1e-9,
        "the pace follows the game day, not the wall clock"
    );
}

/// Off is off: no bedtimes resolved, no queue, no seed diagnostic.
#[test]
fn the_lane_is_inert_until_the_host_asks_for_it() {
    let world = world_with_cast();
    let mut night = NightOffice::new(NightOfficeConfig::default(), 0.0);
    assert!(night.seed(&world, &Round::new()).is_none());
    assert!(!night.enabled());
    assert!(!night.wants_slot(1e6));
}

// --------------------------------------------------------------------------- //
// What a reflection may change
// --------------------------------------------------------------------------- //

/// The night's four verbs land; everything that would happen *in the world* is
/// refused, and refused with a diagnostic rather than an inbox line.
#[test]
fn a_reflection_may_settle_memory_goal_and_one_leg_but_may_not_act() {
    let mut world = world_with_cast();
    let mut night = office(all_tiers(), &world, 0.0);
    let major = ActorId::from_raw("mjr01");

    let reply = concat!(
        r#"remember {"memory": "I sold the last loaf to a stranger."}"#,
        "\n",
        r#"set_goal {"goal": "Buy flour before the Kindling"}"#,
        "\n",
        r#"set_round {"leg": 2, "place_id": "pl_bbbb"}"#,
        "\n",
        r#"say {"text": "Goodnight!"}"#,
        "\n",
        r#"go_to {"place_id": "pl_bbbb"}"#,
    );
    let mut events = Vec::new();
    night.apply_person(&mut world, &major, reply, &mut events);

    let actor = &world.characters[&major];
    assert_eq!(actor.memories(), ["I sold the last loaf to a stranger."]);
    assert_eq!(actor.goal(), "Buy flour before the Kindling");
    assert_eq!(
        actor.state.round_edit,
        Some(crate::character::RoundEdit {
            leg: 1,
            place_id: PlaceId::from_raw("pl_bbbb"),
        }),
        "leg 2 on the sheet is index 1 in the round"
    );
    assert!(actor.state.intent.is_none(), "a reflection cannot walk");
    assert!(actor.inbox().is_empty());

    let refused = diagnostics(&events);
    assert!(refused.iter().any(|line| line.contains("say is not a night verb")));
    assert!(refused.iter().any(|line| line.contains("go_to is not a night verb")));
    // Nobody heard a thing.
    for other in world.characters.values() {
        assert!(other.inbox().is_empty(), "{} heard a private thought", other.name());
    }
}

/// The whitelist stands at midnight: a Major may only point their day at a
/// place they hold a handle for.
#[test]
fn set_round_refuses_a_place_the_actor_does_not_know_the_way_to() {
    let mut world = world_with_cast();
    let mut night = office(all_tiers(), &world, 0.0);
    let minor = ActorId::from_raw("mnr01");

    let mut events = Vec::new();
    night.apply_person(
        &mut world,
        &minor,
        r#"set_round {"leg": 1, "place_id": "pl_bbbb"}"#,
        &mut events,
    );
    assert!(world.characters[&minor].state.round_edit.is_none());
    assert!(
        diagnostics(&events)
            .iter()
            .any(|line| line.contains("set_round failed")),
    );
}

/// A leg number off the end of the sheet is refused rather than silently
/// clamped: the model is naming a leg it did not read.
#[test]
fn set_round_refuses_a_leg_number_that_is_not_on_the_sheet() {
    let mut world = world_with_cast();
    let mut night = office(all_tiers(), &world, 0.0);
    let major = ActorId::from_raw("mjr01");

    for bad in ["0", "3", "-1", r#""two""#] {
        let mut events = Vec::new();
        night.apply_person(
            &mut world,
            &major,
            &format!(r#"set_round {{"leg": {bad}, "place_id": "pl_aaaa"}}"#),
            &mut events,
        );
        assert!(
            world.characters[&major].state.round_edit.is_none(),
            "leg {bad} was accepted"
        );
    }
}

/// The ward's mood reaches every Minor of that ward and nobody else — not the
/// Majors, who reflect for themselves, and not another ward's people.
#[test]
fn a_ward_mood_is_carried_by_that_wards_minors_alone() {
    let mut world = world_with_cast();
    let mut night = office(all_tiers(), &world, 0.0);
    let env = env();

    let mut events = Vec::new();
    night.apply_ward(
        &mut world,
        PlanningWard::Weigh,
        r#"ward_mood {"mood": "The rain has not let up and people are short with one another."}"#,
        &mut events,
    );
    assert_eq!(
        world.ward_moods.get(&PlanningWard::Weigh).map(String::as_str),
        Some("The rain has not let up and people are short with one another.")
    );

    let sheet = |id: &str| render_night_prompt(&world, &ActorId::from_raw(id), &env).unwrap();
    assert!(sheet("mnr01").contains("**the_ward_says**"), "the ward's own Minor");
    assert!(
        !sheet("mjr01").contains("**the_ward_says**"),
        "a Major reflects for themselves"
    );
    assert!(
        !sheet("mnr02").contains("**the_ward_says**"),
        "another ward's Minor"
    );
}

/// A mood long enough to be a token leak is cut to the cap — it rides every
/// prompt of a hundred and twenty people for a game day.
#[test]
fn a_ward_mood_is_bounded() {
    let mut world = world_with_cast();
    let mut night = office(all_tiers(), &world, 0.0);
    let long = "a".repeat(WARD_MOOD_MAX_CHARS * 3);
    let mut events = Vec::new();
    night.apply_ward(
        &mut world,
        PlanningWard::Weigh,
        &format!(r#"ward_mood {{"mood": "{long}"}}"#),
        &mut events,
    );
    assert_eq!(
        world.ward_moods[&PlanningWard::Weigh].chars().count(),
        WARD_MOOD_MAX_CHARS
    );
}

/// A ward may move its own Minors' rounds, and doing so teaches them the way —
/// the ward decided it, so requiring them to have already known the place would
/// make the verb useless. It may not reach outside its own people, and it may
/// not make more than [`WARD_EDITS_MAX`] edits.
#[test]
fn a_ward_moves_its_own_peoples_rounds_and_teaches_them_the_way() {
    let mut world = world_with_cast();
    let mut night = office(all_tiers(), &world, 0.0);
    let minor = ActorId::from_raw("mnr01");
    assert!(
        !world.characters[&minor].state.places_known.contains(&PlaceId::from_raw("pl_bbbb")),
        "the Minor starts knowing no ways"
    );

    let mut events = Vec::new();
    night.apply_ward(
        &mut world,
        PlanningWard::Weigh,
        concat!(
            r#"set_round {"person": "mnr01", "leg": 1, "place_id": "pl_bbbb"}"#,
            "\n",
            // Another ward's person, and a Major: both out of reach.
            r#"set_round {"person": "mnr02", "leg": 1, "place_id": "pl_bbbb"}"#,
            "\n",
            r#"set_round {"person": "mjr01", "leg": 1, "place_id": "pl_bbbb"}"#,
        ),
        &mut events,
    );

    assert_eq!(
        world.characters[&minor].state.round_edit,
        Some(crate::character::RoundEdit {
            leg: 0,
            place_id: PlaceId::from_raw("pl_bbbb"),
        })
    );
    assert!(
        world.characters[&minor].state.places_known.contains(&PlaceId::from_raw("pl_bbbb")),
        "the ward told them the way as part of deciding it"
    );
    assert!(world.characters[&ActorId::from_raw("mnr02")].state.round_edit.is_none());
    assert!(world.characters[&ActorId::from_raw("mjr01")].state.round_edit.is_none());
}

/// The ward speaks for its people; it does not get to rewrite them.
#[test]
fn a_ward_may_make_no_more_than_three_edits() {
    let mut world = world_with_cast();
    let mut night = office(all_tiers(), &world, 0.0);
    let reply = [r#"set_round {"person": "mnr01", "leg": 1, "place_id": "pl_aaaa"}"#;
        WARD_EDITS_MAX + 2]
    .join("\n");

    let mut events = Vec::new();
    night.apply_ward(&mut world, PlanningWard::Weigh, &reply, &mut events);
    assert_eq!(
        diagnostics(&events)
            .iter()
            .filter(|line| line.contains("more than"))
            .count(),
        2
    );
}

// --------------------------------------------------------------------------- //
// The prompts
// --------------------------------------------------------------------------- //

/// The night sheet is the day sheet, numbered — which is the whole reason
/// `set_round` can name a leg at all — and it wraps it in bedtime instructions
/// rather than turn ones.
#[test]
fn the_night_prompt_numbers_the_round_and_offers_only_the_night_verbs() {
    let world = world_with_cast();
    let prompt = render_night_prompt(&world, &ActorId::from_raw("mjr01"), &env()).unwrap();

    assert!(prompt.contains("- leg 1 — at Dayspring: work at The Tallage"));
    assert!(prompt.contains("- leg 2 — at Lamplight: home to sleep"));
    assert!(prompt.contains("pl_aaaa The Tallage"), "handles to name");
    assert!(prompt.contains("set_round"));
    assert!(
        !prompt.contains(r#"say {"target""#),
        "no speech verb is offered at midnight"
    );
    assert!(!prompt.contains("go_to {"));
}

/// A night prompt reads the inbox and leaves it: whatever reached this person
/// on the way home is still news to them in the morning.
#[test]
fn the_night_prompt_does_not_drain_the_inbox() {
    let mut world = world_with_cast();
    let major = ActorId::from_raw("mjr01");
    world
        .characters
        .get_mut(&major)
        .unwrap()
        .notify_percept("Someone said: \"The gate is shut.\"");

    let prompt = render_night_prompt(&world, &major, &env()).unwrap();
    assert!(prompt.contains("The gate is shut."));
    assert_eq!(
        world.characters[&major].inbox().len(),
        1,
        "the morning still gets it"
    );
}

/// The ward digest is not a character sheet: it carries who lives there, what
/// they are set on, and where their feet may be pointed.
#[test]
fn the_ward_prompt_carries_its_people_their_goals_and_their_places() {
    let world = world_with_cast();
    let prompt = render_ward_prompt(&world, PlanningWard::Weigh, &env()).unwrap();

    assert!(prompt.contains("**the_ward** — the Weigh Ward"));
    assert!(prompt.contains("mnr01 — Tam Rud, Baker — set on: Find work for the winter"));
    assert!(!prompt.contains("mjr01"), "a Major is not the ward's to speak for");
    assert!(!prompt.contains("mnr02"), "nor another ward's Minor");
    assert!(prompt.contains("pl_aaaa The Tallage"));
    assert!(prompt.contains("ward_mood"));
    assert!(prompt.contains("**last_night** — nothing yet"));
}

/// The offline fake reads its `set_round` off the rendered sheet, so the whole
/// night path — numbered legs, place handles, the verb, the recorded edit —
/// runs end to end with no provider and no key.
#[test]
fn the_offline_fake_moves_a_leg_it_read_off_the_night_sheet() {
    let mut world = world_with_cast();
    let mut round = Round::new();
    let clock = clock();
    let mut night = office(
        NightOfficeConfig {
            enabled: true,
            majors: true,
            wards: false,
            ambients: false,
        },
        &world,
        0.0,
    );
    let mut cognition = FakeCognition::new();
    let env = env();

    let now = at_office(&clock, Office::Snuffing) + 1.0;
    beat(
        &mut night,
        &mut world,
        &mut round,
        &clock,
        now,
        open(),
        &mut cognition,
        &env,
    );
    let events = beat(
        &mut night,
        &mut world,
        &mut round,
        &clock,
        now + 0.1,
        open(),
        &mut cognition,
        &env,
    );

    let major = &world.characters[&ActorId::from_raw("mjr01")];
    assert!(
        major.memories().iter().any(|memory| memory.contains("Corin Copp")),
        "the fake settled a memory it read off the sheet"
    );
    assert_eq!(
        major.state.round_edit,
        // `places_you_know` is sorted by name, so the sheet's first handle —
        // and the one the fake reads — is The Hungry Ox, not The Tallage.
        Some(crate::character::RoundEdit {
            leg: 0,
            place_id: PlaceId::from_raw("pl_bbbb"),
        }),
    );
    assert!(
        diagnostics(&events)
            .iter()
            .any(|line| line.contains("Corin Copp reflected")),
    );
    let (reflected, _) = night.totals();
    assert_eq!(reflected, 1);
}
