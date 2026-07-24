#!/bin/bash
# The standard performance suite (see features/performance_improvements/findings.md).
# Builds nothing: run `cargo build --release` first. ~10 minutes of runs.
set -u
cd "$(dirname "$0")/.." || exit 1
export DISPLAY="${DISPLAY:-:0}"
xset s off -dpms 2>/dev/null # a blanked screen throttles the window to 1 fps
export BEVY_ASSET_ROOT="$PWD"
BIN=./target/release/cathedralbevy

# Waypoints re-picked 2026-07 for the 0.7x city shrink (lore/places/
# shrink_transform.json): spawn views identity (cathedral core), Wickmarket
# scaled, needle -> resolved needle[2], Shambles/Bellfoot by cluster delta,
# Draper's stop on the re-authored Reach facing the hall row, aerial pulled in
# to z=350 for equivalent framing.
TOUR='wait-online; sleep 5; tp 0 3 95 180; sleep 8; tp 0 3 95 0; sleep 8; tp -20 3 249 0; sleep 8; tp -20 3 249 180; sleep 8; tp -113 3 161 90; sleep 8; tp -294 3 220 0; sleep 8; tp 5 3 -176 0; sleep 8; tp 102 3 212 180; sleep 8; tp 0 120 350 0 -12; sleep 8; tp 0 3 95 180; sleep 8; quit'

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
  CATHEDRAL_DRIVE='wait-online; sleep 2; tp -20 3 249 0; sleep 90; quit' $BIN >/dev/null 2>&1
python3 scripts/perf_report.py stand_1080p 20

echo "== walking 1080p (hold W) =="
# Blind holds: leg 1 walks the Wickmarket corridor north (clear to the wall at
# z=339.5 by the west gate); leg 2 walks Harne Road south from the
# Bellfounders (135 m of straight clear street in the shrunk plan — the old
# needle leg now dead-ends into the toll house at ~90 m).
CATHEDRAL_PERF=1 CATHEDRAL_FAKE_BACKEND=1 CATHEDRAL_DRIVE_TIMEOUT=300 \
  CATHEDRAL_DRIVE_RES=1920x1080 \
  CATHEDRAL_DRIVE='wait-online; sleep 2; tp -20 2 224 180; sleep 3; hold KeyW 30; sleep 2; tp 56 2 -252 17; hold KeyW 30; sleep 2; quit' $BIN >/dev/null 2>&1
python3 scripts/perf_report.py walk_1080p 15

echo "== vsync pacing 1080p stand-still =="
CATHEDRAL_PERF=vsync CATHEDRAL_FAKE_BACKEND=1 CATHEDRAL_DRIVE_TIMEOUT=200 \
  CATHEDRAL_DRIVE_RES=1920x1080 \
  CATHEDRAL_DRIVE='wait-online; sleep 2; tp -20 3 249 0; sleep 90; quit' $BIN >/dev/null 2>&1
python3 scripts/perf_report.py vsync_1080p 20
