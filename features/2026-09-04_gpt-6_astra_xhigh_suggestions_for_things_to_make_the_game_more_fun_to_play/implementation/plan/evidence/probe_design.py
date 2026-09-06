# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Expose finite arithmetic/recipe limitations of the original illustrative GDD model."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
MODEL_PATH = HERE.parents[2] / "quest_model.json"


def old_recipe_matches(model, finding, tokens):
    return any(set(alternative) <= tokens for alternative in model["recipes"][finding])


def main():
    model = json.loads(MODEL_PATH.read_text())
    notes = []

    def check(name, observed, expected, meaning):
        if isinstance(expected, float):
            assert abs(observed - expected) < 1e-8, (name, observed, expected)
        else:
            assert observed == expected, (name, observed, expected)
        notes.append(dict(id=name, observed=observed, expected=expected, meaning=meaning))

    private_distance = sum(edge["metres"] for edge in model["edges"] if edge["access"] == "private")
    public_return = 2 * sum(edge["metres"] for edge in model["edges"] if edge["access"] == "public")
    acts = model["incident"]["encounter_seconds"] + model["incident"]["concealment_seconds"]
    original_private = private_distance / model["speeds"]["npc_walk_mps"] + acts
    complete_reference = original_private + model["incident"]["departure_seconds"]
    gap = model["sighting_gap_seconds"]
    margin = 2.0

    check("A01", private_distance, 48, "Illustrative private edge total; no route was surveyed.")
    check("A02", original_private, 30.857142857142858, "Original arithmetic omits the authored departure delay.")
    check("A03", complete_reference, 33.85714285714286, "Including three departure seconds, before new door/stair/turn costs.")
    check("A04", complete_reference <= gap["minimum"], False, "The complete reference does not fit the original 32-second conservative minimum.")
    check("A05", complete_reference <= gap["reference"], True, "It does fit nominal 36 seconds; nominal and conservative are different claims.")
    check("A06", complete_reference + margin <= gap["minimum"], False, "The plan's proposed two-second separation margin is also unmet.")
    check("A07", public_return, 576, "This is the old two-edge public out-and-back model, not a controller-space certificate.")
    check("A08", public_return / model["speeds"]["player_sprint_ceiling_mps"], 48.0,
          "Distance/sprint arithmetic alone cannot prove all actual public alternatives exceed the gap.")

    physical = {"E01", "E02", "E09", "authentication_verified", "custody_admissible",
                "E10", "E11", "E12", "fragment_origin_admissible", "fracture_match_verified",
                "break_interval_verified"}
    check("E01", old_recipe_matches(model, "property_recovered", physical), True,
          "The original physical bundle supports the model's recovery recipe.")
    check("E02", old_recipe_matches(model, "assault_substantiated", physical), True,
          "The original physical bundle supports the model's assault recipe.")
    check("E03", old_recipe_matches(model, "theft_substantiated", physical), False,
          "The advertised no-confession physical path does not support the old theft recipe; CASE_CONTRACT adds the continuous removal link.")
    later_handling = {"E02", "E09", "authentication_verified", "custody_admissible",
                      "E13", "retrieval_identity_continuous", "E15"}
    check("E04", old_recipe_matches(model, "theft_substantiated", later_handling), True,
          "The old model accepts later handling plus generic E15; the plan instead requires an original-act link and a specifically incompatible proposition.")
    check("E05", old_recipe_matches(model, "assault_substantiated", later_handling), False,
          "Later handling alone supplies no assault support even in the original recipe.")

    clock = model["clock"]
    opening = clock["opening_day"] * 24 + clock["opening_hour"]
    retrieval = clock["retrieval_day"] * 24 + clock["retrieval_hour"]
    review = clock["review_day"] * 24 + clock["review_hour"]
    retrieval_minutes = (retrieval - opening) / 24 * clock["seconds_per_day"] / 60
    review_minutes = (review - opening) / 24 * clock["seconds_per_day"] / 60
    check("T01", retrieval_minutes, 27.5, "Only the original default opening and unchanged clock rate give this real-minute window.")
    check("T02", review_minutes, 72.5, "Normal play continues through the original review deadline.")
    check("T03", 75 <= review_minutes, False, "A 75-minute uninterrupted walkthrough cannot attend a 72.5-minute review that waited for it.")
    check("T04", 100 <= review_minutes, False, "The longer walkthrough needs later/limited review or a lodged autonomous submission.")

    report = {
        "scope": "Finite design-model observations, not implemented gameplay, complete proof-rule validation or surveyed geometry.",
        "source": str(MODEL_PATH.name),
        "source_sha256": hashlib.sha256(MODEL_PATH.read_bytes()).hexdigest(),
        "assertions_checked": len(notes),
        "result": "All expected design observations reproduced, including the documented original-model defects.",
        "checks": notes,
    }
    (HERE / "design_probe.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({key: report[key] for key in ("scope", "assertions_checked", "result")}, indent=2))


if __name__ == "__main__":
    main()
