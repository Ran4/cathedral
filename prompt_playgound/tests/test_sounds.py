"""Sound percepts: radius, the witness split, and the player_sound wire.

These are the S1-S9 cases from features/sounds.md.
"""

from __future__ import annotations

import math
import tempfile
import unittest
from pathlib import Path

from support import MODULE_DIR, decode_output, envelope, hello  # noqa: F401

from server import SmartActorServer
from sim import (
    ActionError,
    Character,
    CharIdStr,
    SpatialUpdateError,
    Vec3,
    World,
    apply_action,
    emit_sound,
    sees,
)
from sounds import SOUNDS, AMBIENT, SOUND_ID_RE, emittable_sound_ids


class _NoSpeech:
    stt_available = False
    tts_available = False

    def transcribe(self, wav_path: Path) -> str:
        raise RuntimeError("unavailable")

    def synthesize(self, text: str, voice_key: str, output_wav: Path) -> None:
        raise RuntimeError("unavailable")


def character(
    actor_id: str,
    name: str,
    position: tuple[float, float, float],
    *,
    facing_yaw: float = 0.0,
    control: str = "llm",
    knows: set[str] | None = None,
) -> Character:
    return Character(
        id=CharIdStr(actor_id),
        name=name,
        control=control,  # type: ignore[arg-type]
        back_story="test",
        location_description="test square",
        position_m=Vec3(*position),
        appearance_key=name.lower(),
        voice_key=None if control == "player" else name.lower(),
        facing_yaw=facing_yaw,
        knows={CharIdStr(known) for known in (knows or set())},
    )


def yaw_towards(observer: Character, subject: Character) -> float:
    """Yaw that points ``observer`` straight at ``subject`` (yaw 0 faces -Z)."""
    dx = subject.position_m.x - observer.position_m.x
    dz = subject.position_m.z - observer.position_m.z
    return math.atan2(-dx, -dz)


class CatalogTests(unittest.TestCase):
    def test_catalog_rows_are_wire_and_filesystem_safe(self) -> None:
        for sound_id, sound in SOUNDS.items():
            self.assertEqual(sound_id, sound.sound_id)
            self.assertTrue(SOUND_ID_RE.fullmatch(sound_id))
            self.assertGreater(sound.audible_distance, 0)
            self.assertGreater(sound.duration_seconds, 0)
            self.assertTrue(sound.sfx_prompt)
            self.assertTrue(sound.heard)
        for sound_id, ambient in AMBIENT.items():
            self.assertEqual(sound_id, ambient.sound_id)
            self.assertTrue(SOUND_ID_RE.fullmatch(sound_id))

    def test_the_town_bell_is_not_actor_emittable(self) -> None:
        self.assertNotIn("town_bell", emittable_sound_ids())
        self.assertIn("fart", emittable_sound_ids())
        self.assertIsNone(SOUNDS["town_bell"].seen)


class SoundEventTests(unittest.TestCase):
    """S1: one event, recipients are exactly those inside the radius."""

    def test_fart_emits_one_event_to_exactly_the_characters_in_range(self) -> None:
        world = World()
        actor = character("actor", "Sven", (0, 0, 0))
        near = character("near", "Near", (5, 0, 0))
        boundary = character("boundary", "Boundary", (20, 0, 0))
        outside = character("outside", "Outside", (20.0001, 0, 0))
        for each in (actor, near, boundary, outside):
            world.add(each)

        line = apply_action(world, actor, "make_sound", {"sound": "fart"})

        self.assertEqual(line, "Sven farted.")
        events = world.drain_events()
        self.assertEqual(len(events), 1)
        event = events[0]
        self.assertEqual(event.event_type, "sound")
        self.assertEqual(event.sound_id, "fart")
        self.assertEqual(event.event_id, f"sound-{event.sequence}")
        self.assertEqual(event.actor_id, actor.id)
        self.assertEqual(event.audible_distance, 20.0)
        self.assertEqual(event.position_m, actor.position_m)
        self.assertEqual(set(event.recipient_ids), {near.id, boundary.id})
        self.assertEqual(outside.inbox, [])

    def test_witness_split_between_facing_and_turned_away(self) -> None:
        """S2: facing the actor upgrades the percept to an attributed one."""
        world = World()
        actor = character("sv1", "Sven", (0, 0, 0))
        facing = character("facing", "Conny", (0, 0, 5), knows={"sv1"})
        facing.facing_yaw = yaw_towards(facing, actor)
        away = character("away", "Mott", (0, 0, -5), knows={"sv1"})
        away.facing_yaw = yaw_towards(away, actor) + math.pi
        for each in (actor, facing, away):
            world.add(each)

        apply_action(world, actor, "make_sound", {"sound": "fart"})

        self.assertEqual(facing.inbox, ["Sven farted."])
        self.assertEqual(away.inbox, ["[You heard a big fart!]"])
        event = world.drain_events()[0]
        self.assertEqual(set(event.recipient_ids), {facing.id, away.id})
        self.assertEqual(event.witness_ids, (facing.id,))

    def test_stranger_witness_is_attributed_by_id_only(self) -> None:
        """S3: identify() keeps strangers strangers, even as witnesses."""
        world = World()
        actor = character("p0", "Sven", (0, 0, 0))
        stranger = character("stranger", "Ilse", (0, 0, 5))
        stranger.facing_yaw = yaw_towards(stranger, actor)
        world.add(actor)
        world.add(stranger)

        apply_action(world, actor, "make_sound", {"sound": "fart"})

        self.assertEqual(stranger.inbox, ["A stranger (id p0) farted."])

    def test_world_sound_is_never_attributed(self) -> None:
        """S4: no actor, no witnesses, one identical percept for everyone."""
        world = World()
        listener = character("listener", "Conny", (0, 0, 10))
        listener.facing_yaw = 0.0  # facing the bell: must make no difference
        world.add(listener)

        line = emit_sound(world, None, SOUNDS["town_bell"], position_m=Vec3(0, 0, 0))

        self.assertEqual(line, "[The town bell is ringing.]")
        event = world.drain_events()[0]
        self.assertIsNone(event.actor_id)
        self.assertEqual(event.witness_ids, ())
        self.assertEqual(event.recipient_ids, (listener.id,))
        self.assertEqual(listener.inbox, ["[The town bell is ringing.]"])

    def test_audible_distance_is_honoured_per_sound(self) -> None:
        """S5: at 30 m the bell (600 m) is heard and the fart (20 m) is not."""
        world = World()
        actor = character("actor", "Sven", (0, 0, 0))
        distant = character("distant", "Pike", (30, 0, 0))
        world.add(actor)
        world.add(distant)

        apply_action(world, actor, "make_sound", {"sound": "fart"})
        fart = world.drain_events()[0]
        self.assertEqual(fart.recipient_ids, ())
        self.assertEqual(distant.inbox, [])

        emit_sound(world, None, SOUNDS["town_bell"], position_m=Vec3(0, 0, 0))
        bell = world.drain_events()[0]
        # A world sound excludes nobody, so the erstwhile farter hears it too.
        self.assertEqual(set(bell.recipient_ids), {actor.id, distant.id})
        self.assertEqual(distant.inbox, ["[The town bell is ringing.]"])

    def test_view_cone_is_horizontal_only(self) -> None:
        """S6: elevation is ignored; a bearing-less overhead pair fails dark."""
        world = World()
        actor = character("actor", "Sven", (0, 0, 0))
        # 15 m above and only 3 m out: far outside a 3D 67.5-degree half-angle,
        # but the horizontal bearing points straight at the actor.
        balcony = character("balcony", "Conny", (0, 15, 3), knows={"actor"})
        balcony.facing_yaw = yaw_towards(balcony, actor)
        # Directly overhead there is no horizontal bearing at all.
        overhead = character("overhead", "Mott", (0, 15, 0), knows={"actor"})
        for each in (actor, balcony, overhead):
            world.add(each)

        apply_action(world, actor, "make_sound", {"sound": "fart"})

        event = world.drain_events()[0]
        self.assertEqual(event.witness_ids, (balcony.id,))
        self.assertEqual(balcony.inbox, ["Sven farted."])
        self.assertEqual(overhead.inbox, ["[You heard a big fart!]"])

    def test_unknown_or_non_emittable_sound_is_rejected_without_event(self) -> None:
        """S8: bad ids error back to the actor and emit nothing."""
        world = World()
        actor = character("actor", "Sven", (0, 0, 0))
        world.add(actor)

        with self.assertRaisesRegex(ActionError, "burp"):
            apply_action(world, actor, "make_sound", {"sound": "burp"})
        with self.assertRaisesRegex(ActionError, "town_bell"):
            apply_action(world, actor, "make_sound", {"sound": "town_bell"})
        with self.assertRaises(ActionError):
            apply_action(world, actor, "make_sound", {"sound": None})

        self.assertEqual(world.drain_events(), [])

    def test_disabled_sounds_reject_make_sound(self) -> None:
        world = World()
        world.sounds_enabled = False
        actor = character("actor", "Sven", (0, 0, 0))
        world.add(actor)

        with self.assertRaisesRegex(ActionError, "disabled"):
            apply_action(world, actor, "make_sound", {"sound": "fart"})
        self.assertEqual(world.drain_events(), [])

    def test_view_cone_boundary_is_inclusive(self) -> None:
        world = World()
        world.view_cone_degrees = 90.0
        actor = character("actor", "Sven", (0, 0, 0))
        observer = character("observer", "Conny", (0, 0, 5))
        world.add(actor)
        world.add(observer)
        # Exactly on the 45-degree half-angle edge.
        observer.facing_yaw = yaw_towards(observer, actor) + math.radians(45.0)
        self.assertTrue(sees(observer, actor, world.view_cone_degrees))
        observer.facing_yaw = yaw_towards(observer, actor) + math.radians(45.5)
        self.assertFalse(sees(observer, actor, world.view_cone_degrees))

    def test_positions_may_carry_facing_and_reject_non_finite_yaw(self) -> None:
        world = World()
        actor = character("actor", "Sven", (0, 0, 0))
        world.add(actor)

        world.update_positions(1, [(actor.id, Vec3(1, 0, 0), 2.5)])
        self.assertEqual(actor.facing_yaw, 2.5)

        with self.assertRaises(SpatialUpdateError):
            world.update_positions(2, [(actor.id, Vec3(1, 0, 0), float("nan"))])

    def test_facing_changes_never_bump_the_public_revision(self) -> None:
        world = World()
        actor = character("actor", "Sven", (0, 0, 0))
        world.add(actor)
        revision = world.world_revision

        changed = world.update_positions(1, [(actor.id, actor.position_m, 1.0)])

        self.assertFalse(changed)
        self.assertEqual(world.world_revision, revision)
        self.assertEqual(actor.facing_yaw, 1.0)


class ServerSoundTests(unittest.TestCase):
    """The sound wire message, the player's HUD percept, and rate limiting."""

    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.lines: list[str] = []
        self.now = 0.0
        self.server = SmartActorServer(
            Path(self.temp.name),
            output=self.lines.append,
            llm_complete=lambda prompt: "wait {}",
            llm_available=False,
            speech_backend=_NoSpeech(),
            clock=lambda: self.now,
        )
        self.addCleanup(self.server.close)
        self.server.handle_envelope(hello())

    def sound_messages(self) -> list[dict]:
        return [
            message
            for message in decode_output(self.lines)
            if message["type"] == "sound"
        ]

    def move_player(self, seq: int, facing_yaw: float) -> None:
        self.server.handle_envelope(
            envelope(
                "spatial_update",
                {
                    "spatial_seq": seq,
                    "updates": [
                        {
                            "actor_id": "player",
                            "position_m": {"x": 0.0, "y": 0.91, "z": 111.0},
                            "facing_yaw": facing_yaw,
                        }
                    ],
                },
                message_id=f"move-{seq}",
            )
        )

    def test_ready_snapshot_ships_actor_facing(self) -> None:
        ready = decode_output(self.lines)[0]
        self.assertEqual(ready["type"], "ready")
        for actor in ready["payload"]["snapshot"]["actors"]:
            self.assertIsInstance(actor["facing_yaw"], float)

    def test_player_facing_gates_the_player_hud_percept(self) -> None:
        """S7: the player is subject to the identical cone test."""
        world = self.server.world
        player = world.characters[CharIdStr("player")]
        sven = world.characters[CharIdStr("sv3n1")]
        towards_sven = yaw_towards(player, sven)

        self.move_player(1, towards_sven)
        apply_action(world, sven, "make_sound", {"sound": "fart"})
        self.server.poll()
        seen = self.sound_messages()[0]["payload"]
        self.assertEqual(seen["text_for_player"], "Sven farted.")
        self.assertEqual(seen["actor_id"], "sv3n1")
        self.assertIn("player", seen["witness_ids"])
        self.assertEqual(seen["sound_id"], "fart")
        self.assertEqual(seen["class"], "body")
        self.assertEqual(seen["audible_distance"], 20.0)

        self.move_player(2, towards_sven + math.pi)
        apply_action(world, sven, "make_sound", {"sound": "fart"})
        self.server.poll()
        heard = self.sound_messages()[1]["payload"]
        self.assertEqual(heard["text_for_player"], "[You heard a big fart!]")
        # Fail dark: an unattributed sound must not leak its actor's id.
        self.assertIsNone(heard["actor_id"])
        self.assertNotIn("player", heard["witness_ids"])
        self.assertIn("player", heard["recipient_ids"])

    def test_player_sound_is_rate_limited_and_confirmed(self) -> None:
        """S9: a second player_sound inside the cooldown is dropped silently."""
        fart = lambda message_id: envelope(  # noqa: E731
            "player_sound", {"sound_id": "fart"}, message_id=message_id
        )
        self.server.handle_envelope(fart("fart-1"))
        self.assertEqual(len(self.sound_messages()), 1)
        confirmation = self.sound_messages()[0]["payload"]
        self.assertEqual(confirmation["text_for_player"], "You farted.")
        self.assertEqual(confirmation["actor_id"], "player")

        self.now = 1.0
        self.server.handle_envelope(fart("fart-2"))
        self.assertEqual(len(self.sound_messages()), 1)

        self.now = 2.5
        self.server.handle_envelope(fart("fart-3"))
        self.assertEqual(len(self.sound_messages()), 2)
        # No command_result: player_sound is fire-and-forget.
        self.assertEqual(
            [m for m in decode_output(self.lines) if m["type"] == "command_result"],
            [],
        )

    def test_player_sound_nudges_the_nearest_witness(self) -> None:
        self.server.handle_envelope(
            envelope("player_sound", {"sound_id": "fart"}, message_id="fart-1")
        )
        recipients = self.sound_messages()[0]["payload"]["recipient_ids"]
        self.assertTrue(recipients, "seeded NPCs should be in fart range")
        # Recipients arrive nearest-first; the scheduler was handed one of them.
        self.assertIn(
            str(self.server.scheduler._priority_actor_id),  # noqa: SLF001
            recipients,
        )

    def test_debug_sound_rings_the_bell_without_an_actor(self) -> None:
        """The CATHEDRAL_DRIVE world-sound trigger for causes the sim lacks."""
        self.server.handle_envelope(
            envelope(
                "debug_sound",
                {
                    "sound_id": "town_bell",
                    "position_m": {"x": 0.0, "y": 40.0, "z": 140.0},
                },
                message_id="bell-1",
            )
        )
        bell = self.sound_messages()[0]["payload"]
        self.assertEqual(bell["sound_id"], "town_bell")
        self.assertIsNone(bell["actor_id"])
        self.assertEqual(bell["witness_ids"], [])
        self.assertEqual(bell["text_for_player"], "[The town bell is ringing.]")
        self.assertIn("player", bell["recipient_ids"])
