use super::*;
use crate::checkpoint::{Cohort, MAX_RESIDENT_BYTES, Reservation};

const ROW: &str = "sound_id='chime', sound_class='bell', audible_distance=10.0, heard='A chime', seen='{actor} chimes', sfx_prompt='A bell', duration_seconds=1.5, actor_emittable=true";
const SOURCE: &str = include_str!("../../../../../assets/sounds/catalog.toml");
fn budget() -> (CheckpointBudget, Reservation) {
    let budget = CheckpointBudget::default();
    let root = budget.reserve(Cohort::Running, 4096).unwrap();
    (budget, root)
}
fn fingerprint(catalog: &SoundCatalog) -> Vec<u8> {
    let sounds: Vec<_> = catalog
        .sounds()
        .iter()
        .map(|s| {
            (
                &s.sound_id,
                &s.sound_class,
                s.audible_distance,
                &s.heard,
                &s.seen,
                &s.sfx_prompt,
                s.duration_seconds,
                s.actor_emittable,
            )
        })
        .collect();
    let ambients: Vec<_> = catalog
        .ambients()
        .iter()
        .map(|s| (&s.sound_id, &s.sfx_prompt, s.duration_seconds))
        .collect();
    serde_json::to_vec(&(sounds, ambients)).unwrap()
}

#[test]
fn installed_content_order_debug_and_legacy_constructors_are_preserved() {
    use sha2::{Digest, Sha256};
    let (budget, root) = budget();
    let catalog = SoundCatalog::from_toml_str_admitted(SOURCE, &budget).unwrap();
    let legacy = SoundCatalog::from_toml_str(SOURCE).unwrap();
    assert_eq!(catalog, legacy);
    assert_eq!(catalog.emittable_sound_ids(), legacy.emittable_sound_ids());
    assert_eq!(format!("{catalog:?}"), format!("{legacy:?}"));
    assert_eq!(fingerprint(&catalog), fingerprint(&legacy));
    println!(
        "installed_sounds source_bytes={} sounds={} ambients={} retained={} rows_sha256={:x}",
        SOURCE.len(),
        catalog.sounds().len(),
        catalog.ambients().len(),
        catalog.admitted_storage_bytes(),
        Sha256::digest(fingerprint(&catalog))
    );
    assert_eq!(legacy.admitted_storage_bytes(), 0);
    assert!(SoundCatalog::empty().storage.is_none());
    // Public AmbientSound fields historically allow invalid constructed rows:
    // SoundCatalog::new checks duplicate ambient IDs only. Preserve that API.
    let ambient = AmbientSound {
        sound_id: "INVALID".into(),
        sfx_prompt: String::new(),
        duration_seconds: -1.0,
    };
    assert!(SoundCatalog::new(Vec::new(), vec![ambient]).is_ok());
    drop(catalog);
    assert_eq!(budget.retained_bytes(), root.bytes());
}

#[test]
fn full_toml_encodings_unicode_multiline_and_struct_sequences_match_legacy() {
    let (budget, root) = budget();
    let inline = format!("sounds=[{{{ROW}}}]");
    let table = format!("[[sounds]]\n{}\n", ROW.replace(", ", "\n"));
    let sequence = "sounds=[['chime','bell',10.0,'A chime','{actor} chimes','A bell',1.5,true]]";
    let escaped = inline
        .replace("sounds=", "\"\\u0073ounds\"=")
        .replace("'A chime'", "\"\"\"A chime\\n\\u263a\"\"\"");
    for source in [
        inline.as_str(),
        table.as_str(),
        sequence,
        escaped.as_str(),
        "",
        "\u{feff}# empty\r\n",
        "sounds=[]\nambients=[]",
    ] {
        let legacy = SoundCatalog::from_toml_str(source).unwrap();
        let admitted = SoundCatalog::from_toml_str_admitted(source, &budget).unwrap();
        assert_eq!(admitted, legacy, "source={source}");
        drop(admitted);
        assert_eq!(budget.retained_bytes(), root.bytes());
    }
}

#[test]
fn pressure_precedes_tokens_and_full_parser_expansion() {
    let (budget, root) = budget();
    let pressure = budget
        .reserve(Cohort::LoadCandidate, MAX_RESIDENT_BYTES - root.bytes())
        .unwrap();
    assert_eq!(
        SoundCatalog::from_toml_str_admitted("invalid", &budget),
        Err(SoundAdmissionError::Admission)
    );
    drop(pressure);
    let source = format!("sounds=[{{{ROW}}}]");
    let token_bytes = token_peak(source.len()).unwrap();
    let pressure = budget
        .reserve(
            Cohort::LoadCandidate,
            MAX_RESIDENT_BYTES - root.bytes() - token_bytes,
        )
        .unwrap();
    assert_eq!(
        SoundCatalog::from_toml_str_admitted(&source, &budget),
        Err(SoundAdmissionError::Admission)
    );
    assert_eq!(budget.retained_bytes(), root.bytes() + pressure.bytes());
    drop(pressure);
    assert!(SoundCatalog::from_toml_str_admitted(&source, &budget).is_ok());
    assert_eq!(budget.retained_bytes(), root.bytes());
}

#[test]
fn malformed_escapes_deep_paths_types_and_duplicate_diagnostics_drop_under_lease() {
    let (budget, root) = budget();
    let short_hex = "\\x".repeat(16_384);
    let long_id = "a".repeat(16_384);
    let duplicate_row = ROW.replace("chime'", &format!("{long_id}'"));
    let sources = [
        format!(
            "sounds=[{{{}}}]",
            ROW.replace("'A chime'", &format!("\"{short_hex}\""))
        ),
        format!("\"{short_hex}\"=1"),
        format!("sounds={}0{}", "[".repeat(81), "]".repeat(81)),
        format!("{}x=1", "a.".repeat(80)),
        format!("sounds=[{{{duplicate_row}}},{{{duplicate_row}}}]"),
        format!("sounds=[{{{}}}]", ROW.replace("10.0", "{ a=[1,2,3] }")),
        format!("\"{}\"=1", "\\u0000".repeat(16_384)),
        format!(
            "sounds=[{{{}}}]",
            ROW.replace("'A chime'", "1979-05-27T07:32:00Z")
        ),
        "sounds=[{heard=\"unterminated".into(),
    ];
    for source in sources {
        assert!(SoundCatalog::from_toml_str(&source).is_err());
        assert_eq!(
            SoundCatalog::from_toml_str_admitted(&source, &budget),
            Err(SoundAdmissionError::InvalidDefinition)
        );
        assert_eq!(budget.retained_bytes(), root.bytes());
    }
}

#[test]
fn arbitrary_long_numeric_tokens_use_std_float_semantics() {
    let (budget, root) = budget();
    for number in [
        format!("1.{}1", "0".repeat(16_384)),
        "1e300".into(),
        "1e-300".into(),
        "1_000".into(),
        "+inf".into(),
        "nan".into(),
    ] {
        let source = format!("sounds=[{{{}}}]", ROW.replace("10.0", &number));
        let legacy = SoundCatalog::from_toml_str(&source);
        let admitted = SoundCatalog::from_toml_str_admitted(&source, &budget);
        match (legacy, admitted) {
            (Ok(a), Ok(b)) => assert_eq!(a, b),
            (Err(_), Err(SoundAdmissionError::InvalidDefinition)) => {}
            other => panic!("numeric semantics differ: {other:?}"),
        }
        assert_eq!(budget.retained_bytes(), root.bytes());
    }
}

#[test]
fn admitted_catalog_is_immutable_and_charge_survives_last_by_value_clone() {
    let (budget, root) = budget();
    let mut source = format!("sounds=[{{{ROW}}}]");
    let catalog = SoundCatalog::from_toml_str_admitted(&source, &budget).unwrap();
    source.clear();
    let retained = catalog.admitted_storage_bytes();
    let config = catalog.clone();
    let world = config.clone();
    assert!(std::sync::Arc::ptr_eq(
        catalog.storage.as_ref().unwrap(),
        world.storage.as_ref().unwrap()
    ));
    drop(root);
    drop(catalog);
    drop(config);
    assert_eq!(budget.retained_bytes(), retained);
    assert_eq!(world.sounds()[0].sound_id, "chime");
    drop(world);
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn source_limit_and_eof_growth_inputs_keep_legacy_meaning() {
    let (budget, root) = budget();
    // Production source capture supports4MiB per file; comments still cause a
    // byte-sized token reservation even though only two tokens are produced.
    let mut comment = "#".to_owned();
    comment.extend(std::iter::repeat_n('x', 4 * 1024 * 1024 - 1));
    for source in [comment.as_str(), " ", "\n", "=", "[", "''"] {
        let legacy = SoundCatalog::from_toml_str(source);
        let admitted = SoundCatalog::from_toml_str_admitted(source, &budget);
        match (legacy, admitted) {
            (Ok(a), Ok(b)) => assert_eq!(a, b),
            (Err(_), Err(SoundAdmissionError::InvalidDefinition)) => {}
            other => panic!("source edge semantics differ: {other:?}"),
        }
        assert_eq!(budget.retained_bytes(), root.bytes());
    }
}

#[test]
fn arithmetic_overflow_refuses_without_parser_work() {
    assert_eq!(token_peak(usize::MAX), Err(SoundAdmissionError::Admission));
    for field in 0..8 {
        let mut shape = Shape::default();
        match field {
            0 => shape.bytes = usize::MAX,
            1 => shape.tokens = usize::MAX,
            2 => shape.events = usize::MAX,
            3 => shape.keys = usize::MAX,
            4 => shape.scalars = usize::MAX,
            5 => shape.arrays = usize::MAX,
            6 => shape.containers = usize::MAX,
            _ => shape.text = usize::MAX,
        }
        assert_eq!(shape.peak(), Err(SoundAdmissionError::Admission));
    }
}
