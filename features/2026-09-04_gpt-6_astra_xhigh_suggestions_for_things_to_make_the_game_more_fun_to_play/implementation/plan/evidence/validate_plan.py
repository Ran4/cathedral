# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Validate roadmap structure/links/provenance, never certify game acceptance."""

from __future__ import annotations

import argparse
from collections import Counter
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import subprocess
from urllib.parse import unquote, urlsplit

import plan_catalog

HERE = Path(__file__).resolve().parent
PLAN = HERE.parent
ROOT = HERE.parents[4]
FEATURE = PLAN.parent.parent


def without_fences(value):
    return re.sub(r"(?ms)^```[^\n]*\n.*?^```[ \t]*$", "", value)


def anchors(path):
    body = without_fences(path.read_text())
    found = set(re.findall(r'<a\s+id=[\"\']([^\"\']+)[\"\']', body))
    counts = Counter()
    for text in re.findall(r"(?m)^#{1,6}\s+(.+?)\s*#*\s*$", body):
        text = re.sub(r"!?\[([^\]]+)\]\([^)]*\)", r"\1", text)
        text = re.sub(r"<[^>]*>", "", text).lower().replace("`", "")
        slug = "".join(c for c in text if c.isalnum() or c in " _-").replace(" ", "-")
        count = counts[slug]
        counts[slug] += 1
        found.add(f"{slug}-{count}" if count else slug)
    return found


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true", help="write structural_validation.json")
    args = parser.parse_args()
    milestones = plan_catalog.milestone_files()
    milestone_statuses = {}
    for number, filename in sorted(milestones.items()):
        first = (PLAN / filename).read_text().splitlines()[0]
        assert re.match(
            r"^Status: (?:Planned|In progress|Partial|Accepted|Implemented|Blocked)\b", first
        ), f"Missing or unrecognized milestone status in M{number}: {first}"
        assert re.search(r"\b\d{4}-\d{2}-\d{2}\b", first), \
            f"Milestone status needs an absolute date in M{number}: {first}"
        # Status is an author's evidence-backed record, not a conclusion this
        # structural checker can reach. Keep the exact claim visible for review.
        milestone_statuses[f"M{number}"] = first.removeprefix("Status: ")

    for path, expected in plan_catalog.outputs().items():
        assert path.read_text() == expected, f"Generated catalog drift: {path.name}"

    markdown = sorted(PLAN.glob("*.md")) + [FEATURE / "README.md"]
    checked_links = 0
    external_links = 0
    failures = []
    anchor_cache = {}
    for source in markdown:
        body = without_fences(source.read_text())
        for raw in re.findall(r"!?\[[^\]]*\]\(([^)]+)\)", body):
            target = raw.strip()
            if target.startswith("<") and ">" in target:
                target = target[1:target.index(">")]
            else:
                target = target.split(' "', 1)[0]
            parsed = urlsplit(target)
            if parsed.scheme or parsed.netloc:
                external_links += 1
                continue
            destination = (source.parent / unquote(parsed.path)).resolve() if parsed.path else source.resolve()
            # Never follow links beyond this repository or into a denied tree.
            if not destination.is_relative_to(ROOT):
                failures.append(f"{source.name}: out-of-repository target {target}")
                continue
            if not destination.exists():
                failures.append(f"{source.name}: missing {target}")
                continue
            if parsed.fragment and destination.suffix == ".md":
                if destination not in anchor_cache:
                    anchor_cache[destination] = anchors(destination)
                if unquote(parsed.fragment) not in anchor_cache[destination]:
                    failures.append(f"{source.name}: missing anchor {target}")
                    continue
            checked_links += 1
    assert not failures, "\n".join(failures)

    site = json.loads((HERE / "site_audit.json").read_text())
    for relative, expected in site["sources"].items():
        source = (ROOT / relative).resolve()
        assert source.is_relative_to(ROOT)
        actual = hashlib.sha256(source.read_bytes()).hexdigest()
        assert actual == expected, f"Site source changed: {relative}; regenerate illustration"
    assert (HERE / "tallage_existing.png").stat().st_size > 0
    assert (HERE / "tallage_existing.svg").stat().st_size > 0

    probe = json.loads((HERE / "design_probe.json").read_text())
    assert probe["source"] == "quest_model.json"
    assert hashlib.sha256((FEATURE / probe["source"]).read_bytes()).hexdigest() == probe["source_sha256"], \
        "Original model changed: rerun and review the finite design probe"
    assert probe["assertions_checked"] == len(probe["checks"])
    assert len({check["id"] for check in probe["checks"]}) == len(probe["checks"])

    head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT,
                          check=True, capture_output=True, text=True).stdout.strip()
    report = {
        "checked_at_utc": datetime.now(timezone.utc).isoformat(),
        "head_observed": head,
        "scope": "Local roadmap structure, generated traceability and current site-source hashes only.",
        "game_acceptance": "Not run or certified by this tool; consult milestone evidence for acceptance results.",
        "milestones": len(milestones),
        "milestone_statuses": milestone_statuses,
        "requirements": len(plan_catalog.REQUIREMENTS),
        "planned_scenarios": len(plan_catalog.SCENARIOS),
        "markdown_files_checked": len(markdown),
        "local_links_and_anchors_checked": checked_links,
        "external_links_not_fetched": external_links,
        "site_source_hashes_checked": len(site["sources"]),
        "original_model_hashes_checked": 1,
        "design_probe_assertions_recorded": probe["assertions_checked"],
        "plan_markdown_words": sum(len(p.read_text().split()) for p in PLAN.glob("*.md")),
        "result": "passed",
    }
    if args.write:
        (HERE / "structural_validation.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
