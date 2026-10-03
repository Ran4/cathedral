#!/usr/bin/env python3
"""Plain dev runner for the CPU-capped worker (scripts/capped_verification).

The worker only accepts runners at evidence/mN_<name>/owner/run_capped.py, so
this lives here. It runs jobs/<run-name>.sh with bash from the repository root;
the worker captures its output in logs/capped_verification/claimed/<run-name>/.
"""
import argparse
import subprocess
import sys
from pathlib import Path

here = Path(__file__).resolve().parent
parser = argparse.ArgumentParser()
parser.add_argument("--run-name", required=True)
args = parser.parse_args()
script = here / "jobs" / f"{args.run_name}.sh"
root = next(p for p in here.parents if (p / ".git").exists())
sys.exit(subprocess.call(["bash", "-eo", "pipefail", str(script)], cwd=root,
                         stdin=subprocess.DEVNULL))
