//! Opt-in movement evidence. Positions on the two sides of one normal poll
//! establish actual motion; a pending path, proximity to a post, and turning
//! alone do not. No routine reads this module's counters or reference cells.

use super::*;
use crate::{Control, Presence};

/// Retained at route commitment, so a wander inside a workplace's arrival
/// radius does not become a stationary worker in the evidence.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MotionCause {
    Wander,
    Recall,
    Domestic,
    ExplicitIntent,
    Weather,
    Custody,
    Round,
    Water,
    Food,
    Market,
    Lamp,
    RoadParty,
    #[default]
    Other,
}

/// Disjoint motion totals, with overlapping context counters clearly separate.
#[derive(Debug, Default, Serialize)]
pub struct MotionPopulation {
    pub total: usize,
    pub present: usize,
    pub absent: usize,
    pub stationary: usize,
    pub optional_walking: usize,
    pub domestic_walking: usize,
    pub explicit_intent_walking: usize,
    pub emergency_custody_walking: usize,
    pub routine_needs_walking: usize,
    pub other_walking: usize,
    pub walking_by_cause: BTreeMap<MotionCause, usize>,
    pub pending_path: usize,
    /// Sanity witness alongside displacement (not used as the classifier).
    pub positive_speed: usize,
    pub stationary_with_path: usize,
    /// There is no indoors Presence in the current sim. Keep this null and
    /// retain every InCity actor in `present`, including the home proxy below.
    pub indoors: Option<usize>,
    pub resting_at_home_proxy: usize,
    /// Resident-specific outdoor proxy. These bodies remain in `present` and
    /// the original door-distance proxy above keeps its old meaning.
    pub residents: usize,
    pub resident_lingering: usize,
    pub resident_resting_at_household_frontage: usize,
    pub resident_resting_without_home: usize,
    pub resident_occupied_patches: BTreeMap<String, usize>,
    pub resident_destination_claims: usize,
    pub housed: usize,
    pub with_occupation: usize,
    pub with_round_legs: usize,
    pub water_queue: usize,
    pub food_queue: usize,
    pub queues: BTreeMap<String, usize>,
    pub famished: usize,
    pub hunger_mean: f64,
    /// Baseline cells, NOT reserved resident patches. Stable 20 m XZ bins let
    /// later runs expose migration without inventing not-yet-baked geometry.
    pub occupied_cells: BTreeMap<String, usize>,
    pub in_initial_cell: usize,
    pub away_from_initial_15m: usize,
    /// Rectangle-rule integration at the explicitly reported sample cadence.
    /// These measure time away, not the amount of walking between samples.
    pub sampled_away_seconds: f64,
    pub longest_sampled_away_episode_seconds: f64,
}

#[derive(Debug, Serialize)]
pub struct MotionCensus {
    pub elapsed_seconds: f64,
    pub sample_interval_seconds: f64,
    pub game_days: f64,
    pub office: String,
    pub weather: Option<String>,
    pub generated: MotionPopulation,
    pub authored: MotionPopulation,
}

#[derive(Debug, Clone)]
struct Reference {
    position: Vec3,
    away_seconds: f64,
    current_away_seconds: f64,
    longest_away_seconds: f64,
}

/// Host-owned, pure diagnostic state. Construct before the first poll to
/// preserve the generated spawn distribution as a fixed baseline reference.
#[derive(Debug, Default)]
pub struct MotionProbe {
    initial: BTreeMap<ActorId, Reference>,
    last_sample: Option<f64>,
}

/// Position and controller before the measured movement slice. The ladder runs
/// after movement and may finish or replace a route in the same poll.
#[derive(Debug)]
pub struct MotionBefore {
    position: Vec3,
    cause: MotionCause,
}

pub type MotionPositions = BTreeMap<ActorId, MotionBefore>;

/// Capture immediately before the single poll being measured. This avoids
/// aliasing a walker which leaves and returns between sparse census samples.
pub fn positions(world: &World, round: &Round) -> MotionPositions {
    world
        .characters
        .iter()
        .filter(|(_, c)| c.control() != Control::Player)
        .map(|(id, c)| {
            (
                id.clone(),
                MotionBefore {
                    position: c.position_m(),
                    cause: round.motion_cause(world, id),
                },
            )
        })
        .collect()
}

fn cell(point: Vec3) -> String {
    format!(
        "{}:{}",
        (point.x / 20.0).floor() as i64,
        (point.z / 20.0).floor() as i64
    )
}

fn planar_distance(a: Vec3, b: Vec3) -> f64 {
    (a.x - b.x).hypot(a.z - b.z)
}

impl MotionProbe {
    pub fn new(world: &World) -> Self {
        Self {
            initial: world
                .characters
                .iter()
                .filter(|(_, c)| c.control() != Control::Player)
                .map(|(id, c)| {
                    (
                        id.clone(),
                        Reference {
                            position: c.position_m(),
                            away_seconds: 0.0,
                            current_away_seconds: 0.0,
                            longest_away_seconds: 0.0,
                        },
                    )
                })
                .collect(),
            last_sample: None,
        }
    }

    pub(crate) fn sample(
        &mut self,
        world: &World,
        round: &Round,
        clock: &WorldClock,
        now: f64,
        before: &MotionPositions,
    ) -> MotionCensus {
        let interval = self.last_sample.map_or(0.0, |last| (now - last).max(0.0));
        self.last_sample = Some(now);
        let mut census = MotionCensus {
            elapsed_seconds: now,
            sample_interval_seconds: interval,
            game_days: clock.game_days(now),
            office: clock.at(now).office.label().into(),
            weather: world.current_weather.map(|w| w.kind.to_string()),
            generated: MotionPopulation::default(),
            authored: MotionPopulation::default(),
        };
        // Index actual queue membership once, including a currently served
        // member if still in the queue. No per-actor scan of a whole queue.
        let mut queues: BTreeMap<ActorId, Vec<(bool, String)>> = BTreeMap::new();
        for source in &round.sources {
            for id in &source.queue {
                queues
                    .entry(id.clone())
                    .or_default()
                    .push((true, format!("water:{}", source.name)));
            }
        }
        for stall in &round.stalls {
            for id in &stall.queue {
                queues
                    .entry(id.clone())
                    .or_default()
                    .push((false, format!("food:{}", stall.name)));
            }
        }
        for (id, character) in &world.characters {
            if character.control() == Control::Player {
                continue;
            }
            let generated = character.lore().is_some_and(|l| l.generated);
            let population = if generated {
                &mut census.generated
            } else {
                &mut census.authored
            };
            population.total += 1;
            if character.state.presence != Presence::InCity {
                population.absent += 1;
                continue;
            }
            population.present += 1;
            let position = character.position_m();
            let moved = before
                .get(id)
                .is_some_and(|previous| planar_distance(previous.position, position) > 1e-7);
            let pending = character.is_walking();
            population.pending_path += usize::from(pending);
            population.positive_speed += usize::from(
                character
                    .state
                    .movement
                    .as_ref()
                    .is_some_and(|m| m.speed > 1e-7),
            );
            population.stationary_with_path += usize::from(pending && !moved);
            if moved {
                population.count_walker(before[id].cause);
            } else {
                population.stationary += 1;
            }
            let person = round.people.get(id);
            population.housed += usize::from(person.is_some_and(|p| p.home.is_some()));
            population.with_round_legs += usize::from(person.is_some_and(|p| !p.legs.is_empty()));
            population.with_occupation +=
                usize::from(character.lore().is_some_and(|l| l.occupation_id.is_some()));
            population.resting_at_home_proxy += usize::from(
                !moved
                    && person.is_some_and(|p| {
                        p.home
                            .is_some_and(|home| position.distance(home) <= HOME_ARRIVE_RADIUS_M)
                    }),
            );
            if let Some(resident) = round.residents.people.get(id) {
                population.residents += 1;
                let claims = round.residents.reservations.claims(id);
                population.resident_destination_claims +=
                    usize::from(claims.is_some_and(|c| c.destination.is_some()));
                let settled = !moved
                    && !pending
                    && resident.spot.is_some()
                    && claims.is_some_and(|c| c.occupied.is_some());
                population.resident_lingering +=
                    usize::from(settled && resident.phase == residents::ResidentPhase::Lingering);
                let resting = settled && resident.phase == residents::ResidentPhase::Resting;
                let housed = person.is_some_and(|p| p.home.is_some());
                population.resident_resting_at_household_frontage += usize::from(resting && housed);
                population.resident_resting_without_home += usize::from(resting && !housed);
                if resident.spot.is_some()
                    && let Some(crate::crowd::GeneratedRoutine::Resident { patch, .. }) =
                        character.lore().and_then(|l| l.generated_routine.as_ref())
                {
                    *population
                        .resident_occupied_patches
                        .entry(patch.clone())
                        .or_default() += 1;
                }
            }
            if let Some(memberships) = queues.get(id) {
                for (water, name) in memberships {
                    if *water {
                        population.water_queue += 1;
                    } else {
                        population.food_queue += 1;
                    }
                    *population.queues.entry(name.clone()).or_default() += 1;
                }
            }
            population.famished += usize::from(character.needs().hunger < HUNGER_FAMISHED);
            population.hunger_mean += character.needs().hunger;
            let current_cell = cell(position);
            *population
                .occupied_cells
                .entry(current_cell.clone())
                .or_default() += 1;
            let reference = self.initial.entry(id.clone()).or_insert(Reference {
                position,
                away_seconds: 0.0,
                current_away_seconds: 0.0,
                longest_away_seconds: 0.0,
            });
            population.in_initial_cell += usize::from(current_cell == cell(reference.position));
            let away = planar_distance(reference.position, position) > 15.0;
            population.away_from_initial_15m += usize::from(away);
            if away {
                reference.away_seconds += interval;
                reference.current_away_seconds += interval;
                reference.longest_away_seconds = reference
                    .longest_away_seconds
                    .max(reference.current_away_seconds);
            } else {
                reference.current_away_seconds = 0.0;
            }
            population.sampled_away_seconds += reference.away_seconds;
            population.longest_sampled_away_episode_seconds = population
                .longest_sampled_away_episode_seconds
                .max(reference.longest_away_seconds);
        }
        for population in [&mut census.generated, &mut census.authored] {
            if population.present > 0 {
                population.hunger_mean /= population.present as f64;
            }
        }
        census
    }
}

impl MotionPopulation {
    fn count_walker(&mut self, cause: MotionCause) {
        *self.walking_by_cause.entry(cause).or_default() += 1;
        match cause {
            MotionCause::Wander | MotionCause::Recall => self.optional_walking += 1,
            MotionCause::Domestic => self.domestic_walking += 1,
            MotionCause::ExplicitIntent => self.explicit_intent_walking += 1,
            MotionCause::Weather | MotionCause::Custody => self.emergency_custody_walking += 1,
            MotionCause::Other => self.other_walking += 1,
            _ => self.routine_needs_walking += 1,
        }
    }
}

impl Round {
    /// The controller of a committed journey, independent of whether its body
    /// moved in this slice. Custody followers have no path of their own.
    pub fn motion_cause(&self, world: &World, id: &ActorId) -> MotionCause {
        if !world.custody.is_empty() && (world.custody.holds(id) || world.custody.is_escorting(id))
        {
            return MotionCause::Custody;
        }
        if world
            .characters
            .get(id)
            .is_some_and(|c| c.state.leaving_city)
        {
            return MotionCause::RoadParty;
        }
        if self.market_errands.contains_key(id) {
            return MotionCause::Market;
        }
        let Some(person) = self.people.get(id) else {
            return MotionCause::Other;
        };
        if person.food.is_some() {
            return MotionCause::Food;
        }
        if person.travel_for_intent && person.phase == Phase::Travelling {
            return MotionCause::ExplicitIntent;
        }
        match person.phase {
            Phase::Approaching | Phase::Queued | Phase::Drawing => MotionCause::Water,
            Phase::Returning => {
                if person.is_household {
                    MotionCause::Domestic
                } else {
                    MotionCause::Water
                }
            }
            _ => person.motion_cause,
        }
    }
}
