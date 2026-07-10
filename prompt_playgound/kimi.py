#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["openai", "python-dotenv"]
# ///
"""Send a file to the configured LLM (see llm_client.py / .env) and print the reply."""

import sys
from pathlib import Path

import llm_client


def main() -> None:
    path = Path(sys.argv[1] if len(sys.argv) > 1 else "think.md")
    try:
        content = path.read_text()
    except OSError as e:
        print(f"error: cannot read {path}: {e}", file=sys.stderr)
        sys.exit(1)

    try:
        result = llm_client.complete(content)
    except Exception as e:
        print(f"error: API request failed: {e}", file=sys.stderr)
        sys.exit(1)

    print(result)


if __name__ == "__main__":
    main()
