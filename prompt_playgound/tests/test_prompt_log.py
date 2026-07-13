from __future__ import annotations

import json
import tempfile
import unittest
from datetime import datetime
from pathlib import Path

from support import MODULE_DIR, wait_until  # noqa: F401

from main import build_world
from prompt_log import PromptLog
from scheduler import NpcScheduler


class PromptLogTests(unittest.TestCase):
    def setUp(self) -> None:
        self.directory = Path(tempfile.mkdtemp(prefix="cathedral-prompt-log-"))
        self.moment = datetime(2026, 7, 13, 9, 52, 30)
        self.log = PromptLog(
            self.directory, model="kimi-k2.5", now=lambda: self.moment
        )

    def record(self, **overrides) -> None:
        arguments = dict(
            actor_id="k0fb1",
            actor_name="Ilse",
            prompt="the prompt",
            answer="wait {}",
            duration_seconds=1.234567,
        )
        arguments.update(overrides)
        self.log.record(**arguments)

    def test_md_and_json_pair_uses_the_schema_name(self) -> None:
        self.record()

        base = "2026-07-13_09_52_30__00__k0fb1__Ilse_prompt"
        self.assertEqual(
            sorted(path.name for path in self.directory.iterdir()),
            [f"{base}.json", f"{base}.md"],
        )

        markdown = (self.directory / f"{base}.md").read_text(encoding="utf-8")
        self.assertIn("# Prompt\n\nthe prompt\n", markdown)
        self.assertIn("# Answer\n\nwait {}\n", markdown)
        self.assertIn("# Meta\n\n- actor_id: k0fb1", markdown)
        self.assertIn("- model: kimi-k2.5", markdown)
        self.assertIn("- duration_seconds: 1.235", markdown)

        data = json.loads((self.directory / f"{base}.json").read_text("utf-8"))
        self.assertEqual(data["prompt"], "the prompt")
        self.assertEqual(data["answer"], "wait {}")
        self.assertEqual(data["meta"]["actor_name"], "Ilse")
        self.assertEqual(data["meta"]["duration_seconds"], 1.235)
        self.assertNotIn("error", data["meta"])

    def test_same_second_exchanges_get_increasing_suffixes(self) -> None:
        self.record()
        self.record()
        self.moment = datetime(2026, 7, 13, 9, 52, 31)
        self.record()

        names = sorted(path.name for path in self.directory.glob("*.md"))
        self.assertEqual(
            names,
            [
                "2026-07-13_09_52_30__00__k0fb1__Ilse_prompt.md",
                "2026-07-13_09_52_30__01__k0fb1__Ilse_prompt.md",
                "2026-07-13_09_52_31__00__k0fb1__Ilse_prompt.md",
            ],
        )

    def test_failed_exchange_keeps_the_prompt_and_records_the_error(self) -> None:
        self.record(answer=None, error="TimeoutError('provider')")

        markdown = next(self.directory.glob("*.md")).read_text(encoding="utf-8")
        self.assertIn("# Answer\n\n*(no answer)*\n", markdown)
        self.assertIn("- error: TimeoutError('provider')", markdown)
        data = json.loads(next(self.directory.glob("*.json")).read_text("utf-8"))
        self.assertIsNone(data["answer"])
        self.assertEqual(data["meta"]["error"], "TimeoutError('provider')")

    def test_hostile_name_components_are_sanitized(self) -> None:
        self.record(actor_id="../evil", actor_name="Olof Skötkonung")

        name = next(self.directory.glob("*.md")).name
        self.assertEqual(
            name, "2026-07-13_09_52_30__00__evil__Olof-Sk-tkonung_prompt.md"
        )

    def test_without_a_directory_the_log_is_disabled(self) -> None:
        log = PromptLog(None)
        self.assertFalse(log.enabled)
        log.record(
            actor_id="k0fb1",
            actor_name="Ilse",
            prompt="p",
            answer="a",
            duration_seconds=0.0,
        )


class SchedulerPromptLogTests(unittest.TestCase):
    def test_scheduler_reports_each_exchange_to_the_prompt_log(self) -> None:
        world = build_world()
        recorded: list[dict] = []

        scheduler = NpcScheduler(
            world,
            lambda prompt: "wait {}",
            minimum_delay_seconds=100,
            prompt_log=lambda **exchange: recorded.append(exchange),
        )
        self.addCleanup(scheduler.close)
        scheduler.start()
        scheduler.poll()
        wait_until(lambda: scheduler.in_flight_actor_id is None, scheduler.poll)

        self.assertEqual(len(recorded), 1)
        exchange = recorded[0]
        self.assertEqual(exchange["actor_name"], "Sven")
        self.assertIn("since_your_last_turn", exchange["prompt"])
        self.assertEqual(exchange["answer"], "wait {}")
        self.assertIsNone(exchange["error"])
        self.assertGreaterEqual(exchange["duration_seconds"], 0.0)


if __name__ == "__main__":
    unittest.main()
