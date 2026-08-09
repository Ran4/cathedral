#!/bin/bash
# The load-robustness curve.
#
# Measuring this game on an idle machine flatters it: at load ~15 of 20 cores it
# already holds p99 ~14 ms with 0.3% of frames over budget. What the player
# actually feels is what happens when the machine is busy — and the fall is
# steep, because a main thread that cannot feed the GPU lets it fall back to a
# low P-state, at which point everything doubles again.
#
# So the number worth optimizing is not one frame-time percentile but the shape
# of the curve: run the same route against 0, 4, 8 and 12 extra busy cores and
# see how fast it degrades. A change that flattens this curve is a change that
# removed a serialization point; one that only shifts it down removed work.
#
# The synthetic load runs at normal priority on purpose (nice'd load would be
# preempted by the game and measure nothing), so the desktop WILL feel sluggish
# for the ~3 minutes this takes. Nothing is left running afterwards.
#
# Builds nothing: run `cargo build --release` first.
# Usage: perf_under_load.sh [label]
set -u
cd "$(dirname "$0")/.." || exit 1
LABEL="${1:-run}"
export DISPLAY="${DISPLAY:-:0}"
xset s off -dpms 2>/dev/null
export BEVY_ASSET_ROOT="$PWD"
export CATHEDRAL_HEADLESS=1
unset CATHEDRAL_HEADLESS_AUDIO
BIN=./target/release/cathedralbevy

ROUTE='wait-online; sleep 5; tp 0 3 95 180; sleep 10; tp -20 3 249 0; sleep 12; quit'

HOGS=()
spin_up() { # spin_up <n>
  local n="$1"
  for _ in $(seq 1 "$n"); do
    # A pure ALU loop in awk: no allocation, no syscalls, no disk — contention
    # for CPU time and nothing else, so the curve measures scheduling and not
    # some accident of memory bandwidth.
    awk 'BEGIN{x=0;while(1){x=(x*1103515245+12345)%2147483648}}' </dev/null >/dev/null 2>&1 &
    HOGS+=($!)
  done
}
spin_down() {
  for pid in "${HOGS[@]:-}"; do kill "$pid" 2>/dev/null; done
  HOGS=()
  # Let the run-queue drain before the next measurement starts.
  sleep 3
}
trap 'spin_down' EXIT

for extra in 0 4 8 12; do
  spin_up "$extra"
  sleep 2
  CATHEDRAL_PERF=1 CATHEDRAL_FAKE_BACKEND=1 CATHEDRAL_DRIVE_TIMEOUT=180 \
    CATHEDRAL_DRIVE_RES=1920x1080 CATHEDRAL_DRIVE="$ROUTE" $BIN >/dev/null 2>&1
  CLK=$(nvidia-smi --query-gpu=clocks.sm --format=csv,noheader,nounits 2>/dev/null)
  python3 scripts/perf_report.py "${LABEL}_load+${extra}" 15
  spin_down
done
