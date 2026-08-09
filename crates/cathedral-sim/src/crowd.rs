//! The crowd knob: `extra_ambient_npcs` generated citizens on top of the cast.
//!
//! The shipped roster is hand-authored, deliberately dispersed, and canon
//! (`lore/characters/AGENTS.md`). Nothing here is any of those things. This
//! module exists to answer one question — *what does the city do with a lot
//! more people in it* — and it answers it by minting bodies that are ordinary
//! in every way the sim can observe (ambient significance, a ward, a walk, a
//! purse, and for three in four of them a trade) and authored in none.
//!
//! The fourth is [`NO_TRADE_SHARE`]: no occupation at all, and a support
//! circumstance instead — the `no_fixed_trade/` shape ten authored sheets
//! already carry. A crowd where everybody has somewhere to be at eight in the
//! morning is a traffic jam; a quarter of it standing about is a population.
//!
//! Two rules keep the two populations from blurring:
//!
//! 1. **Ids cannot collide.** A lore id is exactly five characters
//!    ([`crate::lore::LoreCharacterSheet::validate`]); every id minted here is
//!    six — `x00000`, `x00001`, … — so a generated person can never shadow an
//!    authored one, in a save, a prompt or a `seize` handle.
//! 2. **Generated people hold no authored post.** [`LoreProfile::generated`]
//!    is the flag, and the round reads it where it would otherwise hand a
//!    civic job to whichever ambient body happened to be nearest (the well
//!    curbs). A crowd should fill the streets, not quietly take the cast's
//!    work off them.
//!
//! Everything is a pure function of the index, so the same `count` produces
//! the same city twice, and no `rand` crate crosses the sim's IO-free line.

use crate::{
    appearance::AppearanceSnapshot,
    character::{CharacterSheet, Control, EconomicClass, Presence},
    ids::ActorId,
    lore::{LoreProfile, PlanningWard, Significance, default_voice_key},
    math::Vec3,
    nav::NavData,
};

/// The ceiling on `config.ron: smart_actors.extra_ambient_npcs`. Not a
/// performance promise — it is the point past which the number stops being an
/// experiment and starts being a typo.
pub const MAX_EXTRA_AMBIENT_NPCS: u32 = 20_000;

/// How far off their nav node a generated citizen may stand.
///
/// The graph is the welded *through-route* skeleton: 3,972 nodes over a
/// walkable bitset of 322,670 m², i.e. 81 m² of ground per node. Standing the
/// crowd within a lane's half-width of a node therefore stood all of it in the
/// middle of the roads everybody routes down, which is what a traffic jam is.
/// A 12 m disc is 452 m² against that 81, so consecutive discs overlap and the
/// coverage is continuous — the yards, the widenings, the alley dead ends and
/// the strips beside the walls all fall inside one.
///
/// Anchored to a node rather than sampled uniformly over the bitset, and that
/// is the load-bearing part: [`NavData::route_between`] snaps both endpoints to
/// the *nearest node*, so a body dropped in a walkable pocket the graph never
/// enters would walk through a wall on its first errand. Twelve metres bounds
/// the offset to ground the graph plausibly reaches and leaves the sealed
/// pockets empty, which is the right answer rather than a limitation.
const SPREAD_RADIUS_M: f64 = 12.0;

/// The share of the crowd that holds no trade at all: `occupation_id`, `title`
/// and `rank` all null, and a support circumstance in their place.
///
/// A quarter, because the whole crowd having somewhere to be at eight in the
/// morning is precisely what made it a traffic jam. This cohort is the city's
/// standing population — the people the street is *lived in* by rather than
/// walked through — and the sim needs no new code to run them: with no
/// occupation `round.rs build_legs` finds no archetype, returns no legs, and the
/// ladder falls through to the social-pull and wander rungs around the person's
/// base. That is "hanging out", already written.
///
/// Not a special case, either: it is the `no_fixed_trade/` shape ten authored
/// sheets already carry, down to the pairing
/// [`crate::lore::LoreCharacterSheet::validate`] demands of them.
const NO_TRADE_SHARE: f64 = 0.25;

/// Age bands. The trade branch keeps the original 16..=65 exactly; the rest are
/// the youngest a given way of living is not absurd at.
const WORKING_AGE_FROM: u16 = 16;
const WIDOWED_AGE_FROM: u16 = 34;
const RETIRED_AGE_FROM: u16 = 58;
const AGE_UNTIL: u16 = 66;

/// Draws inside that disc before a citizen simply stands on their node. Eight
/// is enough that 1.4% of a crowd falls back (measured over 2,000 against the
/// shipped graph), and each one is a single bitset lookup — but the whole
/// thing runs once, at seed time. It must never migrate into the pump, which
/// at 20,000 is already 179 ms of a 204 ms frame.
const SPREAD_ATTEMPTS: usize = 8;

/// Walkable spawn points for a crowd of `count`, spread over the whole graph.
///
/// The graph has ~4,000 nodes, so a full crowd stacks several people on each.
/// Consecutive indices are pushed apart by striding the node list with a
/// coprime step rather than walking it in order: filling nodes 0..n in
/// sequence would lay the first thousand citizens down in one quarter of the
/// city and leave the rest of it empty until the count got high enough. The
/// stride picks *which* 12 m of city each citizen belongs to;
/// [`spread_point`] picks where in it they stand.
pub fn spread_over_walkable(nav: &NavData, count: usize) -> Vec<Vec3> {
    let nodes = nav.node_count();
    if nodes == 0 || count == 0 {
        return Vec::new();
    }
    let stride = coprime_stride(nodes);
    let mut points = Vec::with_capacity(count);
    for index in 0..count {
        let node = index.wrapping_mul(stride) % nodes;
        points.push(spread_point(nav, index as u64, nav.node_point(node)));
    }
    points
}

/// One citizen's stand, within [`SPREAD_RADIUS_M`] of their anchor node.
///
/// The rejection sample `round.rs wander_target` already uses for the same
/// problem, with the same shape: polar so the offsets do not favour the
/// diagonals, the radius through a square root so they do not favour the
/// centre either, and the first draw the bitset accepts wins. Each attempt is
/// salted apart, so eight attempts are eight different points rather than one
/// point tested eight times. Returns the node itself — where the whole crowd
/// stood before this — when the disc is all wall.
fn spread_point(nav: &NavData, seed: u64, centre: Vec3) -> Vec3 {
    for attempt in 0..SPREAD_ATTEMPTS as u64 {
        let angle = unit(seed, 0x9E37_79B9_u64.wrapping_add(attempt)) * std::f64::consts::TAU;
        let radius = unit(seed, 0x85EB_CA6B_u64.wrapping_add(attempt)).sqrt() * SPREAD_RADIUS_M;
        let x = centre.x + radius * angle.cos();
        let z = centre.z + radius * angle.sin();
        if nav.is_walkable(x, z) {
            return Vec3::new(x, centre.y, z);
        }
    }
    centre
}

/// A step that visits every node exactly once per pass around the list. Odd
/// and near the golden ratio of `nodes`, then walked down until it shares no
/// factor with `nodes` — which it must, or the stride would cycle early and
/// pile the whole crowd onto a fraction of the graph.
fn coprime_stride(nodes: usize) -> usize {
    let mut stride = ((nodes as f64 * 0.618_033_988_75) as usize).max(1) | 1;
    while gcd(stride, nodes) != 1 {
        stride += 2;
        if stride >= nodes {
            return 1;
        }
    }
    stride
}

fn gcd(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

/// Mint one ambient citizen per point, in point order.
///
/// The caller owns the count (it is `points.len()`) and the placement; this
/// owns who each of them turns out to be. Ids run `x00000` upward from
/// `first_index`, so a host that generates in one pass gets a contiguous
/// block and a test can mint two people without minting nineteen thousand.
pub fn extra_ambient_sheets(points: &[Vec3], first_index: u32) -> Vec<CharacterSheet> {
    points
        .iter()
        .enumerate()
        .map(|(offset, point)| ambient_sheet(first_index + offset as u32, *point))
        .collect()
}

/// What a generated citizen lives on. Two shapes, because the authored roster
/// has exactly two: a trade under its own `lore/characters/<occupation>/`
/// folder, or the `no_fixed_trade/` shape — every trade field null and a
/// support circumstance in their place.
#[derive(Clone, Copy)]
enum Living {
    Trade(&'static Trade),
    NoTrade(&'static Support),
}

fn ambient_sheet(index: u32, position: Vec3) -> CharacterSheet {
    let seed = u64::from(index);
    let id = ActorId::from_raw(format!("x{index:05}"));
    let female = hash(seed, 0x5EED_0001).is_multiple_of(2);
    let gender = if female { "f" } else { "m" };
    let name = format!(
        "{} {}",
        pick(if female { WOMEN } else { MEN }, seed, 0x5EED_0002),
        pick(BYNAMES, seed, 0x5EED_0003)
    );
    let ward = PlanningWard::ALL[(hash(seed, 0x5EED_0005) % 8) as usize];
    let district = format!("{} streets", ward_name(ward));

    // The one branch in this file. Everything downstream of it — the round, the
    // outfit, the curiosity, the prompt — already knows what to do with a
    // person who has no trade, because the authored cast contains ten of them.
    let living = if unit(seed, 0x5EED_000C) < NO_TRADE_SHARE {
        Living::NoTrade(pick(SUPPORTS, seed, 0x5EED_000D))
    } else {
        Living::Trade(pick(TRADES, seed, 0x5EED_0004))
    };
    let (occupation_id, occupation_display, title) = match living {
        Living::Trade((occupation_id, occupation_display, title)) => (
            Some(occupation_id.to_string()),
            Some(occupation_display.to_string()),
            Some(title.to_string()),
        ),
        Living::NoTrade(_) => (None, None, None),
    };
    // `widow` and `widower` are one circumstance with two spellings, and the
    // bank writes the first; a man carries the other.
    let circumstances: Vec<String> = match living {
        Living::Trade(_) => Vec::new(),
        Living::NoTrade((circumstances, _, _)) => circumstances
            .iter()
            .map(|circumstance| match (*circumstance, female) {
                ("widow", false) => "widower".to_string(),
                _ => (*circumstance).to_string(),
            })
            .collect(),
    };
    // A way of living carries its own youngest plausible age: nobody is retired
    // at sixteen, and nobody is widowed much younger than thirty here. The
    // trade branch keeps the original 16..=65, so a tradesman at index *n* is
    // the same person they were before this milestone.
    let age_from = match living {
        Living::Trade(_) => WORKING_AGE_FROM,
        Living::NoTrade((_, age_from, _)) => *age_from,
    };
    let age = age_from + (hash(seed, 0x5EED_0006) % u64::from(AGE_UNTIL - age_from)) as u16;

    let (concern, goal) = match living {
        Living::Trade(_) => *pick(CONCERNS, seed, 0x5EED_0007),
        Living::NoTrade(_) => *pick(NO_TRADE_CONCERNS, seed, 0x5EED_000F),
    };
    let manner = pick(MANNER, seed, 0x5EED_0009);
    let feature = pick(FEATURES, seed, 0x5EED_000A);
    // Two templates, because "You are a {title} in {district}" means nothing
    // without a title, and the second of the five ambient authoring questions
    // (`lore/characters/AGENTS.md`: *how do you materially survive?*) is the one
    // the no-fixed-trade shape is required to answer.
    let description = match living {
        Living::Trade(_) => format!(
            "You are a {} in {district}. {}. {manner}. Today {concern}. {feature}.",
            title.as_deref().unwrap_or_default().to_lowercase(),
            pick(SURVIVAL, seed, 0x5EED_0008),
        ),
        Living::NoTrade((_, _, support)) => format!(
            "{} in {district}. {support}. {manner}. Today {concern}. {feature}.",
            pick(NO_TRADE_OPENINGS, seed, 0x5EED_000E),
        ),
    };

    CharacterSheet {
        appearance: AppearanceSnapshot::compose(
            &id,
            gender,
            occupation_id.as_deref(),
            None,
            &circumstances,
        ),
        voice_key: Some(default_voice_key(gender, &id)),
        control: Control::Llm,
        back_story: description.clone(),
        location_description: district.clone(),
        position_m: position,
        facing_yaw: unit(seed, 0x5EED_000B) * std::f64::consts::TAU,
        holds: Vec::new(),
        pockets: Vec::new(),
        frontbutt: None,
        goal: goal.to_string(),
        memories: Vec::new(),
        knows: Default::default(),
        lore: Some(LoreProfile {
            significance: Significance::Ambient,
            planning_ward: ward,
            age,
            gender: gender.to_string(),
            occupation_id,
            occupation_display,
            title,
            rank: None,
            faction_role: None,
            illegal_activity: None,
            district,
            father: None,
            mother: None,
            children: Vec::new(),
            circumstances,
            conditions: Vec::new(),
            // No bed: `homes.json` is baked per authored id, and the round
            // already carries ~100 bedless people through a night without one.
            home: None,
            core_character_description: description,
            extended_character_description: String::new(),
            // Derived, like the whole authored cast's. A generated citizen is
            // an ordinary citizen — the crowd is the experiment, not a second
            // set of rules about who speaks first.
            curiosity: None,
            generated: true,
        }),
        presence: Presence::InCity,
        presence_epoch: 0,
        economic_class: EconomicClass::Resident,
        id,
        name,
    }
}

fn ward_name(ward: PlanningWard) -> &'static str {
    match ward {
        PlanningWard::Fabric => "Fabric Ward",
        PlanningWard::Wick => "Wick Ward",
        PlanningWard::Cloth => "Cloth Ward",
        PlanningWard::Wallwright => "Wallwright Ward",
        PlanningWard::Cinder => "Cinder Ward",
        PlanningWard::Weigh => "Weigh Ward",
        PlanningWard::Reed => "Reed Ward",
        PlanningWard::BellAndSluice => "Bell and Sluice Ward",
    }
}

// --------------------------------------------------------------------------- //
// Determinism
// --------------------------------------------------------------------------- //
/// SplitMix64's finalizer over the index and a salt. The sim owns no clock and
/// no RNG, so every choice above is a hash: the same count builds the same
/// city, run after run, save after save.
///
/// The mixing matters more than it looks. Every `pick` below takes the value
/// *modulo* a bank length, i.e. reads the low bits — and a plain FNV-1a, which
/// this started as, avalanches its low bits so poorly that five hundred
/// citizens drew only 171 distinct names out of a bank of 1,440. That is
/// exactly the "wall of identical strangers" a crowd feature exists to avoid.
fn hash(index: u64, salt: u64) -> u64 {
    let mut value = index
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ salt.wrapping_mul(0xD6E8_FEB8_6659_FD93);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

fn unit(index: u64, salt: u64) -> f64 {
    (hash(index, salt) % 1_000_003) as f64 / 1_000_003.0
}

fn pick<T>(items: &'static [T], index: u64, salt: u64) -> &'static T {
    &items[(hash(index, salt) % items.len() as u64) as usize]
}

// --------------------------------------------------------------------------- //
// The banks
// --------------------------------------------------------------------------- //
// Given names and bynames are the approved banks and constructions from
// `lore/core_lore/naming_language.md`. Repeats across a crowd of twenty
// thousand are not a defect — the Reed Ward's own saying is that there are
// more Hawsers than Alders.
const MEN: &[&str] = &[
    "Aubin", "Colm", "Corin", "Dunstan", "Grigor", "Hamel", "Jos", "Noll", "Segwin", "Ansel",
    "Bertran", "Ewart", "Gile", "Renn", "Tobin", "Pin", "Cobb", "Dob", "Sef", "Ib",
];
const WOMEN: &[&str] = &[
    "Aldith", "Betriss", "Ede", "Havise", "Idonea", "Jonet", "Lise", "Osanne", "Averil",
    "Clemence", "Petronel", "Rohese", "Sibbe", "Nan", "Tib", "Mote",
];
const BYNAMES: &[&str] = &[
    "Sparr",
    "Copp",
    "Stott",
    "Dorn",
    "Alder",
    "Ashe",
    "Marle",
    "Fitch",
    "Pike",
    "Brant",
    "Vell",
    "Rasp",
    "Ferrant",
    "Crake",
    "Hobbe",
    "Skell",
    "Tarn",
    "Salter",
    "Roper",
    "Hawser",
    "Quern",
    "Threefinger",
    "Wetalms",
    "Scaffold",
    "Upstream",
    "the Long",
    "the Lesser",
    "Shortmeasure",
    "Coldhands",
    "Nineteen",
    "Latchkey",
    "of the Needle",
    "of the Bellstand",
    "of the Slip",
    "of the Shambles",
    "of Ostrelle",
    "of the Low Bridge",
    "of Cinder Row",
    "of the Tenters",
    "of the Gutter Steps",
];

/// Street trades, with the `occupation_id` the appearance and the daily round
/// join on, the family's display name, and one registered title from
/// `lore/core_lore/occupations.json`. Deliberately the *ordinary* families —
/// nothing here carries an office, a faction or a guild rank.
type Trade = (&'static str, &'static str, &'static str);
const TRADES: &[Trade] = &[
    ("cargo_worker", "Cargo worker", "Porter"),
    ("general_labourer", "General labourer", "Day labourer"),
    ("cloth_worker", "Cloth worker", "Fuller"),
    ("garment_worker", "Garment worker", "Mender"),
    ("laundress", "Laundress", "Washerwoman"),
    ("market_seller", "Market seller", "Street seller"),
    ("food_provisioner", "Food provisioner", "Milk seller"),
    ("fish_trader", "Fish trader", "Fish seller"),
    ("boatworker", "Boatworker", "Waterman"),
    ("scavenger", "Scavenger", "Rag-picker"),
    ("sanitation_worker", "Sanitation worker", "Gutter raker"),
    ("domestic_servant", "Domestic servant", "Chamber servant"),
    ("animal_worker", "Animal worker", "Stable hand"),
    ("leather_worker", "Leather worker", "Hide carrier"),
    ("mason", "Mason", "Quarry worker"),
    ("carpenter_and_builder", "Carpenter and builder", "Scaffold worker"),
    ("cooper", "Cooper", "Hoop-setter"),
    ("potter", "Potter", "Kiln worker"),
    ("shoemaker", "Shoemaker", "Shoe mender"),
    ("chandler", "Chandler", "Candle-dipper"),
    ("roper", "Roper", "Rope-walker"),
    ("baker", "Baker", "Oven keeper"),
    ("cook", "Cook", "Kitchen worker"),
    ("tavern_worker", "Tavern worker", "Pot-boy"),
    ("brewer", "Brewer", "Brewhouse worker"),
    ("grocer_and_spicer", "Grocer and spicer", "Oil seller"),
    ("messenger", "Messenger", "Message-runner"),
    ("guide", "Guide", "Street guide"),
    ("farmer", "Farmer", "Haymaker"),
    ("pilgrim", "Pilgrim", "Pilgrim"),
    ("entertainer", "Entertainer", "Storyteller"),
    ("healer", "Healer", "Herb-seller"),
];

/// How somebody with **no trade at all** stays alive: the loader's own
/// vocabulary for it, the youngest age it makes sense at, and the same fact
/// said to the citizen in the second person.
///
/// The circumstances and the prose are one entry rather than two banks drawn
/// independently, because a citizen whose sheet says `alms_dependent` and whose
/// description says they live off piece-work is a citizen who cannot answer
/// "how do you eat?" twice the same way — the drift the risk ledger of
/// `features/give_the_crowd_somewhere_to_be.md` names.
///
/// Every entry carries at least one of [`crate::lore::SUPPORT_CIRCUMSTANCES`],
/// which is what `validate` requires of a no-trade sheet and what the test
/// below re-checks against that same list. `prisoner` is deliberately absent:
/// it is not a way of living, it is a cell, and `custody.rs` seeds anybody who
/// carries it into the Stone House.
type Support = (&'static [&'static str], u16, &'static str);
const SUPPORTS: &[Support] = &[
    (
        &["pauper", "alms_dependent"],
        WORKING_AGE_FROM,
        "The dole at the church door and the ends of other people's loaves are what you eat",
    ),
    (
        &["pauper", "alms_dependent", "begs_regularly"],
        WORKING_AGE_FROM,
        "You ask, plainly and often, wherever the crowd slows down enough to hear you",
    ),
    (
        &["pauper", "alms_dependent", "begs_regularly", "unhoused"],
        WORKING_AGE_FROM,
        "You sleep under whatever overhang is dry and eat what is handed down or left behind",
    ),
    (
        &["pauper", "intermittently_employed"],
        WORKING_AGE_FROM,
        "Two days of carrying in a week is the whole of your income, and there is no third day",
    ),
    (
        &["pauper", "intermittently_employed", "insecure_lodging"],
        WORKING_AGE_FROM,
        "A bed in a crowded room, paid for a night at a time out of whatever the week turns up",
    ),
    (
        &["pauper", "dependent"],
        WORKING_AGE_FROM,
        "You eat from a household pot that is not your own and earn only an occasional penny",
    ),
    (
        &["pauper", "alms_dependent", "dependent"],
        WORKING_AGE_FROM,
        "Kin feed you, not gladly and not every day, and the parish makes up the rest",
    ),
    (
        &["pauper", "unhoused", "intermittently_employed"],
        WORKING_AGE_FROM,
        "You hold horses, carry messages and hold out your hand between the two, and one of the three usually pays",
    ),
    (
        &["pauper", "widow", "alms_dependent"],
        WIDOWED_AGE_FROM,
        "You were widowed, and the trade in the house was buried with the one who held it; the parish and the neighbours make up the difference",
    ),
    (
        &["pauper", "retired", "dependent"],
        RETIRED_AGE_FROM,
        "Your working years are mostly behind you, and what is left is a little put by, a little from kin, and a good deal of going without",
    ),
];

/// The opening the no-trade cohort gets instead of "You are a {title}", which
/// says nothing at all when there is no title. Each reads straight into
/// `in {district}`, and each answers the first ambient authoring question —
/// *what are you doing here?* — with the honest answer: standing about.
const NO_TRADE_OPENINGS: &[&str] = &[
    "You have no fixed trade and pass your days",
    "You are sworn to no trade, and spend the whole of the daylight",
    "No guild has your name, and you are a familiar figure",
    "You have no work to go to, so you keep to the same few doorways",
    "You do no fixed labour, and everybody already knows your face",
    "You have no bench, no stall and no master, and you wait the day out",
    "There is no trade behind your name, and you spend your hours",
    "You keep no trade and no set hours, and are usually to be found",
    "You fell out of the work you had and have found none since, so your day is spent",
    "Nothing is expected of you before dark, and you spend the time",
];

/// The no-trade cohort's own concerns. [`CONCERNS`] presumes a trade in two of
/// its twelve entries — an employer who is late, a strap on your load — and
/// every goal here is textually distinct from every goal there, so the test
/// that counts distinct goals counts both banks.
const NO_TRADE_CONCERNS: &[Concern] = &[
    (
        "you have not eaten since the bread somebody gave you yesterday",
        "Beg or find a meal today",
    ),
    (
        "somebody has taken the corner you sleep in",
        "Get your sleeping corner back",
    ),
    (
        "your blanket is wet through and there is nowhere indoors to dry it",
        "Find somewhere to dry your blanket",
    ),
    (
        "the doorway you stand in belongs to a man who wants you gone",
        "Find a doorway nobody will move you from",
    ),
    (
        "your feet are bare and the stones have turned cold",
        "Find something to put on your feet",
    ),
    (
        "you were promised a penny for holding a horse and never saw it",
        "Get the penny you were promised for the horse",
    ),
    (
        "you want a day's work, any work, before the hiring line thins",
        "Get taken on for a day's work",
    ),
    (
        "the alms bowl you eat out of has gone missing",
        "Replace your lost alms bowl",
    ),
    (
        "your hands are cracked open and you have nothing to put on them",
        "Find a salve for your hands",
    ),
    (
        "you have been moved on twice already this morning",
        "Find somewhere you will be left alone",
    ),
];

const SURVIVAL: &[&str] = &[
    "Piece-work and whatever the morning hiring line offers keep you fed",
    "You are paid in bread and a corner to sleep in, and count that a wage",
    "Your money is small, late, and mostly spent before it arrives",
    "You live off the trade's offcuts and what your own hands can carry",
    "A shared pot in a crowded lodging is what stands between you and hunger",
    "You sell at the edge of somebody else's pitch and pay for the privilege",
    "You work another's bench and take a thin cut of what leaves it",
    "Odd hours at two trades add up to very nearly a living",
    "What the water brings in you carry out, and are paid by the load",
    "Errands, alms and an old employer's guilt keep you upright between them",
];

const MANNER: &[&str] = &[
    "You answer strangers plainly and then get on with what you were doing",
    "You are wary of questions and slow to give your name",
    "You talk too much when you are nervous, and you know it",
    "You are cheerful with anybody who is not wearing the Watch's colours",
    "You speak in short sentences with long gaps between them",
    "You are quick to complain and quicker to lend a hand",
    "You greet everybody the same way and remember none of them",
    "You are polite in the manner of somebody who cannot afford not to be",
    "You laugh first and think about it afterwards",
    "You watch a good deal more than you say",
];

/// The immediate material concern, and the goal it becomes. Both halves are
/// the same fact — the description says it in the second person, the goal in
/// the imperative — so a citizen's prompt and their errand never disagree.
type Concern = (&'static str, &'static str);
const CONCERNS: &[Concern] = &[
    ("your shoes are letting water and the stones are cold", "Get your shoes mended"),
    ("you are owed a penny by somebody who is avoiding the street", "Collect the penny you are owed"),
    ("your blanket got wet and will not dry indoors", "Dry your blanket before dark"),
    ("you must hold your place in the hiring line", "Keep your place in the hiring line"),
    ("your employer is late again and you cannot start without them", "Find your employer"),
    ("you have not eaten since yesterday's bread", "Find something to eat"),
    ("a strap on your load has frayed through", "Get a new strap for your load"),
    ("you need water and the nearer curb had a queue at dawn", "Draw water"),
    ("your knife wants a stone taken to it", "Sharpen your knife"),
    ("somebody has moved your sleeping place", "Find somewhere to sleep tonight"),
    ("your hands are cracked and the salve costs more than you have", "Get something for your hands"),
    ("you owe the lodging keeper for two nights", "Pay what you owe for your lodging"),
];

const FEATURES: &[&str] = &[
    "Your sleeves are pinned back with a strip of sail-cloth",
    "There is a pale scar across one knuckle where a tool slipped",
    "You carry your load on the left because the right shoulder went years ago",
    "Your hem is caked to the knee with the same grey mud as everyone else's",
    "One ear is notched, and you do not explain it",
    "You keep a length of cord wound twice around your wrist",
    "Your hair is cropped short against the lice",
    "You wear somebody larger's coat, taken in badly at the seams",
    "There is charcoal under your nails that will not wash out",
    "You walk with a short step, favouring one foot",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GOAL_NONE;
    use crate::lore::{CONTROLLED_CIRCUMSTANCES, SUPPORT_CIRCUMSTANCES};
    use std::collections::BTreeSet;

    const NAV_JSON: &str = include_str!("../../../assets/world/navigation.json");
    const NAV_BIN: &[u8] = include_bytes!("../../../assets/world/navigation.bin");

    /// The shipped graph, loaded exactly as the host loads it. The spread is
    /// only as good as the ground it is tested against: a hand-built nav is an
    /// open plain, where every draw lands and every claim below is free.
    fn shipped_nav() -> NavData {
        NavData::from_parts(NAV_JSON, NAV_BIN).expect("the committed nav loads")
    }

    fn points(count: usize) -> Vec<Vec3> {
        (0..count)
            .map(|i| Vec3::new(i as f64, 0.91, 0.0))
            .collect()
    }

    #[test]
    fn ids_are_six_characters_and_cannot_shadow_a_lore_id() {
        let sheets = extra_ambient_sheets(&points(3), 0);
        let ids: Vec<&str> = sheets.iter().map(|sheet| sheet.id.as_str()).collect();
        assert_eq!(ids, ["x00000", "x00001", "x00002"]);
        // Lore ids are exactly five characters, so no generated id can collide
        // with one — nor with the six-character "player", which starts 'p'.
        for sheet in &sheets {
            assert_eq!(sheet.id.as_str().chars().count(), 6);
            assert!(sheet.id.is_valid());
            assert_ne!(sheet.id.as_str(), "player");
        }
    }

    #[test]
    fn the_same_index_is_always_the_same_person() {
        let once = extra_ambient_sheets(&points(64), 0);
        let twice = extra_ambient_sheets(&points(64), 0);
        assert_eq!(once, twice);
        // …and the block is a function of the index, not of the batch: minting
        // two people starting at 40 gives exactly the 41st and 42nd citizens.
        let tail = extra_ambient_sheets(&points(64)[40..42], 40);
        assert_eq!(tail, once[40..42]);
    }

    #[test]
    fn every_generated_citizen_is_an_ambient_with_a_goal() {
        for sheet in extra_ambient_sheets(&points(200), 0) {
            let lore = sheet.lore.as_ref().expect("a generated citizen has a profile");
            assert_eq!(lore.significance, Significance::Ambient);
            assert!(lore.generated, "{} must be flagged generated", sheet.id);
            // Both authored shapes and nothing between them: a trade with its
            // title, or the `no_fixed_trade/` nulls straight across.
            assert_eq!(
                lore.occupation_id.is_some(),
                lore.title.is_some(),
                "{} has half a trade",
                sheet.id
            );
            assert_eq!(
                lore.occupation_id.is_some(),
                lore.occupation_display.is_some(),
                "{} has half a trade",
                sheet.id
            );
            assert!(lore.rank.is_none());
            assert!(lore.home.is_none(), "generated citizens have no baked bed");
            assert_ne!(sheet.goal, GOAL_NONE);
            assert!(sheet.facing_yaw.is_finite());
            assert!(sheet.voice_key.is_some());
            assert!(sheet.knows.is_empty(), "a crowd knows nobody by name");
        }
    }

    /// The banks are wide enough that a street is not twenty of the same
    /// person — the failure mode a crowd this size actually shows the player.
    #[test]
    fn a_crowd_is_not_one_person_repeated() {
        let sheets = extra_ambient_sheets(&points(500), 0);
        let names: BTreeSet<&str> = sheets.iter().map(|sheet| sheet.name.as_str()).collect();
        assert!(names.len() > 400, "only {} distinct names in 500", names.len());
        let bodies: BTreeSet<String> = sheets
            .iter()
            .map(|sheet| {
                let body = &sheet.appearance;
                format!("{:?}/{:?}/{:?}", body.build, body.outfit, body.headgear)
            })
            .collect();
        assert!(bodies.len() >= 10, "only {} distinct silhouettes", bodies.len());
        let goals: BTreeSet<&str> = sheets.iter().map(|sheet| sheet.goal.as_str()).collect();
        // Both concern banks, in full: no goal appears in both, so the sum is
        // the count, and a bank that stopped being drawn from would show here.
        assert_eq!(goals.len(), CONCERNS.len() + NO_TRADE_CONCERNS.len());
    }

    /// M2. A quarter of the crowd holds no trade at all, and every one of them
    /// carries the pairing the loader demands of an authored no-trade sheet.
    #[test]
    fn a_quarter_of_the_crowd_has_no_trade_and_says_how_it_eats() {
        let sheets = extra_ambient_sheets(&points(4_000), 0);
        let no_trade: Vec<&CharacterSheet> = sheets
            .iter()
            .filter(|sheet| {
                sheet
                    .lore
                    .as_ref()
                    .is_some_and(|lore| lore.occupation_id.is_none())
            })
            .collect();

        // Within a point of the target over 4,000 — the share is a hash, so
        // this is the sampling error of the hash and nothing else.
        let share = no_trade.len() as f64 / sheets.len() as f64;
        assert!(
            (share - NO_TRADE_SHARE).abs() < 0.02,
            "{} of {} have no trade ({share:.3}), against a target of {NO_TRADE_SHARE}",
            no_trade.len(),
            sheets.len()
        );

        let mut supports_seen = BTreeSet::new();
        for sheet in &no_trade {
            let lore = sheet
                .lore
                .as_ref()
                .expect("a generated citizen has a profile");
            // The `no_fixed_trade/` shape, exactly: all three trade fields null.
            assert!(lore.title.is_none() && lore.rank.is_none());
            assert!(lore.occupation_display.is_none());
            // …and the pairing `LoreCharacterSheet::validate` requires of it,
            // checked against the loader's own list rather than a copy of it.
            assert!(
                lore.circumstances
                    .iter()
                    .any(|circumstance| SUPPORT_CIRCUMSTANCES.contains(&circumstance.as_str())),
                "{} has no trade and no support circumstance: {:?}",
                sheet.id,
                lore.circumstances
            );
            for circumstance in &lore.circumstances {
                assert!(
                    CONTROLLED_CIRCUMSTANCES.contains(&circumstance.as_str()),
                    "{} carries the uncontrolled circumstance '{circumstance}'",
                    sheet.id
                );
                supports_seen.insert(circumstance.clone());
            }
            // `widow`/`widower` is one circumstance in two spellings and must
            // agree with the gender the rest of the sheet was built from.
            let wrong = if lore.gender == "f" {
                "widower"
            } else {
                "widow"
            };
            assert!(
                !lore.circumstances.iter().any(|c| c == wrong),
                "{} is a '{}' and a '{wrong}'",
                sheet.id,
                lore.gender
            );
            // The description must answer *how do you materially survive*, and
            // must not open with the title it does not have.
            assert!(!sheet.back_story.starts_with("You are a  "));
            assert!(sheet.back_story.contains(" in "));
            assert_ne!(sheet.goal, GOAL_NONE);
            // With no trade the outfit falls to Poor with no new mesh — the
            // whole visual half of this milestone.
            assert_eq!(
                sheet.appearance.outfit,
                crate::appearance::OutfitClass::Poor
            );
        }

        // Every support line is drawn from, so none of the bank is dead prose.
        let openings: BTreeSet<&str> = no_trade
            .iter()
            .map(|sheet| {
                sheet
                    .back_story
                    .split(" in ")
                    .next()
                    .expect("an opening before the district")
            })
            .collect();
        assert_eq!(openings.len(), NO_TRADE_OPENINGS.len());
    }

    /// The people leaning on the walls are the ones who speak to you first —
    /// the numbers `features/give_the_crowd_somewhere_to_be.md` M2 claims,
    /// measured against the crowd this file actually mints.
    #[test]
    fn the_loiterers_are_the_curious_ones() {
        let sheets = extra_ambient_sheets(&points(2_000), 0);
        let curiosity = |sheet: &CharacterSheet| {
            crate::attention::curiosity_from_lore(
                sheet
                    .lore
                    .as_ref()
                    .expect("a generated citizen has a profile"),
            )
        };
        let (no_trade, tradesmen): (Vec<f64>, Vec<f64>) = sheets
            .iter()
            .map(|sheet| {
                let has_trade = sheet
                    .lore
                    .as_ref()
                    .is_some_and(|lore| lore.occupation_id.is_some());
                (curiosity(sheet), has_trade)
            })
            .fold(
                (Vec::new(), Vec::new()),
                |(mut idle, mut working), (value, has_trade)| {
                    if has_trade {
                        working.push(value);
                    } else {
                        idle.push(value);
                    }
                    (idle, working)
                },
            );

        let span = |values: &[f64]| {
            (
                values.iter().copied().fold(f64::MAX, f64::min),
                values.iter().copied().fold(f64::MIN, f64::max),
                values.iter().sum::<f64>() / values.len() as f64,
            )
        };
        let (idle_low, idle_high, idle_mean) = span(&no_trade);
        let (work_low, work_high, work_mean) = span(&tradesmen);
        println!(
            "no trade: {idle_low:.3}..={idle_high:.3} mean {idle_mean:.3}  |  \
             trades: {work_low:.3}..={work_high:.3} mean {work_mean:.3}"
        );
        // Measured over 2,000: the no-trade cohort spans 0.162..=0.322 with a
        // mean of 0.232; the trades span 0.082..=0.192 with a mean of 0.118.
        // The spec's ≈0.30 is the *top* of the no-trade band and not its
        // middle: 0.082 base + 0.10 (no trade) + 0.10 (begs/alms, counted once)
        // + 0.02 (unhoused) is the entry that carries all three, and a citizen
        // of that entry who is also under 25 takes the age term to 0.322. The
        // floor, 0.162, is the retired pauper — 0.182 less the retirement 0.02.
        //
        // The two bands **overlap**, which is right and is why the assertion is
        // on the means: a young milk seller who hails strangers for a living
        // ought to beat a retired pauper who is done with the world. What must
        // hold is that the loitering cohort is the talkative one on average,
        // and that its floor is above the trades' floor.
        assert!(
            idle_mean > 1.9 * work_mean,
            "no-trade mean {idle_mean:.3} against trade mean {work_mean:.3}"
        );
        assert!(
            idle_low > work_low,
            "the least curious loiterer ({idle_low:.3}) is below the least \
             curious tradesman ({work_low:.3})"
        );
        assert!(idle_high > 0.29, "the top of the band is {idle_high:.3}");
    }

    /// The M1 claim, against the real city: a crowd stands on the *width* of
    /// the open ground, not on the ribbon down the middle of it — and never
    /// inside a wall, because every offset is rejection-tested.
    #[test]
    fn the_crowd_stands_off_its_nodes_and_all_of_it_on_walkable_ground() {
        let nav = shipped_nav();
        let nodes = nav.node_count();
        let stride = coprime_stride(nodes);
        let points = spread_over_walkable(&nav, 2_000);
        assert_eq!(points.len(), 2_000);

        let mut off_node = 0;
        let mut on_node = 0;
        for (index, point) in points.iter().enumerate() {
            assert!(
                nav.is_walkable(point.x, point.z),
                "citizen {index} stands at ({}, {}), which is not walkable",
                point.x,
                point.z
            );
            let anchor = nav.node_point(index.wrapping_mul(stride) % nodes);
            let offset = ((point.x - anchor.x).powi(2) + (point.z - anchor.z).powi(2)).sqrt();
            assert!(
                offset <= SPREAD_RADIUS_M + 1e-9,
                "citizen {index} is {offset:.2} m off their anchor node"
            );
            if offset > 3.0 {
                off_node += 1;
            }
            // The fallback: every draw was wall, so they stand on the node.
            if offset == 0.0 {
                on_node += 1;
            }
        }
        // 1,843 of 2,000 as measured; the old 1.6 m jitter put it at zero by
        // construction, since 1.6 m is never more than 3.
        assert!(
            off_node > 1_000,
            "only {off_node} of 2000 stand more than 3 m off their node"
        );
        // Eight draws inside a disc anchored on walkable ground rarely all
        // fail; when they do it is a node walled in on every side, and the
        // citizen stands on it as the whole crowd used to. 28 of 2,000.
        assert!(on_node < 100, "{on_node} of 2000 fell back to their node");
    }

    #[test]
    fn the_stride_visits_every_node_before_repeating_one() {
        for nodes in [1usize, 2, 7, 64, 3972, 4096] {
            let stride = coprime_stride(nodes);
            assert_eq!(gcd(stride, nodes), 1, "stride {stride} for {nodes} nodes");
            let visited: BTreeSet<usize> =
                (0..nodes).map(|i| i.wrapping_mul(stride) % nodes).collect();
            assert_eq!(visited.len(), nodes, "{nodes} nodes, stride {stride}");
        }
    }
}
