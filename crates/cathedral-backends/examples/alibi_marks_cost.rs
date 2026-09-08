//! M2a7 existing-marks component diagnostic. No full-save or synchronous-host budget claim.
use cathedral_backends::world_data::load_world_seed;
use cathedral_sim::{
    ActorId, AreaMap, Capabilities, Cognition, CognitionBusy, Engine, EngineConfig,
    IdleCognitionMode, NavData, NullSight, NullTranscription, NullTts, Office, PromptEnv,
    RequestId, ShelterMap, SoundCatalog, TtsBackendKind, Vec3, WorldClock,
    checkpoint::{CheckpointBudget, Cohort},
    engine::marks_checkpoint::EngineMarksDtoV1,
    marks::{self, MarkAnchor, MarkKind, checkpoint::MarksCheckpointContext},
    notices,
    timeline::LogicalTime,
};
use clap::{Parser, ValueEnum};
use serde::Serialize;
use serde_json::json;
use std::{fs, path::PathBuf, sync::Arc, time::Instant};
struct Unavailable;
impl Cognition for Unavailable {
    fn request(&mut self, _: String) -> Result<RequestId, CognitionBusy> {
        Err(CognitionBusy)
    }
}
#[derive(Clone, Copy, Debug, ValueEnum)]
enum Mode {
    Authored,
    Populated,
}
#[derive(Parser)]
struct Args {
    #[arg(long, value_enum)]
    mode: Mode,
    #[arg(long, default_value_t = 100)]
    samples: usize,
    #[arg(long)]
    output: PathBuf,
}
#[derive(Default, Serialize)]
struct Phases {
    preflight_us: Vec<f64>,
    export_us: Vec<f64>,
    encode_us: Vec<f64>,
    decode_validate_us: Vec<f64>,
    candidate_validate_us: Vec<f64>,
    drop_us: Vec<f64>,
}
fn time<T>(out: &mut Vec<f64>, f: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let result = f();
    out.push(start.elapsed().as_secs_f64() * 1e6);
    result
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if !(1..=1000).contains(&args.samples) {
        return Err("samples must be 1..=1000".into());
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let assets = root.join("assets");
    let read = |name: &str| fs::read_to_string(assets.join(name));
    let mut seed = load_world_seed(&assets, &root.join("lore"))?;
    let nav = NavData::from_parts(
        &read("world/navigation.json")?,
        &fs::read(assets.join("world/navigation.bin"))?,
    )?;
    let placement = if matches!(args.mode, Mode::Populated) {
        let occupied: Vec<_> = seed.characters.iter().map(|c| c.position_m).collect();
        let crowd = cathedral_sim::generate_ambient(&nav, 2000, 0, &occupied, &[])?;
        let p = json!({"requested":crowd.placement.requested,"placed":crowd.placement.placed,"unplaced":crowd.placement.unplaced});
        if crowd.placement.placed != 2000 {
            return Err("+2,000 placement incomplete".into());
        }
        seed = seed.with_extra_ambient(crowd.sheets)?;
        p
    } else {
        json!({"requested":0,"placed":0,"unplaced":0})
    };
    let mut engine = Engine::new(
        EngineConfig {
            fake_mode: true,
            idle_mode: IdleCognitionMode::Stage,
            idle_requires_news: true,
            tts_selected: TtsBackendKind::Off,
            clock: WorldClock::new(60.0, Office::Dayspring, 2, 0.05),
            nav: Some(Arc::new(nav)),
            shelters: Arc::new(ShelterMap::from_json_str(&read("world/shelters.json")?)?),
            ..Default::default()
        },
        &seed,
        AreaMap::from_json_str(&read("world/areas.json")?)?,
        SoundCatalog::from_toml_str(&read("sounds/catalog.toml")?)?,
        PromptEnv::new(
            &read("prompts/turn.j2")?,
            &read("prompts/night.j2")?,
            &read("prompts/strings.toml")?,
        )?,
        Box::new(Unavailable),
        Box::new(NullTranscription),
        Box::new(NullTts),
        Box::new(NullSight),
        Capabilities::new(false, false, false, false, false, TtsBackendKind::Off),
        (Vec3::new(-21.0, cathedral_sim::WALK_Y, 252.0), 0.0),
        0,
        0.0,
    )?;
    engine.poll(0.0, Vec::new());
    // Ordinary writers on actual authored geometry. The consumed ward beat
    // leaves a scrubbed door clean; independent chalk supplies all three kinds.
    let w = engine.world_mut();
    let days = w.current_time.unwrap().game_days();
    let player = ActorId::from_raw("player");
    let homes: Vec<_> = w
        .characters
        .keys()
        .filter(|a| **a != player && w.places.home_of(a).is_some())
        .take(2)
        .cloned()
        .collect();
    if homes.len() != 2 {
        return Err("insufficient authored homes".into());
    }
    let scrub_anchor = MarkAnchor::Household(homes[0].clone());
    w.notices
        .raise(
            "an authored neighbour".into(),
            "an unsettled debt".into(),
            None,
            None,
            Some(days - 3.0),
            ActorId::from_raw("diagnostic_former_officer"),
            Some(homes[0].clone()),
            None,
            None,
        )
        .ok_or("notice raise refused")?;
    notices::chalk_the_debtors(w, days);
    let scrubbed = marks::scrub(
        w,
        w.marks
            .find(MarkKind::ChalkCross, &scrub_anchor)
            .ok_or("no ward cross")?
            .0,
    )
    .is_some();
    let same_day_lines = notices::chalk_the_debtors(w, days);
    let same_day_suppressed =
        same_day_lines.is_empty() && w.marks.find(MarkKind::ChalkCross, &scrub_anchor).is_none();
    let cross_anchor = MarkAnchor::Household(homes[1].clone());
    let cross = marks::draw_or_refresh(
        w,
        MarkKind::ChalkCross,
        cross_anchor.clone(),
        Some(ActorId::from_raw("diagnostic_departed_hand")),
        days - 1.0,
    )
    .ok_or("cross refused")?
    .id;
    // Sweep changes strength through the real precipitation/shelter model.
    w.marks.decay_scale = 20.0;
    w.marks.rewind_sweep_clock(days - 1.0);
    let sweep_changed = marks::sweep(w, days);
    w.marks.decay_scale = 1.0;
    let cross_strength = w
        .marks
        .get(cross)
        .ok_or("cross washed off during diagnostic setup")?
        .strength;
    let well = w
        .places
        .named("Chain Well")
        .ok_or("missing authored well")?
        .name
        .clone();
    let tally = marks::draw_or_refresh(w, MarkKind::WellTally, MarkAnchor::Place(well), None, days)
        .ok_or("tally refused")?
        .id;
    // This is the production public mark count field; the cap/next-notch owner
    // is independently exercised by Round's ordinary source tests.
    w.marks.get_mut(tally).unwrap().strokes = marks::TALLY_STROKES_MAX;
    let resort = w
        .mark_catalog
        .spec(MarkKind::WardSign)
        .unwrap()
        .places
        .values()
        .find(|p| w.places.named(p).is_some())
        .ok_or("no authored resort")?
        .clone();
    marks::draw_or_refresh(
        w,
        MarkKind::WardSign,
        MarkAnchor::Place(resort),
        Some(ActorId::from_raw("diagnostic_departed_hand")),
        days,
    )
    .ok_or("ward sign refused")?;
    let at = w.places.home_of(&homes[0]).unwrap().point;
    w.characters.get_mut(&player).unwrap().state.position_m = at;
    let pen = cathedral_sim::ItemId::from_raw("diagnostic_chalk_pen");
    w.add_item(cathedral_sim::Item::new(pen.clone(), "chalk_pen"));
    w.characters.get_mut(&player).unwrap().state.holds.push(pen);
    let before_revision = w.world_revision;
    let out = engine.poll(0.01, Vec::new());
    let published = out
        .iter()
        .filter(|m| matches!(m, cathedral_sim::EngineMessage::ChalkStanding { .. }))
        .count();
    let cache_deduped = !engine
        .poll(0.011, Vec::new())
        .iter()
        .any(|m| matches!(m, cathedral_sim::EngineMessage::ChalkStanding { .. }));
    let witnesses = json!({"scrubbed":scrubbed,"same_day_suppressed":same_day_suppressed,"sweep_changed":sweep_changed,"cross_strength":cross_strength,"chalk_publications":published,"cache_deduped":cache_deduped,"revision_after_setup":before_revision});
    assert!(
        scrubbed
            && same_day_suppressed
            && sweep_changed
            && cross_strength > 0.0
            && cross_strength < 1.0
            && published == 1
            && cache_deduped,
        "{witnesses}"
    );
    let boundary = LogicalTime::new(0.011).unwrap();
    let w = engine.world();
    let context = MarksCheckpointContext::from_world(w, boundary);
    let mut phases = Phases::default();
    let mut metadata = None;
    for _ in 0..args.samples {
        let budget = CheckpointBudget::default();
        let initial = || budget.reserve(Cohort::SavePayload, 4096).unwrap();
        let preflight = time(&mut phases.preflight_us, || {
            engine.checkpoint_marks_cost(boundary, initial())
        })?;
        let cost = *preflight.value();
        drop(preflight);
        let dto = time(&mut phases.export_us, || {
            engine.export_marks_checkpoint(boundary, initial())
        })?;
        let counts = dto.value().counts(context);
        assert!(
            counts.marks.crosses > 0
                && counts.marks.tallies > 0
                && counts.marks.ward_signs > 0
                && counts.marks.households > 0
                && counts.marks.places > 0
                && counts.marks.faint > 0
                && counts.marks.historical_authors > 0
                && counts.marks.sweep_taken
                && counts.marks.beat_taken
                && counts.chalk_cached
                && counts.cached_anchors > 0
                && counts.cached_pen,
            "{counts:?}"
        );
        let bytes = time(&mut phases.encode_us, || dto.encode())?;
        let input = budget.reserve(Cohort::LoadCandidate, bytes.value().len() + 4096)?;
        let decoded = time(&mut phases.decode_validate_us, || {
            EngineMarksDtoV1::decode(bytes.value(), input, context)
        })?;
        let candidate = time(&mut phases.candidate_validate_us, || {
            decoded.into_candidate(context)
        })?;
        assert_eq!(candidate.value().counts(context), counts);
        let current = json!({"scenario":"marks-obligations-v1","counts":counts,"witnesses":witnesses,"cost":cost,"shared_reserved_peak_excluding_running_bytes":budget.retained_bytes()});
        if let Some(prior) = &metadata {
            assert_eq!(prior, &current);
        } else {
            metadata = Some(current);
        }
        time(&mut phases.drop_us, || {
            drop(candidate);
            drop(bytes);
        });
        assert_eq!(budget.retained_bytes(), 0);
    }
    let mut output = metadata.unwrap();
    output["mode"] = json!(match args.mode {
        Mode::Authored => "authored",
        Mode::Populated => "populated",
    });
    output["samples"] = json!(args.samples);
    output["placement"] = placement;
    for (k, v) in serde_json::to_value(phases)?.as_object().unwrap() {
        output[k] = v.clone();
    }
    fs::write(args.output, serde_json::to_vec_pretty(&output)?)?;
    Ok(())
}
