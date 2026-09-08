//! M2a6 existing-law component diagnostic. No full-save or synchronous-host budget claim.
use cathedral_backends::world_data::load_world_seed;
use cathedral_sim::{
    ActorId, AreaMap, Capabilities, Cognition, CognitionBusy, Engine, EngineConfig,
    IdleCognitionMode, NavData, NullSight, NullTranscription, NullTts, Office, PlaceId, PromptEnv,
    RequestId, ShelterMap, SoundCatalog, TtsBackendKind, Vec3, WorldClock,
    checkpoint::{CheckpointBudget, Cohort},
    custody::Station,
    engine::law_checkpoint::EngineLawDtoV1,
    notices::{self, checkpoint::LawCheckpointContext},
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
    // Deliberate diagnostic setup through existing notice/custody writers.
    // Production cast/placement, registry and normal cache publishing remain.
    let w = engine.world_mut();
    let days = w.current_time.unwrap().game_days();
    let player = ActorId::from_raw("player");
    let officer = w
        .characters
        .values()
        .find(|c| notices::is_law(c) && w.is_present(c.id()))
        .ok_or("no law officer")?
        .id()
        .clone();
    let at = w.characters[&officer].position_m();
    let others: Vec<_> = w
        .characters
        .keys()
        .filter(|id| {
            **id != player && **id != officer && w.custody.get(id).is_none() && w.is_present(id)
        })
        .take(2)
        .cloned()
        .collect();
    if others.len() != 2 {
        return Err("insufficient diagnostic cast".into());
    }
    w.characters.get_mut(&player).unwrap().state.position_m = at + Vec3::new(12.0, 0.0, 0.0);
    let station = |point, stone_house| Station {
        place_id: PlaceId::from_raw("diagnostic_frozen_station"),
        name: "The former watch station".into(),
        point,
        stone_house,
    };
    let raise = |w: &mut cathedral_sim::World, raised, accused| {
        w.notices
            .raise(
                "a familiar stranger".into(),
                "a missing stack".into(),
                Some("the old quay".into()),
                Some("before dawn".into()),
                raised,
                officer.clone(),
                Some(accused),
                Some(ActorId::from_raw("diagnostic_wronged")),
                Some(cathedral_sim::ItemId::from_raw(
                    "diagnostic_historical_item",
                )),
            )
            .unwrap()
    };
    let historical = raise(w, Some(days - 30.0), others[0].clone());
    w.notices.expire(days);
    raise(w, Some(days), player.clone());
    let future = raise(w, Some(days), player.clone());
    w.notices
        .summon(future, officer.clone(), Office::Lamplight, Some(days + 0.5));
    let warrant = raise(w, None, player.clone());
    w.notices
        .summon(warrant, officer.clone(), Office::Watch, Some(days));
    w.notices.issue_warrants(days);
    let undated = w
        .notices
        .raise_hearsay(
            "someone gone".into(),
            "an unproven debt".into(),
            None,
            None,
            None,
            officer.clone(),
            Some(ActorId::from_raw("diagnostic_absent")),
            None,
            None,
        )
        .unwrap();
    w.notices
        .summon(undated, officer.clone(), Office::Watch, None);
    notices::confront(w);
    w.custody.seize(
        player.clone(),
        officer.clone(),
        Some(future),
        station(at + Vec3::new(100.0, 0.0, 0.0), false),
        0.0,
    );
    for _ in 0..7 {
        w.custody.get_mut(&player).unwrap().note_struggle();
    }
    let departed = ActorId::from_raw("diagnostic_departed_officer");
    let held = &others[0];
    let held_at = w.characters[held].position_m();
    w.custody.seize(
        held.clone(),
        departed.clone(),
        Some(historical),
        station(held_at, true),
        0.0,
    );
    w.custody.commit(held, 0.0);
    w.custody.forget(&departed);
    let chase = &others[1];
    w.characters.get_mut(chase).unwrap().state.position_m = at + Vec3::new(12.0, 0.0, 0.0);
    w.custody.seize(
        chase.clone(),
        officer.clone(),
        None,
        station(at + Vec3::new(100.0, 0.0, 0.0), false),
        0.0,
    );
    w.custody.grab(chase, officer.clone());
    engine.poll(0.01, Vec::new());
    let boundary = LogicalTime::new(0.01).unwrap();
    let w = engine.world();
    let context = LawCheckpointContext::from_world(w, boundary);
    let mut phases = Phases::default();
    let mut metadata = None;
    for _ in 0..args.samples {
        let budget = CheckpointBudget::default();
        let initial = || budget.reserve(Cohort::SavePayload, 4096).unwrap();
        let preflight = time(&mut phases.preflight_us, || {
            engine.checkpoint_law_cost(boundary, initial())
        })?;
        let cost = *preflight.value();
        drop(preflight);
        let dto = time(&mut phases.export_us, || {
            engine.export_law_checkpoint(boundary, initial())
        })?;
        let counts = dto.value().counts(context);
        let witnesses = dto.value().history_counts(context);
        assert!(
            counts.law.served_notices > 0
                && counts.law.dated_unissued_summons > 0
                && counts.law.warrants > 0
                && counts.law.undated_summons > 0
        );
        assert!(
            counts.law.holders > 0
                && counts.law.closing > 0
                && counts.law_cached
                && counts.cached_custody,
            "{counts:?}"
        );
        assert!(witnesses.historical_notice_links > 0 && witnesses.historical_officers > 0);
        let bytes = time(&mut phases.encode_us, || dto.encode())?;
        let input = budget.reserve(Cohort::LoadCandidate, bytes.value().len() + 4096)?;
        let decoded = time(&mut phases.decode_validate_us, || {
            EngineLawDtoV1::decode(bytes.value(), input, context)
        })?;
        let candidate = time(&mut phases.candidate_validate_us, || {
            decoded.into_candidate(context)
        })?;
        assert_eq!(candidate.value().counts(context), counts);
        assert_eq!(candidate.value().history_counts(context), witnesses);
        let current = json!({"scenario":"law-obligations-v1","counts":counts,"witnesses":witnesses,"cost":cost,"shared_reserved_peak_excluding_running_bytes":budget.retained_bytes()});
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
