//! Explicit strict v1 records, separate from authoring defaults.
#![allow(dead_code, private_interfaces)] // Internal remote adapters retain runtime owner privacy.
use super::*;
use crate::checkpoint::{
    records as common,
    serde_support::{remote_adapters, required_option, unique_map, unique_set},
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(remote = "Resident", deny_unknown_fields)]
pub(crate) struct ResidentV1 {
    patch: usize,
    preferred_spot: usize,
    #[serde(deserialize_with = "required_option")]
    spot: Option<usize>,
    #[serde(deserialize_with = "required_option")]
    target: Option<usize>,
    phase: ResidentPhase,
    dwell_until: f64,
    epoch: u64,
    retiring: bool,
    support_was_eligible: bool,
    missed_meal: bool,
    recovery_remaining: f64,
    #[serde(with = "crate::round::residents::checkpoint::resident_weather::option")]
    weather: Option<ResidentWeather>,
    weather_retry_until: f64,
    #[serde(with = "crate::round::residents::checkpoint::projection_anchor::option")]
    projection: Option<ProjectionAnchor>,
}
remote_adapters!(resident, Resident, ResidentV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "ResidentWeather", deny_unknown_fields)]
pub(crate) struct ResidentWeatherV1 {
    slot: usize,
    #[serde(with = "crate::round::checkpoint::records::WeatherShelterIntentV1")]
    intent: WeatherShelterIntent,
    arrived: bool,
}
remote_adapters!(resident_weather, ResidentWeather, ResidentWeatherV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "Residents", deny_unknown_fields)]
pub(crate) struct ResidentsV1 {
    #[serde(with = "crate::round::residents::checkpoint::resident::map")]
    people: BTreeMap<ActorId, Resident>,
    #[serde(with = "crate::nav::residents::checkpoint::SpotReservationsV1")]
    reservations: SpotReservations,
    order: Vec<ActorId>,
    cursor: usize,
    #[serde(with = "unique_set")]
    optional: BTreeSet<ActorId>,
    #[serde(with = "unique_set")]
    changed: BTreeSet<ActorId>,
    #[serde(with = "unique_set")]
    departing: BTreeSet<usize>,
    #[serde(with = "unique_map")]
    local_shelters: BTreeMap<usize, Vec<(usize, usize)>>,
}
remote_adapters!(residents, Residents, ResidentsV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "ProjectionAnchor", deny_unknown_fields)]
pub(crate) struct ProjectionAnchorV1 {
    at: f64,
    phase: ResidentPhase,
    #[serde(deserialize_with = "required_option")]
    spot: Option<usize>,
    #[serde(deserialize_with = "required_option")]
    target: Option<usize>,
    #[serde(deserialize_with = "required_option")]
    weather_slot: Option<usize>,
    #[serde(with = "common::text::option")]
    occupied: Option<String>,
    dwell_until: f64,
    #[serde(with = "crate::math::vec3_serde")]
    position: Vec3,
    walking: bool,
    housed: bool,
    optional: bool,
    cause: motion::MotionCause,
    sheltered: bool,
}
remote_adapters!(projection_anchor, ProjectionAnchor, ProjectionAnchorV1);

pub(crate) fn validate(
    round: &Round,
    context: crate::round::checkpoint::RoundCheckpointContext<'_>,
) -> crate::checkpoint::Result<()> {
    use crate::checkpoint::{CheckpointError, logical};
    let invalid = |s| CheckpointError::new("residents", s);
    let check = |v: bool, s| if v { Ok(()) } else { Err(invalid(s)) };
    let r = &round.residents;
    check(
        r.people.len() <= crate::round::checkpoint::MAX_PEOPLE
            && r.order.len() <= crate::round::checkpoint::MAX_PEOPLE,
        "resident count limit",
    )?;
    let ids: BTreeSet<_> = r.people.keys().collect();
    let order: BTreeSet<_> = r.order.iter().collect();
    check(
        order.len() == r.order.len() && ids.is_subset(&order),
        "resident order duplicates/missing controller",
    )?;
    check(
        (r.order.is_empty() && r.cursor == 0) || r.cursor < r.order.len(),
        "resident cursor outside order",
    )?;
    for id in &r.order {
        check(id.is_valid(), "invalid historical resident order id")?;
    }
    for id in r.optional.iter().chain(&r.changed) {
        check(ids.contains(id), "resident sparse/optional actor missing")?;
    }
    let nav = context.nav;
    crate::nav::residents::checkpoint::validate(
        &r.reservations,
        nav.filter(|_| round.seeded).map(NavData::resident_places),
        &ids,
    )?;
    let Some(nav) = nav else {
        return check(
            r.people.is_empty() && r.local_shelters.is_empty() && r.departing.is_empty(),
            "resident geometry unavailable",
        );
    };
    let places = nav.resident_places();
    if round.seeded {
        check(
            r.local_shelters.len() == places.patches.len(),
            "local shelter index count mismatch",
        )?;
        for (p, patch) in places.patches.iter().enumerate() {
            let saved = r
                .local_shelters
                .get(&p)
                .ok_or_else(|| invalid("local shelter patch index missing"))?;
            check(
                local_shelter_candidates(patch, places, context.shelters).eq(saved.iter().copied()),
                "local shelter index mismatch",
            )?;
        }
    } else {
        check(
            r.local_shelters.is_empty(),
            "virgin shelter index not empty",
        )?;
    }
    let departing: BTreeSet<_> = r
        .optional
        .iter()
        .filter_map(|id| r.people.get(id).map(|p| p.patch))
        .collect();
    check(
        r.departing == departing,
        "optional departure patch index mismatch",
    )?;
    for (id, p) in &r.people {
        let character = context
            .backbone
            .characters
            .get(id)
            .ok_or_else(|| invalid("resident actor missing"))?;
        let person = round
            .people
            .get(id)
            .ok_or_else(|| invalid("resident townsperson missing"))?;
        let patch = places
            .patches
            .get(p.patch)
            .ok_or_else(|| invalid("resident patch missing"))?;
        check(
            p.preferred_spot < patch.spots.len(),
            "resident preferred spot missing",
        )?;
        check(
            character.lore().is_some_and(|l| {
                l.generated
                    && matches!(&l.generated_routine,
            Some(crate::crowd::GeneratedRoutine::Resident { patch: declared_patch, spot })
                if *declared_patch == patch.id && *spot == patch.spots[p.preferred_spot].id)
            }),
            "resident generated identity/preference mismatch",
        )?;
        let claims = r.reservations.claims(id);
        let destination = claims.and_then(|c| c.destination.as_deref());
        let occupied = claims.and_then(|c| c.occupied.as_deref());
        let target = p
            .target
            .map(|s| patch.spots.get(s).map(|s| s.id.as_str()))
            .flatten();
        if let Some(w) = &p.weather {
            let slot = places
                .shelter_spots
                .get(w.slot)
                .ok_or_else(|| invalid("resident weather spot missing"))?;
            if w.arrived {
                check(
                    occupied == Some(slot.spot.id.as_str()) && destination.is_none(),
                    "arrived weather claim mismatch",
                )?;
            } else {
                check(
                    destination == Some(slot.spot.id.as_str()),
                    "weather destination claim mismatch",
                )?;
            }
        } else {
            check(
                destination == target,
                "resident target/destination claim mismatch",
            )?;
        }
        if let Some(i) = p.spot {
            check(
                occupied == patch.spots.get(i).map(|s| s.id.as_str()),
                "resident occupied claim mismatch",
            )?;
        }
        // r.spot may already be cleared at 0.15m while the origin claim remains
        // until 0.85m departure clearance. Do not require its converse.

        for s in [p.spot, p.target].into_iter().flatten() {
            check(s < patch.spots.len(), "resident spot/target missing")?;
        }
        logical("residents", p.dwell_until)?;
        logical("residents", p.weather_retry_until)?;
        check(
            p.recovery_remaining.is_finite() && (0.0..=HUNGER_MAX).contains(&p.recovery_remaining),
            "invalid resident recovery balance",
        )?;
        if let Some(w) = &p.weather {
            let slot = places
                .shelter_spots
                .get(w.slot)
                .ok_or_else(|| invalid("resident shelter slot missing"))?;
            super::super::checkpoint::validate::weather(&w.intent, context)?;
            check(
                context.shelters.shelters()[w.intent.shelter].id == slot.shelter,
                "resident shelter slot/intent mismatch",
            )?;
        }
        check(
            character.state.daily_round == daily_round(&patch.description, person.home.is_some()),
            "resident daily projection mismatch",
        )?;
        let a = p
            .projection
            .as_ref()
            .ok_or_else(|| invalid("resident projection sample missing"))?;
        logical("residents", a.at)?;
        logical("residents", a.dwell_until)?;
        crate::character::checkpoint::point(a.position)?;
        for i in [a.spot, a.target].into_iter().flatten() {
            check(i < patch.spots.len(), "sampled resident spot missing")?;
        }
        if let Some(i) = a.weather_slot {
            check(
                i < places.shelter_spots.len(),
                "sampled resident shelter slot missing",
            )?;
        }
        if let Some(id) = &a.occupied {
            check(
                patch.spots.iter().any(|s| s.id == *id)
                    || places.shelter_spots.iter().any(|s| s.spot.id == *id),
                "sampled occupied spot missing",
            )?;
        }
        check(
            a.sheltered == context.shelters.is_sheltered(a.position),
            "sampled shelter projection mismatch",
        )?;
        check(
            character.state.resident.as_ref() == Some(&a.status(patch, nav)),
            "resident sampled projection mismatch",
        )?;
        // The saved sample owns the displayed values. interrupt and later Engine
        // actions can change the live phase/pose/claims after this sampling pass.
        // Never refresh the sample or test those stale values against live state.
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn check_layout() {
    for (name, n, max) in [
        ("Resident", std::mem::size_of::<Resident>(), 512),
        (
            "ResidentWeather",
            std::mem::size_of::<ResidentWeather>(),
            128,
        ),
        ("Residents", std::mem::size_of::<Residents>(), 256),
        (
            "ProjectionAnchor",
            std::mem::size_of::<ProjectionAnchor>(),
            192,
        ),
    ] {
        println!("{name}={n}");
        assert!(n <= max);
    }
}
