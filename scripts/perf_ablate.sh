#!/bin/bash
# Ablation quadrant: run the same route with one subsystem switched off at a
# time, so a frame-time tail can be attributed to a subsystem instead of
# guessed at. Uses the `CATHEDRAL_NO_*` levers already in src/main.rs.
#
# Headless and muted throughout (see perf_suite_headless.sh) — these runs are
# started on somebody else's desktop.
#
# Builds nothing: run `cargo build --release` first. ~8 minutes.
set -u
cd "$(dirname "$0")/.." || exit 1
export DISPLAY="${DISPLAY:-:0}"
xset s off -dpms 2>/dev/null
export BEVY_ASSET_ROOT="$PWD"
export CATHEDRAL_HEADLESS=1
unset CATHEDRAL_HEADLESS_AUDIO
BIN=./target/release/cathedralbevy

# A short tour: five vantages, the ones the full suite's tour spikes hardest on.
ROUTE='wait-online; sleep 5; tp 0 3 95 180; sleep 10; tp -20 3 249 0; sleep 10; tp -113 3 161 90; sleep 10; tp 102 3 212 180; sleep 10; tp 0 120 350 0 -12; sleep 10; quit'

run() { # run <label> [env assignments...]
  local label="$1"; shift
  env "$@" CATHEDRAL_PERF=1 CATHEDRAL_FAKE_BACKEND=1 CATHEDRAL_DRIVE_TIMEOUT=180 \
    CATHEDRAL_DRIVE_RES=1920x1080 CATHEDRAL_DRIVE="$ROUTE" $BIN >/dev/null 2>&1
  python3 scripts/perf_report.py "$label" 15
}

run full
run no_vermin   CATHEDRAL_NO_VERMIN=1
run no_dogs     CATHEDRAL_NO_DOGS=1
run no_marks    CATHEDRAL_NO_MARKS=1
run no_actors   CATHEDRAL_NO_ACTORS=1
run no_weather  CATHEDRAL_NO_WEATHER=1
run bare        CATHEDRAL_NO_VERMIN=1 CATHEDRAL_NO_DOGS=1 CATHEDRAL_NO_MARKS=1 \
                CATHEDRAL_NO_ACTORS=1 CATHEDRAL_NO_WEATHER=1
