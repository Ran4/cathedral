"""Summarize the bounded 20,000-request capacity/performance observation."""
import json
import re
from pathlib import Path

from summarize_final import summarize

directory = Path(__file__).resolve().parent
lines = (directory / "stress_20000.log").read_text().splitlines()
rows = [json.loads(s[9:]) for s in lines if s.startswith("[motion] ")]
cost = next(json.loads(s[14:]) for s in lines if s.startswith("[motion-cost] "))
stderr = (directory / "stress_20000.err").read_text()
placement = re.search(r"requested (\d+), placed (\d+), unplaced (\d+); (\d+) housed, (\d+) hardship, (\d+) workers; door cap (\d+)", stderr)
assert placement
allocation = dict(zip(["requested", "placed", "unplaced", "housed", "hardship", "workers", "door_cap"], map(int, placement.groups())))
assert allocation["requested"] == allocation["placed"] + allocation["unplaced"] == 20000
assert len(rows) == 5 and cost["polls"] == 2400 and abs(rows[-1]["elapsed_seconds"] - 120) < 1e-6
assert all(r["generated"]["present"] == allocation["placed"] for r in rows)
result = {"allocation": allocation, "cost": cost, "observed_after_initial": summarize(rows[1:]),
          "scope": "120 simulation seconds at 3600s/day,1x,.05s polls. Capacity and CPU stress observation only; no two-day or GPU frame-rate claim at this request.",
          "rows": rows}
(directory / "stress_20000_summary.json").write_text(json.dumps(result, indent=2) + "\n")
print(allocation, cost)
