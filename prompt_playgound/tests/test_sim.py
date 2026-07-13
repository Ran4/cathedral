from __future__ import annotations

import unittest

from support import MODULE_DIR  # noqa: F401 - installs module path

from main import build_world
from sim import (
    HEARING_RADIUS_M,
    ITEM_INTERACTION_RADIUS_M,
    PLAYER_SPEECH_MAX_CHARS,
    RECENT_HISTORY_MAX_ENTRIES,
    ActionError,
    Character,
    CharIdStr,
    Item,
    ItemIdStr,
    Vec3,
    World,
    apply_action,
)


def character(actor_id: str, name: str, x: float, *, control: str = "llm") -> Character:
    return Character(
        id=CharIdStr(actor_id),
        name=name,
        control=control,  # type: ignore[arg-type]
        back_story="test",
        location_description="test square",
        position_m=Vec3(x, 0, 0),
        appearance_key=name.lower(),
        voice_key=None if control == "player" else name.lower(),
    )


def speech_world() -> tuple[World, Character, Character, Character, Character]:
    world = World()
    speaker = character("speaker", "Speaker", 0)
    target = character("target", "Target", 10)
    bystander = character("bystander", "Bystander", 5)
    distant = character("distant", "Distant", 20.0001)
    for actor in (speaker, target, bystander, distant):
        world.add(actor)
    return world, speaker, target, bystander, distant


class DistanceTests(unittest.TestCase):
    def test_hearing_boundary_is_inclusive(self) -> None:
        world = World()
        origin = character("origin", "Origin", 0)
        inside = character("inside", "Inside", HEARING_RADIUS_M - 1e-6)
        exact = character("exact", "Exact", HEARING_RADIUS_M)
        outside = character("outside", "Outside", HEARING_RADIUS_M + 1e-6)
        for actor in (origin, inside, exact, outside):
            world.add(actor)
        self.assertEqual(
            [
                actor.id
                for actor in world.characters_within(origin, HEARING_RADIUS_M, origin)
            ],
            [inside.id, exact.id],
        )

    def test_interaction_boundary_is_inclusive(self) -> None:
        for distance, succeeds in (
            (ITEM_INTERACTION_RADIUS_M - 1e-6, True),
            (ITEM_INTERACTION_RADIUS_M, True),
            (ITEM_INTERACTION_RADIUS_M + 1e-6, False),
        ):
            with self.subTest(distance=distance):
                world = World()
                giver = character("giver", "Giver", 0)
                target = character("target", "Target", distance)
                item = Item(ItemIdStr("item"), "apple")
                giver.holds.append(item.id)
                for entity in (item, giver, target):
                    world.add(entity)
                if succeeds:
                    apply_action(
                        world,
                        giver,
                        "offer_item",
                        {"item_id": "item", "target": "target"},
                    )
                else:
                    with self.assertRaisesRegex(ActionError, "more than 4 metres"):
                        apply_action(
                            world,
                            giver,
                            "offer_item",
                            {"item_id": "item", "target": "target"},
                        )

    def test_order_is_distance_then_id(self) -> None:
        world = World()
        origin = character("origin", "Origin", 0)
        for actor in (
            origin,
            character("z", "Z", 2),
            character("b", "B", 1),
            character("a", "A", -1),
        ):
            world.add(actor)
        self.assertEqual(
            [actor.id for actor in world.characters_within(origin, 20, origin.id)],
            [CharIdStr("a"), CharIdStr("b"), CharIdStr("z")],
        )

    def test_vec3_rejects_nonfinite_and_bool(self) -> None:
        for value in (float("nan"), float("inf"), float("-inf"), True, "1"):
            with self.subTest(value=value), self.assertRaises(ValueError):
                Vec3(value, 0, 0)  # type: ignore[arg-type]


class SpeechTests(unittest.TestCase):
    def test_targeted_speech_reaches_target_and_nearby_bystander(self) -> None:
        world, speaker, target, bystander, distant = speech_world()
        apply_action(world, speaker, "say", {"target": target.id, "text": "  hello  "})
        self.assertIn("said to you", target.inbox[-1])
        self.assertIn("said to a stranger (id target)", bystander.inbox[-1])
        self.assertEqual(distant.inbox, [])
        event = world.drain_events()[-1]
        self.assertEqual(event.text, "hello")
        self.assertEqual(event.recipient_ids, (bystander.id, target.id))

    def test_recent_history_keeps_received_and_own_speech(self) -> None:
        world, speaker, target, bystander, _ = speech_world()
        speaker.knows.add(target.id)

        apply_action(world, speaker, "say", {"target": target.id, "text": "hello"})

        self.assertEqual(
            speaker.recent_history,
            ['You said to Target: "hello"'],
        )
        self.assertEqual(
            target.recent_history,
            ['A stranger (id speaker) said to you: "hello"'],
        )
        self.assertIn("said to a stranger", bystander.recent_history[-1])

    def test_recent_history_is_bounded(self) -> None:
        world, speaker, target, *_ = speech_world()
        for index in range(RECENT_HISTORY_MAX_ENTRIES + 3):
            apply_action(world, speaker, "say", {"text": f"line {index}"})

        self.assertEqual(len(target.recent_history), RECENT_HISTORY_MAX_ENTRIES)
        self.assertNotIn("line 0", target.recent_history[0])
        self.assertIn(
            f"line {RECENT_HISTORY_MAX_ENTRIES + 2}",
            target.recent_history[-1],
        )

    def test_broadcast_and_exact_boundary(self) -> None:
        world, speaker, target, bystander, distant = speech_world()
        distant.position_m = Vec3(20, 0, 0)
        apply_action(world, speaker, "say", {"target": None, "text": "hear ye"})
        self.assertTrue(all(actor.inbox for actor in (target, bystander, distant)))
        self.assertNotIn("said to", target.inbox[-1])

    def test_bad_self_and_distant_target_never_broadcast(self) -> None:
        world, speaker, target, bystander, distant = speech_world()
        for bad_target in ("missing", speaker.id, distant.id):
            with self.subTest(target=bad_target), self.assertRaises(ActionError):
                apply_action(
                    world, speaker, "say", {"target": bad_target, "text": "secret"}
                )
            self.assertEqual(target.inbox, [])
            self.assertEqual(bystander.inbox, [])
            self.assertEqual(world.drain_events(), [])

    def test_speech_schema_and_limits_are_strict(self) -> None:
        world, speaker, *_ = speech_world()
        invalid = (
            {},
            {"text": ""},
            {"text": 7},
            {"text": "x", "unexpected": 1},
            {"text": "x" * (PLAYER_SPEECH_MAX_CHARS + 1)},
            {"text": "bad\0speech"},
            {"text": "bad\ud800speech"},
            {"text": "x", 1: "bad key"},
            [],
        )
        for args in invalid:
            with self.subTest(args=args), self.assertRaises(ActionError):
                apply_action(world, speaker, "say", args)

    def test_speaker_never_receives_own_utterance(self) -> None:
        world, speaker, *_ = speech_world()
        apply_action(world, speaker, "say", {"text": "hello"})
        self.assertEqual(speaker.inbox, [])
        self.assertEqual(speaker.recent_history, ['You said aloud: "hello"'])

    def test_wait_is_a_valid_no_op(self) -> None:
        world, speaker, *_ = speech_world()
        revision = world.world_revision

        self.assertEqual(apply_action(world, speaker, "wait", {}), "Speaker waits")
        self.assertEqual(world.world_revision, revision)
        self.assertEqual(world.drain_events(), [])
        with self.assertRaises(ActionError):
            apply_action(world, speaker, "wait", {"unexpected": True})

    def test_player_receives_structured_speech_without_private_inbox_growth(
        self,
    ) -> None:
        world = World()
        speaker = character("speaker", "Speaker", 0)
        player = character("player", "Player", 1, control="player")
        world.add(speaker)
        world.add(player)

        for index in range(3):
            apply_action(world, speaker, "say", {"text": f"hello {index}"})

        events = world.drain_events()
        self.assertEqual(player.inbox, [])
        self.assertEqual(len(events), 3)
        self.assertTrue(all(event.recipient_ids == (player.id,) for event in events))


class OfferTests(unittest.TestCase):
    def setUp(self) -> None:
        self.world = World()
        self.giver = character("giver", "Giver", 0)
        self.receiver = character("receiver", "Receiver", 3)
        self.other = character("other", "Other", 2)
        self.apple = Item(ItemIdStr("apple"), "apple", "apple")
        self.pear = Item(ItemIdStr("pear"), "pear", "pear")
        self.giver.holds[:] = [self.apple.id, self.pear.id]
        for entity in (self.apple, self.pear, self.giver, self.receiver, self.other):
            self.world.add(entity)

    def offer(self, item: str = "apple", target: str | None = "receiver") -> None:
        apply_action(
            self.world,
            self.giver,
            "offer_item",
            {"item_id": item, "target": target},
        )

    def test_offer_does_not_transfer_and_multiple_offers_coexist(self) -> None:
        revision = self.world.world_revision
        self.offer("apple")
        self.offer("pear", None)
        self.assertEqual(self.giver.holds, [self.apple.id, self.pear.id])
        self.assertEqual(set(self.world.offers), {self.apple.id, self.pear.id})
        self.assertGreater(self.world.world_revision, revision)
        self.assertLess(
            self.world.offers[self.apple.id].created_seq,
            self.world.offers[self.pear.id].created_seq,
        )

    def test_targeted_accept_is_only_transfer(self) -> None:
        self.offer()
        with self.assertRaises(ActionError):
            apply_action(
                self.world, self.other, "accept_offered_item", {"item_id": "apple"}
            )
        apply_action(
            self.world, self.receiver, "accept_offered_item", {"item_id": "apple"}
        )
        self.assertNotIn(self.apple.id, self.giver.holds)
        self.assertIn(self.apple.id, self.receiver.holds)
        self.assertNotIn(self.apple.id, self.world.offers)
        self.world.assert_invariants()

    def test_broadcast_first_accept_wins(self) -> None:
        self.offer(target=None)
        apply_action(
            self.world, self.other, "accept_offered_item", {"item_id": "apple"}
        )
        with self.assertRaises(ActionError):
            apply_action(
                self.world, self.receiver, "accept_offered_item", {"item_id": "apple"}
            )

    def test_accept_revalidates_distance_but_offer_persists(self) -> None:
        self.offer()
        self.receiver.position_m = Vec3(5, 0, 0)
        with self.assertRaisesRegex(ActionError, "more than 4 metres"):
            apply_action(
                self.world, self.receiver, "accept_offered_item", {"item_id": "apple"}
            )
        self.assertIn(self.apple.id, self.world.offers)

    def test_targeted_decline_and_broadcast_cannot_decline(self) -> None:
        self.offer()
        apply_action(self.world, self.receiver, "decline_offer", {"item_id": "apple"})
        self.assertIn(self.apple.id, self.giver.holds)
        self.assertNotIn(self.apple.id, self.world.offers)
        self.offer(target=None)
        with self.assertRaisesRegex(ActionError, "open to anyone"):
            apply_action(
                self.world, self.receiver, "decline_offer", {"item_id": "apple"}
            )

    def test_decline_revalidates_distance(self) -> None:
        self.offer()
        self.receiver.position_m = Vec3(100, 0, 0)
        with self.assertRaises(ActionError):
            apply_action(
                self.world, self.receiver, "decline_offer", {"item_id": "apple"}
            )
        self.assertIn(self.apple.id, self.world.offers)

    def test_retract_needs_no_proximity_and_distant_target_gets_no_magic_history(
        self,
    ) -> None:
        self.offer()
        self.receiver.inbox.clear()
        self.receiver.position_m = Vec3(100, 0, 0)
        apply_action(self.world, self.giver, "retract_offer", {"item_id": "apple"})
        self.assertEqual(self.receiver.inbox, [])
        self.assertNotIn(self.apple.id, self.world.offers)

    def test_reoffer_replaces_and_notifies_displaced_near_target(self) -> None:
        self.offer()
        self.world.drain_events()
        self.receiver.inbox.clear()
        apply_action(
            self.world,
            self.giver,
            "offer_item",
            {"item_id": "apple", "target": "other"},
        )
        self.assertIn("withdrew", self.receiver.inbox[0])
        self.assertEqual(self.world.offers[self.apple.id].target_id, self.other.id)
        events = self.world.drain_events()
        self.assertEqual(
            [event.kind for event in events], ["retract_offer", "offer_item"]
        )
        self.assertEqual(events[0].target_id, self.receiver.id)
        self.assertEqual(events[0].recipient_ids, (self.receiver.id,))

    def test_reoffer_gives_displaced_player_structured_feedback_only(self) -> None:
        self.receiver.control = "player"
        self.offer()
        self.world.drain_events()

        apply_action(
            self.world,
            self.giver,
            "offer_item",
            {"item_id": "apple", "target": "other"},
        )

        self.assertEqual(self.receiver.inbox, [])
        events = self.world.drain_events()
        self.assertEqual(events[0].kind, "retract_offer")
        self.assertEqual(events[0].recipient_ids, (self.receiver.id,))
        self.assertEqual(events[1].kind, "offer_item")
        self.assertIn(self.receiver.id, events[1].recipient_ids)

    def test_eating_retracts_and_removes_singular_item(self) -> None:
        self.offer()
        apply_action(self.world, self.giver, "eat", {"item_id": "apple"})
        self.assertNotIn(self.apple.id, self.world.items)
        self.assertNotIn(self.apple.id, self.world.offers)
        self.assertNotIn(self.apple.id, self.giver.holds)

    def test_offer_rejects_name_unknown_self_and_extra_fields(self) -> None:
        invalid = (
            {"item_id": "apple", "target": "giver"},
            {"item_id": "apple", "target": "missing"},
            {"item_id": "apple", "extra": 1},
            {"item_id": "apple", "target": 4},
            {"item_id": "not-an-id"},
        )
        for args in invalid:
            with self.subTest(args=args), self.assertRaises(ActionError):
                apply_action(self.world, self.giver, "offer_item", args)


class SeedAndSnapshotTests(unittest.TestCase):
    def test_seed_preserves_cast_inventory_control_and_positions(self) -> None:
        world = build_world()
        self.assertEqual(world.characters[CharIdStr("player")].control, "player")
        self.assertEqual(
            world.characters[CharIdStr("player")].knows,
            {CharIdStr("sv3n1"), CharIdStr("cb947"), CharIdStr("k0fb1")},
        )
        player = world.characters[CharIdStr("player")]
        self.assertEqual(player.position_m, Vec3(0, 0.91, 95))
        self.assertEqual(
            world.characters[CharIdStr("sv3n1")].position_m, Vec3(-1.8, 0.91, 114)
        )
        self.assertEqual(
            world.characters[CharIdStr("cb947")].position_m, Vec3(0, 0.91, 112)
        )
        self.assertEqual(
            world.characters[CharIdStr("k0fb1")].position_m, Vec3(1.8, 0.91, 114)
        )
        self.assertTrue(
            all(
                player.position_m.distance_squared(actor.position_m) <= 20**2
                for actor in world.characters.values()
                if actor.control == "llm"
            )
        )

    def test_player_uses_same_action_validator(self) -> None:
        world = build_world()
        player = world.characters[CharIdStr("player")]
        player.position_m = Vec3(0, 0.91, 111)
        apply_action(world, player, "say", {"target": "k0fb1", "text": "Hello"})
        self.assertIn("said to you", world.characters[CharIdStr("k0fb1")].inbox[-1])

    def test_snapshot_is_public_and_monotonic(self) -> None:
        world = build_world()
        snapshot = world.public_snapshot()
        encoded = str(snapshot)
        self.assertNotIn("back_story", encoded)
        self.assertNotIn("memories", encoded)
        before = snapshot["world_revision"]
        world.update_positions(1, [(CharIdStr("player"), Vec3(1, 2, 3))])
        self.assertGreater(world.public_snapshot()["world_revision"], before)
        self.assertEqual(len(snapshot["actors"]), 4)


if __name__ == "__main__":
    unittest.main()
