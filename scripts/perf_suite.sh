#!/bin/bash
# The standard performance suite (see features/performance_improvements/findings.md).
# Builds nothing: run `cargo build --release` first. ~10 minutes of runs.
set -u
cd "$(dirname "$0")/.." || exit 1
export DISPLAY="${DISPLAY:-:0}"
xset s off -dpms 2>/dev/null # a blanked screen throttles the window to 1 fps
export BEVY_ASSET_ROOT="$PWD"
BIN=./target/release/cathedralbevy

TOUR='wait-online; sleep 5; tp 0 3 95 180; sleep 8; tp 0 3 95 0; sleep 8; tp -28 3 356 0; sleep 8; tp -28 3 356 180; sleep 8; tp -162 3 230 90; sleep 8; tp -395 3 315 0; sleep 8; tp 24 3 -257 0; sleep 8; tp 120 3 260 0; sleep 8; tp 0 120 500 0 -12; sleep 8; tp 0 3 95 180; sleep 8; quit'

echo "== tour 720p =="
CATHEDRAL_PERF=1 CATHEDRAL_FAKE_BACKEND=1 CATHEDRAL_DRIVE_TIMEOUT=300 \
  CATHEDRAL_DRIVE="$TOUR" $BIN >/dev/null 2>&1
python3 scripts/perf_report.py tour_720p 20

echo "== tour 1080p =="
CATHEDRAL_PERF=1 CATHEDRAL_FAKE_BACKEND=1 CATHEDRAL_DRIVE_TIMEOUT=300 \
  CATHEDRAL_DRIVE_RES=1920x1080 CATHEDRAL_DRIVE="$TOUR" $BIN >/dev/null 2>&1
python3 scripts/perf_report.py tour_1080p 20

echo "== stand-still wickmarket 1080p =="
CATHEDRAL_PERF=1 CATHEDRAL_FAKE_BACKEND=1 CATHEDRAL_DRIVE_TIMEOUT=200 \
  CATHEDRAL_DRIVE_RES=1920x1080 \
  CATHEDRAL_DRIVE='wait-online; sleep 2; tp -28 3 356 0; sleep 90; quit' $BIN >/dev/null 2>&1
python3 scripts/perf_report.py stand_1080p 20

echo "== walking 1080p (hold W) =="
CATHEDRAL_PERF=1 CATHEDRAL_FAKE_BACKEND=1 CATHEDRAL_DRIVE_TIMEOUT=300 \
  CATHEDRAL_DRIVE_RES=1920x1080 \
  CATHEDRAL_DRIVE='wait-online; sleep 2; tp -28 2 320 180; sleep 3; hold KeyW 30; sleep 2; tp -162 2 230 90; hold KeyW 30; sleep 2; quit' $BIN >/dev/null 2>&1
python3 scripts/perf_report.py walk_1080p 15

echo "== vsync pacing 1080p stand-still =="
CATHEDRAL_PERF=vsync CATHEDRAL_FAKE_BACKEND=1 CATHEDRAL_DRIVE_TIMEOUT=200 \
  CATHEDRAL_DRIVE_RES=1920x1080 \
  CATHEDRAL_DRIVE='wait-online; sleep 2; tp -28 3 356 0; sleep 90; quit' $BIN >/dev/null 2>&1
python3 scripts/perf_report.py vsync_1080p 20
