#!/bin/bash
# The standard performance suite, run with the window created but never mapped
# (`CATHEDRAL_HEADLESS=1`) so a measurement session never appears on, or steals
# focus from, somebody's desktop — and stays silent, because a run somebody
# cannot see is one they must not have to hear either. Audio is therefore
# outside what this suite measures; attribute audio cost with a single
# `CATHEDRAL_HEADLESS_AUDIO=1` run when that is the question, and warn first.
#
# Builds nothing: run `cargo build --release` first. ~10 minutes of runs.
# Pass a label prefix as $1 to tag the report lines (e.g. `before`, `after`).
set -u
cd "$(dirname "$0")/.." || exit 1
TAG="${1:-run}"
export DISPLAY="${DISPLAY:-:0}"
xset s off -dpms 2>/dev/null # a blanked screen throttles the window to 1 fps
export BEVY_ASSET_ROOT="$PWD"
export CATHEDRAL_HEADLESS=1
unset CATHEDRAL_HEADLESS_AUDIO
BIN=./target/release/cathedralbevy

TOUR='wait-online; sleep 5; tp 0 3 95 180; sleep 8; tp 0 3 95 0; sleep 8; tp -20 3 249 0; sleep 8; tp -20 3 249 180; sleep 8; tp -113 3 161 90; sleep 8; tp -294 3 220 0; sleep 8; tp 5 3 -176 0; sleep 8; tp 102 3 212 180; sleep 8; tp 0 120 350 0 -12; sleep 8; tp 0 3 95 180; sleep 8; quit'

echo "== tour 1080p =="
CATHEDRAL_PERF=1 CATHEDRAL_FAKE_BACKEND=1 CATHEDRAL_DRIVE_TIMEOUT=300 \
  CATHEDRAL_DRIVE_RES=1920x1080 CATHEDRAL_DRIVE="$TOUR" $BIN >/dev/null 2>&1
python3 scripts/perf_report.py "${TAG}_tour_1080p" 20

echo "== stand-still wickmarket 1080p =="
CATHEDRAL_PERF=1 CATHEDRAL_FAKE_BACKEND=1 CATHEDRAL_DRIVE_TIMEOUT=200 \
  CATHEDRAL_DRIVE_RES=1920x1080 \
  CATHEDRAL_DRIVE='wait-online; sleep 2; tp -20 3 249 0; sleep 90; quit' $BIN >/dev/null 2>&1
python3 scripts/perf_report.py "${TAG}_stand_1080p" 20

echo "== walking 1080p (hold W) =="
CATHEDRAL_PERF=1 CATHEDRAL_FAKE_BACKEND=1 CATHEDRAL_DRIVE_TIMEOUT=300 \
  CATHEDRAL_DRIVE_RES=1920x1080 \
  CATHEDRAL_DRIVE='wait-online; sleep 2; tp -20 2 224 180; sleep 3; hold KeyW 30; sleep 2; tp 56 2 -252 17; hold KeyW 30; sleep 2; quit' $BIN >/dev/null 2>&1
python3 scripts/perf_report.py "${TAG}_walk_1080p" 15

echo "== vsync pacing 1080p stand-still =="
CATHEDRAL_PERF=vsync CATHEDRAL_FAKE_BACKEND=1 CATHEDRAL_DRIVE_TIMEOUT=200 \
  CATHEDRAL_DRIVE_RES=1920x1080 \
  CATHEDRAL_DRIVE='wait-online; sleep 2; tp -20 3 249 0; sleep 90; quit' $BIN >/dev/null 2>&1
python3 scripts/perf_report.py "${TAG}_vsync_1080p" 20
