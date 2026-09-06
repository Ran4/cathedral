# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Validate the written design model, not the game's implementation or geometry."""
from __future__ import annotations
import hashlib
import heapq
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent


def findings(model, evidence):
    """Only submitted evidence enters this evaluator; no culprit identity input."""
    have = set(evidence)
    return {name: any(set(recipe) <= have for recipe in recipes)
            for name, recipes in model["recipes"].items()}


def shortest(model, start, goal, allowed):
    queue = [(0.0, start)]
    best = {start: 0.0}
    while queue:
        cost, node = heapq.heappop(queue)
        if node == goal:
            return cost
        if cost > best[node]:
            continue
        for edge in model["edges"]:
            if edge["access"] not in allowed:
                continue
            nxt = edge["to"] if edge["from"] == node else edge["from"] if edge["to"] == node else None
            if nxt is not None and cost + edge["metres"] < best.get(nxt, float("inf")):
                best[nxt] = cost + edge["metres"]
                heapq.heappush(queue, (best[nxt], nxt))
    return float("inf")


def validate():
    path = HERE / "quest_model.json"
    model = json.loads(path.read_text())
    checks = []
    def check(name, passed, detail):
        checks.append({"check": name, "passed": bool(passed), "detail": detail})

    public_distance = 2 * shortest(model, "A", "C", {"public"})
    public_min = public_distance / model["speeds"]["player_sprint_ceiling_mps"]
    route = model["incident"]["path"]
    distance = 0
    for a, b in zip(route, route[1:]):
        matches = [e for e in model["edges"] if {e["from"], e["to"]} == {a, b}]
        assert len(matches) == 1, (a, b)
        distance += matches[0]["metres"]
    inc = model["incident"]
    speed = model["speeds"]["npc_walk_mps"]
    private_time = distance / speed + inc["encounter_seconds"] + inc["concealment_seconds"]
    arrival = inc["departure_seconds"] + shortest(model, "A", "C", {"private"}) / speed
    exit_room = arrival + inc["encounter_seconds"]
    returned = inc["departure_seconds"] + private_time
    check("public lower bound", public_min > model["sighting_gap_seconds"]["maximum"],
          {"distance_m": public_distance, "seconds_at_sprint_ceiling": public_min, "largest_gap": model["sighting_gap_seconds"]["maximum"]})
    check("private route robustly fits", private_time < model["sighting_gap_seconds"]["minimum"],
          {"distance_m": distance, "execution_seconds": private_time, "smallest_gap": model["sighting_gap_seconds"]["minimum"]})
    check("reference return before second sighting", returned < model["sighting_gap_seconds"]["reference"], {"return_at": returned})
    check("cry occurs within encounter", arrival <= inc["cry_seconds"] <= exit_room, {"arrival": arrival, "departure": exit_room, "cry": inc["cry_seconds"]})
    check("break dated across encounter", inc["intact_catch_observed_seconds"] < arrival < exit_room < inc["damaged_catch_observed_seconds"], "Intact-before and damaged-after sightings enclose the physical encounter.")
    check("finder arrives after culprit left", inc["warin_finds_victim_seconds"] > returned, {"finder_at": inc["warin_finds_victim_seconds"], "culprit_return_at": returned})

    clock = model["clock"]
    def minutes_to(day, hour):
        hours = (day - clock["opening_day"]) * 24 + hour - clock["opening_hour"]
        return hours * clock["seconds_per_day"] / 24 / 60
    timing = {"retrieval_minutes_after_opening": minutes_to(clock["retrieval_day"], clock["retrieval_hour"]),
              "review_minutes_after_opening": minutes_to(clock["review_day"], clock["review_hour"]),
              "incident_gap_in_game_minutes": model["sighting_gap_seconds"]["reference"] * 1440 / clock["seconds_per_day"]}
    check("appointment arithmetic", timing == {"retrieval_minutes_after_opening": 27.5, "review_minutes_after_opening": 72.5, "incident_gap_in_game_minutes": 14.4}, timing)

    opportunity = model["recipes"]["private_opportunity"][0]
    f = findings(model, opportunity)
    check("opportunity does not accuse", f["private_opportunity"] and not f["theft_substantiated"] and not f["assault_substantiated"], f)
    property_only = model["recipes"]["property_recovered"][0]
    f = findings(model, property_only)
    check("shared-location recovery does not accuse", f["property_recovered"] and not f["theft_substantiated"] and not f["assault_substantiated"], f)
    f = findings(model, model["recipes"]["warin_clear_by_witnesses"][0])
    check("Warin can be cleared independently", f["warin_clear_by_witnesses"] and not f["theft_substantiated"] and not f["assault_substantiated"], f)
    check("rumour and guessed culprit are insufficient", not any(findings(model, {"rumour", "accuse_Corin", "player_sure"}).values()), "No finding from unsupported accusation.")

    # Scenario expectations are stated independently of the recipes above.
    cases = [
        ("possession alone is not assault", {"E02", "E09", "authentication_verified", "custody_admissible", "E13", "retrieval_identity_continuous", "E15"}, {"theft_substantiated": True, "assault_substantiated": False}),
        ("undated matching fragment is insufficient", {"E01", "E10", "E11", "fragment_origin_admissible", "fracture_match_verified"}, {"assault_substantiated": False}),
        ("dated physical chain supports assault", {"E01", "E10", "E11", "E12", "fragment_origin_admissible", "fracture_match_verified", "break_interval_verified"}, {"assault_substantiated": True}),
        ("hearsay cannot become a second witness", {"E01", "E06", "E07", "shared_cry_verified", "rumour"}, {"warin_clear_by_witnesses": False}),
        ("unidentified bundle is not proven retrieval", {"E02", "E09", "authentication_verified", "custody_admissible", "E13", "E15"}, {"theft_substantiated": False}),
        ("bare admission is insufficient", {"E14_theft", "E14_assault", "admission_voluntary"}, {"theft_substantiated": False, "assault_substantiated": False}),
        ("corroborated private admission can bypass route", {"E02", "E09", "authentication_verified", "custody_admissible", "E14_theft", "admission_voluntary", "E01", "E14_assault", "independent_nonpublic_detail_verified"}, {"theft_substantiated": True, "assault_substantiated": True, "private_opportunity": False}),
        ("today's route does not establish historical access", {"E03", "E04", "E05", "E08"}, {"private_opportunity": False})
    ]
    for name, evidence, expected in cases:
        actual = findings(model, evidence)
        check(name, all(actual[key] == value for key, value in expected.items()),
              {"expected": expected, "actual": {key: actual[key] for key in expected}})

    report = {"scope": "Design arithmetic and illustrative proof recipes only; no Bevy navigation, live NPC run, perception or human enjoyment tested.",
              "model_sha256": hashlib.sha256(path.read_bytes()).hexdigest(), "passed": all(c["passed"] for c in checks), "check_count": len(checks), "checks": checks}
    (HERE / "quest_validation.json").write_text(json.dumps(report, indent=2) + "\n")
    failures = [c for c in checks if not c["passed"]]
    print(json.dumps({"design_checks": len(checks), "passed": report["passed"], "failed": failures}, indent=2))
    return report["passed"]


if __name__ == "__main__":
    raise SystemExit(0 if validate() else 1)
