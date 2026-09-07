//! Local resident ownership, separate from the occupational ladder. Catalogue
//! indices are resolved once at enrollment; no per-poll linear spot lookup.
use super::*;
use crate::{
    crowd::GeneratedRoutine,
    nav::{
        LocalPathBudget,
        residents::{SpotReservations, StandingSpot},
    },
};

pub const DWELL_MIN_SECONDS: f64 = 45.0;
pub const DWELL_MAX_SECONDS: f64 = 150.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResidentPhase {
    Lingering,
    MovingLocally,
    Resting,
    Returning,
    Interrupted,
    Sheltering,
    WaitingToReturn,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Resident {
    pub patch: usize,
    pub preferred_spot: usize,
    pub spot: Option<usize>,
    pub target: Option<usize>,
    pub phase: ResidentPhase,
    pub dwell_until: f64,
    pub epoch: u64,
    pub retiring: bool,
    pub support_was_eligible: bool,
    pub missed_meal: bool,
    pub recovery_remaining: f64,
    weather: Option<ResidentWeather>,
    weather_retry_until: f64,
}

#[derive(Debug, Clone, PartialEq)]
struct ResidentWeather {
    slot: usize,
    intent: WeatherShelterIntent,
    arrived: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Residents {
    pub people: BTreeMap<ActorId, Resident>,
    pub reservations: SpotReservations,
    order: Vec<ActorId>,
    cursor: usize,
    pub optional: BTreeSet<ActorId>,
    /// Sparse presentation changes; local footwork must not dirty the whole
    /// city's inventory/appearance snapshot.
    changed: BTreeSet<ActorId>,
    departing: BTreeSet<usize>,
    local_shelters: BTreeMap<usize, Vec<(usize, usize)>>,
}

/// Read-only integration seam for prompts, debug panels and motion evidence.
/// Resting describes an outdoor position beside the household, never indoors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResidentStatus {
    pub phase: ResidentPhase,
    pub patch: String,
    pub patch_description: String,
    pub spot: Option<String>,
    pub destination_spot: Option<String>,
    pub dwell_remaining_seconds: f64,
    pub resting_at_household_frontage: bool,
    pub resting_without_home: bool,
    pub optional_walk: bool,
    pub movement_cause: motion::MotionCause,
    pub sheltered: bool,
}

impl ResidentStatus {
    /// Human/LLM prose deliberately omits private patch/spot handles.
    pub fn description(&self) -> String {
        let phase = match self.phase {
            ResidentPhase::Lingering => "lingering",
            ResidentPhase::MovingLocally => "changing standing position locally",
            ResidentPhase::Resting => "resting outdoors beside the familiar frontage",
            ResidentPhase::Returning => "returning to the familiar frontage",
            ResidentPhase::Interrupted => "routine paused for another activity",
            ResidentPhase::WaitingToReturn => "waiting to return to the familiar frontage",
            ResidentPhase::Sheltering => {
                if self.sheltered {
                    "waiting under local cover"
                } else {
                    "walking toward local cover"
                }
            }
        };
        format!(
            "{phase}. Familiar place: {}. {}",
            self.patch_description,
            if self.sheltered {
                "Under nearby cover."
            } else {
                "Outdoors, exposed to the weather."
            }
        )
    }
}

fn project(round: &mut Round, world: &mut World, nav: &NavData, now: f64) {
    for (id, r) in &round.residents.people {
        let Some(c) = world.characters.get(id) else {
            continue;
        };
        let patch = &nav.resident_places().patches[r.patch];
        let spot = round
            .residents
            .reservations
            .claims(id)
            .and_then(|c| c.occupied.as_deref());
        let target = r.target.map(|s| patch.spots[s].id.as_str()).or_else(|| {
            r.weather
                .as_ref()
                .filter(|w| !w.arrived)
                .map(|w| nav.resident_places().shelter_spots[w.slot].spot.id.as_str())
        });
        let phase = if matches!(r.phase, ResidentPhase::Lingering | ResidentPhase::Resting)
            && !r
                .spot
                .is_some_and(|s| patch.spots[s].contains(c.position_m()))
        {
            ResidentPhase::WaitingToReturn
        } else {
            r.phase
        };
        let sheltered = world.shelters.is_sheltered(c.position_m());
        let cause = round.motion_cause(world, id);
        let remaining = (r.dwell_until - now).max(0.0);
        let changed = c.state.resident.as_ref().is_none_or(|s| {
            s.phase != phase
                || s.spot.as_deref() != spot
                || s.destination_spot.as_deref() != target
                || s.sheltered != sheltered
                || s.movement_cause != cause
                || s.optional_walk != round.residents.optional.contains(id)
                || remaining > s.dwell_remaining_seconds
        });
        if changed {
            let status = round.resident_status(nav, world, id, now);
            world.characters.get_mut(id).unwrap().state.resident = status;
            round.residents.changed.insert(id.clone());
        } else {
            let c = world.characters.get_mut(id).unwrap();
            let status = c.state.resident.as_mut().unwrap();
            status.dwell_remaining_seconds = remaining;
            status.movement_cause = cause;
            status.sheltered = sheltered;
            let resting = r.phase == ResidentPhase::Resting
                && c.state.movement.as_ref().is_none_or(|m| m.path.is_empty())
                && r.spot
                    .is_some_and(|s| patch.spots[s].contains(c.state.position_m));
            status.resting_at_household_frontage = resting && round.people[id].home.is_some();
            status.resting_without_home = resting && round.people[id].home.is_none();
        }
    }
}

pub(super) fn is_resident(character: &Character) -> bool {
    character.lore().is_some_and(|l| {
        l.generated
            && l.generated_routine
                .as_ref()
                .is_some_and(GeneratedRoutine::is_resident)
    })
}

pub(super) fn dwell(id: &ActorId, epoch: u64) -> f64 {
    DWELL_MIN_SECONDS
        + hash01("resident_dwell", id, epoch) * (DWELL_MAX_SECONDS - DWELL_MIN_SECONDS)
}

fn retiring(id: &ActorId, time: WorldTime) -> bool {
    let hour = time.fraction * 24.0;
    let morning = 5.0 + 2.0 * hash01("resident_morning", id, time.day as u64);
    let evening = 18.0 + 2.5 * hash01("resident_evening", id, time.day as u64);
    hour < morning || hour >= evening
}

impl Round {
    pub(crate) fn drain_resident_updates(
        &mut self,
        world: &World,
    ) -> Vec<(ActorId, ResidentStatus)> {
        std::mem::take(&mut self.residents.changed)
            .into_iter()
            .filter(|id| world.is_present(id))
            .filter_map(|id| {
                world
                    .characters
                    .get(&id)?
                    .state
                    .resident
                    .clone()
                    .map(|status| (id, status))
            })
            .collect()
    }

    pub fn resident_status(
        &self,
        nav: &NavData,
        world: &World,
        id: &ActorId,
        now: f64,
    ) -> Option<ResidentStatus> {
        let r = self.residents.people.get(id)?;
        let p = &nav.resident_places().patches[r.patch];
        let resting = r.phase == ResidentPhase::Resting
            && world.characters.get(id).is_some_and(|c| {
                !c.is_walking() && r.spot.is_some_and(|s| p.spots[s].contains(c.position_m()))
            });
        let housed = self.people.get(id).is_some_and(|p| p.home.is_some());
        Some(ResidentStatus {
            phase: if matches!(r.phase, ResidentPhase::Lingering | ResidentPhase::Resting)
                && !r.spot.is_some_and(|s| {
                    world
                        .characters
                        .get(id)
                        .is_some_and(|c| p.spots[s].contains(c.position_m()))
                }) {
                ResidentPhase::WaitingToReturn
            } else {
                r.phase
            },
            patch: p.id.clone(),
            patch_description: p.description.clone(),
            spot: self
                .residents
                .reservations
                .claims(id)
                .and_then(|c| c.occupied.clone()),
            destination_spot: r.target.map(|s| p.spots[s].id.clone()).or_else(|| {
                r.weather
                    .as_ref()
                    .filter(|w| !w.arrived)
                    .map(|w| nav.resident_places().shelter_spots[w.slot].spot.id.clone())
            }),
            dwell_remaining_seconds: (r.dwell_until - now).max(0.0),
            resting_at_household_frontage: resting && housed,
            resting_without_home: resting && !housed,
            optional_walk: self.residents.optional.contains(id),
            movement_cause: self.motion_cause(world, id),
            sheltered: world
                .characters
                .get(id)
                .is_some_and(|c| world.shelters.is_sheltered(c.position_m())),
        })
    }
    pub fn resident_count(&self) -> usize {
        self.residents.people.len()
    }
    pub fn resident_reservations(&self) -> &SpotReservations {
        &self.residents.reservations
    }
}

pub(super) fn seed(round: &mut Round, world: &mut World, nav: &NavData, time: WorldTime, now: f64) {
    let places = nav.resident_places();
    let index: BTreeMap<&str, (usize, usize)> = places
        .patches
        .iter()
        .enumerate()
        .flat_map(|(p, patch)| {
            patch
                .spots
                .iter()
                .enumerate()
                .map(move |(s, spot)| (spot.id.as_str(), (p, s)))
        })
        .collect();
    let mut residents = Residents {
        reservations: SpotReservations::new(places),
        ..Residents::default()
    };
    for (p, patch) in places.patches.iter().enumerate() {
        let candidates = places
            .shelter_spots
            .iter()
            .enumerate()
            .filter_map(|(slot, s)| {
                let index = world.shelters.shelters().iter().position(|shelter| {
                    shelter.id == s.shelter
                        && shelter.access == ShelterAccess::Public
                        && shelter.contains(s.spot.position())
                })?;
                (patch.spots[0].position().distance(s.spot.position()) <= 30.0)
                    .then_some((slot, index))
            })
            .collect();
        residents.local_shelters.insert(p, candidates);
    }
    for (id, person) in &mut round.people {
        let character = world.characters.get_mut(id).unwrap();
        let Some(GeneratedRoutine::Resident { patch, spot }) = character
            .lore()
            .filter(|l| l.generated)
            .and_then(|l| l.generated_routine.as_ref())
        else {
            continue;
        };
        let Some(&(p, s)) = index.get(spot.as_str()) else {
            continue;
        };
        if places.patches[p].id != *patch
            || !residents
                .reservations
                .occupy(id, spot, character.position_m())
        {
            continue;
        }
        let retiring = retiring(id, time);
        residents.people.insert(
            id.clone(),
            Resident {
                patch: p,
                preferred_spot: s,
                spot: Some(s),
                target: None,
                phase: if retiring {
                    ResidentPhase::Resting
                } else {
                    ResidentPhase::Lingering
                },
                dwell_until: now + dwell(id, 0),
                epoch: 0,
                retiring,
                support_was_eligible: true,
                missed_meal: false,
                // Session creation can begin after breakfast with a seeded need.
                // One bounded share of household provision settles that initial
                // need at the normal refill rate; never a per-poll full gauge.
                recovery_remaining: HUNGER_MAX,
                weather: None,
                weather_retry_until: 0.0,
            },
        );
        person.base = character.position_m();
        person.legs.clear();
        person.source = None;
        person.is_household = false;
        person.leash_m = 0.0;
        person.leg_lag_share = 0.0;
        character.state.daily_round = vec![
            format!(
                "Spend the day locally {}, lingering and occasionally changing position",
                places.patches[p].description
            ),
            if person.home.is_some() {
                "At night: rest beside your household frontage".into()
            } else {
                "At night: rest at your familiar local frontage; you have no settled household door"
                    .into()
            },
            "Meals: household or neighbourhood provision here, including breakfast at the Kindling"
                .into(),
        ];
    }
    residents.order = residents.people.keys().cloned().collect();
    round.residents = residents;
    project(round, world, nav, now);
}

/// Integrate the same hearth rate over real elapsed game time. Only a settled
/// reservation at BOTH ends of the interval qualifies, preventing a passerby
/// from receiving the interval's meals. Breakfast is resident-only (05–07);
/// dinner/supper retain the existing 12–21 offices. Missed meals permit at most
/// one gauge's recovery after returning, never items, stock, or money.
pub(super) fn support_gain(
    r: &mut Resident,
    character: &Character,
    nav: &NavData,
    from_days: f64,
    to_days: f64,
) -> f64 {
    let eligible = !character.is_walking()
        && character.state.intent.is_none()
        // A settled conversation is compatible with sharing the household
        // meal. An interrupted walker still fails the exact standing check.
        && matches!(r.phase, ResidentPhase::Lingering | ResidentPhase::Resting | ResidentPhase::Interrupted)
        && r.spot.is_some_and(|s| {
            nav.resident_places().patches[r.patch].spots[s].contains(character.position_m())
        });
    let meal_seconds = meal_overlap(from_days, to_days) * 86400.0;
    if !eligible && meal_seconds > 0.0 {
        r.missed_meal = true;
    }
    let continuous = eligible && r.support_was_eligible;
    r.support_was_eligible = eligible;
    if !continuous {
        return 0.0;
    }
    if r.missed_meal {
        r.recovery_remaining = HUNGER_MAX;
        r.missed_meal = false;
    }
    let regular = meal_seconds * HEARTH_REFILL_PER_GAME_SECOND;
    if regular > 0.0 {
        // A normal meal closes recovery. A later interruption can accrue a
        // fresh missed meal, but merely lingering cannot reset the allowance.
        r.recovery_remaining = 0.0;
        return regular;
    }
    let recovery = ((to_days - from_days).max(0.0) * 86400.0 * HEARTH_REFILL_PER_GAME_SECOND)
        .min(r.recovery_remaining);
    r.recovery_remaining -= recovery;
    recovery
}

fn meal_overlap(from: f64, to: f64) -> f64 {
    if !from.is_finite() || !to.is_finite() || to <= from {
        return 0.0;
    }
    // Integrate complete days arithmetically, not a loop over a debug time jump.
    fn cumulative(t: f64) -> f64 {
        let day = t.floor();
        let f = t - day;
        day * (11.0 / 24.0)
            + (f - 5.0 / 24.0).clamp(0.0, 2.0 / 24.0)
            + (f - 12.0 / 24.0).clamp(0.0, 9.0 / 24.0)
    }
    let overlap = (cumulative(to) - cumulative(from)).max(0.0);
    // Subtracting the whole-day epoch can leave an infinitesimal breakfast
    // tail at an exact 07:00 session start. It is not a meal and must not
    // consume the initial recovery allowance (nor mark a missed meal).
    if overlap < 1e-12 { 0.0 } else { overlap }
}

fn stop_owned(
    round: &mut Round,
    world: &mut World,
    people: &mut Residents,
    id: &ActorId,
    r: &mut Resident,
) {
    let claimed_target = r.target.take().is_some();
    let claimed = r.weather.take().is_some() || claimed_target;
    // tick_intents or custody may already have replaced the path this poll.
    // Reservation ownership alone is not ownership of the current feet.
    let owned = claimed
        && !world.custody.holds(id)
        && round.people.get(id).is_some_and(|p| {
            !p.travel_for_intent
                && matches!(
                    p.motion_cause,
                    motion::MotionCause::Wander
                        | motion::MotionCause::Domestic
                        | motion::MotionCause::Weather
                )
        });
    if owned {
        if let Some(c) = world.characters.get_mut(id) {
            c.state.movement = None;
        }
    }
    people.reservations.cancel_destination(id);
    if people.optional.remove(id) {
        people.departing.remove(&r.patch);
    }
    if owned && let Some(person) = round.people.get_mut(id) {
        person.phase = Phase::Idle;
        person.travel_target = None;
    }
}

fn settle(r: &mut Resident, id: &ActorId, now: f64) {
    r.epoch = r.epoch.wrapping_add(1);
    r.dwell_until = now + dwell(id, r.epoch);
    r.phase = if r.retiring {
        ResidentPhase::Resting
    } else {
        ResidentPhase::Lingering
    };
}

fn clear_destination(world: &World, id: &ActorId, spot: &StandingSpot) -> bool {
    // Only on a proposed transition, never for each lingering body/poll. The
    // reservations already exclude generated occupants; check explicit bodies
    // too before paying for a path.
    world.characters.values().all(|c| {
        c.id() == id || !world.is_present(c.id()) || c.position_m().distance(spot.position()) >= 1.2
    })
}

fn start_move(
    round: &mut Round,
    world: &mut World,
    nav: &NavData,
    people: &mut Residents,
    id: &ActorId,
    r: &mut Resident,
    target: usize,
    optional: bool,
) -> bool {
    let spot = &nav.resident_places().patches[r.patch].spots[target];
    if !(people.reservations.is_available(&spot.id)
        || people.reservations.owner(&spot.id) == Some(id))
        || !clear_destination(world, id, spot)
    {
        return false;
    }
    let c = &world.characters[id];
    let budget = if optional {
        LocalPathBudget::CHANGE_SPOT
    } else {
        LocalPathBudget::VISIT
    };
    let route = nav
        .local_route(c.position_m(), spot.position(), budget)
        .or_else(|| {
            (!optional)
                .then(|| nav.connected_route(c.position_m(), spot.position(), lane_fraction(id)))
                .flatten()
        });
    let Some(route) = route else {
        return false;
    };
    if !people.reservations.reserve_destination(id, &spot.id) {
        return false;
    }
    let gait = c.state.movement.as_ref().map_or(0.0, |m| m.gait_phase);
    world.characters.get_mut(id).unwrap().state.movement = Some(route.into_movement(gait));
    r.target = Some(target);
    r.phase = if optional {
        ResidentPhase::MovingLocally
    } else {
        ResidentPhase::Returning
    };
    let person = round.people.get_mut(id).unwrap();
    person.phase = Phase::Travelling;
    person.travel_target = Some(spot.position());
    person.travel_for_intent = false;
    person.motion_cause = if optional {
        motion::MotionCause::Wander
    } else {
        motion::MotionCause::Domestic
    };
    if optional {
        people.optional.insert(id.clone());
        people.departing.insert(r.patch);
    }
    true
}

/// Immediate speech handoff, called before another movement slice. It releases
/// an optional target now while retaining an origin still occupied by the body.
pub(super) fn interrupt(round: &mut Round, world: &mut World, id: &ActorId) {
    let mut residents = std::mem::take(&mut round.residents);
    if let Some(mut r) = residents.people.remove(id) {
        if !r.weather.as_ref().is_some_and(|w| w.arrived) {
            stop_owned(round, world, &mut residents, id, &mut r);
        }
        r.phase = ResidentPhase::Interrupted;
        residents.people.insert(id.clone(), r);
    }
    round.residents = residents;
}

fn weather_tick(
    round: &mut Round,
    world: &mut World,
    nav: &NavData,
    residents: &mut Residents,
    id: &ActorId,
    r: &mut Resident,
    time: WorldTime,
    now: f64,
    due: bool,
    curfew: bool,
) -> bool {
    let position = world.characters[id].position_m();
    if let Some(weather) = &mut r.weather {
        let precipitation = world
            .current_weather
            .unwrap_or(WeatherSample::CLEAR)
            .precipitation;
        let days = time.day as f64 + time.fraction;
        if precipitation + f64::EPSILON < weather.intent.release_threshold {
            weather.intent.below_since_days.get_or_insert(days);
        } else {
            weather.intent.below_since_days = None;
        }
        let release = weather
            .intent
            .below_since_days
            .is_some_and(|since| days - since >= weather.intent.release_after_days)
            || !world.shelters.shelters()[weather.intent.shelter].is_open(time.office)
            || (curfew && r.spot.is_none());
        if release {
            stop_owned(round, world, residents, id, r);
            settle(r, id, now);
            return false;
        }
        if !world.characters[id].is_walking() && !weather.arrived {
            if residents.reservations.settle(id, position) {
                weather.arrived = true;
                r.spot = None;
            } else {
                stop_owned(round, world, residents, id, r);
                settle(r, id, now);
                r.weather_retry_until = r.dwell_until;
                return true;
            }
        }
        r.phase = ResidentPhase::Sheltering;
        return true;
    }
    if !due
        || now < r.weather_retry_until
        || (curfew && r.spot.is_none())
        || world.shelters.is_sheltered(position)
    {
        return false;
    }
    let Some(threshold) =
        weather_shelter_threshold(&world.characters[id], id, world.current_weather)
    else {
        return false;
    };
    // Every candidate belongs to a real roof within 30m of this resident's
    // familiar frontage. Route work happens only at acquisition/backoff.
    let mut candidates = residents
        .local_shelters
        .get(&r.patch)
        .cloned()
        .unwrap_or_default();
    candidates.sort_by(|&(a, _), &(b, _)| {
        let score = |slot: usize| {
            position.distance(nav.resident_places().shelter_spots[slot].spot.position())
                + hash01("resident_shelter", id, slot as u64) * 2.0
        };
        score(a).total_cmp(&score(b))
    });
    for (slot, shelter_index) in candidates {
        let shelter = &world.shelters.shelters()[shelter_index];
        let spot = &nav.resident_places().shelter_spots[slot].spot;
        if !shelter.is_open(time.office)
            || !residents.reservations.is_available(&spot.id)
            || !clear_destination(world, id, spot)
        {
            continue;
        }
        let occupied = world
            .characters
            .iter()
            .filter(|(who, c)| world.is_present(who) && shelter.contains(c.position_m()))
            .count();
        let incoming = residents
            .people
            .values()
            .filter(|other| {
                other
                    .weather
                    .as_ref()
                    .is_some_and(|w| w.intent.shelter == shelter_index && !w.arrived)
            })
            .count();
        let authored_incoming = round
            .weather_shelter_intents
            .values()
            .filter(|i| i.shelter == shelter_index)
            .count();
        if occupied + incoming + authored_incoming >= shelter.capacity {
            continue;
        }
        let Some(route) = nav.local_route(position, spot.position(), LocalPathBudget::VISIT) else {
            continue;
        };
        stop_owned(round, world, residents, id, r);
        if !residents.reservations.reserve_destination(id, &spot.id) {
            continue;
        }
        world.characters.get_mut(id).unwrap().state.movement = Some(route.into_movement(0.0));
        r.weather = Some(ResidentWeather {
            slot,
            arrived: false,
            intent: WeatherShelterIntent {
                shelter: shelter_index,
                target: spot.position(),
                release_threshold: threshold,
                below_since_days: None,
                release_after_days: (SHELTER_RELEASE_MINUTES
                    + hash01("resident_shelter_release", id, slot as u64)
                        * SHELTER_RELEASE_SPREAD_MINUTES)
                    / (24.0 * 60.0),
            },
        });
        r.phase = ResidentPhase::Sheltering;
        let person = round.people.get_mut(id).unwrap();
        person.phase = Phase::Travelling;
        person.travel_target = Some(spot.position());
        person.travel_for_intent = false;
        person.motion_cause = motion::MotionCause::Weather;
        return true;
    }
    // No safe local roof is a truthful exposed wait, never a global conga.
    r.weather_retry_until = now + dwell(id, r.epoch);
    false
}

fn optional_capacity(
    round: &Round,
    residents: &Residents,
    world: &World,
    time: WorldTime,
    held: &BTreeSet<ActorId>,
) -> usize {
    let eligible = residents
        .order
        .iter()
        .filter(|id| {
            world.is_present(id)
                && !world.custody.holds(id)
                && !held.contains(*id)
                && !retiring(id, time)
                && world
                    .characters
                    .get(*id)
                    .is_some_and(|c| c.state.intent.is_none())
                && !round.weather_shelter_intents.contains_key(*id)
                && residents
                    .people
                    .get(*id)
                    .is_some_and(|r| r.weather.is_none())
                && !round.lightning_reflex_until.contains_key(*id)
                && !world.custody.is_escorting(id)
                && world
                    .characters
                    .get(*id)
                    .is_some_and(|c| !c.state.leaving_city)
        })
        .count();
    if eligible == 0 {
        0
    } else {
        (eligible / 10).max(1)
    }
}

pub(super) fn tick(
    round: &mut Round,
    world: &mut World,
    nav: &NavData,
    clock: &WorldClock,
    now: f64,
    held: &BTreeSet<ActorId>,
    nudges: &mut Vec<ActorId>,
) {
    if round.residents.people.is_empty() {
        return;
    }
    let time = clock.at(now);
    let mut residents = std::mem::take(&mut round.residents);
    let cap = optional_capacity(round, &residents, world, time, held);
    // Rotate priority after every admitted departure. A continuously-ready
    // lower id cannot take the next newly freed slot ahead of all its peers.
    let count = residents.order.len();
    let start = residents.cursor;
    for offset in 0..count {
        let index = (start + offset) % count;
        let id = residents.order[index].clone();
        let Some(mut r) = residents.people.remove(&id) else {
            continue;
        };
        let Some(c) = world.characters.get(&id) else {
            residents.reservations.release_actor(&id);
            if residents.optional.remove(&id) {
                residents.departing.remove(&r.patch);
            }
            continue;
        };
        let position = c.position_m();
        if !world.is_present(&id)
            || world.custody.holds(&id)
            || world.characters[&id].state.leaving_city
        {
            stop_owned(round, world, &mut residents, &id, &mut r);
            residents.reservations.release_actor(&id);
            r.spot = None;
            r.phase = ResidentPhase::Interrupted;
            residents.people.insert(id, r);
            continue;
        }
        if residents.reservations.release_departed(&id, position) {
            r.spot = None;
        }
        let curfew = matches!(time.office, Office::Snuffing | Office::Watch)
            && round.people[&id].home.is_some();
        if curfew && world.characters[&id].state.intent.is_some() {
            end_intent(
                round,
                world,
                &id,
                now,
                crate::receipts::ReceiptState::Interrupted,
                "The curfew turned you back toward your household frontage.".into(),
                nudges,
            );
        }
        // A genuine conversation cancels optional feet, even between ladder
        // deadlines. The caller also interrupts at the moment speech arrives.
        if held.contains(&id)
            && !(curfew && r.spot.is_none())
            && (world.characters[&id].state.intent.is_none()
                || !round.people[&id].travel_for_intent)
        {
            if !r.weather.as_ref().is_some_and(|w| w.arrived) {
                stop_owned(round, world, &mut residents, &id, &mut r);
            }
            r.phase = ResidentPhase::Interrupted;
            residents.people.insert(id, r);
            continue;
        }
        let should_rest = retiring(&id, time);
        if world.custody.is_escorting(&id) && world.characters[&id].state.intent.is_none() {
            stop_owned(round, world, &mut residents, &id, &mut r);
            r.phase = ResidentPhase::Interrupted;
            residents.people.insert(id, r);
            continue;
        }
        // Keep held/offered food real. No default stall or well rung follows.
        if now >= round.people[&id].next_decision
            && world.characters[&id].needs().hunger < HUNGER_HUNGRY
            && let Some(item) = held_edible(round, world, &world.characters[&id])
        {
            silent_eat(world, &id, &item);
            round.people.get_mut(&id).unwrap().next_decision = now + decision_jitter(&id, r.epoch);
        }
        // Lightning interrupts even between ladder deadlines. It owns no
        // resident claim and never resumes optional feet in the same poll.
        if round.lightning_reflex_until.contains_key(&id) {
            stop_owned(round, world, &mut residents, &id, &mut r);
            apply_decision(round, world, nav, &id, Decision::WeatherPause);
            r.phase = ResidentPhase::Interrupted;
            residents.people.insert(id, r);
            continue;
        }
        // The resident-aware decision skips generic curfew/hunger/round/wander,
        // retaining explicit intent, lightning, and existing weather ownership.
        let due = now >= round.people[&id].next_decision;
        let decision = if due {
            decide(round, world, nav, &id, r.epoch, time).0
        } else {
            Decision::Stay
        };
        let external = world.characters[&id].state.intent.is_some()
            || round.weather_shelter_intents.contains_key(&id)
            || matches!(
                decision,
                Decision::TravelIntent(_) | Decision::SeekShelter(_) | Decision::WeatherPause
            );
        if external {
            stop_owned(round, world, &mut residents, &id, &mut r);
            r.phase = ResidentPhase::Interrupted;
            let same = match &decision {
                Decision::TravelIntent(p) => {
                    round.people[&id].travel_for_intent
                        && round.people[&id].travel_target == Some(*p)
                }
                Decision::SeekShelter(i) => round.people[&id].travel_target == Some(i.target),
                _ => false,
            };
            if !same && now >= round.people[&id].next_decision {
                apply_decision(round, world, nav, &id, decision);
                round.people.get_mut(&id).unwrap().next_decision =
                    now + decision_jitter(&id, r.epoch);
            }
            residents.people.insert(id, r);
            continue;
        }
        if due {
            round.people.get_mut(&id).unwrap().next_decision = now + decision_jitter(&id, r.epoch);
        }
        if weather_tick(
            round,
            world,
            nav,
            &mut residents,
            &id,
            &mut r,
            time,
            now,
            due,
            curfew,
        ) {
            residents.people.insert(id, r);
            continue;
        }
        if residents.optional.len() > cap && residents.optional.contains(&id) {
            stop_owned(round, world, &mut residents, &id, &mut r);
            settle(&mut r, &id, now);
        }
        if world.characters[&id].is_walking() && r.target.is_none() {
            r.phase = ResidentPhase::Interrupted;
            residents.people.insert(id, r);
            continue;
        }
        if r.target.is_some() {
            if world.characters[&id].is_walking() {
                residents.people.insert(id, r);
                continue;
            }
            let arrived = residents.reservations.settle(&id, position);
            let target = r.target;
            if arrived {
                r.spot = target;
            }
            stop_owned(round, world, &mut residents, &id, &mut r);
            settle(&mut r, &id, now);
            // Failure is a long backoff at the actual position, never arrival
            // at a snapped node and never immediate corrective thrashing.
            residents.people.insert(id, r);
            continue;
        }
        let patch = &nav.resident_places().patches[r.patch];
        if r.spot.is_some_and(|s| !patch.spots[s].contains(position)) {
            r.spot = None;
        }
        if r.phase == ResidentPhase::Interrupted {
            // Reclaim an exact currently occupied spot if possible, then dwell.
            if r.spot.is_none() {
                for (s, spot) in patch.spots.iter().enumerate() {
                    if spot.contains(position)
                        && residents.reservations.occupy(&id, &spot.id, position)
                    {
                        r.spot = Some(s);
                        break;
                    }
                }
            }
            settle(&mut r, &id, now);
        }
        if r.retiring != should_rest {
            r.retiring = should_rest;
            if should_rest {
                // A private safe frontage preference, never the door node.
                if r.spot != Some(r.preferred_spot) {
                    let preferred = r.preferred_spot;
                    if start_move(
                        round,
                        world,
                        nav,
                        &mut residents,
                        &id,
                        &mut r,
                        preferred,
                        false,
                    ) {
                        residents.people.insert(id, r);
                        continue;
                    }
                }
            }
            // No vacant rest target is fine: another reserved position in this
            // same verified household frontage is already a truthful rest proxy.
            settle(&mut r, &id, now);
        }
        if now < r.dwell_until || (r.retiring && r.spot.is_some()) {
            residents.people.insert(id, r);
            continue;
        }
        if r.spot.is_none() {
            // A released explicit activity gets one committed exact return,
            // with bounded local connectors to the measured street graph.
            let mut moved = false;
            for s in 0..patch.spots.len() {
                if start_move(round, world, nav, &mut residents, &id, &mut r, s, false) {
                    moved = true;
                    break;
                }
            }
            if !moved {
                settle(&mut r, &id, now);
            }
            residents.people.insert(id, r);
            continue;
        }
        if !r.retiring
            && residents.optional.len() < cap
            && !residents.departing.contains(&r.patch)
            && hash01("resident_move", &id, r.epoch) < 0.65
        {
            let first =
                (hash01("resident_target", &id, r.epoch) * patch.spots.len() as f64) as usize;
            for o in 0..patch.spots.len() {
                let s = (first + o) % patch.spots.len();
                let distance = position.distance(patch.spots[s].position());
                if r.spot == Some(s) || !(1.6..=8.1).contains(&distance) {
                    continue;
                }
                if start_move(round, world, nav, &mut residents, &id, &mut r, s, true) {
                    residents.cursor = (index + 1) % count;
                    break;
                }
            }
        }
        if r.target.is_none() {
            settle(&mut r, &id, now);
        }
        residents.people.insert(id, r);
    }
    // Shelter/intent admission can shrink eligibility during this same poll.
    // Enforce the final bound as well as the admission-time bound.
    let final_cap = optional_capacity(round, &residents, world, time, held);
    while residents.optional.len() > final_cap {
        let id = residents.optional.last().unwrap().clone();
        let mut r = residents.people.remove(&id).unwrap();
        stop_owned(round, world, &mut residents, &id, &mut r);
        settle(&mut r, &id, now);
        residents.people.insert(id, r);
    }
    round.residents = residents;
    project(round, world, nav, now);
}

#[cfg(test)]
mod tests;
