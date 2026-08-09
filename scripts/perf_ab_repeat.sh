#!/bin/bash
# Paired before/after, repeated, at one load level — and reported as medians.
#
# A single pair cannot decide anything here: this machine's tail moves 10-20%
# run to run, and a GPU that drops a P-state moves it several times that. So
# alternate the two binaries N times and compare the *median* of each
# percentile across the repeats, which is robust to the one run in five that
# gets stepped on.
#
# Usage: perf_ab_repeat.sh <before-binary> <after-binary> [repeats] [extra-load]
set -u
cd "$(dirname "$0")/.." || exit 1
BEFORE="${1:?usage: perf_ab_repeat.sh <before> <after> [repeats] [extra-load]}"
AFTER="${2:?usage: perf_ab_repeat.sh <before> <after> [repeats] [extra-load]}"
REPEATS="${3:-4}"
EXTRA="${4:-0}"
export DISPLAY="${DISPLAY:-:0}"
xset s off -dpms 2>/dev/null
export BEVY_ASSET_ROOT="$PWD"
export CATHEDRAL_HEADLESS=1
OUT=$(mktemp)

HOGS=()
for _ in $(seq 1 "$EXTRA"); do
  awk 'BEGIN{x=0;while(1){x=(x*1103515245+12345)%2147483648}}' </dev/null >/dev/null 2>&1 &
  HOGS+=($!)
done
cleanup() { for pid in "${HOGS[@]:-}"; do kill "$pid" 2>/dev/null; done; rm -f "$OUT"; }
trap cleanup EXIT
[ "$EXTRA" -gt 0 ] && sleep 2

ROUTE='wait-online; sleep 5; tp 0 3 95 180; sleep 10; tp -20 3 249 0; sleep 12; quit'

for i in $(seq 1 "$REPEATS"); do
  for side in before after; do
    bin="$BEFORE"; [ "$side" = after ] && bin="$AFTER"
    CATHEDRAL_PERF=1 CATHEDRAL_FAKE_BACKEND=1 CATHEDRAL_DRIVE_TIMEOUT=90 \
      CATHEDRAL_DRIVE_RES=1920x1080 CATHEDRAL_DRIVE="$ROUTE" "$bin" >/dev/null 2>&1
    python3 scripts/perf_report.py "${side}_${i}" 15 | tee -a "$OUT"
  done
done

echo
echo "== medians over $REPEATS paired runs, +$EXTRA busy cores =="
python3 - "$OUT" <<'PY'
import re, statistics, sys
rows = {"before": [], "after": []}
pat = re.compile(r"^(before|after)_\d+\s+frames=\s*(\d+) mean=\s*([\d.]+) p50=\s*([\d.]+) "
                 r"p95=\s*([\d.]+) p99=\s*([\d.]+) max=\s*([\d.]+) over16\.7=\s*(\d+) \(\s*([\d.]+)%\)")
for line in open(sys.argv[1]):
    m = pat.match(line.strip())
    if m:
        rows[m[1]].append(tuple(float(m[i]) for i in (3, 4, 5, 6, 7, 9)))
if not rows["before"] or not rows["after"]:
    print("  not enough samples"); raise SystemExit
def med(side, idx): return statistics.median(r[idx] for r in rows[side])
names = ["mean", "p50", "p95", "p99", "max", "over16.7%"]
print(f"  {'':10s} " + "".join(f"{n:>11s}" for n in names))
for side in ("before", "after"):
    print(f"  {side:10s} " + "".join(f"{med(side, i):11.2f}" for i in range(6)))
print(f"  {'delta':10s} " + "".join(
    f"{med('after', i) - med('before', i):+11.2f}" for i in range(6)))
PY
