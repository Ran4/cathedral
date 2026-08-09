#!/bin/bash
# Is the headless harness lying to us?
#
# Every performance number in this repo since 2026-08 was measured with
# CATHEDRAL_HEADLESS=1 — the window is created but never mapped. That keeps a
# measurement session off somebody's desktop, but it is also the one thing that
# differs from the game anybody actually plays, and the render half of the frame
# is where the stutter lives. So: the same route twice, back to back, unmapped
# then mapped, and the only difference between them is the window.
#
# Back to back on purpose. This machine's frame-time tail moves by 10-20% with
# ambient load, so two runs taken minutes apart cannot be compared; two runs
# taken seconds apart can.
#
#   THE SECOND RUN PUTS A 1920x1080 WINDOW ON YOUR SCREEN FOR ABOUT 45 SECONDS
#   AND TAKES THE KEYBOARD FOCUS. It closes itself. Nothing else is left behind.
#
# It stays silent: the audio is played into a null sink that exists only for the
# duration of the run, and your default output device is never touched.
#
# Usage:  ./scripts/perf_windowed_ab.sh
set -u
cd "$(dirname "$0")/.." || exit 1
export DISPLAY="${DISPLAY:-:0}"
xset s off -dpms 2>/dev/null
export BEVY_ASSET_ROOT="$PWD"

# Deliberately the binary that produced the headless numbers being compared
# against, not a fresh build: the working tree is mid-change, and rebuilding
# would compare two different games rather than two different windows.
SAVED=/tmp/claude-1000/-home-ran-src-rust-cathedralbevy/6ab4a41c-eda5-460b-baa1-03e9a9671108/scratchpad/cathedralbevy_before
BIN="$SAVED"
[ -x "$BIN" ] || BIN=./target/release/cathedralbevy
[ -x "$BIN" ] || { echo "!! no release binary — run 'cargo build --release' first"; exit 1; }
echo "using $BIN"

ROUTE='wait-online; sleep 5; tp 0 3 95 180; sleep 10; tp -20 3 249 0; sleep 12; tp -113 3 161 90; sleep 10; quit'

SINK=$(pactl load-module module-null-sink sink_name=cathedral_silent \
  sink_properties=device.description=cathedral_silent 2>/dev/null)
[ -n "$SINK" ] && trap 'pactl unload-module "$SINK" 2>/dev/null' EXIT

echo
echo "== 1/2: headless (no window, ~45s) =="
CATHEDRAL_HEADLESS=1 \
  CATHEDRAL_PERF=1 CATHEDRAL_FAKE_BACKEND=1 CATHEDRAL_DRIVE_TIMEOUT=180 \
  CATHEDRAL_DRIVE_RES=1920x1080 CATHEDRAL_DRIVE="$ROUTE" "$BIN" >/dev/null 2>&1
python3 scripts/perf_report.py headless 15

echo
echo "== 2/2: windowed — a window is about to appear for ~45s =="
sleep 2
PULSE_SINK=cathedral_silent \
  CATHEDRAL_PERF=1 CATHEDRAL_FAKE_BACKEND=1 CATHEDRAL_DRIVE_TIMEOUT=180 \
  CATHEDRAL_DRIVE_RES=1920x1080 CATHEDRAL_DRIVE="$ROUTE" "$BIN" >/dev/null 2>&1
python3 scripts/perf_report.py windowed 15

echo
echo "Done — the window is closed. Paste both lines back."
