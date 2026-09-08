use super::*;
use crate::checkpoint::{CheckpointBudget, Cohort};
use serde_json::{Value, json};
use std::sync::Arc;
pub(crate) fn nav() -> Arc<NavData> {
    Arc::new(NavData::from_parts(r#"{"schema_version":1,"grid":{"x0":-64.0,"z0":-64.0,"cell_m":1.0,"w":128,"h":128,"agent_radius_m":0.35,"bitset_file":"x.bin","bitset_bits":16384,"bitset_sha256":""},"nodes":[[0.0,0.0],[10.0,0.0],[20.0,0.0]],"edges":[[0,1,2.0],[1,2,2.0]],"places":[],"sites":[],"doors":[],"reference":{"forecourt":0}}"#, &[255; 2048]).unwrap())
}
pub(crate) fn dog(id: &str) -> Dog {
    Dog {
        id: DogId::from_raw(id),
        name: "A former keeper's name".into(),
        description: "a dog with a private journey".into(),
        coat: DogCoat::Pied,
        build: 0.8,
        base: Vec3::new(0.0, WALK_Y, 0.0),
        leash_m: 12.0,
        position_m: Vec3::new(0.0, WALK_Y, 0.0),
        facing_yaw: -std::f64::consts::FRAC_PI_2,
        speed: 0.0,
        gait_phase: 13.25,
        path: vec![],
        rest_s: 2.0,
        epoch: 17,
    }
}
pub(crate) fn active() -> World {
    let mut w = World::new();
    w.nav = Some(nav());
    let mut moving = dog("moving");
    moving.path = vec![Vec3::new(2.0, WALK_Y, 0.0), Vec3::new(3.0, WALK_Y, 1.0)];
    moving.speed = 0.09;
    moving.rest_s = 5.25;
    let mut turning = dog("turning");
    turning.coat = DogCoat::Black;
    turning.facing_yaw = std::f64::consts::FRAC_PI_2;
    turning.path = vec![Vec3::new(4.0, WALK_Y, 0.0)];
    let mut stop = dog("stop");
    stop.coat = DogCoat::Grey;
    stop.speed = 0.08;
    let mut retry = dog("retry");
    retry.coat = DogCoat::Fawn;
    retry.base = Vec3::new(900.0, 11.0, 900.0);
    retry.rest_s = -0.05;
    retry.epoch = u64::MAX;
    let mut wrap = dog("wrap");
    wrap.coat = DogCoat::White;
    wrap.rest_s = -0.0;
    wrap.epoch = u64::MAX;
    let mut rest = dog("rest");
    rest.coat = DogCoat::Brindle;
    rest.gait_phase = -0.0;
    w.dogs = vec![moving, turning, stop, retry, wrap, rest];
    w
}
fn now() -> LogicalTime {
    LogicalTime::new(10.0).unwrap()
}
fn saved(w: &World) -> Admitted<Vec<u8>> {
    let b = CheckpointBudget::default();
    w.export_animals_checkpoint(now(), b.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap()
}
fn decoded(bytes: &[u8], w: &World, b: &CheckpointBudget) -> Result<Admitted<WorldAnimalsDtoV1>> {
    WorldAnimalsDtoV1::decode(
        bytes,
        b.reserve(Cohort::LoadCandidate, bytes.len() + 4096)?,
        AnimalsCheckpointContext::from_world(w, now()),
    )
}
#[test]
fn checkpoint_animals_closed_records_and_all_required_fields() {
    let w = active();
    let original: Value = serde_json::from_slice(saved(&w).value()).unwrap();
    for path in ["", "/context", "/dogs/0"] {
        for key in original.pointer(path).unwrap().as_object().unwrap().keys() {
            let mut v = original.clone();
            v.pointer_mut(path)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            let b = CheckpointBudget::default();
            assert!(
                decoded(&serde_json::to_vec(&v).unwrap(), &w, &b).is_err(),
                "missing {path}/{key}"
            );
            assert_eq!(b.retained_bytes(), 0);
        }
        let mut v = original.clone();
        v.pointer_mut(path)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unknown".into(), json!(0));
        let b = CheckpointBudget::default();
        assert!(decoded(&serde_json::to_vec(&v).unwrap(), &w, &b).is_err());
        assert_eq!(b.retained_bytes(), 0);
    }
    let raw = String::from_utf8(saved(&w).value().clone()).unwrap();
    for raw in [
        raw.replacen("\"speed\":", "\"speed\":0.0,\"speed\":", 1),
        raw.replacen("\"nav\":", "\"nav\":null,\"nav\":", 1),
    ] {
        let b = CheckpointBudget::default();
        assert!(decoded(raw.as_bytes(), &w, &b).is_err());
        assert_eq!(b.retained_bytes(), 0);
    }
    for (path, bad) in [
        ("/version", json!(2)),
        ("/dogs/0/coat", json!("red")),
        ("/dogs/0/epoch", json!(-1)),
        ("/dogs/0/rest_s", Value::Null),
        ("/dogs/0/path", json!([[0, 0, 0, 0]])),
        ("/dogs/0/base", json!([1_000_001.0, 0, 0])),
        ("/dogs/0/id", json!("")),
    ] {
        let mut v = original.clone();
        *v.pointer_mut(path).unwrap() = bad;
        let b = CheckpointBudget::default();
        assert!(
            decoded(&serde_json::to_vec(&v).unwrap(), &w, &b).is_err(),
            "{path}"
        );
        assert_eq!(b.retained_bytes(), 0);
    }
}
#[test]
fn checkpoint_animals_independent_scrambled_restore_then_all_motion_edges() {
    let mut control = active();
    let mut resumed = active();
    let b = CheckpointBudget::default();
    let bytes = saved(&control);
    let c = AnimalsCheckpointContext::from_world(&resumed, now());
    let d = decoded(bytes.value(), &resumed, &b)
        .unwrap()
        .into_candidate(c)
        .unwrap();
    resumed.dogs = vec![dog("wrong")];
    resumed.dogs[0].epoch = 999;
    resumed.dogs[0].path.push(Vec3::ZERO);
    resumed.dogs = d.value().data.dogs.clone();
    assert_eq!(
        saved(&resumed).value(),
        bytes.value(),
        "canonical equality before ordinary stepping"
    );
    let nav = control.nav.clone().unwrap();
    let rev = control.world_revision;
    let old_turn = control.dogs[1].facing_yaw;
    assert_eq!(
        step_dogs(&mut control.dogs, 0.05, &nav),
        step_dogs(&mut resumed.dogs, 0.05, &nav)
    );
    assert_eq!(control.dogs, resumed.dogs);
    assert!(control.dogs[0].speed > 0.09 && control.dogs[0].speed < DOG_TROT_MPS);
    assert_ne!(control.dogs[1].facing_yaw, old_turn);
    assert_eq!(control.dogs[1].speed, 0.0);
    assert_eq!(control.dogs[2].speed, 0.0);
    assert_eq!(control.dogs[3].epoch, 0);
    assert_eq!(control.dogs[3].rest_s, 4.0);
    assert_eq!(control.dogs[4].epoch, 0);
    assert!(!control.dogs[4].path.is_empty());
    let mut arrival = false;
    let mut next = false;
    for _ in 0..2400 {
        assert_eq!(
            step_dogs(&mut control.dogs, 0.05, &nav),
            step_dogs(&mut resumed.dogs, 0.05, &nav)
        );
        assert_eq!(control.dogs, resumed.dogs);
        arrival |= control.dogs[0].path.is_empty() && control.dogs[0].speed > 0.0;
        next |= control.dogs[0].epoch > 17;
    }
    assert!(arrival && next);
    assert_eq!(control.world_revision, rev);
}
#[test]
fn checkpoint_animals_preserves_signed_public_edits_without_normalization() {
    let mut w = active();
    let d = &mut w.dogs[0];
    d.id = DogId::from_raw("Pärs dog 空");
    d.build = -1.5;
    d.leash_m = -3.0;
    d.speed = -0.5;
    d.gait_phase = -80.5;
    d.facing_yaw = 900.0;
    d.rest_s = -70.0;
    d.position_m.y = 44.0;
    d.path[0].y = -22.0;
    let b = CheckpointBudget::default();
    let d = decoded(saved(&w).value(), &w, &b).unwrap();
    assert_eq!(d.value().dogs, w.dogs);
    for field in 0..6 {
        let mut w = active();
        match field {
            0 => w.dogs[0].build = f32::NAN,
            1 => w.dogs[0].leash_m = f64::INFINITY,
            2 => w.dogs[0].speed = f64::NAN,
            3 => w.dogs[0].gait_phase = f64::INFINITY,
            4 => w.dogs[0].facing_yaw = f64::NAN,
            _ => w.dogs[0].rest_s = f64::NEG_INFINITY,
        };
        let b = CheckpointBudget::default();
        assert!(
            w.export_animals_checkpoint(now(), b.reserve(Cohort::SavePayload, 4096).unwrap())
                .is_err()
        );
        assert_eq!(b.retained_bytes(), 0);
    }
}
#[test]
fn checkpoint_animals_signed_rest_from_negative_direct_dt_is_authority() {
    let mut w = active();
    w.dogs = vec![dog("signed_dt")];
    w.dogs[0].rest_s = -0.25;
    // Public step_dogs accepts a signed dt. Its resting branch adds time back.
    assert!(!step_dogs(&mut w.dogs, -0.5, &nav()));
    assert_eq!(w.dogs[0].rest_s, 0.25);
    let b = CheckpointBudget::default();
    let d = decoded(saved(&w).value(), &w, &b).unwrap();
    assert_eq!(d.value().dogs, w.dogs);
}
#[test]
fn checkpoint_animals_maximum_path_and_collection_refusal() {
    let mut w = World::new();
    let mut d = dog("long_path");
    d.path = vec![Vec3::ZERO; MAX_WAYPOINTS];
    w.dogs.push(d);
    let b = CheckpointBudget::default();
    let d = decoded(saved(&w).value(), &w, &b).unwrap();
    assert_eq!(d.value().dogs[0].path.len(), MAX_WAYPOINTS);
    drop(d);
    w.dogs[0].path.push(Vec3::ZERO);
    assert!(
        w.export_animals_checkpoint(now(), b.reserve(Cohort::SavePayload, 4096).unwrap())
            .is_err()
    );
    assert_eq!(b.retained_bytes(), 0);
    let d = dog("same");
    assert!(validate(&vec![d; MAX_DOGS + 1]).is_err());
    let maximum: Vec<_> = (0..MAX_DOGS).map(|i| dog(&format!("dog_{i}"))).collect();
    validate(&maximum).unwrap();
}
#[test]
fn checkpoint_animals_golden_world() {
    assert_eq!(
        saved(&active()).value().as_slice(),
        include_bytes!("../../../tests/fixtures/checkpoint_v1/world_animals.json")
    );
}
#[test]
#[ignore = "create only the new M2a8 component fixture"]
fn create_animals_world_fixture() {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/checkpoint_v1/world_animals.json");
    assert!(!p.exists() || p.metadata().unwrap().len() == 0);
    std::fs::write(p, saved(&active()).value()).unwrap();
}
#[test]
#[ignore = "closed owner layout diagnostic"]
fn checkpoint_animals_layout() {
    println!(
        "Dog={} DogCoat={} Vec3={} WorldDto={} WorldCandidate={} DogId={} BorrowedId={}",
        std::mem::size_of::<Dog>(),
        std::mem::size_of::<DogCoat>(),
        std::mem::size_of::<Vec3>(),
        std::mem::size_of::<WorldAnimalsDtoV1>(),
        std::mem::size_of::<WorldAnimalsCandidate>(),
        std::mem::size_of::<DogId>(),
        std::mem::size_of::<&DogId>()
    );
}
