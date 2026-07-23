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
//!   captured into the summary.

use std::fs;
use std::time::Duration;

use bevy::diagnostic::{
    DiagnosticsStore, EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin,
};
use bevy::prelude::*;
use bevy::render::diagnostic::RenderDiagnosticsPlugin;
use bevy::window::PresentMode;
use serde_json::{Map, Value, json};

use crate::session_log;

pub struct PerfPlugin;

impl Plugin for PerfPlugin {
    fn build(&self, app: &mut App) {
        let Ok(mode) = std::env::var("CATHEDRAL_PERF") else {
            return;
        };
        app.add_plugins((
            FrameTimeDiagnosticsPlugin::default(),
            EntityCountDiagnosticsPlugin::default(),
            RenderDiagnosticsPlugin,
        ))
        .insert_resource(PerfRecorder::new(mode == "vsync"))
        .add_systems(Startup, apply_present_mode)
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
}

impl PerfRecorder {
    fn new(keep_vsync: bool) -> Self {
        Self {
            keep_vsync,
            samples: Vec::with_capacity(120_000),
            flushed: 0,
            last_flush: 0.0,
            last_summary: 0.0,
        }
    }
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
) {
    let delta = time.delta();
    if delta == Duration::ZERO {
        return; // first frame
    }
    let elapsed = time.elapsed_secs();
    let frame_ms = delta.as_secs_f32() * 1000.0;
    recorder.samples.push((elapsed, frame_ms));

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
