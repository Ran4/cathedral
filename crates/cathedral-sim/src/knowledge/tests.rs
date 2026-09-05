//! The store's own tests: the constants' brackets, the derived heat, the merge
//! rule's four rows, the seal, and the one place a fact becomes words.
//!
//! `Fact::source` is private, so these build facts through
//! [`FactCatalog::from_json`] exactly as an integration test must — which gets
//! the loader's validation exercised for free. A **carried** holding (the only
//! way to reach hops ≥ 1, a low heat or a garbled view in M1) comes from
//! [`learn`], whose reader must not be in the fact's `seeded` set.

use std::collections::BTreeSet;

use super::*;
use crate::character::{Character, CharacterSheet, Control};
use crate::clock::{Office, Weekday, WorldTime};
use crate::ids::ActorId;
use crate::lore::{LoreProfile, PlanningWard, Significance};
use crate::math::Vec3;
use crate::prompt::PromptEnv;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// A profile is what makes `occupation_display`, `planning_ward` and `knows`
/// real, which is most of what [`render_line`] renders.
fn profile(occupation: Option<&str>, display: Option<&str>, ward: PlanningWard) -> LoreProfile {
    LoreProfile {
        significance: Significance::Minor,
        planning_ward: ward,
        age: 30,
        gender: "f".into(),
        occupation_id: occupation.map(str::to_string),
        occupation_display: display.map(str::to_string),
        title: None,
        rank: None,
        faction_role: None,
        illegal_activity: None,
        district: "Wick".into(),
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
    }
}

fn character(id: &str, name: &str, lore: Option<LoreProfile>, knows: &[&str]) -> Character {
    Character::from_sheet(CharacterSheet {
        pockets: Vec::new(),
        frontbutt: None,
        id: ActorId::from_raw(id),
        name: name.to_string(),
        control: Control::Llm,
        back_story: String::new(),
        location_description: String::new(),
        appearance: Default::default(),
        voice_key: None,
        position_m: Vec3::new(0.0, 0.91, 0.0),
        facing_yaw: 0.0,
        holds: Vec::new(),
        goal: crate::GOAL_NONE.into(),
        memories: Vec::new(),
        knows: knows.iter().map(|id| ActorId::from_raw(*id)).collect(),
        lore,
        presence: crate::character::Presence::InCity,
        presence_epoch: 0,
        economic_class: crate::character::EconomicClass::Resident,
    })
}

fn actor(id: &str) -> ActorId {
    ActorId::from_raw(id)
}

/// A world with a clock at day 0, so heat ages and `{day}` resolves.
fn world_at_day(day: i64) -> World {
    let mut world = World::new();
    world.current_time = Some(WorldTime {
        day,
        fraction: 0.0,
        office: Office::Dayspring,
        weekday: Weekday::Bellday,
    });
    world
}

/// The shipped prompt strings, so a hedge assertion reads the same bytes the
/// game does. Read through the real loader, which is what the sheet uses.
fn strings() -> crate::prompt::PromptStrings {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let read = |name: &str| {
        std::fs::read_to_string(root.join("assets/prompts").join(name))
            .unwrap_or_else(|error| panic!("read {name}: {error}"))
    };
    PromptEnv::new(&read("turn.j2"), &read("night.j2"), &read("strings.toml"))
        .expect("the shipped prompt assets must load")
        .strings()
        .clone()
}

/// Seed one inline-JSON fact into `world` and hand back its key.
fn seed_one(world: &mut World, row: &str) -> FactKey {
    let json = format!("{{\"schema_version\": 1, \"facts\": [{row}]}}");
    let catalog = FactCatalog::from_json(&json).expect("the row parses");
    let diagnostics = catalog.seed(world);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let id = catalog.specs()[0].id.clone();
    world.knowledge.key_of(&id).expect("the row installed")
}

// ---------------------------------------------------------------------------
// T1–T3 — the numbers
// ---------------------------------------------------------------------------

/// T1. Pins `02_numbers.md`'s derivation before any measurement exists, so a
/// later retune that breaks a bracket fails here rather than in M5.
///
/// Comparing constants is the whole point: clippy's `assertions_on_constants`
/// reads it as a tautology, but it is a *contract* between two numbers in
/// separate places, and only M5's tuning pass may move either of them.
#[test]
#[allow(clippy::assertions_on_constants)]
fn the_constants_hold_their_own_inequalities() {
    assert!(
        POLLEN_POLL_MAX_GAME_MINUTES < 60.0 / STIRS_PER_GAME_HOUR,
        "the poll gap cannot skip a stir window: at most one roll per person per \
         fact per stir is the whole of the cadence model, and a gap wider than a \
         window makes the effective roll rate a function of the jitter"
    );
    assert!(
        f64::from(HOP_LOSS).powi(4) > f64::from(VOLUNTEER_HEAT),
        "a fourth-hand top-band story must still be repeatable, or the ladder's \
         far rungs are unreachable and a chain is not walkable"
    );
    assert!(
        f64::from(HOP_LOSS) * 0.12 < f64::from(VOLUNTEER_HEAT),
        "an off-affinity trade matter must travel one hop and stop"
    );
    assert!(0.0 < HEAT_GONE_BELOW && HEAT_GONE_BELOW < VOLUNTEER_HEAT);
    assert!(REHEAT_TO > VOLUNTEER_HEAT);
    assert!(KNOWN_SHEET_MAX < crate::notices::NOTICES_SHEET_MAX);
    assert!(AIR_HALF_LIFE_GAME_HOURS > 0.0);
}

/// T2. Heat is derived, not stored, so it cannot drift with the poll rate — and
/// a clock-less world does not age at all.
#[test]
fn heat_is_derived_and_a_clockless_world_does_not_age() {
    let unstamped = Held {
        hops: 0,
        from: None,
        learned_on: None,
        view: FactView::default(),
        heat_at_learn: 1.0,
    };
    for at in [None, Some(0.0), Some(5.0), Some(500.0)] {
        assert_eq!(unstamped.heat(at), 1.0, "an unstamped holding must not age");
    }

    let stamped = Held {
        learned_on: Some(0.0),
        ..unstamped.clone()
    };
    assert_eq!(stamped.heat(None), 1.0, "no clock means nothing ages");
    // Half a game day is 12 game hours, one half-life.
    let one_jump = stamped.heat(Some(0.5));
    assert!(
        (one_jump - 0.5).abs() < 0.001,
        "12 game hours is one half-life, got {one_jump}"
    );
    // The value is a function of the clock, never an accumulation: read at a
    // hundred intermediate instants it never rises, the hundredth read *is* the
    // one jump, and two quarter-day reads compose exactly into the half-day one
    // (λ^6 × λ^6 = λ^12).
    let mut previous = stamped.heat(Some(0.0));
    assert_eq!(previous, 1.0);
    for step in 1..=100 {
        let now = stamped.heat(Some(f64::from(step) * 0.005));
        assert!(
            now <= previous,
            "heat rose from {previous} to {now} at step {step}"
        );
        previous = now;
    }
    assert_eq!(previous, one_jump);
    let quarter = stamped.heat(Some(0.25));
    assert!(
        (quarter * quarter - one_jump).abs() < 0.001,
        "λ^6 squared must be λ^12: {quarter}² against {one_jump}"
    );
}

/// T3. Quantise before any change test, or a raw-`f32` comparison churns forever.
#[test]
fn heat_pct_quantises_before_any_change_test() {
    assert_eq!(heat_pct(0.115), 12);
    assert_eq!(heat_pct(0.1151), 12);
    assert_eq!(heat_pct(f32::NAN), 0);
    assert_eq!(heat_pct(2.0), 100);
    assert_eq!(heat_pct(-1.0), 0);
}

// ---------------------------------------------------------------------------
// T4–T7 — the merge rule and the store
// ---------------------------------------------------------------------------

/// A world holding one fact seeded to nobody, plus one carrier who is not its
/// subject — the only shape `learn` will write to.
fn merge_world() -> (World, ActorId, FactKey) {
    let mut world = world_at_day(0);
    world.add_character(character("c1", "Carrier One", None, &[]));
    // `seed` drops a row nobody in this world holds, so the fact is installed by
    // hand through the same loader path with a seeded holder who is not the
    // carrier.
    let key = seed_one(
        &mut world,
        r#"{"id": "test.merge.row", "topic": "law", "said": "the beam was wrong",
            "seeded": ["c1"]}"#,
    );
    // `c1` is seeded, so it cannot be the carrier; add a second body for that.
    world.add_character(character("c2", "Carrier Two", None, &[]));
    (world, actor("c2"), key)
}

fn telling(hops: u8, heat: f32, subject: Option<&str>) -> Telling {
    Telling {
        hops,
        from: Some(actor("mouth")),
        heat,
        view: FactView {
            subject: subject.map(actor),
            place: None,
            day_offset: 0,
        },
    }
}

/// T4. All four rows of the merge rule, with the heat inputs pinned so this and
/// M3's chain test cannot come to disagree.
#[test]
fn the_merge_rule_all_four_rows() {
    let (mut world, carrier, key) = merge_world();
    let now = Some(0.0);

    assert_eq!(
        learn(&mut world, &carrier, key, telling(3, 0.5, Some("far")), now),
        Learned::Fresh
    );
    let held = holds_key(&world, &carrier, key).expect("the row is stored");
    assert_eq!(held.hops, 3);

    // Fewer hops replaces the view, the chain link and the count.
    assert_eq!(
        learn(
            &mut world,
            &carrier,
            key,
            telling(1, 0.5, Some("near")),
            now
        ),
        Learned::Corrected
    );
    let held = holds_key(&world, &carrier, key).expect("the row is stored");
    assert_eq!(held.hops, 1);
    assert_eq!(held.view.subject, Some(actor("near")));
    assert_eq!(held.from, Some(actor("mouth")));
    assert_eq!(heat_pct(held.heat(now)), 50);

    // Equal hops, warmer: the heat moves and the held view survives — a person
    // does not flip-flop between two equally distant versions.
    assert_eq!(
        learn(
            &mut world,
            &carrier,
            key,
            telling(1, 0.8, Some("other")),
            now
        ),
        Learned::Warmed
    );
    let held = holds_key(&world, &carrier, key).expect("the row is stored");
    assert_eq!(held.view.subject, Some(actor("near")));
    assert_eq!(heat_pct(held.heat(now)), 80);

    // Equal hops, no warmer: nothing at all.
    assert_eq!(
        learn(
            &mut world,
            &carrier,
            key,
            telling(1, 0.8, Some("other")),
            now
        ),
        Learned::Unchanged
    );

    // More hops only warms, and never moves the count.
    assert_eq!(
        learn(&mut world, &carrier, key, telling(4, 0.9, Some("far")), now),
        Learned::Warmed
    );
    let held = holds_key(&world, &carrier, key).expect("the row is stored");
    assert_eq!(held.hops, 1);
    assert_eq!(held.view.subject, Some(actor("near")));
    assert_eq!(heat_pct(held.heat(now)), 90);

    assert_eq!(
        learn(&mut world, &carrier, key, telling(4, 0.1, None), now),
        Learned::Unchanged
    );
    assert_eq!(
        holds_key(&world, &carrier, key).map(|held| heat_pct(held.heat(now))),
        Some(90)
    );
}

/// T5. The fourth row: a witness cannot be talked out of what they saw.
#[test]
fn a_witness_cannot_be_talked_out_of_what_they_saw() {
    let (mut world, _, key) = merge_world();
    let witness = actor("c1");
    assert_eq!(
        learn(
            &mut world,
            &witness,
            key,
            telling(1, 1.0, Some("somebody else")),
            Some(0.0)
        ),
        Learned::Refused
    );
    assert_eq!(world.knowledge.holdings_len(&witness), 0);
    let held = holds_key(&world, &witness, key).expect("the witness still holds it");
    assert_eq!(held.hops, 0);
    assert_eq!(held.heat(Some(9.0)), 1.0);
    assert_eq!(held.from, None);
    assert!(held.view.is_pristine());
}

/// T6. `holds` answers `seeded` before the carrier store, and is pure.
#[test]
fn holds_answers_seeded_before_the_store_and_is_pure() {
    let (mut world, carrier, key) = merge_world();
    learn(&mut world, &carrier, key, telling(4, 0.2, None), Some(0.0));

    // Plant a stray far-off row for somebody who is **also** seeded. `learn`
    // refuses it (row four), so it goes in through the private insert — which is
    // the whole point: a stray row must never be readable as a garbled first-hand
    // holding, however it got there.
    let witness = actor("c1");
    let stray = Held::carried(
        4,
        Some(actor("mouth")),
        0.2,
        Some(0.0),
        FactView {
            subject: Some(actor("somebody else")),
            place: None,
            day_offset: 2,
        },
    );
    insert_holding(
        &mut world.knowledge,
        &witness,
        Holding::of(key, stray),
        Some(0.0),
    );
    assert_eq!(world.knowledge.holdings_len(&witness), 1);

    let held = holds_key(&world, &witness, key).expect("the witness holds it");
    assert_eq!(held.hops, 0, "seeded is answered before the store");
    assert_eq!(held.heat(Some(0.0)), 1.0);
    assert_eq!(held.from, None);
    assert!(held.view.is_pristine());

    let before = world.clone();
    for _ in 0..100 {
        assert_eq!(
            holds_key(&world, &carrier, key),
            holds_key(&before, &carrier, key)
        );
    }
    assert_eq!(world, before, "holds() must not touch the world");
}

/// T7. The store is bounded and eviction is a total order — coldest **now**,
/// then most hops, then highest key — so it is reproducible. One leg per term of
/// the order, plus the one that needs a clock: a row learned hot a game day ago
/// must rank below one learned lukewarm this minute, or a carrier who once
/// caught six hot stories could never take anything cooler than their
/// historical maximum, however dead those six are.
#[test]
fn the_store_is_bounded_and_eviction_is_deterministic() {
    /// Seven facts and seven tellings in order — `(hops, heat, learned_on)` —
    /// and the keys they were installed under, ascending.
    fn hoard(tellings: &[(u8, f32, f64)]) -> (World, ActorId, Vec<FactKey>) {
        let mut world = world_at_day(0);
        world.add_character(character("seed", "Seeded", None, &[]));
        world.add_character(character("hoarder", "Hoarder", None, &[]));
        let rows: String = (0..tellings.len())
            .map(|index| {
                format!(
                    r#"{{"id": "test.hoard.{index}", "topic": "law", "said": "row {index}",
                        "seeded": ["seed"]}}"#
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let catalog =
            FactCatalog::from_json(&format!("{{\"schema_version\": 1, \"facts\": [{rows}]}}"))
                .expect("the rows parse");
        assert!(catalog.seed(&mut world).is_empty());
        let keys: Vec<FactKey> = (0..tellings.len())
            .map(|index| {
                world
                    .knowledge
                    .key_of(&FactId::from_raw(format!("test.hoard.{index}")))
                    .expect("installed")
            })
            .collect();
        let hoarder = actor("hoarder");
        for (key, (hops, heat, learned_on)) in keys.iter().zip(tellings) {
            learn(
                &mut world,
                &hoarder,
                *key,
                telling(*hops, *heat, None),
                Some(*learned_on),
            );
        }
        (world, hoarder, keys)
    }

    /// The one key of the seven that `hoard` threw out.
    fn evicted(world: &World, hoarder: &ActorId, keys: &[FactKey]) -> FactKey {
        assert_eq!(world.knowledge.holdings_len(hoarder), HOLDINGS_MAX);
        let gone: Vec<FactKey> = keys
            .iter()
            .copied()
            .filter(|key| holds_key(world, hoarder, *key).is_none())
            .collect();
        assert_eq!(gone.len(), 1, "exactly one row goes: {gone:?}");
        gone[0]
    }

    // Coldest first: row 3 is the coldest by a whole percent, and the rest
    // descend by hop count so the tie-breaks are never reached for it.
    let coldest: Vec<(u8, f32, f64)> = (0..7)
        .map(|index| {
            let heat = if index == 3 {
                0.05
            } else {
                0.5 + index as f32 * 0.05
            };
            (index as u8 + 1, heat, 0.0)
        })
        .collect();
    let (world, hoarder, keys) = hoard(&coldest);
    assert_eq!(
        evicted(&world, &hoarder, &keys),
        keys[3],
        "the coldest row is the one that goes"
    );
    let (again, _, _) = hoard(&coldest);
    assert_eq!(
        holdings_of(&world, &hoarder),
        holdings_of(&again, &hoarder),
        "the same sequence must evict the same row"
    );

    // Equal heat: the row at the most removes goes.
    let farthest = [
        (1u8, 0.5f32, 0.0f64),
        (1, 0.5, 0.0),
        (5, 0.5, 0.0),
        (1, 0.5, 0.0),
        (1, 0.5, 0.0),
        (1, 0.5, 0.0),
        (1, 0.5, 0.0),
    ];
    let (world, hoarder, keys) = hoard(&farthest);
    assert_eq!(
        evicted(&world, &hoarder, &keys),
        keys[2],
        "at equal heat the most removes goes"
    );

    // Equal heat and equal hops: the highest key — the newest fact — goes, which
    // here is the arrival itself.
    let level = [(1u8, 0.5f32, 0.0f64); 7];
    let (world, hoarder, keys) = hoard(&level);
    assert_eq!(
        evicted(&world, &hoarder, &keys),
        keys[6],
        "at a dead tie the highest key goes"
    );

    // The clock: six rows learned hot on day 0 have cooled through six
    // half-lives by day 3 (1.0 × λ^72 ≈ 0.016), so the lukewarm arrival on day 3
    // is the warmest thing in the store and one of the six goes — by key, since
    // they tie on everything else. Ranking on `heat_at_learn` instead would
    // read the six at 100% and throw out the one warm row.
    let mut aged = vec![(1u8, 1.0f32, 0.0f64); 6];
    aged.push((1, 0.5, 3.0));
    let (world, hoarder, keys) = hoard(&aged);
    assert!(
        holds_key(&world, &hoarder, keys[6]).is_some(),
        "the warm arrival survives"
    );
    assert_eq!(
        evicted(&world, &hoarder, &keys),
        keys[5],
        "the six cold rows tie and the highest key among them goes"
    );
}

/// The same total order, from the other end: an arrival colder than everything
/// already held at a full store is its own eviction victim, so `Fresh` names the
/// merge's decision and `holds` is the only authority on the row's survival.
#[test]
fn a_cold_arrival_at_a_full_store_is_its_own_victim() {
    let mut world = world_at_day(0);
    world.add_character(character("seed", "Seeded", None, &[]));
    world.add_character(character("hoarder", "Hoarder", None, &[]));
    let rows: String = (0..HOLDINGS_MAX + 1)
        .map(|index| {
            format!(
                r#"{{"id": "test.full.{index}", "topic": "law", "said": "row {index}",
                    "seeded": ["seed"]}}"#
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let catalog =
        FactCatalog::from_json(&format!("{{\"schema_version\": 1, \"facts\": [{rows}]}}"))
            .expect("the rows parse");
    assert!(catalog.seed(&mut world).is_empty());
    let hoarder = actor("hoarder");
    let key_of = |index: usize| {
        world
            .knowledge
            .key_of(&FactId::from_raw(format!("test.full.{index}")))
            .expect("installed")
    };
    let keys: Vec<FactKey> = (0..HOLDINGS_MAX + 1).map(key_of).collect();

    for key in keys.iter().take(HOLDINGS_MAX) {
        assert_eq!(
            learn(&mut world, &hoarder, *key, telling(1, 0.9, None), Some(0.0)),
            Learned::Fresh
        );
    }
    assert_eq!(world.knowledge.holdings_len(&hoarder), HOLDINGS_MAX);

    let cold = keys[HOLDINGS_MAX];
    assert_eq!(
        learn(
            &mut world,
            &hoarder,
            cold,
            telling(1, 0.01, None),
            Some(0.0)
        ),
        Learned::Fresh,
        "the merge still calls it fresh"
    );
    assert_eq!(world.knowledge.holdings_len(&hoarder), HOLDINGS_MAX);
    assert!(
        holds_key(&world, &hoarder, cold).is_none(),
        "and it went straight back out again"
    );
}

// ---------------------------------------------------------------------------
// T8–T12 — the types
// ---------------------------------------------------------------------------

/// T8. `FactView` is deltas, never text — asserted structurally, so a `String`
/// field cannot be added without this failing.
///
/// The structural rule, stated because a size check cannot say it: **no `String`
/// field, no constructor taking one, no method returning one, and no `topic`
/// field** — which is what makes "topic is invariant under garbling"
/// unrepresentable rather than merely asserted.
#[test]
fn fact_view_stores_no_sentence() {
    let declared = std::mem::size_of::<Option<ActorId>>()
        + std::mem::size_of::<Option<AreaKey>>()
        + std::mem::size_of::<i8>();
    assert!(
        std::mem::size_of::<FactView>() <= declared.next_multiple_of(8),
        "FactView is {} bytes; its three declared fields are {declared}",
        std::mem::size_of::<FactView>()
    );
    let view = FactView::default();
    assert!(view.is_pristine());
}

/// The stored row carries no sentence: 4 + 1 + 4 + 16 + 24 + 32 = 81, rounded to
/// 88 by the 8-aligned `learned_on`/`from`/`view`. Six of them are ~530 B rather
/// than the ~1.6 KB a rendered `String` per holding would cost, and at 20,000
/// people that difference is the whole footprint argument.
#[test]
fn a_holding_stores_no_sentence() {
    assert!(
        std::mem::size_of::<Holding>() <= 88,
        "Holding is {} bytes",
        std::mem::size_of::<Holding>()
    );
}

/// T9. The source is sealed in `Debug`, which closes the
/// `Diagnostic(format!("{fact:?}"))` route a projection-walking test cannot see.
#[test]
fn the_source_is_sealed_in_debug() {
    let source = FactSource::claimed(actor("fg2sh"));
    assert_eq!(format!("{source:?}"), "FactSource(<sealed>)");
    assert!(source.is_claimed());
    assert_eq!(source.claimant(), Some(&actor("fg2sh")));

    let mut world = world_at_day(0);
    world.add_character(character("fg2sh", "Grigor Ashe", None, &[]));
    let key = seed_one(
        &mut world,
        r#"{"id": "test.sealed", "topic": "law", "said": "a thing happened",
            "seeded": ["fg2sh"], "source": {"custody": "fg2sh"}}"#,
    );
    let fact = world.knowledge.fact(key).expect("the fact is live");
    let debug = format!("{fact:?}");
    assert!(debug.contains("FactSource(<sealed>)"), "{debug}");
    assert!(
        !debug.contains("Custody"),
        "the payload must not reach a Debug string: {debug}"
    );
}

/// T10. `install` refuses a duplicate id and a full store, so a runaway mint is
/// bounded rather than unbounded.
#[test]
fn install_refuses_a_duplicate_id_and_a_full_store() {
    let mut world = world_at_day(0);
    world.add_character(character("w1", "Witness", None, &[]));

    let make = |world: &mut World, id: &str| -> Fact {
        let (key, sequence) = world.knowledge.next_handles();
        Fact {
            id: FactId::from_raw(id),
            key,
            sequence,
            subject: Vec::new(),
            place: None,
            day: None,
            said: "a thing".into(),
            own: BTreeMap::new(),
            seeded: BTreeSet::from([actor("w1")]),
            garble: GarbleMask::NONE,
            decays: true,
            topic: Topic::Law,
            minted_game_days: None,
            source: FactSource::authored(),
        }
    };

    let first = make(&mut world, "test.dup");
    assert!(world.knowledge.install(first).is_some());
    let second = make(&mut world, "test.dup");
    assert!(world.knowledge.install(second).is_none());
    assert_eq!(world.knowledge.len(), 1);

    // A handle already live is refused too, whatever the id: a fact built
    // without `next_handles` must not overwrite the one under that key while
    // `by_id` keeps pointing the old id at it.
    let stale = Fact {
        key: FactKey(0),
        ..make(&mut world, "test.stale.key")
    };
    assert!(world.knowledge.install(stale).is_none());
    assert_eq!(world.knowledge.len(), 1);
    assert_eq!(
        world
            .knowledge
            .fact(FactKey(0))
            .map(|fact| fact.id.as_str()),
        Some("test.dup")
    );
    assert_eq!(
        world.knowledge.key_of(&FactId::from_raw("test.dup")),
        Some(FactKey(0))
    );

    for index in 1..FACTS_MAX_LIVE {
        let fact = make(&mut world, &format!("test.full.{index}"));
        assert!(
            world.knowledge.install(fact).is_some(),
            "row {index} must fit"
        );
    }
    assert_eq!(world.knowledge.len(), FACTS_MAX_LIVE);
    let overflow = make(&mut world, "test.overflow");
    assert!(world.knowledge.install(overflow).is_none());
    assert_eq!(world.knowledge.len(), FACTS_MAX_LIVE);
}

/// T11. Every topic round-trips through its snake_case name, and an unrecognised
/// tag lands on `Talk` — the dullest band, never a high one.
#[test]
fn topic_round_trips_and_an_unknown_tag_lands_on_talk() {
    for topic in Topic::ALL {
        let json = serde_json::to_string(&topic).expect("a topic serialises");
        assert_eq!(json, format!("\"{}\"", topic.as_str()));
        let back: Topic = serde_json::from_str(&json).expect("a topic parses");
        assert_eq!(back, topic);
        assert_eq!(Topic::parse(topic.as_str()), Some(topic));
    }
    // The spec's two `"word"` occurrences are typos: `word` is the city's prose
    // register and must not double as a machine tag.
    assert_eq!(Topic::parse("word"), None);
    assert_eq!(Topic::parse_or_talk("word"), Topic::Talk);
    assert_eq!(Topic::parse_or_talk(""), Topic::Talk);
    assert_eq!(Topic::ALL.len(), 9);
}

/// T12. The comma form round-trips byte-stably, and an unknown token is refused.
#[test]
fn the_garble_mask_round_trips_its_comma_form() {
    assert_eq!(GarbleMask::parse("none").unwrap(), GarbleMask::NONE);
    assert!(GarbleMask::parse("none").unwrap().is_none());
    let mask = GarbleMask::parse("place,day").unwrap();
    assert!(!mask.subject && mask.place && mask.day);
    assert_eq!(mask.as_authored(), "place,day");
    // Any order in, one order out.
    assert_eq!(
        GarbleMask::parse("day,place,subject")
            .unwrap()
            .as_authored(),
        "subject,place,day"
    );
    assert_eq!(GarbleMask::NONE.as_authored(), "none");
    assert!(GarbleMask::ALL.any());
    for topic in Topic::ALL {
        assert_eq!(GarbleMask::default_for(topic), GarbleMask::ALL);
    }
    let error = GarbleMask::parse("colour").expect_err("an unknown token is refused");
    assert!(error.message.contains("unknown garble field 'colour'"));
    // `none` is the whole mask or nothing: an empty string and `none` beside a
    // field both read as one thing and mean another.
    assert_eq!(GarbleMask::parse(" none ").unwrap(), GarbleMask::NONE);
    let error = GarbleMask::parse("").expect_err("an empty mask is refused");
    assert!(
        error.message.contains("unknown garble field ''"),
        "{}",
        error.message
    );
    let error = GarbleMask::parse("none,subject").expect_err("none beside a field is refused");
    assert!(
        error.message.contains("mixes none with a field"),
        "{}",
        error.message
    );
}

// ---------------------------------------------------------------------------
// `render_line` — the one place a fact becomes words
// ---------------------------------------------------------------------------

/// The shipped `vell.stall.pitch`, in a hand-built world with its three real
/// holders: the subject (an `own` line), somebody who knows her, and somebody
/// who does not.
fn wick_world() -> World {
    let mut world = world_at_day(0);
    world.area_map = crate::areas::AreaMap::from_json_str(
        r#"{"schema_version": 1,
            "coordinate_system": {"units": "meters", "north": "+x", "east": "-z", "up": "+y"},
            "areas": [{"id": "wickmarket", "label": "The Wickmarket",
                       "boxes": [{"min_m": {"x": 0.0, "y": 0.0, "z": 0.0},
                                  "max_m": {"x": 10.0, "y": 10.0, "z": 10.0}}]}]}"#,
    )
    .expect("the test area map loads");
    world.add_character(character(
        "dv8ll",
        "Osanne Vell",
        Some(profile(
            Some("chandler"),
            Some("Chandler"),
            PlanningWard::Wick,
        )),
        &[],
    ));
    world.add_character(character(
        "dclsk",
        "Clemence Skep",
        Some(profile(
            Some("market_seller"),
            Some("Market seller"),
            PlanningWard::Wick,
        )),
        &["dv8ll"],
    ));
    world.add_character(character(
        "p000x",
        "Petronel Clove",
        Some(profile(
            Some("market_seller"),
            Some("Market seller"),
            PlanningWard::Wick,
        )),
        &[],
    ));
    world
}

fn shipped_pitch(world: &mut World) -> FactKey {
    let catalog = FactCatalog::default();
    let diagnostics = catalog.seed(world);
    // `ashe.salt.short`'s cast is absent from this world, so it is skipped.
    assert!(
        diagnostics
            .iter()
            .all(|line| line.contains("ashe.salt.short")),
        "{diagnostics:?}"
    );
    world
        .knowledge
        .key_of(&FactId::from_raw("vell.stall.pitch"))
        .expect("the wick row installed")
}

/// Three holders of one fact say three different things — `own` against `said`,
/// and the unknown-people rule in the same assertion. The whole of why `own` and
/// `said` are separate, and the shipped row's own reason for existing.
#[test]
fn three_holders_of_one_fact_say_three_different_things() {
    let mut world = wick_world();
    let key = shipped_pitch(&mut world);
    let strings = strings();
    let now = Some(0.0);

    let line = |world: &World, reader: &str| {
        let reader = actor(reader);
        let held = holds_key(world, &reader, key).expect("a seeded holder");
        render_line(world, &reader, key, &held, &strings, now).expect("a rendered line")
    };

    let subject = line(&world, "dv8ll");
    let knowing = line(&world, "dclsk");
    let stranger = line(&world, "p000x");

    assert!(
        subject.starts_with("First hand, in your own words:"),
        "{subject}"
    );
    assert!(
        subject.contains("The corner has been mine since the Great Rains"),
        "{subject}"
    );

    // `dclsk` has her own line too — she was there — so hers is first-person as
    // well, and different from the subject's.
    assert_ne!(subject, knowing);
    assert_ne!(knowing, stranger);
    assert_ne!(subject, stranger);

    // The one holder with no `own` line renders the subject as a role, never a
    // name, because her authored `knows` does not carry `dv8ll`.
    assert!(
        stranger.contains("a chandler of the Wick Ward (you don't know their name)"),
        "{stranger}"
    );
    assert!(!stranger.contains("Osanne Vell"), "{stranger}");
    assert!(stranger.contains("The Wickmarket"), "{stranger}");
    assert!(stranger.starts_with("You saw this yourself:"), "{stranger}");
}

/// The reader who *has* been told the name gets the name — the other half of the
/// unknown-people ladder.
#[test]
fn a_reader_who_knows_the_subject_reads_the_name() {
    let mut world = wick_world();
    let key = shipped_pitch(&mut world);
    let strings = strings();
    // A fourth body: not seeded, so she carries it, and she knows Osanne.
    world.add_character(character(
        "dtbvl",
        "Tibb Vell",
        Some(profile(
            Some("market_seller"),
            Some("Market seller"),
            PlanningWard::Wick,
        )),
        &["dv8ll"],
    ));
    let reader = actor("dtbvl");
    learn(
        &mut world,
        &reader,
        key,
        Telling {
            hops: 1,
            from: Some(actor("dclsk")),
            heat: 1.0,
            view: FactView::default(),
        },
        Some(0.0),
    );
    let held = holds_key(&world, &reader, key).expect("a carried row");
    let line = render_line(&world, &reader, key, &held, &strings, Some(0.0)).expect("a line");
    assert!(line.contains("Osanne Vell"), "{line}");
    assert!(!line.contains("you don't know their name"), "{line}");
}

/// A subject with no `own` line is never told about themselves in the third
/// person: `holds` still answers, and `render_line` does not.
#[test]
fn a_fact_about_you_is_never_rendered_to_you_as_news() {
    let mut world = world_at_day(0);
    world.add_character(character("subj", "The Subject", None, &[]));
    world.add_character(character("wit", "The Witness", None, &[]));
    let key = seed_one(
        &mut world,
        r#"{"id": "test.about.you", "topic": "law", "said": "{subject} was taken up",
            "subject": ["subj"], "seeded": ["subj", "wit"]}"#,
    );
    let strings = strings();
    let subject = actor("subj");
    let held = holds_key(&world, &subject, key).expect("the subject holds it");
    assert!(render_line(&world, &subject, key, &held, &strings, Some(0.0)).is_none());

    // The witness, who is not the subject, does render it.
    let witness = actor("wit");
    let held = holds_key(&world, &witness, key).expect("the witness holds it");
    assert!(render_line(&world, &witness, key, &held, &strings, Some(0.0)).is_some());
}

/// A version whose swapped subject is the reader is not told back to them
/// either: the self test reads the *effective* subject and not only the
/// canonical one, so however M3's `view_for` picks, a reader's own name cannot
/// stand in front of them in the third person.
#[test]
fn a_view_that_lands_on_the_reader_is_not_told_back_to_them() {
    let (mut world, carrier, key) = merge_world();
    let strings = strings();
    assert_eq!(
        learn(
            &mut world,
            &carrier,
            key,
            telling(2, 1.0, Some("c2")),
            Some(0.0)
        ),
        Learned::Fresh
    );
    let held = holds_key(&world, &carrier, key).expect("a carried row");
    assert_eq!(held.view.subject, Some(carrier.clone()));
    assert!(render_line(&world, &carrier, key, &held, &strings, Some(0.0)).is_none());

    // A closer telling moves the view off them, and the row renders as usual.
    assert_eq!(
        learn(
            &mut world,
            &carrier,
            key,
            telling(1, 1.0, Some("c1")),
            Some(0.0)
        ),
        Learned::Corrected
    );
    let held = holds_key(&world, &carrier, key).expect("a carried row");
    assert!(render_line(&world, &carrier, key, &held, &strings, Some(0.0)).is_some());
}

/// An absurd authored day cannot overflow the day phrase: `day` is data a
/// `--facts` pack can carry, and a sheet render must render a phrase on it,
/// never panic.
#[test]
fn an_absurd_authored_day_renders_a_phrase_and_never_panics() {
    let strings = strings();
    for day in [i64::MIN, i64::MAX] {
        let mut world = world_at_day(0);
        world.add_character(character("wit", "The Witness", None, &[]));
        world.add_character(character("rdr", "The Reader", None, &[]));
        let key = seed_one(
            &mut world,
            &format!(
                r#"{{"id": "test.absurd.day", "topic": "law", "said": "it happened {{day}}",
                    "seeded": ["wit"], "day": {day}}}"#
            ),
        );
        let reader = actor("rdr");
        learn(
            &mut world,
            &reader,
            key,
            Telling {
                hops: 1,
                from: None,
                heat: 1.0,
                view: FactView {
                    subject: None,
                    place: None,
                    day_offset: DAY_OFFSET_MAX,
                },
            },
            Some(0.0),
        );
        let held = holds_key(&world, &reader, key).expect("a carried row");
        let line = render_line(&world, &reader, key, &held, &strings, Some(0.0)).expect("a line");
        assert!(
            !line.chars().any(|character| character.is_ascii_digit()),
            "{line}"
        );
        // A day past the end of time reads as today (the future clamps); one
        // before the beginning reads as long ago.
        let phrase = if day == i64::MAX {
            &strings.day_today
        } else {
            &strings.day_long_ago
        };
        assert!(line.contains(phrase.as_str()), "day {day}: {line}");
    }
}

/// The subject **with** an `own` line reads their own words, whatever the heat —
/// the one departure from the measured rig, which checked cold first.
#[test]
fn a_subject_with_an_own_line_reads_their_own_words() {
    let mut world = world_at_day(0);
    world.add_character(character("subj", "The Subject", None, &[]));
    let key = seed_one(
        &mut world,
        r#"{"id": "test.own.line", "topic": "law", "said": "{subject} was taken up",
            "own": {"subj": "I was taken up at the gate and I said nothing"},
            "subject": ["subj"], "seeded": ["subj"]}"#,
    );
    let strings = strings();
    let subject = actor("subj");
    let held = holds_key(&world, &subject, key).expect("the subject holds it");
    let line = render_line(&world, &subject, key, &held, &strings, Some(0.0)).expect("a line");
    assert_eq!(
        line,
        "First hand, in your own words: I was taken up at the gate and I said nothing"
    );
}

/// Every one of the twenty-one frozen cells, asserted **by key name**, so no
/// band-shift can creep back in: `top`/hops 2 is `know_hedge_top_hops2` and never
/// `default`/hops 1.
///
/// The render half of the hedge-ladder contract; the sheet half is the block's.
#[test]
fn render_line_renders_all_twenty_one_hedge_cells() {
    let strings = strings();

    // Four of the twenty-four frozen values, pinned by their own words. Asserting
    // the rest by field name would be a tautology if the ladder ever went blank,
    // and `top`/hops 2 spelled out is the no-band-shift rule stated once: it is
    // "They say:", never `default`/hops 1's "and the one who told you was there".
    assert_eq!(strings.know_hedge_top_hops2, "They say: %s");
    assert_eq!(
        strings.know_hedge_default_hops0,
        "You saw this yourself: %s"
    );
    assert_eq!(
        strings.know_hedge_low_cold,
        "Somebody mentioned it once, long since, and you would not swear to a word of it: %s"
    );
    assert_eq!(
        strings.unknown_person_role,
        "a %s of %s (you don't know their name)"
    );

    let cells: [(&str, HedgeBand, [&String; 7]); 3] = [
        (
            "law",
            HedgeBand::Default,
            [
                &strings.know_hedge_default_hops0_own,
                &strings.know_hedge_default_hops0,
                &strings.know_hedge_default_hops1,
                &strings.know_hedge_default_hops2,
                &strings.know_hedge_default_hops3,
                &strings.know_hedge_default_hops4,
                &strings.know_hedge_default_cold,
            ],
        ),
        (
            "bed",
            HedgeBand::Top,
            [
                &strings.know_hedge_top_hops0_own,
                &strings.know_hedge_top_hops0,
                &strings.know_hedge_top_hops1,
                &strings.know_hedge_top_hops2,
                &strings.know_hedge_top_hops3,
                &strings.know_hedge_top_hops4,
                &strings.know_hedge_top_cold,
            ],
        ),
        (
            "craft",
            HedgeBand::Low,
            [
                &strings.know_hedge_low_hops0_own,
                &strings.know_hedge_low_hops0,
                &strings.know_hedge_low_hops1,
                &strings.know_hedge_low_hops2,
                &strings.know_hedge_low_hops3,
                &strings.know_hedge_low_hops4,
                &strings.know_hedge_low_cold,
            ],
        ),
    ];

    // Every one of the twenty-one is distinct within its band and carries exactly
    // one `%s`, so no two rungs can silently collapse onto one wording.
    let mut every: Vec<&str> = Vec::new();
    for (_, _, ladder) in &cells {
        for value in ladder {
            assert_eq!(value.matches("%s").count(), 1, "{value}");
            every.push(value.as_str());
        }
    }
    assert_eq!(every.len(), 21);

    for (topic, band, ladder) in cells {
        let mut world = world_at_day(0);
        world.add_character(character("wit", "The Witness", None, &[]));
        world.add_character(character("rdr", "The Reader", None, &[]));
        assert_eq!(
            world.salience.hedge_band(Topic::parse(topic).unwrap()),
            band
        );
        let key = seed_one(
            &mut world,
            &format!(
                r#"{{"id": "test.band.{topic}", "topic": "{topic}",
                     "said": "a thing happened",
                     "own": {{"wit": "I was there for it"}},
                     "seeded": ["wit"]}}"#
            ),
        );
        let reader = actor("rdr");

        // The own rung: the witness's first-person line, alone inside the wrapper.
        let witness = actor("wit");
        let held = holds_key(&world, &witness, key).expect("the witness holds it");
        let line = render_line(&world, &witness, key, &held, &strings, Some(0.0)).expect("a line");
        assert_eq!(
            line,
            ladder[0].replacen("%s", "I was there for it", 1),
            "{topic} own rung"
        );

        // hops 0..=4, all warm.
        for hops in 0u8..=4 {
            let mut world = world.clone();
            learn(
                &mut world,
                &reader,
                key,
                Telling {
                    hops,
                    from: None,
                    heat: 1.0,
                    view: FactView::default(),
                },
                Some(0.0),
            );
            let held = holds_key(&world, &reader, key).expect("a carried row");
            let line =
                render_line(&world, &reader, key, &held, &strings, Some(0.0)).expect("a line");
            assert_eq!(
                line,
                ladder[usize::from(hops) + 1].replacen("%s", "a thing happened", 1),
                "{topic} hops {hops}"
            );
        }

        // Cold beats every hop count including 0.
        for hops in [0u8, 2, 4] {
            let mut world = world.clone();
            learn(
                &mut world,
                &reader,
                key,
                Telling {
                    hops,
                    from: None,
                    heat: 0.001,
                    view: FactView::default(),
                },
                Some(0.0),
            );
            let held = holds_key(&world, &reader, key).expect("a carried row");
            let line =
                render_line(&world, &reader, key, &held, &strings, Some(0.0)).expect("a line");
            assert_eq!(
                line,
                ladder[6].replacen("%s", "a thing happened", 1),
                "{topic} cold at hops {hops}"
            );
        }
    }
}

/// The precedence rule itself, without a world: the own line wins whatever the
/// heat, then cold beats every hop count.
#[test]
fn the_rung_precedence_is_one_rule() {
    assert_eq!(rung_for(0, true, true), Rung::Own);
    assert_eq!(rung_for(4, false, true), Rung::Own);
    assert_eq!(rung_for(0, true, false), Rung::Cold);
    assert_eq!(rung_for(9, true, false), Rung::Cold);
    assert_eq!(rung_for(0, false, false), Rung::Hops0);
    assert_eq!(rung_for(3, false, false), Rung::Hops3);
    assert_eq!(rung_for(4, false, false), Rung::Hops4);
    assert_eq!(rung_for(200, false, false), Rung::Hops4);
}

/// An occupation display that begins with a vowel must not render "a anchoress".
#[test]
fn a_vowel_trade_takes_an_article_that_reads() {
    let mut world = world_at_day(0);
    world.add_character(character(
        "anch",
        "The Anchoress",
        Some(profile(
            Some("anchoress"),
            Some("Anchoress"),
            PlanningWard::Reed,
        )),
        &[],
    ));
    world.add_character(character("rdr", "The Reader", None, &[]));
    let word = person_word(&world, &actor("rdr"), &actor("anch"), &strings());
    assert_eq!(
        word,
        "an anchoress of the Reed Ward (you don't know their name)"
    );
}

/// The no-trade quarter has no `occupation_display`, so the subject falls back to
/// the nameless form the sheet already uses for a stranger.
#[test]
fn a_subject_with_no_trade_falls_back_to_the_nameless_form() {
    let mut world = world_at_day(0);
    world.add_character(character(
        "pauper",
        "A Pauper",
        Some(profile(None, None, PlanningWard::Cinder)),
        &[],
    ));
    world.add_character(character("rdr", "The Reader", None, &[]));
    let strings = strings();
    let word = person_word(&world, &actor("rdr"), &actor("pauper"), &strings);
    assert_eq!(word, strings.unknown_person_name);
    // And an actor this world does not have at all.
    let word = person_word(&world, &actor("rdr"), &actor("nobody"), &strings);
    assert_eq!(word, strings.unknown_person_name);
}

/// `{place}` renders the area's own label, and `place_unknown` when the handle
/// does not resolve — a hermetic world, or a handle from another map.
#[test]
fn an_unresolvable_place_renders_the_unknown_form() {
    let mut world = world_at_day(0);
    world.add_character(character("wit", "The Witness", None, &[]));
    world.add_character(character("rdr", "The Reader", None, &[]));
    // No area map at all, so `wickmarket` cannot resolve — a diagnostic, not an
    // error, and the place is simply left unset.
    let json = r#"{"schema_version": 1, "facts": [
        {"id": "test.noplace", "topic": "law", "said": "it happened at {place}",
         "seeded": ["wit"], "place": "wickmarket"}]}"#;
    let catalog = FactCatalog::from_json(json).expect("the row parses");
    let diagnostics = catalog.seed(&mut world);
    assert!(
        diagnostics
            .iter()
            .any(|line| line.contains("unknown area 'wickmarket'; place left unset")),
        "{diagnostics:?}"
    );
    let key = world
        .knowledge
        .key_of(&FactId::from_raw("test.noplace"))
        .expect("installed");
    let strings = strings();
    let reader = actor("rdr");
    learn(
        &mut world,
        &reader,
        key,
        Telling {
            hops: 1,
            from: None,
            heat: 1.0,
            view: FactView::default(),
        },
        Some(0.0),
    );
    let held = holds_key(&world, &reader, key).expect("a carried row");
    let line = render_line(&world, &reader, key, &held, &strings, Some(0.0)).expect("a line");
    assert!(line.contains(&strings.place_unknown), "{line}");
}

/// The subject cannot carry it, and a standing fact is never volunteered.
#[test]
fn the_subject_cannot_carry_and_a_standing_fact_is_never_volunteered() {
    let mut world = world_at_day(0);
    world.add_character(character("subj", "The Subject", None, &[]));
    world.add_character(character("wit", "The Witness", None, &[]));
    let decaying = seed_one(
        &mut world,
        r#"{"id": "test.decays", "topic": "bed", "said": "{subject} did not sleep at home",
            "subject": ["subj"], "seeded": ["subj", "wit"]}"#,
    );
    let standing = seed_one(
        &mut world,
        r#"{"id": "test.standing", "topic": "bed", "said": "{subject} keeps the ford",
            "subject": ["subj"], "seeded": ["subj", "wit"], "decays": false}"#,
    );

    let subject = actor("subj");
    let witness = actor("wit");
    let fact = world.knowledge.fact(decaying).expect("live").clone();
    assert!(!may_carry(&fact, &subject));
    assert!(may_carry(&fact, &witness));

    let held = holds_key(&world, &subject, decaying).expect("the subject holds it");
    assert_eq!(held.heat(Some(0.0)), 1.0);
    assert!(
        !volunteers(&world, &fact, &subject, &held, Some(0.0)),
        "the subject never volunteers it, however hot"
    );
    let held = holds_key(&world, &witness, decaying).expect("the witness holds it");
    assert!(volunteers(&world, &fact, &witness, &held, Some(0.0)));

    let standing_fact = world.knowledge.fact(standing).expect("live").clone();
    let held = holds_key(&world, &witness, standing).expect("the witness holds it");
    assert_eq!(held.heat(Some(0.0)), 1.0);
    assert!(
        !volunteers(&world, &standing_fact, &witness, &held, Some(0.0)),
        "a standing fact has no warm life: it is answerable, never loud"
    );
}

/// The day phrase is words, never a digit, and every clock-less or undated case
/// lands on `day_long_ago`.
#[test]
fn the_day_phrase_never_shows_a_digit() {
    let strings = strings();
    let expected = [
        "today",
        "yesterday",
        "two days past",
        "three days past",
        "four days past",
        "five days past",
        "six days past",
        "seven days past",
    ];
    for days in 0..=12i64 {
        let mut world = world_at_day(days);
        world.add_character(character("wit", "The Witness", None, &[]));
        world.add_character(character("rdr", "The Reader", None, &[]));
        let key = seed_one(
            &mut world,
            r#"{"id": "test.day", "topic": "law", "said": "it happened {day}",
                "seeded": ["wit"], "day": 0}"#,
        );
        let reader = actor("rdr");
        learn(
            &mut world,
            &reader,
            key,
            Telling {
                hops: 1,
                from: None,
                heat: 1.0,
                view: FactView::default(),
            },
            Some(0.0),
        );
        let held = holds_key(&world, &reader, key).expect("a carried row");
        let line = render_line(&world, &reader, key, &held, &strings, Some(0.0)).expect("a line");
        let want = expected
            .get(usize::try_from(days).expect("a small day"))
            .copied()
            .unwrap_or("a long while back");
        assert!(line.contains(want), "day {days}: {line}");
        assert!(
            !line.chars().any(|character| character.is_ascii_digit()),
            "a digit reached the sheet: {line}"
        );

        // The same fact in a clock-less world renders the long-ago form.
        world.current_time = None;
        let line = render_line(&world, &reader, key, &held, &strings, None).expect("a line");
        assert!(line.contains("a long while back"), "{line}");
    }
}

/// Relevance seats a fact whose id segments, subject name or place label appear in
/// what the actor has just been told — including the adjacent ask, which is the
/// dead end M0 measured moved from prose into selection.
#[test]
fn relevance_seats_the_fact_the_asker_meant() {
    let mut world = wick_world();
    let key = shipped_pitch(&mut world);
    let reader = world
        .characters
        .get(&actor("p000x"))
        .expect("a holder")
        .clone();

    // The id's own segments: "pitch" is what an asker says.
    let seated = relevance_seated(
        &world,
        &reader,
        &["A stranger said: \"What of the pitch at the market?\""],
        &["nothing yet"],
    );
    assert_eq!(seated, vec![key]);

    // The place's label works too, minus the article the length rule drops.
    let seated = relevance_seated(&world, &reader, &["they were at the Wickmarket"], &[]);
    assert_eq!(seated, vec![key]);

    // And the subject's name, for a reader who has been told it.
    let knower = world
        .characters
        .get(&actor("dclsk"))
        .expect("a holder")
        .clone();
    let seated = relevance_seated(&world, &knower, &["Osanne was asking after you"], &[]);
    assert_eq!(seated, vec![key]);

    // Nothing relevant seats nothing.
    let seated = relevance_seated(&world, &reader, &["the fish were good"], &["nothing yet"]);
    assert!(seated.is_empty());

    // The layer's own switch gates the reader, not the store.
    world.knowledge_enabled = false;
    assert!(relevance_seated(&world, &reader, &["the pitch"], &[]).is_empty());
    assert!(holds_key(&world, &actor("p000x"), key).is_some());
}

/// A subject with no `own` line is filtered out of relevance before the haystack
/// is consulted at all, so no adjacent ask can seat a fact about the reader.
#[test]
fn relevance_never_seats_a_fact_about_the_reader() {
    let mut world = world_at_day(0);
    world.add_character(character("subj", "The Subject", None, &[]));
    world.add_character(character("wit", "The Witness", None, &[]));
    seed_one(
        &mut world,
        r#"{"id": "test.about.subj", "topic": "law", "said": "{subject} was taken up",
            "subject": ["subj"], "seeded": ["subj", "wit"]}"#,
    );
    let subject = world
        .characters
        .get(&actor("subj"))
        .expect("the subject")
        .clone();
    assert!(relevance_seated(&world, &subject, &["what about subj"], &[]).is_empty());

    let witness = world.characters.get(&actor("wit")).expect("a body").clone();
    assert_eq!(
        relevance_seated(&world, &witness, &["what about subj"], &[]).len(),
        1
    );
}

/// Invalidation is a sweep: a fact the world no longer bears out leaves the store
/// and every holding with no actor cooperation at all.
#[test]
fn invalidate_stale_drops_a_fact_the_world_stopped_bearing_out() {
    let mut world = world_at_day(0);
    world.add_character(character("prisoner", "The Prisoner", None, &[]));
    world.add_character(character("wit", "The Witness", None, &[]));
    world.add_character(character("rdr", "The Reader", None, &[]));
    let key = seed_one(
        &mut world,
        r#"{"id": "test.custody", "topic": "law", "said": "they are held",
            "seeded": ["wit"], "source": {"custody": "prisoner"}}"#,
    );
    let reader = actor("rdr");
    learn(
        &mut world,
        &reader,
        key,
        Telling {
            hops: 1,
            from: None,
            heat: 1.0,
            view: FactView::default(),
        },
        Some(0.0),
    );
    assert!(holds_key(&world, &reader, key).is_some());

    // Nobody is in custody in this world, so the source never bore it out.
    let dead = invalidate_stale(&mut world);
    assert_eq!(dead, vec![FactId::from_raw("test.custody")]);
    assert!(holds_key(&world, &reader, key).is_none());
    assert!(holds_key(&world, &actor("wit"), key).is_none());
    assert!(world.knowledge.is_empty());
    assert_eq!(world.knowledge.holdings_len(&reader), 0);

    // An authored fact is simply so, and survives every sweep.
    let key = seed_one(
        &mut world,
        r#"{"id": "test.authored", "topic": "law", "said": "it is so", "seeded": ["wit"]}"#,
    );
    assert!(invalidate_stale(&mut world).is_empty());
    assert!(holds_key(&world, &actor("wit"), key).is_some());
}

/// A salience table whose `coin` ear names a trade nobody holds, otherwise the
/// shipped shape.
const PIEMAN_TABLE: &str = r#"{
  "schema_version": 1,
  "topics": {
    "bed": {"base": 1.0, "hedge_band": "top"},
    "blood": {"base": 1.0, "hedge_band": "top"},
    "law": {"base": 0.8, "hedge_band": "default"},
    "omen": {"base": 0.8, "hedge_band": "default"},
    "stranger": {"base": 0.8, "hedge_band": "default"},
    "coin": {"base": 0.45, "hedge_band": "default"},
    "bread": {"base": 0.35, "hedge_band": "default"},
    "craft": {"base": 0.2, "hedge_band": "low"},
    "talk": {"base": 0.15, "hedge_band": "low"}
  },
  "ears": {"coin": {"occupations": ["baker", "pieman"], "multiplier": 1.5}},
  "craft": {"own_trade": 2.0, "other_trade": 0.6},
  "no_trade": 1.4,
  "household": 0.15
}"#;

/// The salience ears are checked where the lore is in the room: an ear naming an
/// occupation nobody in the world holds is one startup diagnostic and never an
/// error, and a world with no occupations at all says nothing — its whole ear
/// list would otherwise be "unknown" in every hermetic test.
#[test]
fn an_ear_naming_an_unknown_occupation_is_one_startup_diagnostic() {
    let mut world = world_at_day(0);
    world.salience = Arc::new(SalienceTable::from_json(PIEMAN_TABLE).expect("the table loads"));
    world.add_character(character("plain", "No Lore", None, &[]));
    assert!(crate::engine::unknown_salience_occupations(&world).is_empty());

    world.add_character(character(
        "bakr",
        "A Baker",
        Some(profile(Some("baker"), Some("Baker"), PlanningWard::Wick)),
        &[],
    ));
    assert_eq!(
        crate::engine::unknown_salience_occupations(&world),
        vec![
            "salience.json: the coin ear names unknown occupation 'pieman'; the ear will match \
             nobody"
                .to_string()
        ]
    );
    // And the shipped table names nothing the shipped cast lacks — asserted
    // against the real roster by the backends canary; here, that a table whose
    // every ear is held is silent.
    world.salience = Arc::new(SalienceTable::default());
    world.add_character(character(
        "lndr",
        "A Laundress",
        Some(profile(
            Some("laundress"),
            Some("Laundress"),
            PlanningWard::Weigh,
        )),
        &[],
    ));
    let lines = crate::engine::unknown_salience_occupations(&world);
    assert!(
        lines
            .iter()
            .all(|line| !line.contains("'laundress'") && !line.contains("'baker'")),
        "{lines:?}"
    );
}

/// The store is behind an `Arc`, so cloning a world is a refcount bump and the
/// deep copy happens on the first knowledge write after the clone — which on the
/// `World::market_sale` path is never.
#[test]
fn the_holdings_map_is_shared_until_it_is_written() {
    let (mut world, carrier, key) = merge_world();
    learn(&mut world, &carrier, key, telling(2, 0.5, None), Some(0.0));
    let staged = world.clone();
    assert!(Arc::ptr_eq(
        &world.knowledge.holdings,
        &staged.knowledge.holdings
    ));
    let mut written = staged.clone();
    learn(
        &mut written,
        &carrier,
        key,
        telling(1, 0.9, None),
        Some(0.0),
    );
    assert!(!Arc::ptr_eq(
        &world.knowledge.holdings,
        &written.knowledge.holdings
    ));
    // The world the write was made from is untouched.
    assert_eq!(
        holds_key(&world, &carrier, key).map(|held| held.hops),
        Some(2)
    );
}
