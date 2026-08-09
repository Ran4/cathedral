#!/bin/bash
# Is a spiking frame the GPU actually working, or the CPU waiting on something
# that is not work? Samples `nvidia-smi` at 100 ms alongside a normal drive run
# and prints the utilization distribution next to the frame-time one.
#
# A frame that takes 40 ms with the GPU at 30% is not a GPU-bound frame, and
# says to go looking on the CPU side of the renderer instead.
set -u
cd "$(dirname "$0")/.." || exit 1
export DISPLAY="${DISPLAY:-:0}"
export BEVY_ASSET_ROOT="$PWD"
export CATHEDRAL_HEADLESS=1
BIN=./target/release/cathedralbevy
SAMPLES=/tmp/cathedral_gpu_samples.csv

ROUTE='wait-online; sleep 5; tp 0 3 95 180; sleep 10; tp -20 3 249 0; sleep 12; tp -113 3 161 90; sleep 10; quit'

nvidia-smi --query-gpu=utilization.gpu,utilization.memory,clocks.sm \
  --format=csv,noheader,nounits -lms 100 > "$SAMPLES" 2>/dev/null &
SMI=$!
trap 'kill $SMI 2>/dev/null' EXIT

CATHEDRAL_PERF=1 CATHEDRAL_FAKE_BACKEND=1 CATHEDRAL_DRIVE_TIMEOUT=180 \
  CATHEDRAL_DRIVE_RES=1920x1080 CATHEDRAL_DRIVE="$ROUTE" $BIN >/dev/null 2>&1
kill $SMI 2>/dev/null

python3 scripts/perf_report.py gpu_sampled 15
python3 - "$SAMPLES" <<'PY'
import statistics, sys
rows = [r.split(", ") for r in open(sys.argv[1]) if r.strip()]
util = [int(r[0]) for r in rows if r[0].strip().isdigit()]
if not util:
    print("  no GPU samples"); raise SystemExit
util.sort()
def pct(q): return util[min(len(util) - 1, round((len(util) - 1) * q))]
print(f"  gpu util over {len(util)} samples: "
      f"p10={pct(.1)}% p50={pct(.5)}% p90={pct(.9)}% max={util[-1]}% "
      f"mean={statistics.fmean(util):.0f}%")
print(f"  samples at/over 95%: {sum(1 for u in util if u >= 95)} "
      f"({100 * sum(1 for u in util if u >= 95) / len(util):.0f}%)")
PY
