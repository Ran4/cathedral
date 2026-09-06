//! Baked neighbourhood geometry and authoritative, bounded spot ownership.
//! No automatic enrollment: the resident routine decides when to claim/move.
use super::{NavData, NavError, WALK_Y};
use crate::{ActorId, math::Vec3};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub const SPOT_SPACING_M: f64 = 1.6;
/// The mover's final separation shove is at most 0.045 m at 20 Hz. Tolerate it
/// without a new trip; even opposite 0.15 m drifts leave 1.3 m between centres.
pub const SPOT_ARRIVAL_M: f64 = 0.15;
pub const DEPARTED_CLEARANCE_M: f64 = 0.85;

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct ResidentPlaces {
    pub patches: Vec<ResidentPatch>,
    #[serde(default)]
    pub shelter_spots: Vec<ShelterSpot>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ShelterSpot {
    pub shelter: String,
    pub spot: StandingSpot,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ResidentPatch {
    pub id: String,
    pub building: String,
    pub description: String,
    pub capacity: usize,
    pub spots: Vec<StandingSpot>,
    /// Validated path from spots[0] to a reachable street graph node.
    pub graph_node: usize,
    pub graph_path: Vec<[f64; 2]>,
    /// Optional nearby reachable door. The path includes its exact graph point;
    /// this is access metadata, not permission to stand in the door approach.
    pub home_building: Option<String>,
    pub home_path: Vec<[f64; 2]>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct StandingSpot {
    pub id: String,
    pub xz: [f64; 2],
    pub facing_yaw: f64,
}

impl StandingSpot {
    pub fn position(&self) -> Vec3 {
        Vec3::new(self.xz[0], WALK_Y, self.xz[1])
    }
    pub fn contains(&self, position: Vec3) -> bool {
        position.is_finite()
            && Vec3::new(position.x, WALK_Y, position.z).distance(self.position()) <= SPOT_ARRIVAL_M
    }
}

impl ResidentPlaces {
    pub fn capacity(&self) -> usize {
        self.patches.iter().map(|p| p.spots.len()).sum()
    }
    pub fn spot(&self, id: &str) -> Option<&StandingSpot> {
        self.patches
            .iter()
            .flat_map(|p| &p.spots)
            .find(|s| s.id == id)
    }
    pub fn patch_for_spot(&self, id: &str) -> Option<&ResidentPatch> {
        self.patches
            .iter()
            .find(|p| p.spots.iter().any(|s| s.id == id))
    }
    pub(crate) fn validate(&self, nav: &NavData) -> Result<(), NavError> {
        let mut ids = BTreeSet::new();
        let mut buckets: HashMap<(i64, i64), Vec<Vec3>> = HashMap::new();
        let point = |p: [f64; 2]| Vec3::new(p[0], WALK_Y, p[1]);
        let path_ok = |path: &[[f64; 2]], from: Vec3, to: Vec3, limit: f64| {
            path.first()
                .is_some_and(|&p| point(p).distance(from) < 1e-6)
                && path.last().is_some_and(|&p| point(p).distance(to) < 1e-6)
                && path
                    .windows(2)
                    .all(|p| nav.segment_walkable_exact(point(p[0]), point(p[1])))
                && path
                    .windows(2)
                    .map(|p| point(p[0]).distance(point(p[1])))
                    .sum::<f64>()
                    <= limit
        };
        for p in &self.patches {
            let invalid = || NavError::new(format!("invalid resident patch {}", p.id));
            if p.id.is_empty()
                || !ids.insert(p.id.clone())
                || p.description.is_empty()
                || !(2..=6).contains(&p.capacity)
                || p.capacity != p.spots.len()
                || p.graph_node >= nav.node_count()
            {
                return Err(invalid());
            }
            let anchor = p.spots[0].position();
            if !path_ok(&p.graph_path, anchor, nav.node_point(p.graph_node), 30.0) {
                return Err(invalid());
            }
            let max_offset = p
                .spots
                .iter()
                .map(|s| s.position().distance(anchor))
                .fold(0.0, f64::max);
            match &p.home_building {
                Some(id) => {
                    let door = nav.door(id).ok_or_else(invalid)?;
                    if !path_ok(
                        &p.home_path,
                        anchor,
                        nav.node_point(door.node),
                        30.0 - max_offset,
                    ) {
                        return Err(invalid());
                    }
                }
                None if !p.home_path.is_empty() => return Err(invalid()),
                None => {}
            }
            for s in &p.spots {
                if s.id.is_empty()
                    || !ids.insert(s.id.clone())
                    || !s.facing_yaw.is_finite()
                    || !nav.segment_walkable_exact(s.position(), anchor)
                    || s.position().distance(anchor) > 15.0
                {
                    return Err(invalid());
                }
                let b = (
                    (s.xz[0] / SPOT_SPACING_M).floor() as i64,
                    (s.xz[1] / SPOT_SPACING_M).floor() as i64,
                );
                for dx in -1..=1 {
                    for dz in -1..=1 {
                        if buckets.get(&(b.0 + dx, b.1 + dz)).is_some_and(|ps| {
                            ps.iter()
                                .any(|p| p.distance(s.position()) < SPOT_SPACING_M - 1e-6)
                        }) {
                            return Err(NavError::new(format!(
                                "resident spot {} overlaps another spot",
                                s.id
                            )));
                        }
                    }
                }
                buckets.entry(b).or_default().push(s.position());
            }
        }
        for sheltered in &self.shelter_spots {
            let s = &sheltered.spot;
            if sheltered.shelter.is_empty()
                || !ids.insert(s.id.clone())
                || !s.facing_yaw.is_finite()
                || !nav.segment_walkable_exact(s.position(), s.position())
            {
                return Err(NavError::new(format!(
                    "invalid resident shelter spot {}",
                    s.id
                )));
            }
            let b = (
                (s.xz[0] / SPOT_SPACING_M).floor() as i64,
                (s.xz[1] / SPOT_SPACING_M).floor() as i64,
            );
            for dx in -1..=1 {
                for dz in -1..=1 {
                    if buckets.get(&(b.0 + dx, b.1 + dz)).is_some_and(|ps| {
                        ps.iter()
                            .any(|p| p.distance(s.position()) < SPOT_SPACING_M - 1e-6)
                    }) {
                        return Err(NavError::new(format!(
                            "overlapping resident shelter spot {}",
                            s.id
                        )));
                    }
                }
            }
            buckets.entry(b).or_default().push(s.position());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpotClaims {
    pub occupied: Option<String>,
    pub destination: Option<String>,
}

/// One current spot plus at most one destination per actor. Updates are atomic
/// in the single-threaded sim; sequential same-poll arrivals cannot double-book.
/// Geometry is indexed once here rather than searched for every resident poll.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SpotReservations {
    positions: BTreeMap<String, Vec3>,
    owners: BTreeMap<String, ActorId>,
    actors: BTreeMap<ActorId, SpotClaims>,
}

impl SpotReservations {
    pub fn new(places: &ResidentPlaces) -> Self {
        Self {
            positions: places
                .patches
                .iter()
                .flat_map(|p| &p.spots)
                .chain(places.shelter_spots.iter().map(|s| &s.spot))
                .map(|s| (s.id.clone(), s.position()))
                .collect(),
            ..Self::default()
        }
    }
    pub fn owner(&self, spot: &str) -> Option<&ActorId> {
        self.owners.get(spot)
    }
    pub fn claims(&self, actor: &ActorId) -> Option<&SpotClaims> {
        self.actors.get(actor)
    }
    pub fn occupied_count(&self) -> usize {
        self.actors
            .values()
            .filter(|c| c.occupied.is_some())
            .count()
    }
    pub fn destination_count(&self) -> usize {
        self.actors
            .values()
            .filter(|c| c.destination.is_some())
            .count()
    }
    pub fn is_available(&self, spot: &str) -> bool {
        self.positions.contains_key(spot) && !self.owners.contains_key(spot)
    }

    pub fn occupy(&mut self, actor: &ActorId, spot: &str, position: Vec3) -> bool {
        if self.actors.contains_key(actor) || !self.is_available(spot) || !self.at(spot, position) {
            return false;
        }
        self.owners.insert(spot.into(), actor.clone());
        self.actors.insert(
            actor.clone(),
            SpotClaims {
                occupied: Some(spot.into()),
                destination: None,
            },
        );
        true
    }
    pub fn reserve_destination(&mut self, actor: &ActorId, spot: &str) -> bool {
        if !(self.is_available(spot) || self.owner(spot) == Some(actor))
            || self
                .actors
                .get(actor)
                .is_some_and(|c| c.destination.is_some())
        {
            return false;
        }
        self.owners.insert(spot.into(), actor.clone());
        self.actors.entry(actor.clone()).or_default().destination = Some(spot.into());
        true
    }
    fn at(&self, spot: &str, position: Vec3) -> bool {
        position.is_finite()
            && self.positions.get(spot).is_some_and(|p| {
                Vec3::new(position.x, WALK_Y, position.z).distance(*p) <= SPOT_ARRIVAL_M
            })
    }
    /// Release the old place only after the body has cleared it. Safe even if
    /// the optional route is interrupted; the caller later cancels its target.
    pub fn release_departed(&mut self, actor: &ActorId, position: Vec3) -> bool {
        let Some(old) = self.actors.get(actor).and_then(|c| c.occupied.clone()) else {
            return false;
        };
        if !position.is_finite()
            || Vec3::new(position.x, WALK_Y, position.z).distance(self.positions[&old])
                <= DEPARTED_CLEARANCE_M
        {
            return false;
        }
        let claims = self.actors.get_mut(actor).unwrap();
        if claims.destination.as_ref() != Some(&old) {
            self.owners.remove(&old);
        }
        claims.occupied = None;
        if claims.destination.is_none() {
            self.actors.remove(actor);
        }
        true
    }
    /// Call only once movement has ended. A small final avoidance displacement
    /// is arrival; a failed/truncated route somewhere else is not.
    pub fn settle(&mut self, actor: &ActorId, position: Vec3) -> bool {
        let Some(target) = self.actors.get(actor).and_then(|c| c.destination.clone()) else {
            return false;
        };
        if !self.at(&target, position) {
            return false;
        }
        let claims = self.actors.get_mut(actor).unwrap();
        if let Some(old) = claims.occupied.replace(target.clone())
            && old != target
        {
            self.owners.remove(&old);
        }
        claims.destination = None;
        true
    }
    pub fn cancel_destination(&mut self, actor: &ActorId) {
        if let Some(claims) = self.actors.get_mut(actor) {
            if let Some(target) = claims.destination.take() {
                if claims.occupied.as_ref() != Some(&target) {
                    self.owners.remove(&target);
                }
            }
            if claims.occupied.is_none() {
                self.actors.remove(actor);
            }
        }
    }
    /// Departure, custody and actor removal all use this same cleanup seam.
    pub fn release_actor(&mut self, actor: &ActorId) {
        if let Some(claims) = self.actors.remove(actor) {
            for spot in [claims.occupied, claims.destination].into_iter().flatten() {
                self.owners.remove(&spot);
            }
        }
    }
}
