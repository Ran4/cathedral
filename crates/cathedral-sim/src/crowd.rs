//! The crowd knob: `extra_ambient_npcs` generated citizens on top of the cast.
//!
//! The shipped roster is hand-authored, deliberately dispersed, and canon
//! (`lore/characters/AGENTS.md`). Nothing here is any of those things. This
//! module exists to answer one question — *what does the city do with a lot
//! more people in it* — and it answers it by minting bodies that are ordinary
//! in every way the sim can observe (ambient significance, a trade, a ward, a
//! walk, a purse) and authored in none.
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

/// How far off their nav node a generated citizen may stand. The routed lanes
/// are 4.6 m at their pinch, so this keeps a crowd off the walls while still
/// breaking up the "beads on a string" look of one body per graph node. Every
/// jittered point is re-tested against the walkable bitset, so the number is a
/// budget, never a promise.
const SPREAD_JITTER_M: f64 = 1.6;

/// Walkable spawn points for a crowd of `count`, spread over the whole graph.
///
/// The graph has ~4,000 nodes, so a full crowd stacks several people on each.
/// Consecutive indices are pushed apart by striding the node list with a
/// coprime step rather than walking it in order: filling nodes 0..n in
/// sequence would lay the first thousand citizens down in one quarter of the
/// city and leave the rest of it empty until the count got high enough.
pub fn spread_over_walkable(nav: &NavData, count: usize) -> Vec<Vec3> {
    let nodes = nav.node_count();
    if nodes == 0 || count == 0 {
        return Vec::new();
    }
    let stride = coprime_stride(nodes);
    let mut points = Vec::with_capacity(count);
    for index in 0..count {
        let node = index.wrapping_mul(stride) % nodes;
        let centre = nav.node_point(node);
        // The jitter is polar so the offsets do not favour the diagonals, and
        // it falls back to the node itself the moment it would put somebody
        // through a wall.
        let angle = unit(index as u64, 0x9E37_79B9) * std::f64::consts::TAU;
        let radius = unit(index as u64, 0x85EB_CA6B).sqrt() * SPREAD_JITTER_M;
        let x = centre.x + radius * angle.cos();
        let z = centre.z + radius * angle.sin();
        points.push(if nav.is_walkable(x, z) {
            Vec3::new(x, centre.y, z)
        } else {
            centre
        });
    }
    points
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
    let (occupation_id, occupation_display, title) = *pick(TRADES, seed, 0x5EED_0004);
    let ward = PlanningWard::ALL[(hash(seed, 0x5EED_0005) % 8) as usize];
    let district = format!("{} streets", ward_name(ward));
    let age = 16 + (hash(seed, 0x5EED_0006) % 50) as u16;
    let (concern, goal) = *pick(CONCERNS, seed, 0x5EED_0007);
    let description = format!(
        "You are a {} in {district}. {}. {}. Today {concern}. {}.",
        title.to_lowercase(),
        pick(SURVIVAL, seed, 0x5EED_0008),
        pick(MANNER, seed, 0x5EED_0009),
        pick(FEATURES, seed, 0x5EED_000A),
    );

    CharacterSheet {
        appearance: AppearanceSnapshot::compose(&id, gender, Some(occupation_id), None, &[]),
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
            occupation_id: Some(occupation_id.to_string()),
            occupation_display: Some(occupation_display.to_string()),
            title: Some(title.to_string()),
            rank: None,
            faction_role: None,
            illegal_activity: None,
            district,
            father: None,
            mother: None,
            children: Vec::new(),
            circumstances: Vec::new(),
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
    "You answer strangers plainly and then get back to work",
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
    use std::collections::BTreeSet;

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
    fn every_generated_citizen_is_an_ambient_with_a_trade_and_a_goal() {
        for sheet in extra_ambient_sheets(&points(200), 0) {
            let lore = sheet.lore.as_ref().expect("a generated citizen has a profile");
            assert_eq!(lore.significance, Significance::Ambient);
            assert!(lore.generated, "{} must be flagged generated", sheet.id);
            assert!(lore.occupation_id.is_some() && lore.title.is_some());
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
        assert_eq!(goals.len(), CONCERNS.len());
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
