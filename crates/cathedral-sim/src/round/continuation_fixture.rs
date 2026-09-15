//! Installed-content setup and observations for full Engine continuation tests.
//! Setup occurs before the captured ordinary poll; future work uses Engine::poll.
use super::tests::{person, stock};
use super::*;

fn add(w: &mut World, n: &NavData, id: &str, occupation: &str) {
    w.add_character(person(
        id,
        n.node_point(n.forecourt()),
        Some(occupation),
        Significance::Minor,
    ));
}
pub(crate) fn production(w: &mut World, r: &mut Round, n: &NavData, c: &WorldClock) {
    add(w, n, "e7mil", "miller");
    *r = Round::new();
    r.seed(w, n, 0.0, c);
    // Put ordinary seeded lingering at its deadline, so the initial poll owns
    // the real resident route/reservation before complete capture. The default
    // 45–150 second dwell would outlast this bounded production observation.
    for resident in r.residents.people.values_mut() {
        resident.dwell_until = 0.0;
    }
    let id = ActorId::from_raw("e7mil");
    let spec = r
        .production_plans
        .iter()
        .find(|p| p.producer == id)
        .unwrap()
        .transforms[0]
        .clone();
    w.characters.get_mut(&id).unwrap().state.position_m = spec.point;
    w.characters.get_mut(&id).unwrap().state.movement = None;
    w.add_stock(&id, &stock("grain", 1), "m2d_grain").unwrap();
    r.tick_production(w, c, 0.0, &BTreeSet::new());
    assert!(w.active_transform_job(&id).is_some());
}
pub(crate) fn production_finished(r: &Round) -> bool {
    r.food_log
        .iter()
        .any(|s| s.starts_with("transform_finish:"))
}
pub(crate) fn market(w: &mut World, r: &mut Round, n: &NavData, c: &WorldClock) {
    let pitch = n.node_point(n.place("The Wickmarket").unwrap().node);
    for (id, job) in [
        ("prov2", "food_provisioner"),
        ("hgry1", "mason"),
        ("hgry2", "mason"),
    ] {
        w.add_character(person(id, pitch, Some(job), Significance::Minor));
    }
    *r = Round::new();
    r.seed(w, n, 0.0, c);
    let seller = ActorId::from_raw("prov2");
    let index = r
        .stalls
        .iter()
        .position(|s| s.vendor.as_ref() == Some(&seller) && s.name.contains("provisions"))
        .unwrap();
    let pitch = r.stalls[index].pitch;
    w.characters.get_mut(&seller).unwrap().state.position_m = pitch;
    w.characters.get_mut(&seller).unwrap().state.needs.hunger = 200.0;
    w.add_stock(&seller, &stock("herring", 4), "m2d_market_stock")
        .unwrap();
    for name in ["hgry1", "hgry2"] {
        let id = ActorId::from_raw(name);
        let a = w.characters.get_mut(&id).unwrap();
        a.state.position_m = pitch;
        a.state.movement = None;
        a.state.needs.hunger = 8.0;
        w.credit_sparks(&id, 4, &format!("m2d_purse_{name}"))
            .unwrap();
        r.people.get_mut(&id).unwrap().food = Some(FoodErrand {
            stall: index,
            phase: FoodPhase::Queued,
        });
        r.stalls[index].queue.push(id);
    }
}
pub(crate) fn market_pending(r: &Round) -> bool {
    r.stalls.iter().any(|s| {
        s.queue == [ActorId::from_raw("hgry1"), ActorId::from_raw("hgry2")]
            && s.serving.as_ref().is_some_and(|(id, _)| id == &s.queue[0])
    })
}
pub(crate) fn market_finished(w: &World) -> bool {
    ["hgry1", "hgry2"].iter().all(|id| {
        w.characters[&ActorId::from_raw(*id)]
            .recent_history()
            .iter()
            .any(|s| s.starts_with("You bought a herring"))
    })
}
pub(crate) fn road(w: &mut World, r: &mut Round, n: &NavData, c: &WorldClock) {
    for (id, occupation) in [
        ("rbrde", "merchant"),
        ("cbred", "cargo_worker"),
        ("dbred", "cargo_worker"),
        ("rlant", "merchant"),
        ("clant", "cargo_worker"),
    ] {
        add(w, n, id, occupation);
    }
    *r = Round::new();
    r.seed(w, n, 0.0, c);
    let id = PartyId::from_raw("brede_wool_gate");
    r.begin_road_return(w, &id, 2, 0.0, &mut Vec::new());
    let p = &r.road_parties[&id];
    for member in &p.members {
        let a = w.characters.get_mut(member).unwrap();
        a.state.position_m = p.gate_point;
        a.state.movement = None;
    }
}
pub(crate) fn road_pending(r: &Round) -> bool {
    r.road_parties[&PartyId::from_raw("brede_wool_gate")]
        .state
        .phase
        == PartyPhase::DeparturePending
}
pub(crate) fn road_away(r: &Round) -> bool {
    r.road_parties[&PartyId::from_raw("brede_wool_gate")]
        .state
        .phase
        == PartyPhase::BeyondTheWalls
}
