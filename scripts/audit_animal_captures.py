#!/usr/bin/env python3
"""Verify completed animal captures against their frozen or working sources."""
import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--snapshot", help="Compare animal sources to this frozen snapshot")
    parser.add_argument("captures", nargs="+", help="Names beneath captures/animals")
    args = parser.parse_args()
    snapshot = ROOT / "gauntlet/snapshots" / args.snapshot if args.snapshot else None
    if snapshot and not snapshot.is_dir():
        parser.error(f"Missing snapshot: {snapshot}")
    report = []
    for name in args.captures:
        directory = ROOT / "captures/animals" / name
        manifest = json.loads((directory / "capture_manifest.json").read_text())
        if manifest.get("completed") is not True:
            raise ValueError(f"Incomplete capture: {name}")
        checked = []
        roles = set()
        for source, expected in manifest["build"]["sources_sha256"].items():
            relative = Path(source)
            if relative.name == "dogs.rs":
                role, frozen = "dog", Path("dogs.rs")
            elif relative.parent.name == "dogs":
                role, frozen = "dog", Path("dogs") / relative.name
            elif relative.name == "vermin.rs":
                role, frozen = "rat", Path("vermin.rs")
            elif relative.parent.name == "vermin":
                role, frozen = "rat", Path("vermin") / relative.name
            else:
                continue
            path = snapshot / frozen if snapshot and (snapshot / frozen).exists() else ROOT / relative
            actual = hashlib.sha256(path.read_bytes()).hexdigest()
            if actual != expected:
                raise ValueError(f"{name}: source mismatch {source} against {path}")
            roles.add(role)
            checked.append(source)
        needed = {"dog", "rat"} if manifest.get("benchmark") else {manifest["species"]}
        if not needed <= roles:
            raise ValueError(f"{name}: missing animal source hashes for {needed - roles}")
        report.append({"capture": name, "completed": True,
                       "snapshot": args.snapshot or "recorded source paths",
                       "matched_sources": checked})
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
