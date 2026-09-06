"""uv run --no-project features/implemented/ambient_residents_evidence/summarize.py LOG..."""

import json
import statistics
import sys
from pathlib import Path


def summary(rows):
    generated = [row["generated"] for row in rows]
    stationary = [row["stationary"] / row["present"] for row in generated]
    return {
        "samples": len(rows),
        "stationary_mean_percent": 100 * statistics.mean(stationary),
        "stationary_min_percent": 100 * min(stationary),
        "stationary_max_percent": 100 * max(stationary),
        "samples_at_least_85_percent_stationary": sum(x >= 0.85 for x in stationary),
        "mean_counts": {
            key: statistics.mean(row[key] for row in generated)
            for key in [
                "stationary", "optional_walking", "domestic_walking",
                "explicit_intent_walking", "emergency_custody_walking",
                "routine_needs_walking", "other_walking", "pending_path",
                "positive_speed", "stationary_with_path", "resting_at_home_proxy",
                "water_queue", "food_queue", "famished", "hunger_mean",
                "away_from_initial_15m", "in_initial_cell",
            ]
        },
        "mean_occupied_20m_cells": statistics.mean(len(row["occupied_cells"]) for row in generated),
        "max_single_cell_population": max(max(row["occupied_cells"].values()) for row in generated),
        "max_water_queue_members": max(row["water_queue"] for row in generated),
        "max_food_queue_members": max(row["food_queue"] for row in generated),
        "max_famished": max(row["famished"] for row in generated),
        "max_motion_vs_positive_speed_count_difference": max(
            abs(row["present"] - row["stationary"] - row["positive_speed"]) for row in generated
        ),
    }


for filename in sys.argv[1:]:
    source = Path(filename)
    lines = source.read_text().splitlines()
    rows = [json.loads(line.removeprefix("[motion] ")) for line in lines if line.startswith("[motion] ")]
    cost = next(json.loads(line.removeprefix("[motion-cost] ")) for line in lines if line.startswith("[motion-cost] "))
    config = next(json.loads(line.removeprefix("[motion-config] ")) for line in lines if line.startswith("[motion-config] "))
    population = rows[0]["generated"]["total"]
    for row in rows:
        assert row["generated"]["present"] == population
        for cohort in [row["generated"], row["authored"]]:
            assert cohort["present"] + cohort["absent"] == cohort["total"]
            assert cohort["stationary"] + sum(cohort["walking_by_cause"].values()) == cohort["present"]
            assert sum(cohort["occupied_cells"].values()) == cohort["present"]
    assert len(rows) == 121, len(rows)
    assert abs(rows[-1]["elapsed_seconds"] - 3600) < 1e-6
    assert cost["polls"] == 72000, cost
    # The preserved baseline binary clipped its final float endpoint just
    # below the mover accumulator's deadline. Its final position census is
    # useful, but that one zero-slice observation cannot measure motion. The
    # final driver removes the clipping. Retain raw evidence; exclude only
    # this documented endpoint, never ordinary stationary samples.
    invalid_end = rows[-1]["generated"]["stationary"] == population and rows[-1]["generated"]["positive_speed"] > 0
    for index, row in enumerate(rows):
        row["motion_sample_valid"] = not (invalid_end and index == len(rows) - 1)
    measured = rows[1:-1] if invalid_end else rows[1:]
    daylight = [row for row in measured if row["elapsed_seconds"] >= 300 and row["office"] in ["Dayspring", "High Wick", "the Waning"]]
    result = {
        "population": population, "config": config, "cost": cost,
        "all_day": summary(measured),
        "excluded_motion_samples": [{"elapsed_seconds": rows[-1]["elapsed_seconds"], "reason": "Final floating-point clamp omitted the movement slice; raw observation retained."}] if invalid_end else [],
        "settled_clear_daytime": summary(daylight),
        "settled_definition": "elapsed >= 300 s; offices Dayspring, High Wick and Waning; full InCity generated denominator",
        "by_office": {office: summary([row for row in measured if row["office"] == office]) for office in sorted({row["office"] for row in measured})},
        "initial_occupied_cells": len(rows[0]["generated"]["occupied_cells"]),
        "final_occupied_cells": len(rows[-1]["generated"]["occupied_cells"]),
        "final_away_from_initial_15m": rows[-1]["generated"]["away_from_initial_15m"],
        "final_sampled_away_seconds_per_actor": rows[-1]["generated"]["sampled_away_seconds"] / population,
        "longest_sampled_away_episode_seconds": rows[-1]["generated"]["longest_sampled_away_episode_seconds"],
    }
    directory = Path(__file__).parent
    (directory / f"baseline_{population}.jsonl").write_text("".join(json.dumps(row, separators=(",", ":")) + "\n" for row in rows))
    (directory / f"baseline_{population}_summary.json").write_text(json.dumps(result, indent=2) + "\n")
    print(population, json.dumps(result["settled_clear_daytime"], indent=2), cost)
