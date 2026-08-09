#!/bin/bash
# What does the soundscape cost the frame?
#
# A headless run is muted (see CATHEDRAL_DRIVE.md), so the standard suite
# measures a game nobody can hear — which is not the game anybody plays. This
# runs the same route twice, once muted and once with the whole audio stack
# live, and keeps the desktop silent by sending the audio to a null sink that
# exists only for the duration of the run. Nothing is played out of the
# speakers and the default sink is never touched.
#
# Builds nothing: run `cargo build --release` first. ~4 minutes.
set -u
cd "$(dirname "$0")/.." || exit 1
export DISPLAY="${DISPLAY:-:0}"
xset s off -dpms 2>/dev/null
export BEVY_ASSET_ROOT="$PWD"
export CATHEDRAL_HEADLESS=1
BIN=./target/release/cathedralbevy

ROUTE='wait-online; sleep 5; tp 0 3 95 180; sleep 10; tp -20 3 249 0; sleep 15; tp -113 3 161 90; sleep 10; tp 102 3 212 180; sleep 10; quit'

echo "== muted (the standard headless run) =="
CATHEDRAL_PERF=1 CATHEDRAL_FAKE_BACKEND=1 CATHEDRAL_DRIVE_TIMEOUT=180 \
  CATHEDRAL_DRIVE_RES=1920x1080 CATHEDRAL_DRIVE="$ROUTE" $BIN >/dev/null 2>&1
python3 scripts/perf_report.py audio_off 15

SINK=$(pactl load-module module-null-sink sink_name=cathedral_silent \
  sink_properties=device.description=cathedral_silent 2>/dev/null)
if [[ -z "$SINK" ]]; then
  echo "!! could not create a null sink — refusing to run audio out of the speakers"
  exit 1
fi
trap 'pactl unload-module "$SINK" 2>/dev/null' EXIT

echo "== audio on, played into a null sink =="
PULSE_SINK=cathedral_silent CATHEDRAL_HEADLESS_AUDIO=1 \
  CATHEDRAL_PERF=1 CATHEDRAL_FAKE_BACKEND=1 CATHEDRAL_DRIVE_TIMEOUT=180 \
  CATHEDRAL_DRIVE_RES=1920x1080 CATHEDRAL_DRIVE="$ROUTE" $BIN >/dev/null 2>&1
python3 scripts/perf_report.py audio_on 15
