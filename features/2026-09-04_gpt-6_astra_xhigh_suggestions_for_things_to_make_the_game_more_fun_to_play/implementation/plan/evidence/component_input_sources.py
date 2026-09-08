"""Shared source/input scope for component verification starting with M2a10.

This is the union of the earlier owner-freeze and measurement scopes, plus the
committed root defaults read by configuration tests. Historical records retain
their original scopes. Evidence and build outputs are outside this enumeration.
"""
from __future__ import annotations

import hashlib
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[5]
SOURCE_SCOPE = "component-inputs-v2"
OWNER_EXTENSIONS = {".rs", ".json", ".bin", ".ron", ".toml", ".j2"}
MEASUREMENT_PATHS = (
    "Cargo*", "config.ron", "src", "crates", "assets/world", "assets/prompts",
    "assets/sounds/catalog.toml", "lore/characters", "lore/core_lore/occupations.json",
)


def source_paths() -> list[str]:
    tracked = subprocess.check_output(
        ["/usr/bin/git", "ls-files", "-z", "--cached", "--others",
         "--exclude-standard", "--", *MEASUREMENT_PATHS], cwd=ROOT,
    )
    paths = {Path(p.decode()) for p in tracked.split(b"\0") if p}
    for folder in ("crates", "src", "assets", "lore"):
        paths.update(p.relative_to(ROOT) for p in (ROOT / folder).rglob("*")
                     if p.suffix in OWNER_EXTENSIONS)
    paths.update(Path(p) for p in ("Cargo.toml", "Cargo.lock", "build.rs", "default_config.ron"))
    result = []
    for relative in sorted(paths):
        path = ROOT / relative
        if not path.resolve().is_relative_to(ROOT):
            raise ValueError(f"source/input path escapes repository: {relative}")
        if path.is_file():
            result.append(relative.as_posix())
    return result


def sources() -> dict[str, str]:
    return {p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in source_paths()}
