#!/usr/bin/env -S uv run --script
"""Fire an explicitly selected evidence directory once, preserving raw replies.

uv run --script fire_probes.py --sheets DIR --out DIR --provider openai
Every invocation without an existing successful reply is billed. No automatic
retry: a failed call remains visible in RUN.json and may be retried explicitly.
"""

import argparse
import concurrent.futures
import hashlib
import itertools
import json
import os
from pathlib import Path
import re
import subprocess
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--sheets", type=Path, required=True)
parser.add_argument("--out", type=Path, required=True)
parser.add_argument("--provider", choices=("openai", "moonshot"), required=True)
args = parser.parse_args()
root = Path(subprocess.check_output(["git", "rev-parse", "--show-toplevel"], text=True).strip())
binary = root / "target/debug/cathedral-headless"
args.out.mkdir(parents=True, exist_ok=True)
sheets = sorted(args.sheets.glob("*.txt"))
assert sheets, "No sheets selected"
model = "kimi-k3" if args.provider == "moonshot" else "gpt-5.6-luna"
env = {**os.environ, "LLM_MODEL": model}


def fire(sheet):
    reply = args.out / sheet.name
    error = reply.with_suffix(".err")
    if reply.exists() and reply.stat().st_size:
        return {"sheet": sheet.name, "status": "skipped"}
    command = [str(binary), "--provider", args.provider, "--one-shot", str(sheet.resolve())]
    start = time.monotonic()
    try:
        result = subprocess.run(command, capture_output=True, env=env, timeout=120, cwd=root)
        reply.write_bytes(result.stdout)
        error.write_bytes(result.stderr)
        status = "ok" if result.returncode == 0 and result.stdout.strip() else "failed"
        code = result.returncode
    except subprocess.TimeoutExpired as exc:
        reply.write_bytes(b"")
        error.write_bytes((exc.stderr or b"") + b"\nProbe timed out after 120 seconds.\n")
        status, code = "timeout", None
    record = {
        "sheet": sheet.name, "status": status, "returncode": code,
        "seconds": round(time.monotonic() - start, 3), "command": command,
        "sheet_sha256": hashlib.sha256(sheet.read_bytes()).hexdigest(),
    }
    print(json.dumps(record), flush=True)
    return record


with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
    records = list(executor.map(fire, sheets))
run = args.out / "RUN.json"
if not run.exists() or any(row["status"] != "skipped" for row in records):
    prior = json.loads(run.read_text()) if run.exists() else None
    run.write_text(json.dumps({
        "provider": args.provider, "model": model, "calls": records,
        "git_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip(),
        "previous": prior,
    }, indent=2) + "\n")

# Exactly the lexical rule in NOTES.md; this corroborates editorial scoring,
# and never substitutes a lexical threshold for the mouth's own clause.
stop = set(("a an the and or but of to in on at for with as is it its i you he she they them "
            "him her was were be been am are that this these those his hers their my mine your "
            "yours not no so if then than there here now s t d ll ve m re do does did done have "
            "has had will would shall should can could may might must aye nay o oh").split())
spoken = {}
for path in sorted(args.out.glob("q4_wick_*.txt")):
    texts = []
    for line in path.read_text().splitlines():
        match = re.match(r"^say\s+(\{.*)", line.strip().strip("`"))
        if match:
            value, _ = json.JSONDecoder().raw_decode(match[1])
            texts.append(value["text"])
    spoken[path.stem] = " ".join(texts)
if len(spoken) == 8 and all(spoken.values()):
    bags = {key: {word for word in re.findall("[a-z']+", text.lower())
                  if len(word) > 1 and word not in stop} for key, text in spoken.items()}
    pairs = [{"a": a, "b": b, "jaccard": len(bags[a] & bags[b]) / len(bags[a] | bags[b])}
             for a, b in itertools.combinations(bags, 2)]
    metrics = {"spoken": spoken, "pairs": pairs,
               "mean": sum(pair["jaccard"] for pair in pairs) / 28,
               "max": max(pair["jaccard"] for pair in pairs),
               "pairs_ge_0_60": sum(pair["jaccard"] >= 0.60 for pair in pairs),
               "md5": {key: hashlib.md5(text.encode()).hexdigest() for key, text in spoken.items()}}
    (args.out / "METRICS.json").write_text(json.dumps(metrics, indent=2) + "\n")
    print(json.dumps({key: metrics[key] for key in ("mean", "max", "pairs_ge_0_60")}), flush=True)
if any(row["status"] not in ("ok", "skipped") for row in records):
    raise SystemExit("One or more probes failed; inspect RUN.json and .err files.")
