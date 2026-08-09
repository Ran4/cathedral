#!/bin/bash
# Before/after, interleaved, along the load-robustness curve.
#
# Two things make a naive A/B lie on this machine: the frame-time tail moves
# 10-20% with whatever else is running, and it moves much more than that when
# the GPU drops a P-state. So this does not run all the "before" measurements
# and then all the "after" ones — it alternates them at each load level, so a
# drift in ambient load lands on both sides of the comparison equally.
#
# The curve is the point, not any single row. A change that makes the game
# cheaper moves every row down; a change that removes a serialization point
# flattens the right-hand rows toward the left-hand ones. The second is what
# was asked for.
#
# Usage: perf_ab_under_load.sh <before-binary> <after-binary>
set -u
cd "$(dirname "$0")/.." || exit 1
BEFORE="${1:?usage: perf_ab_under_load.sh <before-binary> <after-binary>}"
AFTER="${2:?usage: perf_ab_under_load.sh <before-binary> <after-binary>}"
for b in "$BEFORE" "$AFTER"; do
  [ -x "$b" ] || { echo "!! not executable: $b"; exit 1; }
done
export DISPLAY="${DISPLAY:-:0}"
xset s off -dpms 2>/dev/null
export BEVY_ASSET_ROOT="$PWD"
export CATHEDRAL_HEADLESS=1
unset CATHEDRAL_HEADLESS_AUDIO

ROUTE='wait-online; sleep 5; tp 0 3 95 180; sleep 10; tp -20 3 249 0; sleep 12; quit'

HOGS=()
spin_up() {
  for _ in $(seq 1 "$1"); do
    awk 'BEGIN{x=0;while(1){x=(x*1103515245+12345)%2147483648}}' </dev/null >/dev/null 2>&1 &
    HOGS+=($!)
  done
}
spin_down() {
  for pid in "${HOGS[@]:-}"; do kill "$pid" 2>/dev/null; done
  HOGS=()
}
trap 'spin_down' EXIT

measure() { # measure <binary> <label>
  CATHEDRAL_PERF=1 CATHEDRAL_FAKE_BACKEND=1 CATHEDRAL_DRIVE_TIMEOUT=180 \
    CATHEDRAL_DRIVE_RES=1920x1080 CATHEDRAL_DRIVE="$ROUTE" "$1" >/dev/null 2>&1
  # The GPU's P-state is the difference between a bad run and a meaningless
  # one, so it is reported next to every row rather than trusted to be P0.
  local clk
  clk=$(nvidia-smi --query-gpu=clocks.sm --format=csv,noheader,nounits 2>/dev/null)
  printf "%s" "$(python3 scripts/perf_report.py "$2" 15)"
  printf "  [sm %s MHz]\n" "${clk:-?}"
}

for extra in 0 4 8 12; do
  echo "== +${extra} busy cores =="
  spin_up "$extra"
  sleep 2
  measure "$BEFORE" "before_load+${extra}"
  measure "$AFTER"  "after_load+${extra}"
  spin_down
  sleep 3
done
