//! Explicit strict v1 records, separate from authoring defaults.
#![allow(dead_code)]
use super::*;
use crate::checkpoint::{
    records as common,
    serde_support::{remote_adapters, unique_map},
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(remote = "SpotClaims", deny_unknown_fields)]
pub(crate) struct SpotClaimsV1 {
    #[serde(with = "common::text::option")]
    occupied: Option<String>,
    #[serde(with = "common::text::option")]
    destination: Option<String>,
}
remote_adapters!(spot_claims, SpotClaims, SpotClaimsV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "SpotReservations", deny_unknown_fields)]
pub(crate) struct SpotReservationsV1 {
    #[serde(with = "crate::checkpoint::records::point::map")]
    positions: BTreeMap<String, Vec3>,
    #[serde(with = "unique_map")]
    owners: BTreeMap<String, ActorId>,
    #[serde(with = "crate::nav::residents::checkpoint::spot_claims::map")]
    actors: BTreeMap<ActorId, SpotClaims>,
}
remote_adapters!(spot_reservations, SpotReservations, SpotReservationsV1);

pub(crate) fn validate(
    value: &SpotReservations,
    places: Option<&ResidentPlaces>,
    actors: &BTreeSet<&ActorId>,
) -> crate::checkpoint::Result<()> {
    use crate::checkpoint::CheckpointError;
    let invalid = |s| CheckpointError::new("resident_reservations", s);
    match places {
        Some(places) => {
            let spots = || {
                places
                    .patches
                    .iter()
                    .flat_map(|p| &p.spots)
                    .chain(places.shelter_spots.iter().map(|s| &s.spot))
            };
            if value.positions.len() != spots().count()
                || !spots().all(|s| value.positions.get(&s.id) == Some(&s.position()))
            {
                return Err(invalid("spot geometry index mismatch"));
            }
        }
        None if !value.positions.is_empty() => {
            return Err(invalid("virgin spot geometry index is not empty"));
        }
        None => {}
    }
    let mut owners = BTreeMap::new();
    for (actor, claims) in &value.actors {
        if !actors.contains(actor) || (claims.occupied.is_none() && claims.destination.is_none()) {
            return Err(invalid("spot claimant missing/empty"));
        }
        for spot in [&claims.occupied, &claims.destination]
            .into_iter()
            .flatten()
        {
            if !value.positions.contains_key(spot) {
                return Err(invalid("unknown reserved spot"));
            }
            if let Some(previous) = owners.insert(spot.clone(), actor.clone()) {
                if previous != *actor {
                    return Err(invalid("spot reserved by multiple actors"));
                }
            }
        }
    }
    // One actor may reserve its own occupied destination; it is one owner.
    // Positions may be stale until release_departed; no body-position gate here.
    if owners != value.owners {
        return Err(invalid("spot owner/claim index mismatch"));
    }
    Ok(())
}
