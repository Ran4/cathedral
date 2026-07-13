"""Per-session archive of every LLM exchange, one .md + .json pair per turn."""

from __future__ import annotations

import json
import re
import sys
from collections.abc import Callable
from datetime import datetime
from pathlib import Path


def _safe_component(value: str) -> str:
    """Filename-safe actor ids/names; `_` is reserved as the field separator."""
    cleaned = re.sub(r"[^A-Za-z0-9-]+", "-", value).strip("-")
    return cleaned or "unknown"


class PromptLog:
    """Writes `<stamp>__<nn>__<actor id>__<actor name>_prompt.{md,json}` pairs
    into a session's ``prompts/`` directory.

    The .md has ``# Prompt`` / ``# Answer`` / ``# Meta`` sections for reading;
    the .json carries the same ``{prompt, answer, meta}`` for tooling. ``nn``
    disambiguates multiple exchanges within one wall-clock second. Without a
    directory (terminal prototype, tests, sidecar launched outside the game)
    the log is disabled, and write failures never propagate into the turn
    loop.
    """

    def __init__(
        self,
        directory: Path | str | None,
        *,
        model: str | None = None,
        now: Callable[[], datetime] = datetime.now,
    ) -> None:
        self._directory = Path(directory) if directory is not None else None
        self._model = model
        self._now = now
        self._last_stamp = ""
        self._next_index = 0

    @property
    def enabled(self) -> bool:
        return self._directory is not None

    def record(
        self,
        *,
        actor_id: str,
        actor_name: str,
        prompt: str,
        answer: str | None,
        duration_seconds: float,
        error: str | None = None,
    ) -> None:
        if self._directory is None:
            return
        moment = self._now()
        stamp = moment.strftime("%Y-%m-%d_%H_%M_%S")
        if stamp != self._last_stamp:
            self._last_stamp = stamp
            self._next_index = 0
        index = self._next_index
        self._next_index += 1

        base = (
            f"{stamp}__{index:02d}"
            f"__{_safe_component(actor_id)}__{_safe_component(actor_name)}_prompt"
        )
        meta: dict[str, object] = {
            "actor_id": actor_id,
            "actor_name": actor_name,
            "model": self._model,
            "duration_seconds": round(duration_seconds, 3),
            "timestamp": moment.isoformat(timespec="seconds"),
        }
        if error is not None:
            meta["error"] = error

        try:
            self._directory.mkdir(parents=True, exist_ok=True)
            (self._directory / f"{base}.json").write_text(
                json.dumps(
                    {"prompt": prompt, "answer": answer, "meta": meta},
                    ensure_ascii=False,
                    indent=2,
                )
                + "\n",
                encoding="utf-8",
            )
            (self._directory / f"{base}.md").write_text(
                _markdown(prompt, answer, meta), encoding="utf-8"
            )
        except OSError as write_error:
            print(
                f"[smart actors] prompt log write failed: {write_error}",
                file=sys.stderr,
            )


def _markdown(prompt: str, answer: str | None, meta: dict[str, object]) -> str:
    lines = ["# Prompt", "", prompt.rstrip("\n"), "", "# Answer", ""]
    lines.append(answer.rstrip("\n") if answer is not None else "*(no answer)*")
    lines.extend(["", "# Meta", ""])
    lines.extend(f"- {key}: {value}" for key, value in meta.items())
    lines.append("")
    return "\n".join(lines)
