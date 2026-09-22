use super::*;
use cathedral_sim::checkpoint::{Cohort, MAX_RESIDENT_BYTES};
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
mod native_probe;

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "cathedral-source-capture-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let fixture = Self(root);
        for (index, name) in FIXED.iter().enumerate() {
            let base = if index == 1 { "lore" } else { "assets" };
            fixture.write(
                &format!("{base}/{name}"),
                format!("source-{index}").as_bytes(),
            );
        }
        fixture.write("lore/characters/z/z.json", b"last");
        fixture.write("lore/characters/a/b.json", b"second");
        fixture.write("lore/characters/a/a.json", b"first");
        fixture
    }
    fn write(&self, relative: &str, bytes: &[u8]) {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
    fn capture(&self, budget: &CheckpointBudget) -> Result<CapturedActorSources> {
        CapturedActorSources::capture_admitted(budget, &self.0.join("assets"), &self.0.join("lore"))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn budget() -> (CheckpointBudget, Reservation) {
    let budget = CheckpointBudget::default();
    let running = budget.reserve(Cohort::Running, 4096).unwrap();
    (budget, running)
}

#[test]
fn pressure_refuses_before_invalid_roots_can_be_read() {
    let (budget, root) = budget();
    let pressure = budget
        .reserve(Cohort::LoadCandidate, MAX_RESIDENT_BYTES - root.bytes())
        .unwrap();
    let result = CapturedActorSources::capture_admitted(
        &budget,
        Path::new("/nonexistent/cathedral-capture-assets"),
        Path::new("/nonexistent/cathedral-capture-lore"),
    );
    assert!(matches!(result, Err(SourceCaptureError::Admission)));
    assert_eq!(budget.retained_bytes(), MAX_RESIDENT_BYTES);
    drop(pressure);
    let result = CapturedActorSources::capture_admitted(
        &budget,
        Path::new("/nonexistent/cathedral-capture-assets"),
        Path::new("/nonexistent/cathedral-capture-lore"),
    );
    assert!(matches!(result, Err(SourceCaptureError::Io)));
    assert_eq!(budget.retained_bytes(), root.bytes());
}

#[test]
fn ordered_snapshot_survives_file_changes_and_last_owner_keeps_charge() {
    let fixture = Fixture::new();
    let (budget, root) = budget();
    let sources = fixture.capture(&budget).unwrap();
    assert_eq!(
        sources.characters().collect::<Vec<_>>(),
        vec![
            ("a/a.json", "first"),
            ("a/b.json", "second"),
            ("z/z.json", "last")
        ]
    );
    assert_eq!(sources.seed(), "source-0");
    fixture.write("assets/world/seed.json", b"changed");
    fixture.write("lore/characters/a/a.json", b"changed");
    let last = sources.clone();
    let charge = sources.charged_bytes();
    assert!(charge < peak_bytes());
    assert_eq!(budget.retained_bytes(), root.bytes() + charge);
    drop(fixture);
    drop(sources);
    drop(root);
    assert_eq!(budget.retained_bytes(), charge);
    assert_eq!(last.seed(), "source-0");
    assert_eq!(last.characters().next(), Some(("a/a.json", "first")));
    drop(last);
    assert_eq!(budget.retained_bytes(), 0);
}

#[cfg(unix)]
#[test]
fn discovery_ignores_symlinks_and_refuses_an_empty_cast() {
    let fixture = Fixture::new();
    let (budget, root) = budget();
    std::os::unix::fs::symlink(
        fixture.0.join("lore/characters/a/a.json"),
        fixture.0.join("lore/characters/symlink.json"),
    )
    .unwrap();
    std::os::unix::fs::symlink(
        fixture.0.join("lore/characters"),
        fixture.0.join("lore/characters/loop"),
    )
    .unwrap();
    assert_eq!(fixture.capture(&budget).unwrap().characters().len(), 3);
    fs::remove_dir_all(fixture.0.join("lore/characters/a")).unwrap();
    fs::remove_dir_all(fixture.0.join("lore/characters/z")).unwrap();
    assert!(matches!(
        fixture.capture(&budget),
        Err(SourceCaptureError::EmptyCast)
    ));
    assert_eq!(budget.retained_bytes(), root.bytes());
}

#[test]
fn reader_enforces_exact_cap_sentinel_total_and_utf8_before_retention() {
    let fixture = Fixture::new();
    let path = fixture.0.join("large");
    let file = fs::File::create(&path).unwrap();
    file.set_len(MAX_SOURCE_BYTES as u64).unwrap();
    let mut buffer = vec![0; MAX_SOURCE_BYTES + 1];
    let mut total = MAX_TOTAL_SOURCE_BYTES - MAX_SOURCE_BYTES;
    assert_eq!(
        read_source(&path, &mut buffer, &mut total).unwrap().len(),
        MAX_SOURCE_BYTES
    );
    assert_eq!(total, MAX_TOTAL_SOURCE_BYTES);
    assert!(matches!(
        read_source(&path, &mut buffer, &mut total),
        Err(SourceCaptureError::TotalSizeLimit)
    ));
    assert_eq!(total, MAX_TOTAL_SOURCE_BYTES);
    file.set_len((MAX_SOURCE_BYTES + 1) as u64).unwrap();
    total = 0;
    assert!(matches!(
        read_source(&path, &mut buffer, &mut total),
        Err(SourceCaptureError::SourceSizeLimit)
    ));
    assert_eq!(total, 0);
    fs::write(&path, [0xff]).unwrap();
    assert!(matches!(
        read_source(&path, &mut buffer, &mut total),
        Err(SourceCaptureError::Utf8)
    ));
    assert_eq!(total, 0);
}

#[test]
fn failed_capture_releases_partial_sources_after_oversized_late_file() {
    let fixture = Fixture::new();
    let path = fixture.0.join("lore/characters/z/z.json");
    fs::OpenOptions::new()
        .write(true)
        .open(path)
        .unwrap()
        .set_len((MAX_SOURCE_BYTES + 1) as u64)
        .unwrap();
    let (budget, root) = budget();
    assert!(matches!(
        fixture.capture(&budget),
        Err(SourceCaptureError::SourceSizeLimit)
    ));
    assert_eq!(budget.retained_bytes(), root.bytes());
}

#[test]
fn discovery_bounds_files_directories_depth_and_paths() {
    let fixture = Fixture::new();
    let directory = fixture.0.join("many");
    fs::create_dir(&directory).unwrap();
    for i in 0..=MAX_CHARACTERS {
        fs::write(directory.join(format!("{i}.json")), b"").unwrap();
    }
    assert!(matches!(
        discover(&directory),
        Err(SourceCaptureError::SourceCountLimit)
    ));
    fs::remove_dir_all(&directory).unwrap();
    fs::create_dir(&directory).unwrap();
    for i in 0..MAX_DIRECTORIES {
        fs::create_dir(directory.join(i.to_string())).unwrap();
    }
    assert!(matches!(
        discover(&directory),
        Err(SourceCaptureError::DirectoryLimit)
    ));
    fs::remove_dir_all(&directory).unwrap();
    let mut nested = directory.clone();
    for _ in 0..=MAX_DEPTH {
        nested.push("next");
    }
    fs::create_dir_all(nested).unwrap();
    assert!(matches!(
        discover(&directory),
        Err(SourceCaptureError::DepthLimit)
    ));
    fs::remove_dir_all(&directory).unwrap();
    let mut nested = directory.clone();
    for _ in 0..5 {
        nested.push("x".repeat(220));
    }
    fs::create_dir_all(nested).unwrap();
    assert!(matches!(
        discover(&directory),
        Err(SourceCaptureError::PathLimit)
    ));
}

#[test]
fn ignored_non_json_entries_still_consume_the_discovery_limit() {
    let fixture = Fixture::new();
    let directory = fixture.0.join("ignored-entry-limit");
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join("character.json"), b"{}").unwrap();
    for i in 0..MAX_ENTRIES - 1 {
        fs::write(directory.join(format!("ignored-{i}.txt")), b"").unwrap();
    }
    assert_eq!(discover(&directory).unwrap().len(), 1);
    fs::write(directory.join("one-too-many.txt"), b"").unwrap();
    assert!(matches!(
        discover(&directory),
        Err(SourceCaptureError::EntryLimit)
    ));
}

#[cfg(unix)]
#[test]
fn invalid_byte_relative_names_keep_the_existing_lossy_conversion_and_charge() {
    use std::os::unix::ffi::OsStringExt;
    let fixture = Fixture::new();
    // Exercise repeated invalid bytes and expansion, not only ASCII names.
    // The synthetic source is captured without invoking the lore parser.
    let mut raw_name = vec![0xff; 250];
    raw_name.extend_from_slice(b".json");
    let name = std::ffi::OsString::from_vec(raw_name);
    fs::write(
        fixture.0.join("lore/characters/a").join(name),
        b"invalid-name",
    )
    .unwrap();
    let (budget, root) = budget();
    let sources = fixture.capture(&budget).unwrap();
    let expected = format!("a/{}.json", "\u{fffd}".repeat(250));
    let (relative, text) = sources
        .characters()
        .find(|(_, text)| *text == "invalid-name")
        .unwrap();
    assert_eq!(relative, expected);
    assert_eq!(text, "invalid-name");
    assert_eq!(relative.len(), 757);
    let owned = sources
        .0
        .characters
        .iter()
        .find(|(_, text)| text.as_ref() == "invalid-name")
        .unwrap();
    assert!(owned.0.capacity() >= relative.len());
    assert_eq!(
        budget.retained_bytes(),
        root.bytes() + sources.charged_bytes()
    );
    let last = sources.clone();
    drop(sources);
    drop(fixture);
    drop(root);
    assert_eq!(budget.retained_bytes(), last.charged_bytes());
    assert!(last.characters().any(|(path, _)| path == expected));
    drop(last);
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn captured_public_assets_compose_the_same_world_as_existing_loader() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (budget, _running) = budget();
    let sources =
        CapturedActorSources::capture_admitted(&budget, &root.join("assets"), &root.join("lore"))
            .unwrap();
    for knowledge in [PlayerKnowledge::PublicFigures, PlayerKnowledge::Everyone] {
        let original = super::super::load_world_seed_with_knowledge(
            &root.join("assets"),
            &root.join("lore"),
            knowledge,
        )
        .unwrap();
        assert_eq!(sources.world_seed(knowledge).unwrap(), original);
    }
}
