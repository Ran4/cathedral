use super::*;
use crate::checkpoint::{Cohort, MAX_RESIDENT_BYTES};

const ROW: &str = r#"{"id":"porch","label":"The porch","polygon_xz":[[0,0],[1,0],[0,1]],"route_node":0,"cover":"tile"}"#;
fn budget() -> (CheckpointBudget, Reservation) {
    let budget = CheckpointBudget::default();
    let root = budget.reserve(Cohort::Running, 4096).unwrap();
    (budget, root)
}

#[test]
fn installed_rows_defaults_order_debug_and_content_fingerprint_match_legacy() {
    let (budget, root) = budget();
    let admitted = ShelterMap::installed_admitted(&budget).unwrap();
    let legacy = ShelterMap::from_json_str(SOURCE).unwrap();
    assert_eq!(*admitted, legacy);
    assert_eq!(format!("{admitted:?}"), format!("{legacy:?}"));
    // Existing checkpoint fingerprints serialize precisely this ordered row slice.
    use sha2::{Digest, Sha256};
    let bytes = serde_json::to_vec(legacy.shelters()).unwrap();
    assert_eq!(bytes, serde_json::to_vec(admitted.shelters()).unwrap());
    println!(
        "installed_shelters source_bytes={} rows={} peak={} retained={} rows_sha256={:x}",
        SOURCE.len(),
        admitted.shelters().len(),
        Shape::scan(SOURCE).unwrap().peak().unwrap(),
        admitted.admitted_storage_bytes(),
        Sha256::digest(&bytes)
    );
    assert_eq!(legacy.admitted_storage_bytes(), 0);
    let source = format!(r#"{{"schema_version":1,"shelters":[{ROW}]}}"#);
    let defaults = ShelterMap::admit_source(&budget, &source).unwrap();
    assert_eq!(defaults.shelters()[0].capacity, 12);
    assert_eq!(defaults.shelters()[0].spread_radius_m, 2.0);
    assert!(defaults.shelters()[0].offices.is_empty());
    assert_eq!(
        defaults.shelters()[0].access,
        super::super::ShelterAccess::Public
    );
    let empty = ShelterMap::from_json_str(r#"{"schema_version":1,"shelters":[]}"#).unwrap();
    assert_eq!(empty, ShelterMap::default());
    assert!(ShelterMap::default().storage.is_none());
    drop(admitted);
    drop(defaults);
    assert_eq!(budget.retained_bytes(), root.bytes());
}

#[test]
fn pressure_precedes_parsing_and_errors_release_all_staging_storage() {
    let (budget, root) = budget();
    let pressure = budget
        .reserve(Cohort::LoadCandidate, MAX_RESIDENT_BYTES - root.bytes())
        .unwrap();
    assert!(matches!(
        ShelterMap::admit_source(&budget, "not JSON"),
        Err(ShelterAdmissionError::Admission)
    ));
    drop(pressure);
    for source in [
        "not JSON".to_owned(),
        format!(r#"{{"schema_version":1,"shelters":[{ROW},{ROW}]}}"#),
        format!(
            r#"{{"schema_version":1,"shelters":[{ROW}],"unknown":"{}"}}"#,
            "\\u0000".repeat(16_384)
        ),
        format!(
            r#"{{"schema_version":1,"shelters":[{{"id":"{}"#,
            "\\u0000".repeat(16_384)
        ),
    ] {
        assert!(matches!(
            ShelterMap::admit_source(&budget, &source),
            Err(ShelterAdmissionError::InvalidDefinition)
        ));
        assert_eq!(budget.retained_bytes(), root.bytes());
    }
}

#[test]
fn scanner_handles_escaped_delimiters_unfinished_tails_and_struct_sequences() {
    let shape = Shape::scan(r#"["[ { \" still string",{"key":"tail\\"} ]"#).unwrap();
    assert_eq!(shape.arrays, 1);
    assert_eq!(shape.objects, 1);
    assert_eq!(shape.strings, 3);
    let tail = Shape::scan("\"abc\\").unwrap();
    assert_eq!(tail.longest, 4);
    assert_eq!(tail.strings, 1);
    let source =
        r#"[1,[["porch","The porch",[[0,0],[1,0],[0,1]],0,"public",12,2.0,"tile",["watch"]]]]"#;
    let (budget, root) = budget();
    let admitted = ShelterMap::admit_source(&budget, source).unwrap();
    assert_eq!(*admitted, ShelterMap::from_json_str(source).unwrap());
    assert_eq!(admitted.shelters().len(), 1);
    drop(admitted);
    assert_eq!(budget.retained_bytes(), root.bytes());
}

#[test]
fn large_duplicate_diagnostic_and_malformed_types_stay_inside_staging() {
    let (budget, root) = budget();
    let id = "\\u0000".repeat(16_384);
    let row = ROW.replace("porch", &id);
    let duplicate = format!(r#"{{"schema_version":1,"shelters":[{row},{row}]}}"#);
    assert!(matches!(
        ShelterMap::admit_source(&budget, &duplicate),
        Err(ShelterAdmissionError::InvalidDefinition)
    ));
    assert_eq!(budget.retained_bytes(), root.bytes());
    for wrong in ["{\"x\":[1,2,3]}", "[0,1,2]", "true", "null", "\"bad\""] {
        let row = ROW.replace("\"tile\"", wrong);
        let source = format!(r#"{{"schema_version":1,"shelters":[{row}]}}"#);
        assert!(matches!(
            ShelterMap::admit_source(&budget, &source),
            Err(ShelterAdmissionError::InvalidDefinition)
        ));
        assert_eq!(budget.retained_bytes(), root.bytes());
    }
}

#[test]
fn shared_rows_and_charge_survive_outer_arcs_and_last_shelter_map_clone() {
    let (budget, root) = budget();
    let installed = ShelterMap::installed_admitted(&budget).unwrap();
    let retained = installed.admitted_storage_bytes();
    let config = installed.clone();
    let map_clone = (*installed).clone();
    assert!(Arc::ptr_eq(
        installed.storage.as_ref().unwrap(),
        map_clone.storage.as_ref().unwrap()
    ));
    assert!(std::ptr::eq(
        installed.shelters().as_ptr(),
        map_clone.shelters().as_ptr()
    ));
    drop(root);
    drop(installed);
    assert_eq!(budget.retained_bytes(), retained);
    drop(config);
    assert_eq!(budget.retained_bytes(), retained);
    assert_eq!(map_clone.shelters().len(), 28);
    drop(map_clone);
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn every_structural_arithmetic_overflow_refuses() {
    for (bytes, objects, arrays, strings, longest) in [
        (usize::MAX, 0, 0, 0, 0),
        (0, usize::MAX, 0, 0, 0),
        (0, 0, usize::MAX, 0, 0),
        (0, 0, 0, usize::MAX, 0),
        (0, 0, 0, 0, usize::MAX),
    ] {
        let shape = Shape {
            bytes,
            objects,
            arrays,
            strings,
            longest,
        };
        assert_eq!(shape.peak(), Err(ShelterAdmissionError::Admission));
    }
}
