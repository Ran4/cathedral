"""Summarize final two-day traces without modifying historical M0 evidence.

uv run --no-project features/implemented/ambient_residents_evidence/summarize_final.py LOG...
"""
import json
import statistics
import sys
from pathlib import Path


def summarize(rows):
    population = [r["generated"] for r in rows]
    stationary = [100 * p["stationary"] / p["present"] for p in population]
    scalars = [k for k, v in population[0].items() if isinstance(v, (int, float))]
    return {
        "samples": len(rows),
        "stationary_mean_percent": statistics.mean(stationary),
        "stationary_min_percent": min(stationary),
        "stationary_max_percent": max(stationary),
        "samples_at_least_85_percent_stationary": sum(p >= 85 for p in stationary),
        "mean_counts": {k: statistics.mean(p[k] for p in population) for k in scalars},
        "minimum_counts": {k: min(p[k] for p in population) for k in scalars},
        "maximum_counts": {k: max(p[k] for p in population) for k in scalars},
        "occupied_patches_range": [min(len(p["resident_occupied_patches"]) for p in population),
                                   max(len(p["resident_occupied_patches"]) for p in population)],
        "occupied_cells_range": [min(len(p["occupied_cells"]) for p in population),
                                 max(len(p["occupied_cells"]) for p in population)],
        "largest_cell_population": max(max(p["occupied_cells"].values()) for p in population),
        "largest_patch_population": max(max(p["resident_occupied_patches"].values()) for p in population),
        "walking_causes": {k: statistics.mean(p["walking_by_cause"].get(k, 0) for p in population)
                           for k in sorted(set().union(*(p["walking_by_cause"] for p in population)))},
    }


def main(filename):
    source = Path(filename)
    lines = source.read_text().splitlines()
    rows = [json.loads(s.removeprefix("[motion] ")) for s in lines if s.startswith("[motion] ")]
    cost = next(json.loads(s.removeprefix("[motion-cost] ")) for s in lines if s.startswith("[motion-cost] "))
    config = next(json.loads(s.removeprefix("[motion-config] ")) for s in lines if s.startswith("[motion-config] "))
    assert len(rows) == 241 and abs(rows[-1]["elapsed_seconds"] - 7200) < 1e-6
    assert cost["polls"] == 144000
    count = rows[0]["generated"]["total"]
    for row in rows:
        assert row["generated"]["present"] == count == row["generated"]["residents"]
        for cohort in [row["generated"], row["authored"]]:
            assert cohort["present"] + cohort["absent"] == cohort["total"]
            assert cohort["stationary"] + sum(cohort["walking_by_cause"].values()) == cohort["present"]
            assert sum(cohort["occupied_cells"].values()) == cohort["present"]
            assert cohort["indoors"] is None
        row["motion_sample_valid"] = True
    measured = rows[1:]
    daylight = [r for r in measured if r["elapsed_seconds"] >= 300
                and r["office"] in ["Dayspring", "High Wick", "the Waning"]]
    first_day = [r for r in daylight if r["elapsed_seconds"] < 3600 - 1e-6]
    baseline = json.loads((source.parent / f"baseline_{count}_summary.json").read_text())
    initial, final = (r["generated"] for r in [rows[0], rows[-1]])
    result = {
        "population": count, "config": config, "cost": cost,
        "all_two_days": summarize(measured),
        "settled_clear_daytime": summarize(daylight),
        "matching_first_day_daytime": summarize(first_day),
        "baseline_first_day_daytime": baseline["settled_clear_daytime"],
        "by_office": {office: summarize([r for r in measured if r["office"] == office])
                      for office in sorted({r["office"] for r in measured})},
        "excluded_motion_samples": [{"elapsed_seconds": rows[0]["elapsed_seconds"],
                                     "reason": "Initial reference; all 240 subsequent motion slices retained."}],
        "settled_definition": "Elapsed >=300s, Dayspring/High Wick/Waning; full InCity generated denominator, including all outdoor resting proxies.",
        "distribution": {
            "initial_final_cells": [len(p["occupied_cells"]) for p in [initial, final]],
            "initial_final_patches": [len(p["resident_occupied_patches"]) for p in [initial, final]],
            "new_final_patch_ids": sorted(set(final["resident_occupied_patches"]) - set(initial["resident_occupied_patches"])),
            "final_away_from_initial_15m": final["away_from_initial_15m"],
            "final_sampled_away_seconds_per_actor": final["sampled_away_seconds"] / count,
            "longest_sampled_away_episode_seconds": final["longest_sampled_away_episode_seconds"],
        },
        "scope": "Sparse actual-displacement samples at .05-second polls, not integrated walking time. Patch counters reflect resident spot membership; independent position cells and displacement detect physical drift. Optional admission and full route limits are controller invariants covered by mechanical tests.",
    }
    output = source.with_name(f"final_{count}_summary.json")
    output.write_text(json.dumps(result, indent=2) + "\n")
    source.with_suffix(".jsonl").write_text("".join(json.dumps(r) + "\n" for r in rows))
    print(output, result["settled_clear_daytime"]["stationary_mean_percent"], cost)


if __name__ == "__main__":
    for name in sys.argv[1:]:
        main(name)
