//! Frame-time measurement, active only when the `CATHEDRAL_PERF` environment
//! variable is set. Player runs without the variable pay nothing.
//!
//! With `CATHEDRAL_PERF=1`:
//! - vsync is switched off so frame times reflect real cost, not pacing
//!   (`CATHEDRAL_PERF=vsync` keeps vsync to observe pacing instead);
//! - every second, the frame times of that second are appended to
//!   `<session>/perf_frames.jsonl`;
//! - every 5 seconds, percentiles are logged to `logs.jsonl` (source "perf")
//!   and a cumulative `<session>/perf_summary.json` is rewritten, so the data
//!   survives a crash or a watchdog kill;
//! - Bevy's frame/entity diagnostics and the render-pass GPU timings are
//!   captured into the summary;
//! - the [`Probe`] spans below attribute each *spiking* frame to the systems
//!   that spent the time, so stutter is measured rather than guessed at.

use std::fs;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use bevy::diagnostic::{
    DiagnosticsStore, EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin,
};
use bevy::prelude::*;
use bevy::render::diagnostic::RenderDiagnosticsPlugin;
use bevy::window::PresentMode;
use serde_json::{Map, Value, json};

use crate::session_log;

// ---------------------------------------------------------------------------
// Spike attribution
// ---------------------------------------------------------------------------

/// The systems worth attributing a spiking frame to.
///
/// A fixed enum on purpose: a probe then costs one `Instant` pair and one
/// relaxed atomic add into a static array — no allocation, no lock, and no map
/// lookup — so instrumenting a hot system does not itself become the spike.
/// Adding a probe means adding a variant here and its name to [`PROBE_NAMES`];
/// the compiler will not let the two drift apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Probe {
    EnginePump,
    BridgeDrain,
    BodyPose,
    ActorViews,
    Vermin,
    Dogs,
    Marks,
    Smoke,
    Weather,
    Soundscape,
    Hud,
    Map,
    Controller,
    Interaction,
    Water,
    Lamps,
}

pub const PROBE_NAMES: [&str; Probe::COUNT] = [
    "engine pump",
    "bridge drain",
    "body pose",
    "actor views",
    "vermin",
    "dogs",
    "marks",
    "smoke",
    "weather",
    "soundscape",
    "hud",
    "map",
    "controller",
    "interaction",
    "water",
    "lamps",
];

impl Probe {
    pub const COUNT: usize = 16;
}

/// Nanoseconds charged to each probe so far this frame. Systems run on the
/// scheduler's thread pool, so this has to be shared state rather than a
/// thread-local; `Relaxed` is right because nothing reads a slot until
/// [`record_frame`] does, after the frame's systems have all finished.
static PROBE_NS: [AtomicU64; Probe::COUNT] = [const { AtomicU64::new(0) }; Probe::COUNT];
/// How many times each probe was entered this frame — a system that spikes
/// because it ran 40 times reads very differently from one that ran once.
static PROBE_HITS: [AtomicU64; Probe::COUNT] = [const { AtomicU64::new(0) }; Probe::COUNT];
static PROBES_ENABLED: AtomicBool = AtomicBool::new(false);

/// Frames slower than this get a `[spike]` line naming where the time went.
/// Just over the 16.7 ms budget: the point is the frames a player feels.
const SPIKE_MS: f32 = 20.0;

/// Charges the time until it is dropped to `probe`. Returns `None` — and so
/// costs a single relaxed bool load — unless `CATHEDRAL_PERF` is set, which is
/// what keeps player runs free.
///
/// ```ignore
/// let _span = perf::span(perf::Probe::Vermin);
/// ```
#[inline]
pub fn span(probe: Probe) -> Option<Span> {
    if !PROBES_ENABLED.load(Ordering::Relaxed) {
        return None;
    }
    Some(Span {
        slot: probe as usize,
        started: Instant::now(),
    })
}

pub struct Span {
    slot: usize,
    started: Instant,
}

impl Drop for Span {
    fn drop(&mut self) {
        let nanos = self.started.elapsed().as_nanos() as u64;
        PROBE_NS[self.slot].fetch_add(nanos, Ordering::Relaxed);
        PROBE_HITS[self.slot].fetch_add(1, Ordering::Relaxed);
    }
}

/// Reads and zeroes every slot. Called once per frame, from `Last`.
fn take_probes() -> [(f32, u64); Probe::COUNT] {
    std::array::from_fn(|slot| {
        let nanos = PROBE_NS[slot].swap(0, Ordering::Relaxed);
        let hits = PROBE_HITS[slot].swap(0, Ordering::Relaxed);
        (nanos as f32 / 1.0e6, hits)
    })
}

pub struct PerfPlugin;

impl Plugin for PerfPlugin {
    fn build(&self, app: &mut App) {
        let Ok(mode) = std::env::var("CATHEDRAL_PERF") else {
            return;
        };
        PROBES_ENABLED.store(true, Ordering::Relaxed);
        app.add_plugins((
            FrameTimeDiagnosticsPlugin::default(),
            EntityCountDiagnosticsPlugin::default(),
        ));
        // `RenderDiagnosticsPlugin` writes GPU timestamp queries around every
        // pass and reads them back. That is not free, and it is exactly the
        // kind of cost that lands in the render half of the frame — the half
        // this tooling is pointing at. `CATHEDRAL_PERF=plain` measures frames
        // without it, so the instrument can be told apart from what it
        // measures. Anything that only shows up under `=1` is the instrument.
        if mode != "plain" {
            app.add_plugins(RenderDiagnosticsPlugin);
        }
        app.insert_resource(PerfRecorder::new(mode == "vsync"))
            .init_resource::<FrameStart>()
            .add_systems(Startup, apply_present_mode)
            // `First` and `Last` bracket the whole ECS main schedule, and
            // schedule boundaries are hard barriers, so the difference between
            // this and the frame's own delta is everything the ECS did *not*
            // do: render extract, the render graph, and the wait on present.
            // Splitting a spike across that line is the first question worth
            // asking about it.
            .add_systems(First, stamp_frame_start)
            .add_systems(Last, record_frame);
    }
}

#[derive(Resource)]
struct PerfRecorder {
    keep_vsync: bool,
    /// (elapsed seconds at frame end, frame ms) for every frame since start.
    samples: Vec<(f32, f32)>,
    /// Index into `samples` of the first frame not yet appended to disk.
    flushed: usize,
    last_flush: f32,
    last_summary: f32,
    /// Per-probe milliseconds and entries summed over every *spiking* frame so
    /// far, and how many such frames there were. Averaged over the spikes
    /// alone, this says what a bad frame is made of — which a whole-run average
    /// hides, because the quiet 95% of frames dominate it.
    spike_ms: [f32; Probe::COUNT],
    spike_hits: [u64; Probe::COUNT],
    spike_frames: u32,
    /// Per-probe milliseconds summed over *every* frame of the run, and the
    /// worst single frame each probe ever cost. The mean says what the frame
    /// costs; the peak says what it is capable of, which for stutter is the
    /// more useful of the two.
    total_ms: [f32; Probe::COUNT],
    peak_ms: [f32; Probe::COUNT],
    /// The ECS main schedule's own wall time (`First` to `Last`), against which
    /// the frame's total says how much of a spike the renderer owns.
    total_ecs_ms: f32,
    peak_ecs_ms: f32,
    spike_ecs_ms: f32,
}

impl PerfRecorder {
    fn new(keep_vsync: bool) -> Self {
        Self {
            keep_vsync,
            samples: Vec::with_capacity(120_000),
            flushed: 0,
            last_flush: 0.0,
            last_summary: 0.0,
            spike_ms: [0.0; Probe::COUNT],
            spike_hits: [0; Probe::COUNT],
            spike_frames: 0,
            total_ms: [0.0; Probe::COUNT],
            peak_ms: [0.0; Probe::COUNT],
            total_ecs_ms: 0.0,
            peak_ecs_ms: 0.0,
            spike_ecs_ms: 0.0,
        }
    }
}

/// `"engine pump 18.2 (x1), vermin 9.1 (x8)"` — the probes that actually spent
/// time, worst first, skipping the noise below `floor_ms`.
fn attribution(per_probe: &[(f32, u64)], floor_ms: f32) -> String {
    let mut ranked: Vec<(usize, f32, u64)> = per_probe
        .iter()
        .enumerate()
        .filter(|&(_, &(ms, _))| ms >= floor_ms)
        .map(|(slot, &(ms, hits))| (slot, ms, hits))
        .collect();
    ranked.sort_by(|a, b| f32::total_cmp(&b.1, &a.1));
    ranked.truncate(6);
    if ranked.is_empty() {
        return "nothing probed".into();
    }
    ranked
        .iter()
        .map(|&(slot, ms, hits)| format!("{} {ms:.1} (x{hits})", PROBE_NAMES[slot]))
        .collect::<Vec<_>>()
        .join(", ")
}

/// When this frame's `First` ran. `None` only before the first one.
#[derive(Resource, Default)]
struct FrameStart(Option<Instant>);

fn stamp_frame_start(mut start: ResMut<FrameStart>) {
    start.0 = Some(Instant::now());
}

fn apply_present_mode(recorder: Res<PerfRecorder>, mut windows: Query<&mut Window>) {
    if recorder.keep_vsync {
        return;
    }
    for mut window in &mut windows {
        window.present_mode = PresentMode::AutoNoVsync;
    }
}

fn record_frame(
    time: Res<Time<Real>>,
    mut recorder: ResMut<PerfRecorder>,
    diagnostics: Res<DiagnosticsStore>,
    frame_start: Res<FrameStart>,
) {
    let delta = time.delta();
    if delta == Duration::ZERO {
        return; // first frame
    }
    let elapsed = time.elapsed_secs();
    let frame_ms = delta.as_secs_f32() * 1000.0;
    recorder.samples.push((elapsed, frame_ms));

    // What the ECS spent, against what the frame cost. The remainder is the
    // render half of the frame — extract, the render graph, and the present
    // wait — which no probe in Update can see.
    let ecs_ms = frame_start
        .0
        .map_or(0.0, |start| start.elapsed().as_secs_f32() * 1000.0);
    recorder.total_ecs_ms += ecs_ms;
    recorder.peak_ecs_ms = recorder.peak_ecs_ms.max(ecs_ms);

    // Attribution has to happen here, in `Last`, and unconditionally: the slots
    // must be zeroed every frame or a quiet frame would inherit the previous
    // one's charges and every spike would look like the system that ran before
    // it.
    let per_probe = take_probes();
    for (slot, &(ms, _)) in per_probe.iter().enumerate() {
        recorder.total_ms[slot] += ms;
        recorder.peak_ms[slot] = recorder.peak_ms[slot].max(ms);
    }
    if frame_ms > SPIKE_MS {
        recorder.spike_frames += 1;
        recorder.spike_ecs_ms += ecs_ms;
        for (slot, &(ms, hits)) in per_probe.iter().enumerate() {
            recorder.spike_ms[slot] += ms;
            recorder.spike_hits[slot] += hits;
        }
        // One line per spike is the point: stutter is an event, and an average
        // over a run cannot tell you which event. 0.3 ms floor keeps the line
        // to the systems that could plausibly be the cause.
        session_log::log_line(
            "perf",
            "INFO",
            &format!(
                "[spike] t={elapsed:.1}s frame {frame_ms:.1} ms (ecs {ecs_ms:.1}, \
                 render+present {:.1}): {}",
                (frame_ms - ecs_ms).max(0.0),
                attribution(&per_probe, 0.3)
            ),
        );
    }

    if elapsed - recorder.last_flush >= 1.0 {
        recorder.last_flush = elapsed;
        flush_frames(&mut recorder);
    }
    if elapsed - recorder.last_summary >= 5.0 {
        recorder.last_summary = elapsed;
        write_summary(&recorder, &diagnostics);
    }
}

fn flush_frames(recorder: &mut PerfRecorder) {
    let Some(paths) = session_log::paths() else {
        return;
    };
    let fresh = &recorder.samples[recorder.flushed..];
    if fresh.is_empty() {
        return;
    }
    let line = json!({
        "t": fresh[0].0,
        "frames_ms": fresh.iter().map(|(_, ms)| (ms * 100.0).round() / 100.0).collect::<Vec<_>>(),
    });
    let path = paths.root.join("perf_frames.jsonl");
    let mut body = line.to_string();
    body.push('\n');
    let result = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut file| std::io::Write::write_all(&mut file, body.as_bytes()));
    if let Err(error) = result {
        warn!("perf: could not append {}: {error}", path.display());
    }
    recorder.flushed = recorder.samples.len();
}

fn percentile(sorted: &[f32], q: f32) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() - 1) as f32 * q).round() as usize;
    sorted[idx]
}

fn stats(samples: &[(f32, f32)]) -> Value {
    let mut sorted: Vec<f32> = samples.iter().map(|&(_, ms)| ms).collect();
    sorted.sort_by(f32::total_cmp);
    json!({
        "frames": sorted.len(),
        "mean_ms": sorted.iter().sum::<f32>() / sorted.len().max(1) as f32,
        "p50_ms": percentile(&sorted, 0.50),
        "p95_ms": percentile(&sorted, 0.95),
        "p99_ms": percentile(&sorted, 0.99),
        "max_ms": percentile(&sorted, 1.0),
        "over_16_7ms": sorted.iter().filter(|&&ms| ms > 16.7).count(),
        "over_33ms": sorted.iter().filter(|&&ms| ms > 33.0).count(),
    })
}

fn write_summary(recorder: &PerfRecorder, diagnostics: &DiagnosticsStore) {
    let Some(paths) = session_log::paths() else {
        return;
    };
    let all = &recorder.samples[..];
    // Small on purpose: only the last window is sorted on the main thread for
    // the live log line; the full-history percentiles below run on their own
    // thread so the recorder never distorts the frames it measures.
    let recent: Vec<(f32, f32)> = {
        let now = all.last().map(|&(t, _)| t).unwrap_or(0.0);
        all.iter().copied().filter(|&(t, _)| t >= now - 5.0).collect()
    };
    let recent_stats = stats(&recent);
    session_log::log_line("perf", "INFO", &format!("window {recent_stats}"));

    // What a spiking frame has been made of, so far this run. This is the line
    // to read when asking "why does it stutter": the per-frame `[spike]` lines
    // say which events, this says which system across all of them.
    if recorder.spike_frames > 0 {
        let spikes = f32::from(u16::try_from(recorder.spike_frames).unwrap_or(u16::MAX));
        let per_spike: Vec<(f32, u64)> = (0..Probe::COUNT)
            .map(|slot| {
                (
                    recorder.spike_ms[slot] / spikes,
                    recorder.spike_hits[slot] / u64::from(recorder.spike_frames),
                )
            })
            .collect();
        session_log::log_line(
            "perf",
            "INFO",
            &format!(
                "[spike mean] {} frames over {SPIKE_MS:.0} ms, per spiking frame: {}",
                recorder.spike_frames,
                attribution(&per_spike, 0.05)
            ),
        );
    }

    let mut diag_map = Map::new();
    for diagnostic in diagnostics.iter() {
        let path = diagnostic.path().as_str().to_string();
        let mut entry = Map::new();
        if let Some(smoothed) = diagnostic.smoothed() {
            entry.insert("smoothed".into(), round2(smoothed));
        }
        if let Some(average) = diagnostic.average() {
            entry.insert("avg".into(), round2(average));
        }
        let max = diagnostic.values().copied().fold(f64::NAN, f64::max);
        if max.is_finite() {
            entry.insert("recent_max".into(), round2(max));
        }
        if !entry.is_empty() {
            diag_map.insert(path, Value::Object(entry));
        }
    }

    // One entry per probe that ever cost anything: mean over every frame, worst
    // single frame, and mean over the spiking frames alone.
    let frames = recorder.samples.len().max(1) as f32;
    let spike_frames = recorder.spike_frames.max(1) as f32;
    let mut probe_map = Map::new();
    for slot in 0..Probe::COUNT {
        if recorder.total_ms[slot] <= 0.0 {
            continue;
        }
        probe_map.insert(
            PROBE_NAMES[slot].into(),
            json!({
                "mean_ms": round2(f64::from(recorder.total_ms[slot] / frames)),
                "peak_ms": round2(f64::from(recorder.peak_ms[slot])),
                "mean_ms_in_spikes": round2(f64::from(recorder.spike_ms[slot] / spike_frames)),
            }),
        );
    }
    let spike_frame_count = recorder.spike_frames;
    let ecs = json!({
        "mean_ms": round2(f64::from(recorder.total_ecs_ms / frames)),
        "peak_ms": round2(f64::from(recorder.peak_ecs_ms)),
        "mean_ms_in_spikes": round2(f64::from(recorder.spike_ecs_ms / spike_frames)),
    });

    let samples = recorder.samples.clone();
    let keep_vsync = recorder.keep_vsync;
    let path = paths.root.join("perf_summary.json");
    std::thread::spawn(move || {
        // The first minute is allowed to stutter (loading, shader
        // compilation); the steady-state window is what the 60 fps promise
        // is about.
        let steady: Vec<(f32, f32)> = samples.iter().copied().filter(|&(t, _)| t >= 60.0).collect();
        let mut worst: Vec<(f32, f32)> = samples.clone();
        worst.sort_by(|a, b| f32::total_cmp(&b.1, &a.1));
        worst.truncate(25);

        let summary = json!({
            "elapsed_s": samples.last().map(|&(t, _)| t).unwrap_or(0.0),
            "vsync": keep_vsync,
            "all": stats(&samples),
            "steady_after_60s": stats(&steady),
            "worst_frames": worst
                .iter()
                .map(|&(t, ms)| json!({"t": t, "ms": ms}))
                .collect::<Vec<_>>(),
            "diagnostics": Value::Object(diag_map),
            "spike_frames_over_20ms": spike_frame_count,
            "ecs_main_schedule": ecs,
            "probes": Value::Object(probe_map),
        });
        if let Err(error) =
            fs::write(&path, serde_json::to_string_pretty(&summary).unwrap_or_default())
        {
            eprintln!("perf: could not write {}: {error}", path.display());
        }
    });
}

fn round2(value: f64) -> Value {
    Value::from((value * 100.0).round() / 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_probe_has_a_name() {
        assert_eq!(PROBE_NAMES.len(), Probe::COUNT);
        assert_eq!(PROBE_NAMES[Probe::EnginePump as usize], "engine pump");
        // The last variant must land on the last name, which is what catches a
        // variant added without its name (or the other way round).
        assert_eq!(PROBE_NAMES[Probe::Lamps as usize], "lamps");
        assert_eq!(Probe::Lamps as usize, Probe::COUNT - 1);
    }

    #[test]
    fn attribution_ranks_worst_first_and_drops_the_floor() {
        let mut per_probe = [(0.0, 0u64); Probe::COUNT];
        per_probe[Probe::Vermin as usize] = (9.1, 8);
        per_probe[Probe::EnginePump as usize] = (18.2, 1);
        per_probe[Probe::Marks as usize] = (0.01, 3); // below the floor
        assert_eq!(
            attribution(&per_probe, 0.3),
            "engine pump 18.2 (x1), vermin 9.1 (x8)"
        );
    }

    #[test]
    fn attribution_says_so_when_nothing_was_probed() {
        assert_eq!(
            attribution(&[(0.0, 0); Probe::COUNT], 0.3),
            "nothing probed"
        );
    }

    /// A player run must not pay for the instrumentation: with probes off,
    /// `span` hands back nothing at all and charges no slot.
    #[test]
    fn a_span_is_free_until_perf_is_asked_for() {
        PROBES_ENABLED.store(false, Ordering::Relaxed);
        assert!(span(Probe::Smoke).is_none());
        let before = PROBE_HITS[Probe::Smoke as usize].load(Ordering::Relaxed);
        drop(span(Probe::Smoke));
        assert_eq!(PROBE_HITS[Probe::Smoke as usize].load(Ordering::Relaxed), before);
    }

    /// The slots are process-wide, so this test owns `Probe::Water` alone —
    /// the others are left to `a_span_is_free_until_perf_is_asked_for`.
    #[test]
    fn a_charged_span_lands_in_its_own_slot() {
        PROBES_ENABLED.store(true, Ordering::Relaxed);
        PROBE_NS[Probe::Water as usize].store(0, Ordering::Relaxed);
        PROBE_HITS[Probe::Water as usize].store(0, Ordering::Relaxed);
        {
            let _span = span(Probe::Water);
            std::thread::yield_now();
        }
        assert_eq!(PROBE_HITS[Probe::Water as usize].load(Ordering::Relaxed), 1);
        assert!(PROBE_NS[Probe::Water as usize].load(Ordering::Relaxed) > 0);
        PROBES_ENABLED.store(false, Ordering::Relaxed);
    }
}
