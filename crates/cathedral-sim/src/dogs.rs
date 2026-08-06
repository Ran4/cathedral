//! Street dogs — sim-owned wanderers that are *seen* but never simulated as
//! minds (`features/implemented/dogs.md`).
//!
//! A dog is deliberately none of the things a [`Character`](crate::character)
//! is: it has no sheet, no inbox, no `knows`, takes no turns, enters no
//! scheduler lane and never appears in `characters_within` — so it can never
//! cost a token, wake the novelty gate, or receive speech. What it *does* do
//! is stand in the world with a position, drift about its own few streets on
//! the same walkable bitset the cast uses, and appear on every nearby sheet
//! under `**dogs_nearby**` — unconditionally, with no `knows` gating, because
//! nobody needs an introduction to see a dog.
//!
//! The pack is hand-authored like the cast (the no-procedural-characters
//! rule): each dog has a name, a coat and a home patch, anchored where the
//! city already asserts dogs — the market bark anchors, the Sparr furnace
//! yard, the Shambles. Motion is pure [`hash01`]-style rolls, never an RNG,
//! so a replayed world walks every dog down the same lanes.
//!
//! Positions reach the host on their own hot channel
//! ([`EngineMessage::Dogs`](crate::engine::EngineMessage::Dogs)), the
//! `Movement`/`Lamps` shape: republished whole when any dog moves, never
//! bumping `world_revision`.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::ids::DogId;
use crate::math::Vec3;
use crate::nav::{NavData, WALK_Y};

/// A dog's amble — slower than the cast's [`WALK_SPEED_MPS`](crate::WALK_SPEED_MPS)
/// (2.1): a street dog drifts, it does not march.
pub const DOG_TROT_MPS: f64 = 1.7;

/// Gait cycles per metre — quicker than the human `GAIT_CADENCE` (0.67)
/// because the stride is half as long.
pub const DOG_GAIT_CADENCE: f64 = 1.15;

/// Below this a dog reads as standing — the sheet's `moving` flag and the
/// host's gait blend both use it, mirroring `SETTLED_SPEED_MPS` for people.
pub const DOG_SETTLED_MPS: f64 = 0.15;

/// The coat, for the host's material choice. Render vocabulary only — the
/// sheet renders the authored `description` prose instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DogCoat {
    Brindle,
    Black,
    Grey,
    Fawn,
    White,
    Pied,
}

/// One street dog. World state like a character's position: stepped on the
/// movement tick, never part of the cold public snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct Dog {
    pub id: DogId,
    /// The authored name — for logs and the lore record. Never rendered on a
    /// sheet: people know *of* the dog, not its kennel name.
    pub name: String,
    /// The noun phrase every sheet shows, lowercase, article included:
    /// `a lean brindle dog`.
    pub description: String,
    pub coat: DogCoat,
    /// Body scale for the host's puppet, `1.0` = a middling street dog.
    pub build: f32,
    /// The home patch the leash circles.
    pub base: Vec3,
    pub leash_m: f64,
    pub position_m: Vec3,
    pub facing_yaw: f64,
    /// Current amble speed in m/s; `0.0` while resting.
    pub speed: f64,
    /// Advances with distance walked, like a character's — the host swings
    /// legs off it.
    pub gait_phase: f64,
    /// Remaining waypoints of the current drift.
    path: Vec<Vec3>,
    /// Seconds of sitting/sniffing left before the next drift.
    rest_s: f64,
    /// Decision counter salting the wander rolls.
    epoch: u64,
}

/// What the host sees of one dog, on the hot channel. The static fields
/// (name, coat, build) ride along so the first message is enough to spawn a
/// body from — ten dogs at 20 Hz is nothing.
#[derive(Debug, Clone, PartialEq)]
pub struct DogView {
    pub id: DogId,
    pub name: String,
    pub coat: DogCoat,
    pub build: f32,
    pub position_m: Vec3,
    pub facing_yaw: f64,
    pub speed: f32,
    pub gait_phase: f32,
}

impl Dog {
    pub fn view(&self) -> DogView {
        DogView {
            id: self.id.clone(),
            name: self.name.clone(),
            coat: self.coat,
            build: self.build,
            position_m: self.position_m,
            facing_yaw: self.facing_yaw,
            speed: self.speed as f32,
            gait_phase: self.gait_phase as f32,
        }
    }

    pub fn is_moving(&self) -> bool {
        self.speed >= DOG_SETTLED_MPS
    }
}

/// One authored row of the pack.
struct DogSeedRow {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    coat: DogCoat,
    build: f32,
    /// The anchor, XZ on the walk plane; snapped to the nearest graph node at
    /// seed time when the authored point misses the walkable bitset.
    x: f64,
    z: f64,
    leash_m: f64,
}

/// The whole dog population — authored, never spread, exactly like the cast.
/// Anchors sit where the city already asserts dogs: the six market-bark
/// anchors the soundscape plays (`src/soundscape.rs::MARKET_DOG_ANCHORS`),
/// Jonet Sparr's yard-dogs at the furnace yard (`lore/families/family_sparr.md`),
/// the Shambles' slaughter courts, the Eelback fish lanes, and the
/// cathedral's own forecourt paving.
const PACK: &[DogSeedRow] = &[
    DogSeedRow {
        id: "dog_wick",
        name: "Bracken",
        description: "a rangy brindle dog",
        coat: DogCoat::Brindle,
        build: 1.0,
        x: -43.4,
        z: 248.5,
        leash_m: 30.0,
    },
    DogSeedRow {
        id: "dog_tallage",
        name: "Marrow",
        description: "a heavy black dog with a grey muzzle",
        coat: DogCoat::Black,
        build: 1.12,
        x: -222.6,
        z: 63.0,
        leash_m: 28.0,
    },
    DogSeedRow {
        id: "dog_coswald",
        name: "Sedge",
        description: "a smoke-grey wolfhound",
        coat: DogCoat::Grey,
        build: 1.15,
        x: 249.4,
        z: 129.5,
        leash_m: 32.0,
    },
    DogSeedRow {
        id: "dog_green",
        name: "Pip",
        description: "a small white ratting dog",
        coat: DogCoat::White,
        build: 0.72,
        x: -221.2,
        z: -255.5,
        leash_m: 24.0,
    },
    DogSeedRow {
        id: "dog_stonegate",
        name: "Warden",
        description: "a broad-headed fawn mastiff",
        coat: DogCoat::Fawn,
        build: 1.2,
        x: 335.0,
        z: 94.5,
        leash_m: 26.0,
    },
    DogSeedRow {
        id: "dog_rivergate",
        name: "Eel",
        description: "a wiry black-and-white dog",
        coat: DogCoat::Pied,
        build: 0.9,
        x: -353.5,
        z: -94.5,
        leash_m: 26.0,
    },
    DogSeedRow {
        id: "dog_sparr",
        name: "Cinder",
        description: "a chain-galled tawny yard dog",
        coat: DogCoat::Fawn,
        build: 1.1,
        x: -112.0,
        z: 270.0,
        leash_m: 20.0,
    },
    DogSeedRow {
        id: "dog_shambles",
        name: "Gnaw",
        description: "a butcher's cur with a torn ear",
        coat: DogCoat::Brindle,
        build: 0.95,
        x: -294.0,
        z: 220.0,
        leash_m: 22.0,
    },
    DogSeedRow {
        id: "dog_eelback",
        name: "Smoke",
        description: "a grey-muzzled fish-lane dog",
        coat: DogCoat::Grey,
        build: 0.9,
        x: -275.5,
        z: -328.6,
        leash_m: 22.0,
    },
    DogSeedRow {
        id: "dog_forecourt",
        name: "Alms",
        description: "a patient dun beggar's dog",
        coat: DogCoat::Fawn,
        build: 0.95,
        x: 0.0,
        z: 100.0,
        leash_m: 28.0,
    },
];

/// Build the authored pack against the street graph. An anchor that misses
/// the walkable bitset (the city plan shifts under authored numbers) snaps to
/// the nearest graph node instead of being dropped — a dog with a slightly
/// moved home is right; a missing dog is a silent hole.
pub fn seed_pack(nav: &NavData) -> Vec<Dog> {
    PACK.iter()
        .map(|row| {
            let authored = Vec3::new(row.x, WALK_Y, row.z);
            let base = if nav.is_walkable(authored.x, authored.z) {
                authored
            } else {
                nav.nearest_node(authored.x, authored.z)
                    .map_or(authored, |node| nav.node_point(node))
            };
            let id = DogId::from_raw(row.id);
            // Stagger the first drift so the pack does not set off as one.
            let rest_s = 1.0 + dog_hash01("dog_first_rest", &id, 0) * 6.0;
            Dog {
                id,
                name: row.name.to_string(),
                description: row.description.to_string(),
                coat: row.coat,
                build: row.build,
                base,
                leash_m: row.leash_m,
                position_m: base,
                facing_yaw: 0.0,
                speed: 0.0,
                gait_phase: 0.0,
                path: Vec::new(),
                rest_s,
                epoch: 0,
            }
        })
        .collect()
}

/// Advance every dog by `dt` (one movement slice). Returns whether any pose
/// changed — the engine's cue to republish the hot channel.
///
/// The same shape as `World::step_movement`, minus everything a dog does not
/// have: no Needle claim (a dog slips between legs), no separation steering,
/// no lane discipline. Between drifts a dog rests a hashed few seconds where
/// it stands.
pub fn step_dogs(dogs: &mut [Dog], dt: f64, nav: &NavData) -> bool {
    let mut any_moved = false;
    for dog in dogs.iter_mut() {
        if dog.path.is_empty() {
            dog.speed = 0.0;
            dog.rest_s -= dt;
            if dog.rest_s > 0.0 {
                continue;
            }
            dog.epoch = dog.epoch.wrapping_add(1);
            match drift_path(nav, dog) {
                Some(path) => {
                    dog.path = path;
                    // The rest taken after *this* drift arrives, rolled now so
                    // arrival needs no second decision.
                    dog.rest_s = 1.5 + dog_hash01("dog_rest", &dog.id, dog.epoch) * 8.5;
                }
                // Every try landed on stone — sit a little and try again.
                None => {
                    dog.rest_s = 4.0;
                    continue;
                }
            }
        }

        let start = dog.position_m;
        let old_yaw = dog.facing_yaw;
        if let Some(&waypoint) = dog.path.first() {
            let to = Vec3::new(waypoint.x - start.x, 0.0, waypoint.z - start.z);
            let distance = to.length();
            let step = DOG_TROT_MPS * dt;
            if distance > 1e-9 {
                let dir = to / distance;
                // yaw 0 faces -Z, matching the rest of the codebase.
                dog.facing_yaw = (-dir.x).atan2(-dir.z);
            }
            if distance <= step {
                dog.position_m = Vec3::new(waypoint.x, WALK_Y, waypoint.z);
                dog.speed = if dt > 0.0 { distance / dt } else { 0.0 };
                dog.path.remove(0);
            } else {
                let dir = to / distance;
                dog.position_m = Vec3::new(start.x + dir.x * step, WALK_Y, start.z + dir.z * step);
                dog.speed = DOG_TROT_MPS;
            }
            dog.gait_phase += dog.speed * dt * DOG_GAIT_CADENCE;
        }
        if dog.position_m != start || dog.facing_yaw != old_yaw {
            any_moved = true;
        }
    }
    any_moved
}

/// A drift: a hashed walkable point within the leash, routed over the street
/// graph, the exact target appended as the final off-graph stride (the
/// `route_path_to_point` idea without the lane offset — dogs hold no lane) —
/// but only when the bitset vouches for the whole stride, not just its ends.
/// `None` when a few tries all land on stone.
fn drift_path(nav: &NavData, dog: &Dog) -> Option<Vec<Vec3>> {
    let target = (0..6).find_map(|attempt| {
        let angle = dog_hash01("dog_wander_angle", &dog.id, dog.epoch ^ (attempt as u64))
            * std::f64::consts::TAU;
        // 3 m minimum: a drift to your own feet is a rest, and rests are
        // already the other branch.
        let radius = 3.0
            + dog_hash01(
                "dog_wander_radius",
                &dog.id,
                dog.epoch.wrapping_add(attempt as u64),
            ) * (dog.leash_m - 3.0).max(0.0);
        let candidate = Vec3::new(
            dog.base.x + angle.cos() * radius,
            WALK_Y,
            dog.base.z + angle.sin() * radius,
        );
        nav.is_walkable(candidate.x, candidate.z)
            .then_some(candidate)
    })?;

    let mut path: Vec<Vec3> = nav
        .route_between(dog.position_m, target)
        .map(|route| route.points.clone())
        .unwrap_or_default();
    // Trim the leading node when it is where we already stand.
    if path
        .first()
        .is_some_and(|point| planar_close(*point, dog.position_m))
    {
        path.remove(0);
    }
    // The graph ends at a node; the sniff-worthy spot is a stride off it. That
    // stride is a straight line no graph edge vouches for, and a walkable
    // target says nothing about a wall or canal edge between the node and it —
    // so walk the stretch across the bitset first, and let a blocked stride
    // end the drift at the node instead.
    if !path
        .last()
        .is_some_and(|point| planar_close(*point, target))
    {
        let from = path.last().copied().unwrap_or(dog.position_m);
        if nav.segment_walkable(from, target) {
            path.push(target);
        }
    }
    if path.is_empty() { None } else { Some(path) }
}

fn planar_close(a: Vec3, b: Vec3) -> bool {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    dx * dx + dz * dz < 0.25
}

/// [`crate::world::hash01`] for a dog id — the same pure roll, salted by the
/// dog's own id so the pack never moves in lockstep.
fn dog_hash01(salt: &str, id: &DogId, epoch: u64) -> f64 {
    let mut hasher = DefaultHasher::new();
    salt.hash(&mut hasher);
    id.as_str().hash(&mut hasher);
    epoch.hash(&mut hasher);
    (hasher.finish() >> 11) as f64 / (1u64 << 53) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pack_ids_are_unique_and_descriptions_read_as_noun_phrases() {
        let mut ids: Vec<&str> = PACK.iter().map(|row| row.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), PACK.len(), "duplicate dog id in the pack");
        for row in PACK {
            assert!(
                row.description.starts_with("a "),
                "'{}' must be an article-led noun phrase",
                row.description
            );
            assert!(row.leash_m > 3.0, "a leash must leave room to drift");
        }
    }

    #[test]
    fn hashed_rolls_are_deterministic_and_in_range() {
        let id = DogId::from_raw("dog_wick");
        for epoch in 0..64 {
            let roll = dog_hash01("dog_rest", &id, epoch);
            assert!((0.0..1.0).contains(&roll));
            assert_eq!(roll, dog_hash01("dog_rest", &id, epoch));
        }
    }

    /// The final off-graph stride is a straight line no graph edge vouches
    /// for: a walkable target across an unwalkable band used to be appended
    /// anyway, and the dog cut straight through. Every sampled point of a
    /// drift must be walkable, not merely its waypoints.
    #[test]
    fn a_drift_never_strides_through_stone() {
        // Two shores of walkable ground with a 3 m unwalkable band between
        // them: a pocket around the dog — deliberately smaller than the 3 m
        // minimum drift radius, so no hashed target can land in it — holding
        // the only graph node, and open ground from x = 6 on. Every accepted
        // target therefore lies across the band with both endpoints walkable:
        // exactly the layout where the old append cut through.
        let (w, h) = (40usize, 40usize);
        let (x0, z0, cell) = (-20.0, -20.0, 1.0);
        let mut bitset = vec![0u8; (w * h).div_ceil(8)];
        for row in 0..h {
            for col in 0..w {
                let cx = x0 + (col as f64 + 0.5) * cell;
                let cz = z0 + (row as f64 + 0.5) * cell;
                let walkable = cx >= 6.0 || ((cx - 0.5).abs() <= 2.0 && (cz - 0.5).abs() <= 1.0);
                if walkable {
                    let idx = row * w + col;
                    bitset[idx >> 3] |= 1 << (7 - (idx & 7));
                }
            }
        }
        let nav_json = format!(
            r#"{{
              "schema_version": 1,
              "grid": {{"x0": {x0}, "z0": {z0}, "cell_m": {cell}, "w": {w}, "h": {h},
                        "agent_radius_m": 0.35, "bitset_file": "x.bin",
                        "bitset_bits": {bits}, "bitset_sha256": ""}},
              "nodes": [[2.5, 0.5]],
              "edges": [],
              "places": [],
              "sites": [],
              "doors": [],
              "reference": {{"forecourt": 0}}
            }}"#,
            bits = w * h
        );
        let nav = NavData::from_parts(&nav_json, &bitset).expect("the two-shore nav validates");
        // The trap is live: both shores walkable, the band between them not.
        assert!(nav.is_walkable(10.0, 0.5), "the far shore must tempt");
        assert!(!nav.is_walkable(4.5, 0.5), "the band must bar the way");

        let mut dog = Dog {
            id: DogId::from_raw("dog_test"),
            name: "Test".to_string(),
            description: "a test dog".to_string(),
            coat: DogCoat::Brindle,
            build: 1.0,
            base: Vec3::new(0.5, WALK_Y, 0.5),
            leash_m: 12.0,
            position_m: Vec3::new(0.5, WALK_Y, 0.5),
            facing_yaw: 0.0,
            speed: 0.0,
            gait_phase: 0.0,
            path: Vec::new(),
            rest_s: 0.0,
            epoch: 0,
        };
        let mut drifts = 0;
        for epoch in 0..64 {
            dog.epoch = epoch;
            let Some(path) = drift_path(&nav, &dog) else {
                continue;
            };
            drifts += 1;
            // Every quarter-metre of every leg, from where the dog stands.
            let mut from = dog.position_m;
            for &to in &path {
                let steps = (to.distance(from) / 0.25).ceil().max(1.0) as usize;
                for s in 0..=steps {
                    let t = s as f64 / steps as f64;
                    let x = from.x + (to.x - from.x) * t;
                    let z = from.z + (to.z - from.z) * t;
                    assert!(
                        nav.is_walkable(x, z),
                        "epoch {epoch}: the drift strides over stone at ({x:.2}, {z:.2})"
                    );
                }
                from = to;
            }
        }
        assert!(drifts > 0, "the fixture never tempted a single drift");
    }
}
