use super::*;
use crate::lore::{CONTROLLED_CIRCUMSTANCES, SUPPORT_CIRCUMSTANCES};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn nav() -> &'static NavData {
    static NAV: std::sync::OnceLock<NavData> = std::sync::OnceLock::new();
    NAV.get_or_init(|| {
        NavData::from_parts(
            include_str!("../../../../assets/world/navigation.json"),
            include_bytes!("../../../../assets/world/navigation.bin"),
        )
        .unwrap()
    })
}

#[test]
fn deterministic_residents_are_supported_jobless_varied_and_interactable() {
    let crowd = generate_ambient(nav(), 2000, 0, &[], &[]).unwrap();
    assert_eq!(
        crowd.sheets,
        generate_ambient(nav(), 2000, 0, &[], &[]).unwrap().sheets
    );
    assert_eq!(crowd.placement.placed, 2000);
    assert!((450..550).contains(&crowd.placement.hardship));
    assert_eq!(crowd.placement.workers, 0);
    let mut names = BTreeSet::new();
    let mut bodies = BTreeSet::new();
    let mut wards = BTreeSet::new();
    for (index, c) in crowd.sheets.iter().enumerate() {
        assert_eq!(c.id.as_str(), format!("x{index:05}"));
        let l = c.lore.as_ref().unwrap();
        assert!(l.generated && l.significance == Significance::Ambient);
        assert!(
            l.occupation_id.is_none()
                && l.occupation_display.is_none()
                && l.title.is_none()
                && l.rank.is_none()
        );
        assert!(matches!(
            l.generated_routine,
            Some(GeneratedRoutine::Resident { .. })
        ));
        assert!(c.knows.is_empty() && c.control == Control::Llm && c.voice_key.is_some());
        assert!(
            l.circumstances
                .iter()
                .all(|v| CONTROLLED_CIRCUMSTANCES.contains(&v.as_str()))
        );
        assert!(
            l.circumstances
                .iter()
                .any(|v| SUPPORT_CIRCUMSTANCES.contains(&v.as_str()))
        );
        let poor = l.circumstances.iter().any(|v| v == "pauper");
        assert_eq!(l.home.is_none(), poor);
        assert_eq!(l.home.is_some(), l.home_point_m.is_some());
        assert_eq!(
            c.appearance.outfit == crate::appearance::OutfitClass::Poor,
            poor
        );
        if let Some(home) = &l.home {
            assert!(home.starts_with(&format!(
                "a house in the {}",
                district_of_ward(l.planning_ward)
            )));
        }
        assert!(l.district.starts_with(ward_name(l.planning_ward)));
        assert!(!c.back_story.contains("daily wage"));
        assert!(!c.back_story.contains("doorway you stand in"));
        assert!(
            !l.circumstances
                .iter()
                .any(|v| v == if l.gender == "f" { "widower" } else { "widow" })
        );
        names.insert(&c.name);
        bodies.insert(format!(
            "{:?}/{:?}/{:?}",
            c.appearance.build, c.appearance.outfit, c.appearance.headgear
        ));
        wards.insert(l.planning_ward);
    }
    assert!(names.len() > 1000);
    assert!(bodies.len() >= 10);
    assert_eq!(wards.len(), 8);
}

#[test]
fn capacity_is_bounded_balanced_and_never_stacked() {
    for count in [1000, 2000, 20_000] {
        let crowd = generate_ambient(nav(), count, 0, &[], &[]).unwrap();
        println!(
            "placement {}",
            serde_json::to_string(&crowd.placement).unwrap()
        );
        assert_eq!(
            crowd.placement.requested,
            crowd.placement.placed + crowd.placement.unplaced
        );
        if count <= 2000 {
            assert_eq!(crowd.placement.unplaced, 0);
        } else {
            assert!(
                crowd.placement.placed <= nav().resident_places().capacity()
                    && crowd.placement.unplaced == count - crowd.placement.placed
            );
        }
        let mut doors = BTreeMap::<String, usize>::new();
        let mut spots = BTreeSet::new();
        let mut patches = BTreeSet::new();
        for c in &crowd.sheets {
            let l = c.lore.as_ref().unwrap();
            let Some(GeneratedRoutine::Resident { patch, spot }) = &l.generated_routine else {
                panic!()
            };
            assert!(spots.insert(spot));
            patches.insert(patch);
            let p = nav().resident_places().patch_for_spot(spot).unwrap();
            assert_eq!(p.id, *patch);
            assert!(
                p.spots
                    .iter()
                    .any(|s| s.id == *spot && s.contains(c.position_m))
            );
            if l.home.is_some() {
                *doors.entry(p.home_building.clone().unwrap()).or_default() += 1;
            }
        }
        assert!(doors.values().all(|n| *n <= crowd.placement.door_cap));
        if count <= 2000 {
            assert!(doors.len() > 700);
            assert!(patches.len() > count * 3 / 4);
            assert_eq!(crowd.placement.door_cap, count / 1000);
        }
    }
}

#[test]
fn blockers_remove_spots_and_missing_catalogue_places_nobody() {
    let first = generate_ambient(nav(), 32, 0, &[], &[]).unwrap();
    let blocked: Vec<_> = first.sheets.iter().map(|s| s.position_m).collect();
    let next = generate_ambient(nav(), 32, 0, &blocked, &[]).unwrap();
    assert_eq!(next.placement.placed, 32);
    assert!(
        next.sheets
            .iter()
            .all(|s| blocked.iter().all(|b| s.position_m.distance(*b) >= 1.6))
    );
    let mut json: serde_json::Value =
        serde_json::from_str(include_str!("../../../../assets/world/navigation.json")).unwrap();
    json.as_object_mut().unwrap().remove("resident_places");
    let empty = NavData::from_parts(
        &json.to_string(),
        include_bytes!("../../../../assets/world/navigation.bin"),
    )
    .unwrap();
    let result = generate_ambient(&empty, 20_000, 0, &[], &[]).unwrap();
    assert_eq!(result.placement.placed, 0);
    assert_eq!(result.placement.unplaced, 20_000);
}

#[test]
fn workers_require_an_explicit_compatible_occupation_and_workplace() {
    let worker = WorkerOverride {
        index: 0,
        occupation: "mason".into(),
        workplace: "The Masons' Lodge".into(),
    };
    let rounds: serde_json::Value =
        serde_json::from_str(include_str!("../../../../assets/world/rounds.json")).unwrap();
    let workplace = rounds["workplaces"]["mason"][0]
        .as_str()
        .unwrap()
        .to_string();
    let mut worker = WorkerOverride {
        workplace,
        ..worker
    };
    let c = generate_ambient(nav(), 4, 0, &[], &[worker.clone()]).unwrap();
    assert_eq!(c.placement.workers, 1);
    assert_eq!(
        c.sheets[0].lore.as_ref().unwrap().occupation_id.as_deref(),
        Some("mason")
    );
    assert!(matches!(
        c.sheets[0].lore.as_ref().unwrap().generated_routine,
        Some(GeneratedRoutine::Worker { .. })
    ));
    worker.workplace = "The Wickmarket".into();
    assert!(generate_ambient(nav(), 4, 0, &[], &[worker.clone()]).is_err());
    worker.occupation = "invented_job".into();
    assert!(generate_ambient(nav(), 4, 0, &[], &[worker]).is_err());
}
