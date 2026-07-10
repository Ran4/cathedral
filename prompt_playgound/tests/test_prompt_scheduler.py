from __future__ import annotations

import json
import threading
import unittest

from support import MODULE_DIR, wait_until  # noqa: F401

from main import build_world
from prompt import parse_reply, render_prompt, render_prompt_and_drain
from scheduler import NpcScheduler
from sim import CharIdStr, Vec3, apply_action


def sheet(prompt: str) -> dict:
    block = prompt.split("```json\n", 1)[1].split("\n```", 1)[0]
    return json.loads(block)


class PromptTests(unittest.TestCase):
    def setUp(self) -> None:
        self.world = build_world()
        self.sven = self.world.characters[CharIdStr("sv3n1")]

    def test_metric_people_have_distance_and_perspective_name(self) -> None:
        rendered = sheet(render_prompt(self.world, self.sven))
        people = rendered["you_see"]["people"]
        self.assertEqual([person["id"] for person in people], ["cb947", "k0fb1"])
        self.assertEqual(people[0]["name"], "Conny")
        self.assertIn("unknown", people[1]["name"])
        self.assertEqual(people[0]["distance_m"], 2.7)
        self.assertEqual(
            rendered["you_are"]["position_m"], {"x": -1.8, "y": 0.91, "z": 114.0}
        )
        self.assertIn("forecourt", rendered["you_are"]["location_description"])

    def test_player_appears_only_when_near_and_never_gets_a_prompt(self) -> None:
        player = self.world.characters[CharIdStr("player")]
        self.assertNotIn(
            "player",
            [
                p["id"]
                for p in sheet(render_prompt(self.world, self.sven))["you_see"][
                    "people"
                ]
            ],
        )
        player.position_m = Vec3(-2, 0.91, 113)
        self.assertIn(
            "player",
            [
                p["id"]
                for p in sheet(render_prompt(self.world, self.sven))["you_see"][
                    "people"
                ]
            ],
        )
        with self.assertRaises(ValueError):
            render_prompt(self.world, player)

    def test_offered_to_you_is_actionable_only_within_four_metres(self) -> None:
        ilse = self.world.characters[CharIdStr("k0fb1")]
        apply_action(
            self.world,
            ilse,
            "offer_item",
            {"item_id": "c0prs", "target": "sv3n1"},
        )
        self.assertIn("offered_to_you", sheet(render_prompt(self.world, self.sven)))
        self.sven.position_m = Vec3(-20, 0.91, 114)
        self.assertNotIn("offered_to_you", sheet(render_prompt(self.world, self.sven)))
        self.assertIn("you_offer", sheet(render_prompt(self.world, ilse)))

    def test_render_and_drain_moves_old_events_to_prompt(self) -> None:
        self.sven.inbox[:] = ["first", "second"]
        rendered = sheet(render_prompt_and_drain(self.world, self.sven))
        self.assertEqual(rendered["since_your_last_turn"], ["first", "second"])
        self.assertEqual(self.sven.inbox, [])

    def test_failed_prompt_render_restores_drained_events(self) -> None:
        self.sven.inbox[:] = ["must survive"]
        self.sven.control = "player"
        with self.assertRaises(ValueError):
            render_prompt_and_drain(self.world, self.sven)
        self.assertEqual(self.sven.inbox, ["must survive"])

    def test_parser_rejects_non_object_and_trailing_garbage(self) -> None:
        actions, errors = parse_reply(
            'say [1]\nsay {"text":"ok"} garbage\nsay {"text":"# safe"} # comment'
        )
        self.assertEqual(actions, [("say", {"text": "# safe"})])
        self.assertEqual(len(errors), 2)
        self.assertEqual(parse_reply(None), ([], ["reply must be text"]))

    def test_pathologically_nested_reply_becomes_a_parse_error(self) -> None:
        nested = 'remember {"memory":' + "[" * 2_000 + '"x"' + "]" * 2_000 + "}"
        actions, errors = parse_reply(nested)
        self.assertEqual(actions, [])
        self.assertTrue(errors)


class SchedulerTests(unittest.TestCase):
    def test_event_arriving_during_completion_remains_for_next_turn(self) -> None:
        world = build_world()
        player = world.characters[CharIdStr("player")]
        sven = world.characters[CharIdStr("sv3n1")]
        player.position_m = Vec3(-1.8, 0.91, 113)
        sven.inbox[:] = ["old event"]
        started = threading.Event()
        release = threading.Event()

        def complete(prompt: str) -> str:
            self.assertEqual(sheet(prompt)["since_your_last_turn"], ["old event"])
            started.set()
            release.wait(1)
            return 'set_goal {"goal": null}'

        scheduler = NpcScheduler(world, complete, minimum_delay_seconds=100)
        self.addCleanup(scheduler.close)
        scheduler.start()
        scheduler.poll()
        self.assertTrue(started.wait(1))
        apply_action(world, player, "say", {"target": "sv3n1", "text": "new event"})
        release.set()
        wait_until(lambda: scheduler.in_flight_actor_id is None, scheduler.poll)
        self.assertEqual(len(sven.inbox), 1)
        self.assertIn("new event", sven.inbox[0])

    def test_player_is_skipped_and_round_robin_is_global(self) -> None:
        world = build_world()
        calls: list[str] = []

        def complete(prompt: str) -> str:
            calls.append(sheet(prompt)["name"])
            return 'set_goal {"goal": null}'

        scheduler = NpcScheduler(world, complete, minimum_delay_seconds=0)
        self.addCleanup(scheduler.close)
        scheduler.start()
        wait_until(lambda: len(calls) >= 4, scheduler.poll)
        self.assertEqual(calls[:4], ["Sven", "Conny", "Ilse", "Sven"])
        self.assertNotIn("Player", calls)

    def test_priority_runs_after_current_then_round_robin_resumes(self) -> None:
        world = build_world()
        calls: list[str] = []
        first_started = threading.Event()
        release = threading.Event()

        def complete(prompt: str) -> str:
            name = sheet(prompt)["name"]
            calls.append(name)
            if len(calls) == 1:
                first_started.set()
                release.wait(1)
            return 'set_goal {"goal": null}'

        scheduler = NpcScheduler(world, complete, minimum_delay_seconds=0)
        self.addCleanup(scheduler.close)
        scheduler.start()
        scheduler.poll()
        self.assertTrue(first_started.wait(1))
        self.assertTrue(scheduler.prioritize("k0fb1"))
        self.assertFalse(scheduler.prioritize("player"))
        release.set()
        wait_until(lambda: len(calls) >= 3, scheduler.poll)
        self.assertEqual(calls[:3], ["Sven", "Ilse", "Conny"])

    def test_malformed_actions_become_system_events_without_crash(self) -> None:
        world = build_world()

        def complete(prompt: str) -> str:
            return "\n".join(
                [
                    'say {"text": 9}',
                    'offer_item {"item_id": ["bad"]}',
                    "unknown_verb {}",
                    "not even an action",
                ]
            )

        scheduler = NpcScheduler(world, complete, minimum_delay_seconds=100)
        self.addCleanup(scheduler.close)
        scheduler.start()
        scheduler.poll()
        wait_until(lambda: scheduler.in_flight_actor_id is None, scheduler.poll)
        sven = world.characters[CharIdStr("sv3n1")]
        self.assertGreaterEqual(len(sven.inbox), 4)
        self.assertTrue(all(event.startswith("system:") for event in sven.inbox))

    def test_provider_failure_uses_backoff_and_preserves_service(self) -> None:
        world = build_world()
        sven = world.characters[CharIdStr("sv3n1")]
        sven.inbox.append("player question that must survive a failed call")
        now = [0.0]

        def clock() -> float:
            return now[0]

        def complete(prompt: str) -> str:
            raise TimeoutError("offline fake timeout")

        scheduler = NpcScheduler(
            world,
            complete,
            minimum_delay_seconds=1,
            maximum_backoff_seconds=8,
            clock=clock,
        )
        self.addCleanup(scheduler.close)
        scheduler.start()
        scheduler.poll()
        wait_until(
            lambda: scheduler.in_flight_actor_id is None,
            lambda: scheduler.poll(now[0]),
        )
        self.assertEqual(
            sven.inbox[0], "player question that must survive a failed call"
        )
        self.assertIn("provider failed", sven.inbox[-1])
        self.assertIsNone(scheduler.in_flight_actor_id)
        now[0] = 1.0
        scheduler.poll()
        self.assertIsNotNone(scheduler.in_flight_actor_id)


if __name__ == "__main__":
    unittest.main()
