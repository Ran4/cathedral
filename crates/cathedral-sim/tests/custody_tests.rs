//! Custody (`features/law_and_order.md` M4) — the sim-side half of the
//! milestone's test list, headless and with no host anywhere in it.
//!
//! The through-line of every test here is M4's own claim: **custody is a state
//! and the grab is only its enforcement**, so the interesting assertions are
//! about what a person in the law's hands may still *do* (speak, hand things
//! over, remember, be talked round) rather than about a cell. The player is
//! absent from these worlds entirely, which is M4b′'s point — the cast arrests
//! each other with nothing host-side in the loop.

mod prompt_support;

use std::{collections::BTreeSet, sync::Arc};

use cathedral_sim::{
    ActionError, ActionErrorCode, ActorId, Character, CharacterSheet, Control, EconomicClass,
    IntentTarget, Item, ItemId, LoreProfile, NavData, Office, PlaceId, PlaceRegistry, PlanningWard,
    Presence, Significance, Vec3, World, WorldClock, WorldTime, apply_action,
    custody::{
        self, CUSTODY_ESCORT_CONTACT_M, CUSTODY_MAX_ARRESTS, Confinement, STATION_ARRIVE_RADIUS_M,
        Station,
    },
    notices::{self, Rung},
    prompt::render_prompt,
};
use prompt_support::{compact, prompt_env, seed_world};
use serde_json::json;

// ------------------------------------------------------------------- fixtures

fn actor(id: &str) -> ActorId {
    ActorId::from_raw(id)
}

fn item(id: &str) -> ItemId {
    ItemId::from_raw(id)
}

/// An LLM character on the X axis, with a lore occupation when one is given —
/// which is the only thing that makes anybody law (`notices::is_law`).
fn person(id: &str, name: &str, x: f64, occupation: Option<&str>) -> Character {
    let lore = occupation.map(|occupation_id| LoreProfile {
        significance: Significance::Minor,
        planning_ward: PlanningWard::Fabric,
        age: 34,
        gender: "f".into(),
        occupation_id: Some(occupation_id.into()),
        occupation_display: None,
        title: None,
        rank: None,
        faction_role: None,
        illegal_activity: None,
        district: "Bell-and-Sluice streets".into(),
        father: None,
        mother: None,
        children: Vec::new(),
        circumstances: Vec::new(),
        conditions: Vec::new(),
        home: None,
        core_character_description: String::new(),
        extended_character_description: String::new(),
        curiosity: None,
    });
    Character::from_sheet(CharacterSheet {
        pockets: Vec::new(),
        frontbutt: None,
        id: actor(id),
        name: name.to_string(),
        control: Control::Llm,
        back_story: "test".into(),
        location_description: "Bell-and-Sluice streets".into(),
        appearance: Default::default(),
        voice_key: Some(name.to_lowercase()),
        position_m: Vec3::new(x, 0.0, 0.0),
        facing_yaw: 0.0,
        holds: Vec::new(),
        goal: "None".into(),
        memories: Vec::new(),
        knows: BTreeSet::new(),
        lore,
        presence: Presence::InCity,
        presence_epoch: 0,
        economic_class: EconomicClass::Resident,
    })
}

/// The line graph the movement and places tests use, stretched to six nodes ten
/// metres apart. `seize` prices the escort's own walk against the nav graph, so
/// a world without one cannot seize anybody at all.
fn law_nav() -> NavData {
    let (w, h) = (60usize, 10usize);
    let bitset = vec![0xFF_u8; (w * h).div_ceil(8)];
    let json = format!(
        r#"{{
          "schema_version": 1,
          "grid": {{"x0": -5.0, "z0": -5.0, "cell_m": 1.0, "w": {w}, "h": {h},
                    "agent_radius_m": 0.35, "bitset_file": "x.bin",
                    "bitset_bits": {bits}, "bitset_sha256": ""}},
          "nodes": [[0.0, 0.0], [10.0, 0.0], [20.0, 0.0], [30.0, 0.0], [40.0, 0.0], [50.0, 0.0]],
          "edges": [[0, 1, 2.0], [1, 2, 2.0], [2, 3, 2.0], [3, 4, 2.0], [4, 5, 2.0]],
          "places": [{{"name": "a", "node": 0, "kind": "place"}},
                     {{"name": "b", "node": 5, "kind": "place"}}],
          "sites": [],
          "doors": [],
          "reference": {{"forecourt": 0}}
        }}"#,
        bits = w * h
    );
    NavData::from_parts(&json, &bitset).expect("the hand-built nav validates")
}

/// Three of the eight postings [`custody::STATION_PLACE_NAMES`] knows, arranged
/// so that "nearest, and never the Stone House" is an assertion rather than a
/// wish: the toll-house ten metres up the street, the River Gate forty, and the
/// Stone House directly underfoot at the origin.
fn law_places(nav: &NavData) -> PlaceRegistry {
    let json = r#"{
        "schema_version": 1,
        "places": [
            {"id": "pl_toll", "name": "Tallage toll-house", "node": 1, "kind": "major", "ward": "wick"},
            {"id": "pl_rvrg", "name": "The River Gate", "node": 4, "kind": "landmark", "ward": "reed"},
            {"id": "pl_ston", "name": "The Stone House", "node": 0, "kind": "landmark", "ward": "reed"}
        ],
        "wards": []
    }"#;
    PlaceRegistry::from_json(json, nav).expect("the compact station registry loads")
}

/// The shipped clock (`seconds_per_day: 3600`), so "minutes ago" and "a game
/// day ago" are two readings of the same dial — and so `summon` has a real next
/// bell to name. Opens at the Watch, whose next bell is the Kindling.
fn clock() -> WorldClock {
    WorldClock::new(3600.0, Office::Watch, 0, 0.05)
}

fn at(now_seconds: f64) -> WorldTime {
    clock().at(now_seconds)
}

/// A sergeant, a revenue man, a thief and a baker on one street, with a clock,
/// a nav graph and three postings — and **no player**: M4b′'s claim is that the
/// whole chain runs with the player absent from the world entirely.
fn law_world() -> World {
    let nav = Arc::new(law_nav());
    let mut world = World::new();
    world.places = law_places(&nav);
    world.nav = Some(nav);
    // Ashe holds hard (`bailiff_and_gaoler`), Trask does not (`revenue_worker`);
    // `break_free_chance` reads exactly that difference off their sheets.
    world.add_character(person("srgnt", "Havise Ashe", 0.0, Some("bailiff_and_gaoler")));
    world.add_character(person("wrdn", "Odo Trask", 0.5, Some("revenue_worker")));
    world.add_character(person("tamrd", "Tam Rud", 1.0, None));
    world.add_character(person("gossp", "Ulla Brant", 2.0, Some("baker")));
    world.current_time = Some(at(0.0));
    world
}

fn move_to(world: &mut World, actor_id: &str, x: f64) {
    world
        .characters
        .get_mut(&actor(actor_id))
        .expect("character exists")
        .state
        .position_m = Vec3::new(x, 0.0, 0.0);
}

/// A wrong put to the ward by an officer's own eyes — the `raise_notice` verb,
/// which is what `Notices::fresh_own_notice` is really a record of.
fn raise_word(world: &mut World, officer: &str, accused: &str) -> u64 {
    apply_action(
        world,
        &actor(officer),
        "raise_notice",
        &json!({
            "about": "a stranger in a grey hood",
            "deed": "took a boy's spark",
            "accused": accused,
        }),
    )
    .expect("the law may put a wrong to the ward");
    world
        .notices
        .live()
        .last()
        .expect("the word is on the ward's tongues")
        .id
}

/// The only seizure the design allows: a word said in the same turn, then the
/// hand. `seize` reads `World::spoke_this_turn`, which only `say` sets, so this
/// pair is how the verb is reached at all.
fn say_and_seize(world: &mut World, officer: &str, target: &str) -> Result<String, ActionError> {
    apply_action(
        world,
        &actor(officer),
        "say",
        &json!({"text": "Stand still. You are coming with me."}),
    )
    .expect("an officer may always speak");
    apply_action(world, &actor(officer), "seize", &json!({"person": target}))
}

/// The gaol (M5), the one station that holds longer and the one the picker
/// never reaches by distance.
fn stone_house() -> Station {
    Station {
        place_id: PlaceId::from_raw("pl_ston"),
        name: custody::STONE_HOUSE_PLACE_NAME.into(),
        point: Vec3::new(0.0, 0.0, 0.0),
        stone_house: true,
    }
}

/// A station standing in for the registry's, where the test is about the record
/// rather than the picker.
fn toll_house() -> Station {
    Station {
        place_id: PlaceId::from_raw("pl_toll"),
        name: "Tallage toll-house".into(),
        point: Vec3::new(10.0, 0.0, 0.0),
        stone_house: false,
    }
}

// ------------------------------------------------------- the four preconditions

/// M4b: each of `seize`'s preconditions is the design rather than a guard rail.
/// The law only, because watching is the player's whole part in it; four metres,
/// because conversation reaches twenty and the officer's walk across the square
/// is the entire warning system; and a `say` in the same turn, because a
/// wordless seizure reads as the game stealing the controller.
#[test]
fn a_seizure_needs_the_law_four_metres_and_a_word_in_the_same_turn() {
    // A baker with a perfectly good warrantable wrong in front of her still may
    // not take anybody: standing is occupational, not situational.
    let mut world = law_world();
    raise_word(&mut world, "srgnt", "tamrd");
    let error = say_and_seize(&mut world, "gossp", "tamrd").unwrap_err();
    assert_eq!(error.code, ActionErrorCode::InvalidAction);
    assert!(
        error.message.contains("only those who serve the city's law"),
        "{error}"
    );

    // Five metres: inside speaking distance, outside taking distance.
    let mut world = law_world();
    raise_word(&mut world, "srgnt", "tamrd");
    move_to(&mut world, "tamrd", 5.0);
    let error = say_and_seize(&mut world, "srgnt", "tamrd").unwrap_err();
    assert_eq!(error.code, ActionErrorCode::OutOfRange);
    assert!(error.message.contains("more than 4 metres away"), "{error}");
    assert!(!world.custody.holds(&actor("tamrd")));

    // Silence, and then somebody *else's* voice — the verb wants this officer's
    // own words in this same reply, not a noisy street.
    let mut world = law_world();
    raise_word(&mut world, "srgnt", "tamrd");
    let silent = apply_action(
        &mut world,
        &actor("srgnt"),
        "seize",
        &json!({"person": "tamrd"}),
    )
    .unwrap_err();
    assert!(
        silent.message.contains("nobody may be taken in silence"),
        "{silent}"
    );
    apply_action(
        &mut world,
        &actor("gossp"),
        "say",
        &json!({"text": "Someone stop him!"}),
    )
    .expect("a bystander may cry out");
    let borrowed = apply_action(
        &mut world,
        &actor("srgnt"),
        "seize",
        &json!({"person": "tamrd"}),
    )
    .unwrap_err();
    assert!(
        borrowed.message.contains("nobody may be taken in silence"),
        "another mouth is not this officer's mouth: {borrowed}"
    );

    // The positive control: the same world, the same word, the officer's own
    // voice — so every refusal above was about what it claimed to be about.
    say_and_seize(&mut world, "srgnt", "tamrd").expect("law, reach and a word: taken");
    assert!(world.custody.holds(&actor("tamrd")));
}

/// M4b, the lore's two doors: a live warrant lets *any* law-cast actor take the
/// accused, and an officer's own word from within the last game hour stands in
/// for the *immediate breach of the peace the watchman witnessed* — the one
/// thing the sim can actually check, since percepts are prose and there is no
/// record that says "this officer saw that happen".
#[test]
fn seize_opens_on_a_warrant_or_on_the_officers_own_word_within_the_hour() {
    // Door one: the gate keeper's word, ignored past its bell — and the
    // sergeant, who never raised it, may act on it.
    let mut world = law_world();
    let id = raise_word(&mut world, "wrdn", "tamrd");
    apply_action(
        &mut world,
        &actor("wrdn"),
        "summon",
        &json!({"notice_id": id}),
    )
    .expect("the law may summon on its own word");
    world.current_time = Some(at(500.0));
    assert_eq!(
        world.notices.issue_warrants(at(500.0).game_days()),
        [id],
        "the Kindling rang on a word still standing"
    );
    say_and_seize(&mut world, "srgnt", "tamrd").expect("a warrant lets any of the law take them");
    assert_eq!(
        world.custody.get(&actor("tamrd")).unwrap().notice_id,
        Some(id),
        "the record names the word it answers"
    );

    // Door two: no warrant at all, but the sergeant put this wrong to the ward
    // herself a minute ago.
    let mut world = law_world();
    let id = raise_word(&mut world, "srgnt", "tamrd");
    world.current_time = Some(at(60.0));
    assert!(world.notices.warrant_against(&actor("tamrd")).is_none());
    say_and_seize(&mut world, "srgnt", "tamrd").expect("your own eyes, within the hour");
    assert_eq!(
        world.custody.get(&actor("tamrd")).unwrap().notice_id,
        Some(id)
    );
}

/// The other side of the same two doors, and the reason the second one is
/// written as *your own word, within the hour*: somebody else's word is not
/// your own eyes, and yesterday's word needs the warrant like everything else.
/// Both refusals name the ladder's next rung, because a refusal that does not
/// say what would work stalls the model.
#[test]
fn seize_is_refused_on_another_officers_word_and_on_a_stale_word_of_your_own() {
    // Another officer's live, unwarranted word.
    let mut world = law_world();
    raise_word(&mut world, "wrdn", "tamrd");
    let error = say_and_seize(&mut world, "srgnt", "tamrd").unwrap_err();
    assert_eq!(error.code, ActionErrorCode::NoWarrant);
    assert!(
        error.message.contains("summon them to answer first"),
        "the refusal points at the rung below: {error}"
    );
    assert!(!world.custody.holds(&actor("tamrd")));

    // The officer's own word, a whole game day old.
    let mut world = law_world();
    raise_word(&mut world, "srgnt", "tamrd");
    world.current_time = Some(at(3600.0));
    let error = say_and_seize(&mut world, "srgnt", "tamrd").unwrap_err();
    assert_eq!(error.code, ActionErrorCode::NoWarrant);
    assert!(!world.custody.holds(&actor("tamrd")));

    // Naming the number does not create the authority either: an explicit
    // `notice_id` must itself pass one of the two doors.
    let error = apply_action(
        &mut world,
        &actor("srgnt"),
        "seize",
        &json!({"person": "tamrd", "notice_id": 1}),
    )
    .unwrap_err();
    assert_eq!(error.code, ActionErrorCode::NoWarrant);
}

/// The other half of "an explicit `notice_id` must itself pass one of the two
/// doors": it is tried at **both** of them. An officer who says which word she
/// is acting on must not be refused because some *other* word against the same
/// man happens to carry a warrant — the refusal would be false on both counts,
/// and naming nothing at all would have taken him.
#[test]
fn a_named_notice_is_tried_at_the_second_door_as_well_as_the_first() {
    let mut world = law_world();
    // An older word, raised by the gate keeper and ripened into a warrant.
    let old = raise_word(&mut world, "wrdn", "tamrd");
    apply_action(
        &mut world,
        &actor("wrdn"),
        "summon",
        &json!({"notice_id": old}),
    )
    .expect("the law may summon on its own word");
    world.current_time = Some(at(500.0));
    assert_eq!(
        world.notices.issue_warrants(at(500.0).game_days()),
        [old],
        "the Kindling rang on a word still standing"
    );

    // And the sergeant's own word, put to the ward this minute for something she
    // watched happen — the one she names when she takes him.
    let fresh = raise_word(&mut world, "srgnt", "tamrd");
    assert_ne!(fresh, old);
    apply_action(
        &mut world,
        &actor("srgnt"),
        "say",
        &json!({"text": "Stand still. You are coming with me."}),
    )
    .expect("an officer may always speak");
    apply_action(
        &mut world,
        &actor("srgnt"),
        "seize",
        &json!({"person": "tamrd", "notice_id": fresh}),
    )
    .expect("her own eyes, within the hour - the warrant is not the only door");
    let record = world.custody.get(&actor("tamrd")).expect("taken in charge");
    assert_eq!(
        record.notice_id,
        Some(fresh),
        "the record names the word the officer said she was acting on"
    );
    assert_ne!(
        record.station.name,
        custody::STONE_HOUSE_PLACE_NAME,
        "the gaol is for the warrant she did not invoke"
    );
}

/// The cap exists because nothing else in this sim removes a person from the
/// world and the economy is made of named individuals — gaol the wrong baker and
/// the bread round stops. The eight the city was already holding never count
/// against it: it is there to stop a bad-tempered sergeant emptying the
/// Wickmarket, not to evict the lore.
#[test]
fn seize_is_refused_past_the_confinement_cap_and_the_authored_inmates_never_count() {
    let stone_house = Station {
        place_id: PlaceId::from_raw("pl_ston"),
        name: "The Stone House".into(),
        point: Vec3::ZERO,
        stone_house: true,
    };

    // Eight authored inmates and the law still has room for everybody.
    let mut world = law_world();
    for index in 0..8 {
        let inmate = person(&format!("inm{index}"), "An inmate", 200.0, None);
        let id = inmate.id().clone();
        world.add_character(inmate);
        world.custody.seed_inmate(id, stone_house.clone());
    }
    assert_eq!(world.custody.arrest_count(), 0, "the authored are not arrests");
    raise_word(&mut world, "srgnt", "tamrd");
    say_and_seize(&mut world, "srgnt", "tamrd").expect("a full gaol of authored inmates is room");
    assert_eq!(world.custody.arrest_count(), 1);

    // Four arrests standing, and the fifth is refused. The four are handed
    // straight to `Custody` — what is under test is the verb's cap check, not
    // four more seizures.
    let mut world = law_world();
    for index in 0..CUSTODY_MAX_ARRESTS {
        world.custody.seize(
            actor(&format!("hel{index}")),
            actor("wrdn"),
            Some(1),
            stone_house.clone(),
            0.0,
        );
    }
    raise_word(&mut world, "srgnt", "tamrd");
    let error = say_and_seize(&mut world, "srgnt", "tamrd").unwrap_err();
    assert_eq!(error.code, ActionErrorCode::CustodyFull);
    assert!(
        error.message.contains("see one released before you take another"),
        "the refusal names the way out: {error}"
    );

    // Draining is what makes room, which is the whole reason `release` is on
    // every holder's turn.
    world.custody.release(&actor("hel0"));
    say_and_seize(&mut world, "srgnt", "tamrd").expect("one released, one taken");
    assert_eq!(world.custody.arrest_count(), CUSTODY_MAX_ARRESTS);
}

// ------------------------------------------------------------------- the sheet

/// M4b′: the prompt pays for custody only where custody exists. A person being
/// marched across the city can afford four lines about it; everybody else pays
/// zero bytes, which is also what keeps the golden fixtures frozen. And the
/// two states are visibly different on the sheet — in charge is not held, so
/// `struggle` appears only once a hand is actually on the arm.
///
/// Custody is put on through [`cathedral_sim::custody::Custody`] rather than
/// through `seize` on purpose: the verb also writes percepts, and percepts are
/// history by design, so the byte-identity claim is about the custody *state*
/// adding exactly one section and taking it away again.
#[test]
fn custody_adds_one_sheet_section_and_a_verb_and_removes_both_on_release() {
    let env = prompt_env();
    let mut world = seed_world();
    let (prisoner, officer, bystander) = (actor("sv3n1"), actor("k0fb1"), actor("cb947"));

    let before = render_prompt(&world, &prisoner, None, &env).unwrap();
    let bystander_before = render_prompt(&world, &bystander, None, &env).unwrap();
    assert!(!before.contains("you_are_held"));
    assert!(!before.contains("struggle {}"));

    world
        .custody
        .seize(prisoner.clone(), officer.clone(), None, toll_house(), 0.0);
    let in_charge = render_prompt(&world, &prisoner, None, &env).unwrap();
    assert!(
        in_charge.contains("**you_are_held** —"),
        "the section renders under `you_are`: {in_charge}"
    );
    assert!(
        in_charge.contains("walking you to Tallage toll-house"),
        "who has you and where you are being taken: {in_charge}"
    );
    assert!(
        in_charge.contains("Only the law choosing to let you go ends this"),
        "and what would end it — a brand with no door is a bug"
    );
    assert!(
        !in_charge.contains("struggle {}"),
        "in charge is not held: no hand on the arm, no verb to fight it"
    );

    world.custody.grab(&prisoner, officer.clone());
    let held = render_prompt(&world, &prisoner, None, &env).unwrap();
    assert!(held.contains("a hand is on your arm"));
    assert!(held.contains("struggle {}"), "the verb arrives with the grip");
    assert!(
        held.contains("You are in the law's hands"),
        "and the paragraph that explains it: {held}"
    );

    // The escort's own sheet grows the other half of the same fact: whom they
    // have, and `grab` — the officer's deliberate counterpart to the host-side
    // reflex, and nobody else's business.
    let escort = render_prompt(&world, &officer, None, &env).unwrap();
    assert!(escort.contains("grab {\"person\""));
    assert!(
        escort.contains("**you_have_in_charge**"),
        "the ids `release` and `grab` take must be on the sheet that lists them: {escort}"
    );
    assert!(
        escort.contains(prisoner.as_str()),
        "and it names the prisoner by id"
    );
    assert!(!escort.contains("**you_are_held**"), "the officer is not held");

    // The bystander two doors down never moves a byte.
    assert_eq!(
        render_prompt(&world, &bystander, None, &env).unwrap(),
        bystander_before
    );

    // And released, the prisoner's own sheet is byte-identical to before.
    world.custody.release(&prisoner);
    assert_eq!(render_prompt(&world, &prisoner, None, &env).unwrap(), before);
}

/// M4e: *"gaol fees are fixed publicly; inventing a fee is extortion"*
/// (`lore/core_lore/secular_government.md`), so the number is a constant and
/// not a thing a keeper improvises per asker. Two different keepers, two
/// different prisoners, two different stations — one sentence, one number, and
/// it is [`custody::GAOL_FEE_SPARKS`] rather than whatever the template happens
/// to say. The prisoner reads the same number off the same wall, which is what
/// a *posted* fee means and what keeps "what would end this" concrete.
///
/// The load-bearing case is the **third** keeper: the one who took nobody and
/// holds nobody, and is simply the law standing over a prisoner. That is the
/// Stone House's own keeper (M5b seeds the eight with no officer of record at
/// all), and `Custody::prisoners_of` — what `has_custody` reads — cannot see
/// them. If the sheet were gated on that, the one person whose whole job is
/// this fee would be the one person never told it, while `release` let them
/// open the door anyway. Sheet and verb ask `custody::keeps`, once.
#[test]
fn the_posted_gaol_fee_is_one_number_every_keeper_quotes() {
    let env = prompt_env();
    let mut world = law_world();
    let fee = format!("{} sparks", custody::GAOL_FEE_SPARKS);

    // Nobody keeping anybody pays a byte for it — the same claim the whole
    // sheet is built on, and what keeps the golden fixtures frozen.
    for who in ["srgnt", "wrdn", "tamrd", "gossp"] {
        let quiet = render_prompt(&world, &actor(who), None, &env).unwrap();
        assert!(
            !quiet.contains("gaol fee"),
            "{who} keeps nobody and is kept by nobody: {quiet}"
        );
    }

    world
        .custody
        .seize(actor("tamrd"), actor("srgnt"), None, toll_house(), 0.0);
    // The Stone House's case: committed, with no officer and no notice, exactly
    // as the eight the city was already holding arrive.
    world.custody.seed_inmate(actor("gossp"), stone_house());
    move_to(&mut world, "gossp", 0.5);

    let posted = format!("The gaol fee is posted, and it is {fee} —");
    for (who, id) in [
        ("the arresting sergeant", "srgnt"),
        ("the keeper of record for nobody", "wrdn"),
    ] {
        let sheet = compact(&render_prompt(&world, &actor(id), None, &env).unwrap());
        assert!(
            sheet.contains(&posted),
            "{who} quotes the posted fee verbatim: {sheet}"
        );
        assert!(
            sheet.contains("a keeper who invents one is extorting"),
            "{who} is told why the number is not theirs to choose"
        );
        assert!(
            sheet.contains("Coin in your hand settles nothing by itself"),
            "{who} still chooses settle_notice — bribery stays an omission (M3.5)"
        );
        assert!(
            sheet.contains("release {\"person\""),
            "{who} is offered the verb the paragraph promises them"
        );
    }

    // The baker standing in the same street keeps nobody, whoever else does.
    let bystander = render_prompt(&world, &actor("tamrd"), None, &env).unwrap();
    assert!(
        !bystander.contains("The gaol fee is posted"),
        "a prisoner is not their own keeper: {bystander}"
    );

    // And the prisoners read it off the same wall: a public fee is public to
    // the person who has to pay it.
    for prisoner in ["tamrd", "gossp"] {
        let held = compact(&render_prompt(&world, &actor(prisoner), None, &env).unwrap());
        assert!(
            held.contains(&format!("The posted gaol fee is {fee}")),
            "{prisoner} can read what would free them: {held}"
        );
    }
}

/// M4e/M5d: the sheet and the verb must answer "who keeps this person?" with
/// one voice. `custody::keeps` is the shipped precondition of `release` lifted
/// out of it, so a door the prompt offers is a door the action opens — and one
/// it hides is one that was really shut.
#[test]
fn only_the_law_within_hearing_opens_the_door() {
    let mut world = law_world();
    world.custody.seed_inmate(actor("tamrd"), stone_house());

    // A baker standing right there is not the law, whatever she thinks.
    let error = apply_action(&mut world, &actor("gossp"), "release", &json!({"person": "tamrd"}))
        .unwrap_err();
    assert_eq!(error.code, ActionErrorCode::InvalidAction);
    assert!(error.message.contains("not yours to release"), "{error}");
    assert!(world.custody.holds(&actor("tamrd")), "and she opened nothing");

    // The law standing over them is the keeper, though they took nobody and
    // hold nobody: this is the whole of "confined by a person, not a door".
    assert!(custody::keeps(&world, &actor("wrdn"), &actor("tamrd")));
    // …but only while they are actually standing there.
    move_to(&mut world, "wrdn", 45.0);
    assert!(!custody::keeps(&world, &actor("wrdn"), &actor("tamrd")));
    let error = apply_action(&mut world, &actor("wrdn"), "release", &json!({"person": "tamrd"}))
        .unwrap_err();
    assert_eq!(error.code, ActionErrorCode::InvalidAction);

    move_to(&mut world, "wrdn", 0.5);
    apply_action(&mut world, &actor("wrdn"), "release", &json!({"person": "tamrd"}))
        .expect("the keeper at the threshold opens it");
    assert!(!world.custody.holds(&actor("tamrd")));
}

/// M4b: being talked round is an *ending*, and an ending nobody tells the
/// prisoner about is indistinguishable from still being held — their sheet's
/// `you_are_held` paragraph simply vanishes and the model keeps answering as a
/// prisoner. Every other way out says so (`custody::forget_departed`, the freed
/// loop in `tick_custody`), so the verb must too — and the news has to reach
/// them whether the hand that opens the door is on their arm or twenty metres
/// off at a threshold.
#[test]
fn a_release_tells_the_one_let_go_as_well_as_the_street() {
    let mut world = law_world();
    raise_word(&mut world, "srgnt", "tamrd");
    say_and_seize(&mut world, "srgnt", "tamrd").expect("the word and then the hand");

    apply_action(
        &mut world,
        &actor("srgnt"),
        "release",
        &json!({"person": "tamrd"}),
    )
    .expect("the officer of record may always let them go");

    let freed = world.characters[&actor("tamrd")].inbox();
    assert!(
        freed
            .iter()
            .any(|line| line.contains("lets you go") && line.contains("free to walk away")),
        "the one let go hears it in the second person: {freed:?}"
    );
    // The street still watches a release happen, exactly as it watched the
    // seizure — and in the third person, because it did not happen to them.
    let overheard = world.characters[&actor("gossp")].inbox();
    assert!(
        overheard
            .iter()
            .any(|line| line.contains("lets") && line.contains("go") && !line.contains("you")),
        "a bystander hears the ending too: {overheard:?}"
    );
    // The releaser is not a witness to themselves: they remember the act
    // (`recent_history`), like every other emitter in the crate.
    let releaser = &world.characters[&actor("srgnt")];
    assert!(
        !releaser.inbox().iter().any(|line| line.contains("lets")),
        "nobody is told about their own hand: {:?}",
        releaser.inbox()
    );
    assert!(
        releaser
            .recent_history()
            .iter()
            .any(|line| line.contains("You let them go rather than see them to")),
        "…they remember it instead: {:?}",
        releaser.recent_history()
    );

    // And the far arm of `custody::keeps`: the officer of record keeps whoever
    // they took from anywhere at all, so a release said from beyond earshot is
    // still a release the prisoner is owed the news of.
    let mut world = law_world();
    raise_word(&mut world, "srgnt", "tamrd");
    say_and_seize(&mut world, "srgnt", "tamrd").expect("the word and then the hand");
    move_to(&mut world, "srgnt", 45.0);
    apply_action(
        &mut world,
        &actor("srgnt"),
        "release",
        &json!({"person": "tamrd"}),
    )
    .expect("the officer of record is their keeper wherever they stand");
    let freed = world.characters[&actor("tamrd")].inbox();
    assert!(
        freed.iter().any(|line| line.contains("lets you go")),
        "distance may cost the street the news, never the prisoner: {freed:?}"
    );
}

/// M5b: *"it is the same state the player's commitment uses, and the same one
/// an NPC committed under M4b′ arrives in — the authored eight and tonight's
/// arrest are not two mechanisms, they are one flag with different histories."*
///
/// So the authored path gets the same test the arrest path has: `go_to` is
/// refused in both its forms, and everything that makes a cell a room still
/// works. `Custody::holds` is key presence, which is exactly why seeding somebody
/// *is* confining them and no second guard was needed anywhere.
#[test]
fn a_seeded_inmate_is_refused_go_to_exactly_as_a_seized_one_is() {
    let mut world = law_world();
    world.add_item(Item::new(item("purse"), "spark"));
    world
        .characters
        .get_mut(&actor("tamrd"))
        .unwrap()
        .state
        .holds = vec![item("purse")];
    // No officer, no notice, no arrest — the city was simply already holding
    // them when the run began.
    world.custody.seed_inmate(actor("tamrd"), stone_house());

    for args in [json!({"place_id": "pl_toll"}), json!({"person": "srgnt"})] {
        let error = apply_action(&mut world, &actor("tamrd"), "go_to", &args).unwrap_err();
        assert_eq!(error.code, ActionErrorCode::InCustody, "for {args}");
    }
    assert!(world.characters[&actor("tamrd")].state.intent.is_none());

    // Everything that makes it a conversation rather than a cell still works.
    apply_action(
        &mut world,
        &actor("tamrd"),
        "say",
        &json!({"text": "Is there bread today?"}),
    )
    .expect("an inmate may speak");
    apply_action(
        &mut world,
        &actor("tamrd"),
        "offer_item",
        &json!({"item_id": "purse", "target": "wrdn"}),
    )
    .expect("an inmate may offer what they have — paying the fee is the main door");
    apply_action(
        &mut world,
        &actor("tamrd"),
        "remember",
        &json!({"memory": "The keeper says the fee is three sparks."}),
    )
    .expect("an inmate may remember");
}

// --------------------------------------------------------- what a prisoner may do

/// M4b′: "you simply have nowhere to go" is the whole of it. `go_to` is refused
/// in both its forms — the ladder guard in `round::decide` does nothing about a
/// model that simply decides to leave, and a confined actor who announces "I am
/// going to the well" and is not stopped is worse than one who never says it —
/// while every verb that makes the escort a conversation still works. That is
/// what keeps compliance the pleasant path instead of a dead end.
#[test]
fn a_held_actor_may_not_walk_but_may_still_speak_hand_over_and_remember() {
    let mut world = law_world();
    world.add_item(Item::new(item("purse"), "spark"));
    world.add_item(Item::new(item("crust"), "bread"));
    world
        .characters
        .get_mut(&actor("tamrd"))
        .unwrap()
        .state
        .holds = vec![item("purse")];
    world
        .characters
        .get_mut(&actor("srgnt"))
        .unwrap()
        .state
        .holds = vec![item("crust")];
    raise_word(&mut world, "srgnt", "tamrd");
    say_and_seize(&mut world, "srgnt", "tamrd").expect("taken in charge");

    for args in [json!({"place_id": "pl_toll"}), json!({"person": "srgnt"})] {
        let error = apply_action(&mut world, &actor("tamrd"), "go_to", &args).unwrap_err();
        assert_eq!(error.code, ActionErrorCode::InCustody, "for {args}");
        assert!(
            error.message.contains("speak to whoever holds you"),
            "the refusal names the only way out: {error}"
        );
    }
    assert!(
        world.characters[&actor("tamrd")].state.intent.is_none(),
        "a refused go_to leaves no intent behind"
    );

    // The mouth, the hands and the memory all still work — asking what it would
    // take, offering what you have, and sending for someone who would vouch for
    // you are the *content* of the escort.
    apply_action(
        &mut world,
        &actor("tamrd"),
        "say",
        &json!({"text": "What would it take to settle this?"}),
    )
    .expect("a prisoner may still speak");
    apply_action(
        &mut world,
        &actor("tamrd"),
        "offer_item",
        &json!({"item_id": "purse", "target": "srgnt"}),
    )
    .expect("a prisoner may still hand something over");
    apply_action(
        &mut world,
        &actor("srgnt"),
        "offer_item",
        &json!({"item_id": "crust", "target": "tamrd"}),
    )
    .expect("and be handed something");
    apply_action(
        &mut world,
        &actor("tamrd"),
        "accept_offered_item",
        &json!({"item_id": "crust"}),
    )
    .expect("a prisoner may still accept");
    apply_action(
        &mut world,
        &actor("tamrd"),
        "remember",
        &json!({"memory": "The sergeant listens if you speak plainly."}),
    )
    .expect("a prisoner may still think");
    assert!(
        world.custody.holds(&actor("tamrd")),
        "and none of it ended the custody"
    );
}

// ---------------------------------------------------------------- the struggle

/// M4d: `struggle` answers a hand on the arm and nothing else. Being merely in
/// charge is refused with the reason spelled out — you may simply walk away from
/// it — because the difference between *in charge* and *held* is the entire
/// point of the state machine and a model that cannot read it off a refusal will
/// keep pulling against nobody.
#[test]
fn struggle_answers_only_a_hand_on_the_arm() {
    let mut world = law_world();
    let free = apply_action(&mut world, &actor("tamrd"), "struggle", &json!({})).unwrap_err();
    assert_eq!(free.code, ActionErrorCode::InvalidAction);
    assert!(free.message.contains("nobody has hold of you"), "{free}");

    raise_word(&mut world, "srgnt", "tamrd");
    say_and_seize(&mut world, "srgnt", "tamrd").expect("taken in charge");
    let in_charge = apply_action(&mut world, &actor("tamrd"), "struggle", &json!({})).unwrap_err();
    assert!(
        in_charge
            .message
            .contains("in their charge, not in their hands"),
        "{in_charge}"
    );
    assert!(
        in_charge.message.contains("you may simply walk away from it"),
        "the refusal says what to do instead: {in_charge}"
    );

    // With the hand on the arm the verb is answerable, whichever way the roll
    // falls.
    apply_action(
        &mut world,
        &actor("srgnt"),
        "grab",
        &json!({"person": "tamrd"}),
    )
    .expect("the officer of record may take hold");
    apply_action(&mut world, &actor("tamrd"), "struggle", &json!({}))
        .expect("a held body may always try");
}

/// M4d: escape is meant to be **easy enough to be a real choice** — what should
/// make you hesitate is the consequence, not the difficulty — so one holder is a
/// coin-flip and a bit, while being *dragged* by two is what the word actually
/// means. That is why the escort's right move is `say` ("help me hold this
/// one") rather than tightening their own grip.
///
/// The second hand is put on through [`cathedral_sim::custody::Custody`] rather
/// than through the verb, because the shipped `grab` refuses anybody who is not
/// the officer of record or already a holder — so the design's own answer to a
/// struggler ("help me hold this one", and nearby law walking over) does not
/// currently reach the second holder the refcounted `holders` list exists for.
/// What is pinned here is the arithmetic the host's strain meter also reads.
#[test]
fn one_hand_can_be_torn_free_of_and_two_cannot() {
    let (thief, trask, ashe) = (actor("tamrd"), actor("wrdn"), actor("srgnt"));

    // Grip by occupation, off the sheets the cast already carries: Ashe the
    // gaoler holds harder than Trask the revenue man, and a second pair of
    // hands is not twice one pair.
    let one = custody::break_free_chance(&law_world(), &thief, &[trask.clone()]);
    let harder = custody::break_free_chance(&law_world(), &thief, &[ashe.clone()]);
    let two = custody::break_free_chance(&law_world(), &thief, &[trask.clone(), ashe.clone()]);
    assert!(one > 0.5, "one ordinary hand is a coin-flip and a bit: {one}");
    assert!(harder < one, "the gaoler holds harder: {harder} < {one}");
    assert!(two < harder / 4.0, "two hands drag you: {two}");

    // One holder: torn free of, and the custody ends with the grip.
    let mut world = law_world();
    raise_word(&mut world, "wrdn", "tamrd");
    say_and_seize(&mut world, "wrdn", "tamrd").expect("taken in charge");
    world.custody.grab(&thief, trask.clone());
    let broke = apply_action(&mut world, &thief, "struggle", &json!({})).unwrap();
    assert!(broke.contains("tears free and runs"), "{broke}");
    assert!(!world.custody.holds(&thief), "free is free, not merely loose");

    // Two holders, the same thief, the same first attempt: held fast.
    let mut world = law_world();
    raise_word(&mut world, "wrdn", "tamrd");
    say_and_seize(&mut world, "wrdn", "tamrd").expect("taken in charge");
    world.custody.grab(&thief, trask.clone());
    world.custody.grab(&thief, ashe.clone());
    let held = apply_action(&mut world, &thief, "struggle", &json!({})).unwrap();
    assert!(held.contains("struggles and is held fast"), "{held}");
    assert!(world.custody.is_held(&thief));
    // And again, and again. Each pull is its own throw (see
    // [`every_pull_is_its_own_draw_and_not_one_frozen_verdict`]) — but a 5%
    // throw is a 5% throw, so a spammed verb is not a way out of two hands
    // either. What being dragged costs you is turns, not a die roll.
    for _ in 0..8 {
        apply_action(&mut world, &thief, "struggle", &json!({})).unwrap();
        assert!(world.custody.is_held(&thief), "two hands do not slip");
    }
}

/// M4d: a second pull is a *second attempt*, not a replay of the first. The die
/// is a hash rather than a draw — the sim contains no RNG anywhere — so the
/// "which attempt this is" seed has to genuinely advance with the pulling. It
/// used to be the prisoner's `recent_history` length, a buffer capped at 32 that
/// pins there for anybody who has lived a while, and with the seed frozen the
/// same hands answered the same way for ever: an escort no number of tries could
/// break, instead of an independent attempt each time.
#[test]
fn every_pull_is_its_own_draw_and_not_one_frozen_verdict() {
    let (thief, ashe) = (actor("tamrd"), actor("srgnt"));
    let mut world = law_world();
    raise_word(&mut world, "srgnt", "tamrd");
    say_and_seize(&mut world, "srgnt", "tamrd").expect("taken in charge");
    world.custody.grab(&thief, ashe.clone());

    // The gaoler's grip is a little better than one in three, and this thief's
    // first throw against it loses — which is exactly the position the frozen
    // die made permanent.
    let first = apply_action(&mut world, &thief, "struggle", &json!({})).unwrap();
    assert!(first.contains("struggles and is held fast"), "{first}");
    assert_eq!(
        world.custody.get(&thief).unwrap().struggles,
        1,
        "the count the die is seeded from lives on the record, and it moved"
    );

    let mut attempts = 1;
    while world.custody.holds(&thief) && attempts < 20 {
        apply_action(&mut world, &thief, "struggle", &json!({})).unwrap();
        attempts += 1;
    }
    assert!(
        !world.custody.holds(&thief),
        "keeping at it gets you out of one pair of hands: {attempts} tries and still held"
    );
    // Deterministic all the same: the same thief in the same hands takes the
    // same number of tries in every run, so a drive script reproduces.
    assert_eq!(attempts, 5, "and the count itself is reproducible");
}

/// M4d: the person doing the pulling is told about it too. Everything here fans
/// out from `nearby`, which passes its origin as `characters_within`'s *exclude*
/// argument — so the second-person half of the announcement reached nobody at
/// all. The holder and the street heard the hue and cry while the struggler's own
/// history stayed blank, and a model with no evidence it ever tried simply reads
/// its unchanged sheet and tries the same thing again.
#[test]
fn a_struggler_hears_their_own_struggle() {
    let (thief, trask, ashe) = (actor("tamrd"), actor("wrdn"), actor("srgnt"));
    let mut world = law_world();
    raise_word(&mut world, "srgnt", "tamrd");
    say_and_seize(&mut world, "srgnt", "tamrd").expect("taken in charge");
    world.custody.grab(&thief, ashe.clone());
    let held = apply_action(&mut world, &thief, "struggle", &json!({})).unwrap();
    assert!(held.contains("struggles and is held fast"), "{held}");

    // Both moments, in the second person, and remembered rather than delivered:
    // it is what this body just did, not news that reached it.
    let own = world.characters[&thief].recent_history().join("\n");
    assert!(own.contains("You pull against the hands on you."), "{own}");
    assert!(
        own.contains("You fought to get free and could not."),
        "{own}"
    );
    assert!(
        !own.contains("is fighting to get free"),
        "and never the street's third-person line about themselves: {own}"
    );

    // The street still gets exactly what it got before — a hue and cry raises
    // itself, and the holder is owed the interesting call.
    let street = world.characters[&trask].inbox().join("\n");
    assert!(
        street.contains("is fighting to get free of the law's hands"),
        "{street}"
    );
    assert!(
        street.contains("fought against the hands holding them, and did not get free"),
        "{street}"
    );

    // …and the other ending says so in the same voice.
    let mut world = law_world();
    raise_word(&mut world, "wrdn", "tamrd");
    say_and_seize(&mut world, "wrdn", "tamrd").expect("taken in charge");
    world.custody.grab(&thief, trask.clone());
    let broke = apply_action(&mut world, &thief, "struggle", &json!({})).unwrap();
    assert!(broke.contains("tears free and runs"), "{broke}");
    let own = world.characters[&thief].recent_history().join("\n");
    assert!(own.contains("You tore free of the law's hands."), "{own}");
}

/// The sim contains no RNG anywhere, so the die *is* the situation: who is
/// pulling, who is holding, and which attempt this is. Two identical runs must
/// answer identically, or no drive script reproduces and no test above could
/// assert an outcome at all.
#[test]
fn the_struggle_roll_is_the_same_in_every_run() {
    let outcome = || {
        let mut world = law_world();
        raise_word(&mut world, "wrdn", "tamrd");
        say_and_seize(&mut world, "wrdn", "tamrd").expect("taken in charge");
        world.custody.grab(&actor("tamrd"), actor("wrdn"));
        let line = apply_action(&mut world, &actor("tamrd"), "struggle", &json!({})).unwrap();
        (line, world.custody.holds(&actor("tamrd")))
    };
    assert_eq!(outcome(), outcome());

    // The roll underneath is a pure function of the same four things, and the
    // certainties are certain: an empty grip always breaks, and a zero chance
    // never does.
    let (thief, holders) = (actor("tamrd"), [actor("wrdn")]);
    assert_eq!(
        custody::struggle_roll(&thief, &holders, 3, 0.5),
        custody::struggle_roll(&thief, &holders, 3, 0.5)
    );
    assert!(custody::struggle_roll(&thief, &holders, 3, 1.0));
    assert!(!custody::struggle_roll(&thief, &holders, 3, 0.0));
    assert_eq!(
        custody::break_free_chance(&law_world(), &thief, &[]),
        1.0,
        "nobody holding you is not a roll at all"
    );
}

/// M4d: breaking free closes the "you could have just paid the fee" door. The
/// escape notice has **no `wronged` and no `taken`**, which makes it
/// structurally unanswerable by restitution — the only exit left is a law
/// officer choosing `settle_notice`, and that is the cost that makes running a
/// real choice rather than a delay.
#[test]
fn breaking_free_raises_a_word_no_restitution_can_answer() {
    let mut world = law_world();
    world.add_item(Item::new(item("purse"), "spark"));
    world
        .characters
        .get_mut(&actor("tamrd"))
        .unwrap()
        .state
        .holds = vec![item("purse")];
    let first = raise_word(&mut world, "wrdn", "tamrd");
    say_and_seize(&mut world, "wrdn", "tamrd").expect("taken in charge");
    world.custody.grab(&actor("tamrd"), actor("wrdn"));
    apply_action(&mut world, &actor("tamrd"), "struggle", &json!({})).unwrap();

    let escape = world
        .notices
        .live()
        .iter()
        .find(|notice| notice.id != first)
        .expect("tearing free is itself a wrong on the ward's tongues")
        .clone();
    assert_eq!(escape.deed, "broke out of the law's hands and ran");
    assert_eq!(escape.accused, Some(actor("tamrd")));
    assert_eq!(escape.raised_by, actor("wrdn"), "the holder raises it");
    assert_eq!(escape.wronged, None, "there is nobody to pay back");
    assert_eq!(escape.taken, None, "and nothing to hand back");

    // The two settlements no verb can reach cannot reach this one: there is no
    // wronged party for a returned thing to answer to.
    assert!(
        notices::settle_on_return(&mut world, &actor("tamrd"), &actor("wrdn"), &item("purse"))
            .is_empty()
    );
    assert!(world.notices.get(escape.id).is_some());

    // Nor does pressing the purse into the officer's hand for real: the
    // acceptor gets the judgement (M3.5), and the word stands until they use it.
    apply_action(
        &mut world,
        &actor("tamrd"),
        "offer_item",
        &json!({"item_id": "purse", "target": "wrdn"}),
    )
    .expect("the escapee may still try to buy it off");
    apply_action(
        &mut world,
        &actor("wrdn"),
        "accept_offered_item",
        &json!({"item_id": "purse"}),
    )
    .expect("and the officer may still take it");
    assert!(
        world.notices.get(escape.id).is_some(),
        "no payment settles a word by itself"
    );

    // One door, and it is a person: the officer saying so.
    apply_action(
        &mut world,
        &actor("wrdn"),
        "settle_notice",
        &json!({"notice_id": escape.id}),
    )
    .expect("the law may end the word it raised");
    assert!(world.notices.get(escape.id).is_none());
}

// ----------------------------------------------------------------- the summons

/// M4a: the deadline has exactly one thing that clears it — the notice being
/// settled — which is what makes the rung testable at all. Here it is end to
/// end through the verbs: `summon` names the city's own **next bell**, tells the
/// accused in the second person, and a word still standing when that bell rings
/// becomes a warrant on the clock edge.
#[test]
fn a_summons_names_the_next_bell_and_only_settling_answers_it() {
    let mut world = law_world();
    world.add_item(Item::new(item("purse"), "spark"));
    world
        .characters
        .get_mut(&actor("tamrd"))
        .unwrap()
        .state
        .holds = vec![item("purse")];
    let id = raise_word(&mut world, "srgnt", "tamrd");
    assert_eq!(world.notices.get(id).unwrap().rung(), Rung::Word);

    apply_action(
        &mut world,
        &actor("srgnt"),
        "summon",
        &json!({"notice_id": id}),
    )
    .expect("the law may summon on a word that names somebody");
    assert_eq!(world.notices.get(id).unwrap().rung(), Rung::Summoned);
    assert!(
        world.characters[&actor("tamrd")]
            .inbox()
            .iter()
            .any(|line| line.contains("you are called to answer for this by the Kindling")),
        "the accused is told directly, and the deadline is the city's own clock: {:?}",
        world.characters[&actor("tamrd")].inbox()
    );

    // There is no re-summoning and no un-summoning, so the ladder only ever
    // climbs.
    let again = apply_action(
        &mut world,
        &actor("wrdn"),
        "summon",
        &json!({"notice_id": id}),
    )
    .unwrap_err();
    assert!(again.message.contains("already carries a summons"), "{again}");

    // Handing the officer a purse is not answering: the acceptor judges, and
    // until they say so the summons stands.
    apply_action(
        &mut world,
        &actor("tamrd"),
        "offer_item",
        &json!({"item_id": "purse", "target": "srgnt"}),
    )
    .unwrap();
    apply_action(
        &mut world,
        &actor("srgnt"),
        "accept_offered_item",
        &json!({"item_id": "purse"}),
    )
    .unwrap();
    assert_eq!(world.notices.get(id).unwrap().rung(), Rung::Summoned);

    // The bell rings on a word still standing.
    assert!(
        world.notices.issue_warrants(at(400.0).game_days()).is_empty(),
        "the Kindling has not rung yet"
    );
    assert_eq!(world.notices.issue_warrants(at(500.0).game_days()), [id]);
    assert_eq!(world.notices.get(id).unwrap().rung(), Rung::Warranted);
    assert!(
        world
            .notices
            .issue_warrants(at(3600.0).game_days())
            .is_empty(),
        "a warrant issues once"
    );

    // The other order: settled before its bell, and the deadline has nothing
    // left to come due on.
    let mut world = law_world();
    let id = raise_word(&mut world, "srgnt", "tamrd");
    apply_action(
        &mut world,
        &actor("srgnt"),
        "summon",
        &json!({"notice_id": id}),
    )
    .unwrap();
    apply_action(
        &mut world,
        &actor("srgnt"),
        "settle_notice",
        &json!({"notice_id": id}),
    )
    .expect("going and dealing with it is what answering means");
    assert!(
        world
            .notices
            .issue_warrants(at(500.0).game_days())
            .is_empty()
    );
    assert!(world.notices.warrant_against(&actor("tamrd")).is_none());
}

// ---------------------------------------------------------------- the stations

/// M4b: **confinement is a person, not a door**, and which person is a matter of
/// distance. One law site for an 840×700 m city means a three-minute march at
/// 1.8 m/s; eight postings mean fifty to a hundred and fifty metres, and the
/// escort stays a scene rather than a commute. The Stone House is excluded on
/// purpose — a grave matter committed there is a decision, not a distance.
#[test]
fn the_station_picker_takes_the_nearest_posting_and_never_the_stone_house() {
    let nav = law_nav();
    let places = law_places(&nav);

    let near_toll = custody::nearest_station(&places, Vec3::new(1.0, 0.0, 0.0))
        .expect("a posting is in reach of the street");
    assert_eq!(near_toll.name, "Tallage toll-house");
    assert!(!near_toll.stone_house);
    let near_gate = custody::nearest_station(&places, Vec3::new(45.0, 0.0, 0.0)).unwrap();
    assert_eq!(near_gate.name, "The River Gate", "nearest, not first-listed");

    // Standing on the Stone House's own doorstep still sends you up the street.
    let underfoot = custody::nearest_station(&places, Vec3::ZERO).unwrap();
    assert_eq!(underfoot.name, "Tallage toll-house");
    // It is reachable by name for the decision that names it (M5), and it holds
    // longer than a gate arch — the waiting there is the content, not the price.
    let stone = custody::stone_house(&places).expect("the name resolves");
    assert!(stone.stone_house);
    assert!(stone.hold_seconds() > underfoot.hold_seconds());

    // A city with no postings at all resolves to nothing rather than defaulting.
    assert!(custody::nearest_station(&PlaceRegistry::default(), Vec3::ZERO).is_none());

    // The seizure resolves the same way, and sets the escort's own feet: the
    // officer walks to the station like anybody with an errand.
    let mut world = law_world();
    raise_word(&mut world, "srgnt", "tamrd");
    say_and_seize(&mut world, "srgnt", "tamrd").expect("taken in charge");
    let record = world.custody.get(&actor("tamrd")).unwrap();
    assert_eq!(record.station.name, "Tallage toll-house");
    assert_eq!(record.state, Confinement::InCharge);
    assert!(
        record.holders.is_empty(),
        "seize takes nobody by the arm - that is what `grab` is for"
    );
    match &world.characters[&actor("srgnt")]
        .state
        .intent
        .as_ref()
        .expect("the officer set off for the station")
        .target
    {
        IntentTarget::Place { name, .. } => assert_eq!(name, "Tallage toll-house"),
        other => panic!("the escort's errand is a place: {other:?}"),
    }
}

// ------------------------------------------------------------------ the escort

/// M4b′ is cheap for exactly one reason: the sim is already the authoritative
/// mover for the cast, so an NPC in charge is simply **slaved to their escort**
/// — no clamp, no host message, no `controller.rs`, and both parties walk at
/// `WALK_SPEED_MPS`, so there is no speed disparity to engineer around. The
/// whole chain here — a word raised, a seizure, the walk, the commitment — runs
/// with the player absent from the world entirely.
#[test]
fn a_held_actor_follows_their_escort_and_is_committed_at_the_station() {
    let mut world = law_world();
    assert!(world.player_id().is_none(), "no host, no player, no tether");
    raise_word(&mut world, "srgnt", "tamrd");
    say_and_seize(&mut world, "srgnt", "tamrd").expect("taken in charge");

    // A pace behind the shoulder, on the officer's own heading (yaw 0 looks
    // toward -Z, so behind is +Z), and with no walk of their own left: the mover
    // must not fight the escort for the same position.
    let step = custody::follow_escorts(&mut world, 1.0);
    assert_eq!(step.moved, [actor("tamrd")]);
    assert!(step.committed.is_empty(), "the station is ten metres off yet");
    let officer_at = world.characters[&actor("srgnt")].position_m();
    let shoulder = world.characters[&actor("tamrd")].position_m();
    assert!((shoulder.x - officer_at.x).abs() < 1e-9, "{shoulder:?}");
    assert!(
        (shoulder.z - (officer_at.z + CUSTODY_ESCORT_CONTACT_M)).abs() < 1e-9,
        "{shoulder:?}"
    );
    assert!(world.characters[&actor("tamrd")].state.movement.is_none());
    assert!(
        !world.custody.is_confined(&actor("tamrd")),
        "the escort is the content; arriving is the end of it"
    );

    // The officer walks the hundred metres (here, one step of it) and the walk
    // ends by itself: inside the arrival radius the keeper's threshold begins.
    move_to(&mut world, "srgnt", 10.0);
    let arrival = custody::follow_escorts(&mut world, 9.0);
    assert_eq!(arrival.moved, [actor("tamrd")]);
    assert_eq!(
        arrival.committed,
        [(actor("tamrd"), Vec::new())],
        "arriving is reported, with the hands it took off, so the engine can say the escort ended"
    );
    let record = world.custody.get(&actor("tamrd")).unwrap();
    assert_eq!(record.state, Confinement::Committed);
    assert_eq!(record.committed_at, Some(9.0));
    assert!(
        record.holders.is_empty(),
        "committed is held by a person at a threshold, not by a grip"
    );
    assert!(
        world.characters[&actor("tamrd")]
            .position_m()
            .distance(record.station.point)
            <= STATION_ARRIVE_RADIUS_M
    );

    // A committed prisoner is not walked anywhere by anybody, and arrives only
    // once — the escort's own feet may wander off afterwards.
    let idle = custody::follow_escorts(&mut world, 10.0);
    assert!(idle.moved.is_empty() && idle.committed.is_empty());
    move_to(&mut world, "srgnt", 30.0);
    let wandered = custody::follow_escorts(&mut world, 11.0);
    assert!(wandered.moved.is_empty() && wandered.committed.is_empty());

    // And the keeper's release is the same one verb the escort had all along.
    apply_action(
        &mut world,
        &actor("wrdn"),
        "release",
        &json!({"person": "tamrd"}),
    )
    .expect("the law standing over them may let them go");
    assert!(!world.custody.holds(&actor("tamrd")));
}

// --------------------------------------------------------------- the Stone House

/// M5c: **the cell is a decision, never a distance.** `nearest_station` skips
/// the Stone House on purpose, so the gaol is chosen one level up, by what the
/// word says rather than by where the arrest happened: a warrant is the top of
/// the ladder — a summons named a bell and the bell rang on a word still
/// standing — and that is what the lore means by a grave matter.
///
/// Everything below it goes to the nearest posting, which is M2's whole
/// argument, and a drive-mode poke with no notice at all keeps doing so.
#[test]
fn a_warrant_goes_to_the_gaol_and_every_lesser_word_to_the_nearest_arch() {
    // Door two — this officer's own fresh word, no warrant. The wrong is real
    // and the seizure is legal, and it still is not gaol business.
    let mut world = law_world();
    move_to(&mut world, "tamrd", 38.0);
    move_to(&mut world, "srgnt", 38.0);
    raise_word(&mut world, "srgnt", "tamrd");
    world.current_time = Some(at(60.0));
    say_and_seize(&mut world, "srgnt", "tamrd").expect("your own eyes, within the hour");
    let record = world.custody.get(&actor("tamrd")).unwrap();
    assert!(!record.station.stone_house, "a lesser word is not gaol business");
    assert_eq!(record.station.name, "The River Gate", "the nearest arch to x 38");

    // Door one — the warrant. Same street, same officer, same four metres; the
    // only thing that changed is what the ward is saying, and it is the gaol.
    let mut world = law_world();
    move_to(&mut world, "tamrd", 38.0);
    move_to(&mut world, "srgnt", 38.0);
    let id = raise_word(&mut world, "wrdn", "tamrd");
    apply_action(&mut world, &actor("wrdn"), "summon", &json!({"notice_id": id}))
        .expect("the law may summon on its own word");
    world.current_time = Some(at(500.0));
    assert_eq!(world.notices.issue_warrants(at(500.0).game_days()), [id]);
    say_and_seize(&mut world, "srgnt", "tamrd").expect("a warrant lets any of the law take them");
    let record = world.custody.get(&actor("tamrd")).unwrap();
    assert!(record.station.stone_house, "a warrant is a grave matter");
    assert_eq!(record.station.name, custody::STONE_HOUSE_PLACE_NAME);
    assert_eq!(record.notice_id, Some(id));
}

/// M5d: *"broadcast is the gaol's native register."* A `say` with no target
/// reaches everybody within [`cathedral_sim::HEARING_RADIUS_M`], and the room is
/// several times smaller than that, so one line from somebody at the grate gives
/// the whole cell news at once and the stage picks who answers. This is why six
/// on stage out of nine is not a shortfall — and why a visitor needs no
/// mechanism of its own, only a wall with a window in it.
#[test]
fn a_visitor_at_the_grate_is_heard_by_the_whole_cell() {
    let mut world = law_world();
    for (id, name) in [("in1", "Lise Skell"), ("in2", "Aldith Hobbe"), ("in3", "Sible Rud")] {
        world.add_character(person(id, name, 1.5, None));
        world.custody.seed_inmate(actor(id), stone_house());
    }
    // The visitor stands outside the wall — the grate is a window, and the
    // collider behind it is one unbroken piece, so this is as close as kin get.
    move_to(&mut world, "gossp", 2.5);
    apply_action(
        &mut world,
        &actor("gossp"),
        "say",
        &json!({"text": "I have brought bread and a blanket."}),
    )
    .expect("anybody may speak at the grate");

    for id in ["in1", "in2", "in3"] {
        let heard = world.characters[&actor(id)]
            .inbox()
            .iter()
            .any(|line| line.contains("bread and a blanket"));
        assert!(heard, "{id} heard the visitor: one line, the whole room");
    }
}

/// M5d, and the one the review pass caught: a townsman arrested mid-errand kept
/// a live intent, was re-routed, and `World::step_movement` walked him out
/// through the doorway — where the engine's roam check read a stray it did not
/// cause, released him, and branded him with the one word no restitution can
/// answer. A person who never chose to leave, and who *could not have* —
/// `go_to` is refused while held — wanted for the rest of the run for it.
///
/// The round's `set_route` now refuses a prisoner outright, so the re-laying
/// movers cannot fight this; what no guard reaches is the walk that *already
/// existed* at the seizure. So a committed body has no errand and no path,
/// re-asserted every tick rather than cleared once, because a stale intent can
/// land between any two polls and one missed clear is a branding.
#[test]
fn a_committed_body_keeps_no_errand_and_no_path_for_a_mover_to_follow() {
    let mut world = law_world();
    world.custody.seed_inmate(actor("tamrd"), stone_house());
    world.custody.commit(&actor("tamrd"), 0.0);

    // Exactly what an arrest mid-errand leaves behind: `take_into_charge` never
    // touched the intent, and some other mover has just re-laid the route.
    {
        let character = world.characters.get_mut(&actor("tamrd")).unwrap();
        character.state.intent = Some(cathedral_sim::TravelIntent {
            // The follow, which is the one that re-lays: `apply_intents` re-routes
            // to a *visible* person every tick they move.
            target: IntentTarget::Person {
                actor_id: actor("gossp"),
                last_seen: Vec3::new(40.0, 0.0, 0.0),
                visible: true,
            },
            budget_seconds: 600.0,
            deadline: Some(600.0),
        });
        character.state.movement = Some(cathedral_sim::Movement {
            path: vec![Vec3::new(40.0, 0.0, 0.0)],
            speed: cathedral_sim::WALK_SPEED_MPS,
            gait_phase: 0.0,
            patrol: None,
            choke_wait: 0.0,
        });
    }

    custody::follow_escorts(&mut world, 1.0);

    let character = &world.characters[&actor("tamrd")];
    assert!(
        character.state.intent.is_none(),
        "the errand goes, or the next tick re-lays it"
    );
    assert!(
        character.state.movement.is_none(),
        "and the path with it, or `step_movement` walks them through the wall"
    );
    assert!(
        world.custody.is_confined(&actor("tamrd")),
        "and none of that let them out"
    );
}

/// M5d: the two confinement guards make the walking-out branch unreachable for
/// the cast — rung 0 of `round::decide` and the `go_to` refusal between them
/// mean an NPC never takes a step of their own while held. So the branch cannot
/// misfire on an inmate who simply happens to be standing near a wall, which is
/// the failure mode a distance test invites. Escape is the player's door.
#[test]
fn a_confined_inmate_never_walks_out_by_standing_still() {
    let mut world = law_world();
    world.custody.seed_inmate(actor("tamrd"), stone_house());
    // At the far corner of the room, well past `STATION_ARRIVE_RADIUS_M` and
    // still inside the roam — this is exactly why the roam is the leash and not
    // the arrival radius.
    move_to(&mut world, "tamrd", 5.4);
    let at = world.characters[&actor("tamrd")].position_m();
    assert!(
        f64::hypot(at.x, at.z) > custody::STATION_ARRIVE_RADIUS_M,
        "sitting against the back wall is further than arriving"
    );
    assert!(f64::hypot(at.x, at.z) <= custody::COMMITTED_ROAM_M, "and is not leaving");
}

// ---------------------------------------------------------- the departed escort

/// M4: an escort who leaves the city cannot walk anybody anywhere, so their
/// prisoner is simply free — the same answer the dead-man timer gives, for the
/// same reason — and they are **told** so, with the reason. The engine used to
/// call `Custody::forget` and discard the freed list: no percept, no reason, a
/// hold that ended in silence and so looked exactly like one that did not,
/// while every clock-driven release in `tick_custody` says what ended and why.
#[test]
fn a_departing_escort_frees_their_prisoner_audibly_and_with_the_reason() {
    let mut world = law_world();
    world
        .custody
        .seize(actor("tamrd"), actor("srgnt"), Some(1), toll_house(), 0.0);
    // A second hand is still on the arm when the officer of record leaves: the
    // custody dissolves whole, and that grip must come off audibly too.
    world.custody.grab(&actor("tamrd"), actor("wrdn"));

    custody::forget_departed(&mut world, &actor("srgnt"));

    assert!(
        !world.custody.holds(&actor("tamrd")),
        "an escort beyond the walls holds nobody"
    );
    let inbox = world.characters[&actor("tamrd")].inbox();
    assert!(
        inbox.iter().any(|line| {
            line.contains("out of the law's hands")
                && line.contains("Tallage toll-house")
                && line.contains("left the city")
        }),
        "the freed learn they are free, and why: {inbox:?}"
    );
    // The grip's end is announced to whoever is standing there — the same
    // witness line every other release path emits (`announce_grip` says it in
    // the third person to the street and in the second person to the prisoner,
    // whose *reason* is the separate line above).
    let overheard = world.characters[&actor("wrdn")].inbox();
    assert!(
        overheard.iter().any(|line| line.contains("lets go of")),
        "the lingering hand comes off audibly: {overheard:?}"
    );

    // A committed prisoner keeps their cell when the officer who brought them
    // departs — the keeper holds the threshold, not the escort — so the walk-out
    // path stays what frees them, never a departure they had no part in.
    world
        .custody
        .seize(actor("gossp"), actor("wrdn"), Some(2), toll_house(), 1.0);
    world.custody.commit(&actor("gossp"), 2.0);
    custody::forget_departed(&mut world, &actor("wrdn"));
    assert!(
        world.custody.is_confined(&actor("gossp")),
        "commitment survives its officer's departure"
    );
    assert!(
        !world.characters[&actor("gossp")]
            .inbox()
            .iter()
            .any(|line| line.contains("out of the law's hands")),
        "nobody is told they are free who is not"
    );
}
