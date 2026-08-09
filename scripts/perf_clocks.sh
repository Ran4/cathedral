#!/bin/bash
# Does the GPU's own power management explain the stutter?
#
# The same route, run twice in a row, sometimes comes out at p99 13 ms and
# sometimes at p99 45 ms. That is not the game's workload changing. This
# samples the GPU's clock, P-state and utilization at 100 ms alongside the run
# and prints them next to the frame-time percentiles, so a run that stuttered
# can be told apart from one that did not by what the hardware was doing.
#
# Read-only: nothing here changes a setting. ~2 minutes for the default 3 runs.
set -u
cd "$(dirname "$0")/.." || exit 1
export DISPLAY="${DISPLAY:-:0}"
export BEVY_ASSET_ROOT="$PWD"
export CATHEDRAL_HEADLESS=1
BIN=./target/release/cathedralbevy
RUNS="${1:-3}"

ROUTE='wait-online; sleep 5; tp 0 3 95 180; sleep 10; tp -20 3 249 0; sleep 12; quit'

for i in $(seq 1 "$RUNS"); do
  SAMPLES="/tmp/cathedral_clocks_$i.csv"
  nvidia-smi --query-gpu=utilization.gpu,clocks.sm,pstate,power.draw \
    --format=csv,noheader,nounits -lms 100 > "$SAMPLES" 2>/dev/null &
  SMI=$!
  CATHEDRAL_PERF=1 CATHEDRAL_FAKE_BACKEND=1 CATHEDRAL_DRIVE_TIMEOUT=180 \
    CATHEDRAL_DRIVE_RES=1920x1080 CATHEDRAL_DRIVE="$ROUTE" $BIN >/dev/null 2>&1
  kill $SMI 2>/dev/null; wait $SMI 2>/dev/null
  printf "    load %s\n" "$(cut -d' ' -f1-3 /proc/loadavg)"
  python3 scripts/perf_report.py "run_$i" 15
  python3 - "$SAMPLES" <<'PY'
import statistics, sys, collections
rows = [r.strip().split(", ") for r in open(sys.argv[1]) if r.strip()]
util, clk, pst, pwr = [], [], collections.Counter(), []
for r in rows:
    if len(r) < 4 or not r[0].isdigit():
        continue
    util.append(int(r[0])); clk.append(int(r[1])); pst[r[2]] += 1
    try: pwr.append(float(r[3]))
    except ValueError: pass
if not clk:
    print("    (no GPU samples)"); raise SystemExit
clk_sorted = sorted(clk)
print(f"    gpu util mean={statistics.fmean(util):3.0f}%  "
      f"sm clock min={clk_sorted[0]:4d} p50={clk_sorted[len(clk)//2]:4d} max={clk_sorted[-1]:4d} MHz  "
      f"power mean={statistics.fmean(pwr) if pwr else 0:5.1f} W  "
      f"pstates={dict(pst.most_common(4))}")
PY
done
