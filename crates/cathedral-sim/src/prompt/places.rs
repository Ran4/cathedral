//! The character's familiar ways, with current walking estimates. Route tables
//! belong to NavData and are shared; only the short endpoint offsets and the
//! spoken rounding are recomputed when a character's sheet is rendered.

use serde::Serialize;

use crate::{
    PLACE_ARRIVE_RADIUS_M, WALK_SPEED_MPS, WALK_Y, character::Character, ids::PlaceId, math::Vec3,
    nav::NavData, places::PlaceEntry, py_round, world::World,
};

use super::PromptStrings;

/// `place_id` deliberately differs from a person's `id`.
#[derive(Serialize)]
pub(super) struct PlaceRef<'a> {
    pub place_id: &'a PlaceId,
    pub name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    walk: Option<WalkEstimate>,
}

#[derive(Serialize)]
struct WalkEstimate {
    distance_m: f64,
    minutes: f64,
}

pub(super) fn known_places<'a>(world: &'a World, actor: &Character) -> Vec<PlaceRef<'a>> {
    let position = actor.position_m();
    let origin = world.nav.as_deref().and_then(|nav| {
        nav.nearest_node(position.x, position.z)
            .map(|node| (nav, node))
    });
    // Resolve only held handles. Missing handles remain silently skipped and
    // knowledge never grows just because the cache contains another place.
    let mut places: Vec<_> = actor
        .state
        .places_known
        .iter()
        .filter_map(|id| world.places.get(id))
        .map(|place| PlaceRef {
            place_id: &place.id,
            name: &place.name,
            walk: origin.and_then(|(nav, node)| walking_estimate(nav, node, position, place)),
        })
        .collect();
    places.sort_by(|left, right| {
        left.name
            .cmp(right.name)
            .then_with(|| left.place_id.cmp(right.place_id))
    });
    places
}

fn walking_estimate(
    nav: &NavData,
    start: usize,
    position: Vec3,
    place: &PlaceEntry,
) -> Option<WalkEstimate> {
    let goal = nav.nearest_node(place.point.x, place.point.z)?;
    // Check reachability before calling even a nearby place "right here".
    let street_m = nav.cached_distance_m(start, goal)?;
    if position.distance(place.point) <= PLACE_ARRIVE_RADIUS_M {
        return Some(WalkEstimate {
            distance_m: 0.0,
            minutes: 0.0,
        });
    }
    let from = Vec3::new(position.x, WALK_Y, position.z);
    let to = Vec3::new(place.point.x, WALK_Y, place.point.z);
    let approach_m = from.distance(nav.node_point(start));
    let tail_m = to.distance(nav.node_point(goal));
    // The movement layer appends an off-graph destination only on walkable
    // ground. A blocked endpoint too far from its node cannot be reached.
    let tail_m = if nav.is_walkable(to.x, to.z) {
        tail_m
    } else if tail_m <= PLACE_ARRIVE_RADIUS_M {
        0.0
    } else {
        return None;
    };
    let distance_m = approach_m + street_m + tail_m;
    Some(WalkEstimate {
        distance_m: ((distance_m / 10.0).round() * 10.0).max(10.0),
        // Walking is real-time; the office clock and its debug scale are not
        // involved. Use the unrounded route length before rounding minutes.
        minutes: py_round(distance_m / WALK_SPEED_MPS / 60.0, 1).max(0.1),
    })
}

pub(super) fn place_md(place: &PlaceRef<'_>, strings: &PromptStrings) -> String {
    let walk = match &place.walk {
        Some(walk) if walk.distance_m == 0.0 => strings.place_here.clone(),
        Some(walk) => format!("{:.0} m — {} min", walk.distance_m, walk.minutes),
        None => strings.place_walk_unavailable.clone(),
    };
    format!("{} — {} — {walk}", place.place_id, place.name)
}
