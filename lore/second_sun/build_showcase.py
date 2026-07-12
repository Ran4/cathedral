#!/usr/bin/env python3
"""Build the Second Sun showcase.

Reads every .md file in this directory (and in documents/ and design/), injects
them into showcase_template.html as a JSON blob, and writes index.html.

The showcase must work when opened straight off disk via file://, where fetch()
is blocked by the browser's origin rules — so the corpus is baked into the page
rather than loaded at runtime. That is the whole reason this script exists.

Rebuild after editing any lore file:

    python3 build_showcase.py

No dependencies beyond the standard library.
"""

from __future__ import annotations

import html
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
TEMPLATE = ROOT / "showcase_template.html"
OUTPUT = ROOT / "index.html"

# "The Second Sun" is entry #1 in the feature list; the showcase links back to it.
FEATURE_SRC = "../../features/50_cool_suggestions.md"

GROUPS = (
    ("Canon", ["00_canon.md"]),
    ("The World", None),          # 01..12 in the corpus root
    ("Documents (diegetic)", None),
    ("Design Specs", None),
)


def doc_keys() -> list[str]:
    """Corpus files in reading order: canon, the world, documents, design."""
    root_md = sorted(p.name for p in ROOT.glob("*.md") if p.name != "README.md")
    docs = sorted(f"documents/{p.name}" for p in (ROOT / "documents").glob("*.md"))
    design = sorted(f"design/{p.name}" for p in (ROOT / "design").glob("*.md"))
    return root_md + docs + design


def title_of(text: str, key: str) -> str:
    for line in text.splitlines():
        if line.startswith("# "):
            return line[2:].strip()
    return key


def use_of(text: str) -> str:
    m = re.search(r"^>\s*Use:\s*(.+)$", text, re.MULTILINE)
    return m.group(1).strip() if m else ""


def ledger_counts(canon: str) -> dict[str, int]:
    """Tally the truth-value ledger in canon §10: T / F / A."""
    counts = {"T": 0, "F": 0, "A": 0}
    for line in canon.splitlines():
        if not line.startswith("|"):
            continue
        cols = [c.strip() for c in line.strip().strip("|").split("|")]
        # | # | Statement | Value | Notes |
        if len(cols) >= 4 and cols[0].isdigit() and cols[2] in counts:
            counts[cols[2]] += 1
    return counts


def js_json(obj) -> str:
    """JSON safe to embed inside a <script> element."""
    s = json.dumps(obj, ensure_ascii=False, separators=(",", ":"))
    return s.replace("<", "\\u003c").replace(">", "\\u003e").replace("&", "\\u0026")


def main() -> int:
    if not TEMPLATE.exists():
        sys.exit(f"missing template: {TEMPLATE}")

    keys = doc_keys()
    docs: dict[str, dict] = {}
    for key in keys:
        text = (ROOT / key).read_text(encoding="utf-8")
        docs[key] = {
            "title": title_of(text, key),
            "use": use_of(text),
            "words": len(text.split()),
            "text": text,
        }

    # Cross-references: bare filenames in the prose become in-page links.
    # Longest key wins (see documents/trial_records.md before trial_records.md).
    xref: dict[str, dict] = {}
    for key in keys:
        xref[key] = {"key": key}
        base = key.split("/")[-1]
        xref.setdefault(base, {"key": key})
    xref["50_cool_suggestions.md"] = {"ext": FEATURE_SRC}

    stats = {
        "docs": len(keys),
        "words": sum(d["words"] for d in docs.values()),
        "ledger": ledger_counts(docs["00_canon.md"]["text"]),
    }

    noscript = ["<ul>"]
    for key in keys:
        d = docs[key]
        noscript.append(
            f'<li><a href="{key}">{html.escape(d["title"])}</a> '
            f'&mdash; <code>{key}</code>, {d["words"]:,} words</li>'
        )
    noscript.append("</ul>")

    page = TEMPLATE.read_text(encoding="utf-8")
    for placeholder, value in (
        ("/*__DOCS_JSON__*/", js_json(docs)),
        ("/*__STATS_JSON__*/", js_json(stats)),
        ("/*__XREF_JSON__*/", js_json(xref)),
        ("<!--__NOSCRIPT_LINKS__-->", "\n".join(noscript)),
    ):
        if placeholder not in page:
            sys.exit(f"placeholder missing from template: {placeholder}")
        page = page.replace(placeholder, value, 1)

    OUTPUT.write_text(page, encoding="utf-8")

    # --- verify -----------------------------------------------------------
    problems = []
    if "/*__" in page or "<!--__" in page:
        problems.append("an unsubstituted placeholder remains")
    if not page.rstrip().endswith("</html>"):
        problems.append("output does not end with </html>")
    for key in keys:
        if f'"{key}"' not in page:
            problems.append(f"doc key missing from output: {key}")
    if FEATURE_SRC not in page:
        problems.append("feature back-link missing")
    if problems:
        for p in problems:
            print(f"  FAIL  {p}", file=sys.stderr)
        return 1

    ledger = stats["ledger"]
    print(f"wrote {OUTPUT.relative_to(ROOT.parent.parent)}")
    print(f"  documents  {stats['docs']}")
    print(f"  words      {stats['words']:,}")
    print(f"  ledger     {ledger['T']} true · {ledger['F']} false-but-believed "
          f"· {ledger['A']} ambiguous")
    print(f"  size       {OUTPUT.stat().st_size / 1024:.0f} KB")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
