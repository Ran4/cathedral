use super::*;
use crate::{
    checkpoint::{Admitted, CheckpointBudget, Cohort},
    round::checkpoint::{RoundCandidate, RoundCheckpointContext, RoundDtoV1},
};
fn saved(round: &Round, world: &World) -> (Admitted<Vec<u8>>, Admitted<RoundCandidate>) {
    let budget = CheckpointBudget::default();
    let context = RoundCheckpointContext::from_world(world, Some(nav()));
    let bytes = round
        .export_checkpoint(context, budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap();
    let decoded = RoundDtoV1::decode(
        bytes.value(),
        budget
            .reserve(Cohort::LoadCandidate, bytes.value().len() + 4096)
            .unwrap(),
        context,
    )
    .unwrap();
    let candidate = decoded.into_candidate(context).unwrap();
    assert_eq!(candidate.value().round(), round);
    (bytes, candidate)
}
#[test]
fn checkpoint_stale_post_speech_sample_resumes_fairness_and_support() {
    let (mut world, mut round, clock) = fixture(12, Office::Dayspring);
    for r in round.residents.people.values_mut() {
        r.dwell_until = 0.0;
    }
    slice(&mut world, &mut round, &clock, 0.05, &BTreeSet::new());
    let id = round.residents.optional.first().unwrap().clone();
    let status = world.characters[&id].state.resident.clone();
    super::super::interrupt(&mut round, &mut world, &id);
    assert_eq!(world.characters[&id].state.resident, status);
    assert_eq!(
        round.residents.people[&id].phase,
        ResidentPhase::Interrupted
    );
    assert_ne!(status.unwrap().phase, ResidentPhase::Interrupted);
    let (_bytes, candidate) = saved(&round, &world);
    let mut restored = candidate.value().round().clone();
    let mut other = world.clone();
    let held = BTreeSet::from([id.clone()]);
    for step in 2..=200 {
        let now = step as f64 * 0.05;
        slice(&mut world, &mut round, &clock, now, &held);
        slice(&mut other, &mut restored, &clock, now, &held);
        assert_eq!(round, restored);
        assert_eq!(world, other);
    }
    assert!(world.characters[&id].needs().hunger > 0.0);
    saved(&round, &world);
}
#[test]
fn checkpoint_occupied_destination_alias_and_clearance_resume_without_double_owner() {
    let (mut world, mut round, clock) = fixture(1, Office::Dayspring);
    let id = round.residents.order[0].clone();
    let origin = round
        .residents
        .reservations
        .claims(&id)
        .unwrap()
        .occupied
        .clone()
        .unwrap();
    assert!(
        round
            .residents
            .reservations
            .reserve_destination(&id, &origin)
    );
    let r = round.residents.people.get_mut(&id).unwrap();
    r.target = r.spot;
    let (_bytes, candidate) = saved(&round, &world);
    let mut restored = candidate.value().round().clone();
    assert!(
        round
            .residents
            .reservations
            .settle(&id, world.characters[&id].position_m())
    );
    assert!(
        restored
            .residents
            .reservations
            .settle(&id, world.characters[&id].position_m())
    );
    assert_eq!(
        round.residents.reservations,
        restored.residents.reservations
    );
    round.residents.people.get_mut(&id).unwrap().target = None;
    let origin = world.characters[&id].position_m();
    world.characters.get_mut(&id).unwrap().state.position_m.x += 0.3;
    round.residents.people.get_mut(&id).unwrap().spot = None;
    assert!(
        !round
            .residents
            .reservations
            .release_departed(&id, world.characters[&id].position_m())
    );
    saved(&round, &world); // the 0.15m spot cleared; the 0.85m ownership remains.
    world.characters.get_mut(&id).unwrap().state.position_m = origin;
    slice(&mut world, &mut round, &clock, 0.05, &BTreeSet::new());
    saved(&round, &world);
}
#[test]
fn checkpoint_weather_transit_and_arrived_claims_continue() {
    let (mut world, mut round, clock) = fixture(1000, Office::Dayspring);
    world.shelters = std::sync::Arc::new(
        crate::ShelterMap::from_json_str(include_str!("../../../../../assets/world/shelters.json"))
            .unwrap(),
    );
    super::super::seed(&mut round, &mut world, nav(), clock.at(0.0), 0.0);
    world.current_weather = Some(WeatherSample {
        kind: WeatherKind::Downpour,
        precipitation: 0.9,
        ..WeatherSample::CLEAR
    });
    let first = (1..=1200)
        .find(|step| {
            slice(
                &mut world,
                &mut round,
                &clock,
                *step as f64 * 0.05,
                &BTreeSet::new(),
            );
            round.residents.people.values().any(|r| r.weather.is_some())
        })
        .expect("ordinary resident weather acquisition becomes due");
    let (_bytes, candidate) = saved(&round, &world);
    let mut restored = candidate.value().round().clone();
    let mut other = world.clone();
    for step in first + 1..=first + 1200 {
        let now = step as f64 * 0.05;
        slice(&mut world, &mut round, &clock, now, &BTreeSet::new());
        slice(&mut other, &mut restored, &clock, now, &BTreeSet::new());
    }
    assert_eq!(round, restored);
    assert_eq!(world, other);
    assert!(
        round
            .residents
            .people
            .values()
            .any(|r| r.weather.as_ref().is_some_and(|w| w.arrived))
    );
    saved(&round, &world);
}
