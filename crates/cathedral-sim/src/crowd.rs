//! Persistent generated residents. Identity, support, lodging and routine are
//! separate deterministic choices. No automatic occupation selection.
use crate::{
    appearance::AppearanceSnapshot,
    character::{CharacterSheet, Control, EconomicClass, Presence},
    homes,
    ids::ActorId,
    lore::{LoreProfile, PlanningWard, Significance, default_voice_key},
    math::Vec3,
    nav::{NavData, Place},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_EXTRA_AMBIENT_NPCS: u32 = 20_000;
const HARDSHIP_SHARE: f64 = 0.25;
const WORKING_AGE_FROM: u16 = 16;
const WIDOWED_AGE_FROM: u16 = 34;
const RETIRED_AGE_FROM: u16 = 58;
const AGE_UNTIL: u16 = 66;
const NEARBY_M: f64 = 120.0;
const SPREAD_RADIUS_M: f64 = 12.0;
const SPREAD_ATTEMPTS: usize = 8;

/// Explicit generation policy, independent of significance or occupation nulls.
/// Internal geometry handles are not public wayfinding destinations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GeneratedRoutine {
    Resident {
        patch: String,
        spot: String,
    },
    Worker {
        occupation: String,
        workplace: String,
    },
}
impl GeneratedRoutine {
    pub fn is_resident(&self) -> bool {
        matches!(self, Self::Resident { .. })
    }
}

/// Opt-in worker exception. A named registered occupation AND one of its
/// compatible work places are mandatory. Shipped hosts pass an empty list.
#[derive(Debug, Clone)]
pub struct WorkerOverride {
    pub index: u32,
    pub occupation: String,
    pub workplace: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CrowdPlacement {
    pub requested: usize,
    pub placed: usize,
    pub unplaced: usize,
    pub housed: usize,
    pub hardship: usize,
    pub workers: usize,
    pub door_cap: usize,
}
pub struct GeneratedCrowd {
    pub sheets: Vec<CharacterSheet>,
    pub placement: CrowdPlacement,
}

fn hardship(index: u32) -> bool {
    unit(u64::from(index), 0x5EED_000C) < HARDSHIP_SHARE
}

/// Compatibility wrapper: the slice determines the requested count. Residents
/// are always allocated on baked spots; arbitrary graph offsets are never used
/// as a fallback. Hosts use `generate_ambient` to report capacity explicitly.
pub fn extra_ambient_sheets(
    nav: &NavData,
    points: &[Vec3],
    first_index: u32,
) -> Vec<CharacterSheet> {
    generate_ambient(nav, points.len(), first_index, &[], &[])
        .expect("default resident generation has no worker overrides")
        .sheets
}

/// Allocate housing across doors, then hardship places across patches. Stable
/// shuffled door/patch orders prevent lexical city clustering. A breadth-first
/// pass uses one spot per patch before filling it, leaving spare local targets
/// at ordinary counts. Existing bodies exclude candidates during allocation.
pub fn generate_ambient(
    nav: &NavData,
    count: usize,
    first_index: u32,
    occupied: &[Vec3],
    overrides: &[WorkerOverride],
) -> Result<GeneratedCrowd, String> {
    let requested = count.min(MAX_EXTRA_AMBIENT_NPCS as usize);
    let mut workers = BTreeMap::new();
    for worker in overrides {
        if worker.index < first_index || worker.index >= first_index + requested as u32 {
            return Err(format!(
                "worker index {} is outside the request",
                worker.index
            ));
        }
        let trade = TRADES
            .iter()
            .find(|t| t.0 == worker.occupation)
            .ok_or_else(|| format!("unknown generated worker occupation {}", worker.occupation))?;
        if !crate::round::compatible_generated_workplace(nav, trade.0, &worker.workplace) {
            return Err(format!(
                "{} has no compatible routine at {}",
                worker.occupation, worker.workplace
            ));
        }
        if workers.insert(worker.index, (trade, worker)).is_some() {
            return Err(format!("duplicate worker override {}", worker.index));
        }
    }
    let places = nav.resident_places();
    let mut candidates: Vec<Vec<usize>> = places
        .patches
        .iter()
        .map(|patch| {
            let mut spots: Vec<usize> = (0..patch.spots.len())
                .filter(|&s| {
                    occupied
                        .iter()
                        .all(|p| p.distance(patch.spots[s].position()) >= 1.6)
                })
                .collect();
            if let Some(door) = patch.home_building.as_ref().and_then(|b| nav.door(b)) {
                let at = nav.node_point(door.node);
                spots.sort_by(|&a, &b| {
                    patch.spots[a]
                        .position()
                        .distance(at)
                        .total_cmp(&patch.spots[b].position().distance(at))
                });
            }
            spots
        })
        .collect();
    let mut by_door: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (p, patch) in places.patches.iter().enumerate() {
        if let Some(door) = patch.home_building.as_deref() {
            by_door.entry(door).or_default().push(p);
        }
    }
    let housed_seeking = (0..requested)
        .filter(|i| {
            !hardship(first_index + *i as u32) || workers.contains_key(&(first_index + *i as u32))
        })
        .count();
    let door_cap = housed_seeking.div_ceil(by_door.len().max(1)).max(1);
    let mut doors: Vec<_> = by_door.keys().copied().collect();
    doors.sort_by_key(|d| crate::appearance::palette_seed_of(&ActorId::from_raw(*d)));
    let mut occupancy = BTreeMap::<&str, usize>::new();
    let mut assignments = vec![None; requested];
    let mut door_cursor = 0;
    let mut patch_load = vec![0usize; places.patches.len()];
    for (offset, assignment) in assignments.iter_mut().enumerate() {
        let index = first_index + offset as u32;
        if hardship(index) && !workers.contains_key(&index) {
            continue;
        }
        for _ in 0..doors.len() {
            let door = doors[door_cursor % doors.len()];
            door_cursor += 1;
            if occupancy.get(door).copied().unwrap_or(0) >= door_cap {
                continue;
            }
            let best = by_door[door]
                .iter()
                .copied()
                .filter(|&p| !candidates[p].is_empty())
                .min_by_key(|&p| (patch_load[p], p));
            if let Some(patch) = best {
                let spot = candidates[patch].remove(0);
                patch_load[patch] += 1;
                *occupancy.entry(door).or_default() += 1;
                *assignment = Some((patch, spot, true));
                break;
            }
        }
    }
    let mut patches: Vec<usize> = (0..places.patches.len()).collect();
    patches.sort_by_key(|&p| hash(p as u64, 0xB017_D157));
    let mut cursor = 0;
    for (offset, assignment) in assignments.iter_mut().enumerate() {
        let index = first_index + offset as u32;
        if !hardship(index) || workers.contains_key(&index) {
            continue;
        }
        // First fill unoccupied patches (especially the ones without doors).
        let min_load = patches
            .iter()
            .filter(|&&p| !candidates[p].is_empty())
            .map(|&p| patch_load[p])
            .min();
        let Some(min_load) = min_load else { break };
        for _ in 0..patches.len() {
            let patch = patches[cursor % patches.len()];
            cursor += 1;
            if candidates[patch].is_empty() || patch_load[patch] != min_load {
                continue;
            }
            let spot = candidates[patch].remove(0);
            patch_load[patch] += 1;
            *assignment = Some((patch, spot, false));
            break;
        }
    }
    let mut sheets = Vec::new();
    let mut housed = 0;
    let mut hardships = 0;
    let mut worker_count = 0;
    for (offset, assignment) in assignments.into_iter().enumerate() {
        let Some((patch, spot, has_home)) = assignment else {
            continue;
        };
        let index = first_index + offset as u32;
        let patch = &places.patches[patch];
        let spot = &patch.spots[spot];
        let home = has_home.then(|| {
            nav.node_point(
                nav.door(patch.home_building.as_ref().unwrap())
                    .unwrap()
                    .node,
            )
        });
        let worker = workers.get(&index).copied();
        housed += usize::from(home.is_some());
        hardships += usize::from(!has_home);
        worker_count += usize::from(worker.is_some());
        sheets.push(ambient_sheet(nav, index, patch, spot, home, worker));
    }
    let placement = CrowdPlacement {
        requested,
        placed: sheets.len(),
        unplaced: requested - sheets.len(),
        housed,
        hardship: hardships,
        workers: worker_count,
        door_cap,
    };
    Ok(GeneratedCrowd { sheets, placement })
}

fn ambient_sheet(
    nav: &NavData,
    index: u32,
    patch: &crate::nav::residents::ResidentPatch,
    spot: &crate::nav::residents::StandingSpot,
    home: Option<Vec3>,
    worker: Option<(&Trade, &WorkerOverride)>,
) -> CharacterSheet {
    let seed = u64::from(index);
    let id = ActorId::from_raw(format!("x{index:05}"));
    let female = hash(seed, 0x5EED_0001).is_multiple_of(2);
    let gender = if female { "f" } else { "m" };
    let name = format!(
        "{} {}",
        pick(if female { WOMEN } else { MEN }, seed, 0x5EED_0002),
        pick(BYNAMES, seed, 0x5EED_0003)
    );
    let ward = crate::knowledge::pollen::ward_grid()
        .at(home.unwrap_or(spot.position()))
        .unwrap_or(PlanningWard::ALL[(hash(seed, 0x5EED_0005) % 8) as usize]);
    let district = format!("{} streets", ward_name(ward));
    let poor = home.is_none();
    let (tags, age_from, support) = *pick(
        if poor { SUPPORTS } else { HOUSEHOLD_SUPPORTS },
        seed,
        0x5EED_000D,
    );
    let circumstances: Vec<String> = if worker.is_some() {
        Vec::new()
    } else {
        tags.iter()
            .map(|tag| {
                if *tag == "widow" && !female {
                    "widower".into()
                } else {
                    (*tag).into()
                }
            })
            .collect()
    };
    let age_from = if worker.is_some() {
        WORKING_AGE_FROM
    } else {
        age_from
    };
    let age = age_from + (hash(seed, 0x5EED_0006) % u64::from(AGE_UNTIL - age_from)) as u16;
    let (concern, goal) = *pick(
        if poor {
            NO_TRADE_CONCERNS
        } else {
            HOUSEHOLD_CONCERNS
        },
        seed,
        0x5EED_000F,
    );
    let manner = pick(MANNER, seed, 0x5EED_0009);
    let feature = pick(FEATURES, seed, 0x5EED_000A);
    let (occupation_id, occupation_display, title, routine, description) = if let Some((
        trade,
        worker,
    )) = worker
    {
        (
            Some(trade.0.into()),
            Some(trade.1.into()),
            Some(trade.2.into()),
            GeneratedRoutine::Worker {
                occupation: trade.0.into(),
                workplace: worker.workplace.clone(),
            },
            format!(
                "You are a {} in {district}, working at {}. Your trade supports your household. {manner}. {feature}.",
                trade.2.to_lowercase(),
                worker.workplace
            ),
        )
    } else {
        (
            None,
            None,
            None,
            GeneratedRoutine::Resident {
                patch: patch.id.clone(),
                spot: spot.id.clone(),
            },
            format!(
                "{} in {district}. {support}. You spend time {}. {manner}. Today {concern}. {feature}.",
                pick(NO_TRADE_OPENINGS, seed, 0x5EED_000E),
                patch.description
            ),
        )
    };
    let home_description = home.map(|point| match nearest_place(nav, point) {
        Some((place, distance)) => format!(
            "a house in the {}, {}",
            district_of_ward(ward),
            location_phrase(place, distance)
        ),
        None => format!("a house in the {}", district_of_ward(ward)),
    });
    CharacterSheet {
        appearance: if worker.is_some() {
            AppearanceSnapshot::compose(&id, gender, occupation_id.as_deref(), None, &circumstances)
        } else {
            AppearanceSnapshot::resident(&id, gender, poor)
        },
        voice_key: Some(default_voice_key(gender, &id)),
        control: Control::Llm,
        back_story: description.clone(),
        location_description: patch.description.clone(),
        position_m: spot.position(),
        facing_yaw: spot.facing_yaw,
        holds: Vec::new(),
        pockets: Vec::new(),
        frontbutt: None,
        goal: goal.into(),
        memories: Vec::new(),
        knows: Default::default(),
        lore: Some(LoreProfile {
            significance: Significance::Ambient,
            planning_ward: ward,
            age,
            gender: gender.into(),
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
            home: home_description,
            home_point_m: home.map(|p| [p.x, p.z]),
            core_character_description: description,
            extended_character_description: String::new(),
            curiosity: None,
            generated: true,
            generated_routine: Some(routine),
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

/// Legacy graph sampling for callers constructing authored spatial fixtures.
/// Generated residents use `generate_ambient` and never these offsets.
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

/// The city's ward map: every baked home's door with the ward its building
/// belongs to, dropping the 92 in the "Outer wards", which are nobody's ward and
/// are better left out than guessed at. See [`homes::ward_marks`] for why this
/// is the best ward map the sim has and how well it agrees with the authored one.
///
/// `pub(crate)` for one caller: [`crate::knowledge::pollen::WardGrid`] bakes the
/// grid out of it and holds the exact nearest-mark search this module used to
/// own, so there is one answer to "which ward is this point in" and housing and
/// pollen cannot disagree.
pub(crate) fn ward_map() -> Vec<([f64; 2], PlanningWard)> {
    homes::ward_marks()
        .iter()
        .filter_map(|(point, district)| Some((*point, ward_of_district(district)?)))
        .collect()
}

/// The nearest named place on the graph, and how far off it is.
fn nearest_place(nav: &NavData, point: Vec3) -> Option<(&Place, f64)> {
    let mut best: Option<(f64, &Place)> = None;
    for place in nav.places() {
        let at = nav.node_point(place.node);
        let dx = at.x - point.x;
        let dz = at.z - point.z;
        let distance = dx * dx + dz * dz;
        if best.is_none_or(|(closest, _)| distance < closest) {
            best = Some((distance, place));
        }
    }
    best.map(|(distance, place)| (place, distance.sqrt()))
}

/// How a resident hangs their door on the nearest named place — "near the
/// Shambles well", "off Cinder Row", "by the Wool Gate", or, out where nothing
/// is named, "toward" whatever is closest. `scripts/bake_homes.py
/// location_phrase`, word for word, because the cast's homes are already
/// spoken this way and the crowd's must not sound like a different city.
fn location_phrase(place: &Place, distance: f64) -> String {
    let spoken = place
        .name
        .strip_prefix("The ")
        .map_or_else(|| place.name.clone(), |rest| format!("the {rest}"));
    if distance > NEARBY_M {
        return format!("toward {spoken}");
    }
    let preposition = match place.kind.as_str() {
        "route" => "off",
        "gate" | "bridge" => "by",
        _ => "near",
    };
    format!("{preposition} {spoken}")
}

/// A building district as the bake spells it → the planning ward it belongs to.
/// The inverse of `scripts/bake_homes.py WARD_TO_DISTRICTS`; anything outside
/// the eight wards ("Outer wards", "City wall", "Parish reserve") is nobody's
/// ward and is dropped from the map rather than guessed at.
fn ward_of_district(district: &str) -> Option<PlanningWard> {
    Some(match district {
        "Fabric Ward" => PlanningWard::Fabric,
        "Wick Ward" => PlanningWard::Wick,
        "Cloth Ward" => PlanningWard::Cloth,
        "Wallwright Ward" => PlanningWard::Wallwright,
        "Cinder Ward" => PlanningWard::Cinder,
        "Weigh Ward" => PlanningWard::Weigh,
        "Reed Ward" => PlanningWard::Reed,
        "Bell and Sluice Wards" | "Bell Ward" | "Sluice Ward" => PlanningWard::BellAndSluice,
        _ => return None,
    })
}

/// The ward as a home is addressed — the bake's own spelling, which is the
/// plural "Bell and Sluice Wards" where [`ward_name`] says the singular.
fn district_of_ward(ward: PlanningWard) -> &'static str {
    match ward {
        PlanningWard::BellAndSluice => "Bell and Sluice Wards",
        other => ward_name(other),
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
    let mut value =
        index.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt.wrapping_mul(0xD6E8_FEB8_6659_FD93);
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
    (
        "carpenter_and_builder",
        "Carpenter and builder",
        "Scaffold worker",
    ),
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
/// `features/implemented/give_the_crowd_somewhere_to_be.md` names.
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
        "Neighbours share parish bread with you here, and you rest beside the frontage without a settled household door",
    ),
    (
        &["pauper", "alms_dependent", "begs_regularly", "unhoused"],
        WORKING_AGE_FROM,
        "You have no house; local alms and the neighbours' shared pot support you here, and you rest on this familiar edge",
    ),
    (
        &["pauper", "dependent", "insecure_lodging"],
        WORKING_AGE_FROM,
        "Your lodging is uncertain; kin bring a share of their pot to this frontage where you pass the day and rest",
    ),
    (
        &["pauper", "widow", "alms_dependent"],
        WIDOWED_AGE_FROM,
        "Since you were widowed, neighbours and the parish have shared their bread with you here; you have no settled door",
    ),
    (
        &["pauper", "retired", "dependent"],
        RETIRED_AGE_FROM,
        "Your working years are behind you; kin and neighbours share their meals with you here, and you have no fixed room",
    ),
];
const HOUSEHOLD_SUPPORTS: &[Support] = &[
    (
        &["dependent"],
        WORKING_AGE_FROM,
        "You belong to a shared household and eat from its common pot; you have no paid work or work schedule",
    ),
    (
        &["dependent"],
        WORKING_AGE_FROM,
        "Kin support your place in the household and share their meals with you; these nearby frontages are familiar ground",
    ),
    (
        &["retired", "dependent"],
        RETIRED_AGE_FROM,
        "You are retired, supported by what was put by and the household's shared pot; you keep no active trade",
    ),
    (
        &["widow", "dependent"],
        WIDOWED_AGE_FROM,
        "Since you were widowed, your kin have kept a place and a share of the household meals for you",
    ),
];

/// The opening the no-trade cohort gets instead of "You are a {title}", which
/// says nothing at all when there is no title. Each reads straight into
/// `in {district}`, and each answers the first ambient authoring question —
/// *what are you doing here?* — with the honest answer: standing about.
const NO_TRADE_OPENINGS: &[&str] = &[
    "You are a familiar resident",
    "You have no fixed trade and spend time near your neighbours",
    "You pass much of the day beside the frontages",
    "You keep to a few familiar streets",
    "You enjoy watching the life of the neighbourhood",
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
        "a neighbour objects to you spending time beside the frontage",
        "Find a quiet frontage where you are welcome",
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
const HOUSEHOLD_CONCERNS: &[Concern] = &[
    (
        "you hope the mended blanket will last another winter",
        "Keep the household blankets in good order",
    ),
    (
        "you wonder whether your cousin will visit",
        "Hear news from your kin",
    ),
    (
        "a neighbour has not been outside for several days",
        "Find out how your neighbour is keeping",
    ),
    (
        "you are enjoying a quiet hour beside the house",
        "Enjoy a little peace in your neighbourhood",
    ),
    (
        "you hope the rain holds off until evening",
        "Keep your clothes dry",
    ),
    (
        "you remember how different this street used to look",
        "Share a memory of the street",
    ),
];

const FEATURES: &[&str] = &[
    "Your sleeves are pinned back with a strip of sail-cloth",
    "There is a pale scar across one knuckle where a tool slipped",
    "Your right shoulder aches when the weather turns",
    "Your hem is caked to the knee with the same grey mud as everyone else's",
    "One ear is notched, and you do not explain it",
    "You keep a length of cord wound twice around your wrist",
    "Your hair is cropped short against the lice",
    "You wear somebody larger's coat, taken in badly at the seams",
    "There is charcoal under your nails that will not wash out",
    "You walk with a short step, favouring one foot",
];

#[cfg(test)]
mod tests;
