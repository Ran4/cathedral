from __future__ import annotations

import json
import tempfile
import threading
import unittest
from pathlib import Path

from support import MODULE_DIR, decode_output, envelope, hello, wait_until  # noqa: F401

from main import build_world
from protocol import ProtocolError, encode_message, parse_line, server_envelope
from server import SmartActorServer
from sim import CharIdStr, Vec3, apply_action


class NoSpeech:
    stt_available = False
    tts_available = False

    def transcribe(self, wav_path: Path) -> str:
        raise RuntimeError("unavailable")

    def synthesize(self, text: str, voice_key: str, output_wav: Path) -> None:
        raise RuntimeError("unavailable")


class FakeSpeech:
    stt_available = True
    tts_available = True

    def __init__(
        self, *, transcript: str = "Hello Ilse", stt_error=None, tts_error=None
    ):
        self.transcript = transcript
        self.stt_error = stt_error
        self.tts_error = tts_error
        self.synthesized: list[tuple[str, str]] = []

    def transcribe(self, wav_path: Path) -> str:
        if self.stt_error is not None:
            raise self.stt_error
        return self.transcript

    def synthesize(self, text: str, voice_key: str, output_wav: Path) -> None:
        if self.tts_error is not None:
            raise self.tts_error
        self.synthesized.append((text, voice_key))
        output_wav.write_bytes(b"RIFF-fake-wav")


class BlockingSpeech(FakeSpeech):
    def __init__(self, *, transcript: str) -> None:
        super().__init__(transcript=transcript)
        self.started = threading.Event()
        self.release = threading.Event()

    def transcribe(self, wav_path: Path) -> str:
        self.started.set()
        if not self.release.wait(timeout=1):
            raise TimeoutError("test did not release transcription")
        return super().transcribe(wav_path)


class ProtocolPrimitiveTests(unittest.TestCase):
    def test_nonfinite_json_is_rejected(self) -> None:
        line = json.dumps(hello()).replace("111.0", "NaN")
        with self.assertRaisesRegex(ProtocolError, "non-finite"):
            parse_line(line)

    def test_unknown_version_is_fatal(self) -> None:
        value = hello()
        value["protocol_version"] = 2
        with self.assertRaises(ProtocolError) as raised:
            parse_line(json.dumps(value))
        self.assertTrue(raised.exception.fatal)

    def test_pathologically_nested_json_is_rejected_without_recursion_escape(
        self,
    ) -> None:
        nested = "[" * 2_000 + "]" * 2_000
        line = (
            '{"protocol_version":1,"session_id":"s","message_id":"m",'
            '"type":"hello","payload":{"nested":' + nested + "}}"
        )
        with self.assertRaises(ProtocolError):
            parse_line(line)

    def test_server_encoding_is_compact_and_has_event_sequence(self) -> None:
        message = server_envelope("session", 4, "status", {"state": "idle"})
        encoded = encode_message(message)
        self.assertNotIn(" ", encoded)
        self.assertEqual(json.loads(encoded)["event_seq"], 4)
        self.assertEqual(json.loads(encoded)["message_id"], "python-4")


class ServerTestCase(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.lines: list[str] = []

    def make_server(self, **kwargs) -> SmartActorServer:
        server = SmartActorServer(
            Path(self.temp.name),
            output=self.lines.append,
            llm_complete=kwargs.pop(
                "llm_complete", lambda prompt: 'set_goal {"goal": null}'
            ),
            llm_available=kwargs.pop("llm_available", False),
            speech_backend=kwargs.pop("speech_backend", NoSpeech()),
            **kwargs,
        )
        self.addCleanup(server.close)
        return server

    def messages(self, message_type: str | None = None) -> list[dict]:
        messages = decode_output(self.lines)
        if message_type is not None:
            messages = [
                message for message in messages if message["type"] == message_type
            ]
        return messages

    def handshake(self, server: SmartActorServer) -> None:
        server.handle_envelope(hello())


class HandshakeAndStateTests(ServerTestCase):
    def test_ready_contains_capabilities_and_full_snapshot(self) -> None:
        server = self.make_server()
        self.handshake(server)
        ready = self.messages("ready")[0]
        self.assertEqual(ready["event_seq"], 1)
        self.assertEqual(
            ready["payload"]["capabilities"], {"llm": False, "stt": False, "tts": False}
        )
        snapshot = ready["payload"]["snapshot"]
        self.assertEqual(snapshot["player_id"], "player")
        self.assertEqual(len(snapshot["actors"]), 4)
        self.assertNotIn("back_story", str(snapshot))

    def test_session_mismatch_is_rejected_without_mutation(self) -> None:
        server = self.make_server()
        self.handshake(server)
        before = server.world.world_revision
        server.handle_envelope(
            envelope(
                "spatial_update",
                {
                    "spatial_seq": 1,
                    "updates": [
                        {"actor_id": "player", "position_m": {"x": 9, "y": 1, "z": 9}}
                    ],
                },
                message_id="wrong-1",
                session_id="old-session",
            )
        )
        self.assertEqual(server.world.world_revision, before)

    def test_spatial_updates_are_atomic_monotonic_and_snapshotted(self) -> None:
        server = self.make_server()
        self.handshake(server)
        server.handle_envelope(
            envelope(
                "spatial_update",
                {
                    "spatial_seq": 1,
                    "updates": [
                        {"actor_id": "player", "position_m": {"x": 1, "y": 2, "z": 3}}
                    ],
                },
                message_id="spatial-1",
            )
        )
        server.poll()
        self.assertEqual(
            server.world.characters[CharIdStr("player")].position_m, Vec3(1, 2, 3)
        )
        snapshots = self.messages("world_snapshot")
        self.assertTrue(snapshots)
        revisions = [message["payload"]["world_revision"] for message in snapshots]
        self.assertEqual(revisions, sorted(revisions))
        with self.assertRaises(Exception):
            # Direct domain call proves stale sequences cannot corrupt state.
            server.world.update_positions(0, [(CharIdStr("player"), Vec3(8, 8, 8))])
        self.assertEqual(
            server.world.characters[CharIdStr("player")].position_m, Vec3(1, 2, 3)
        )

    def test_unknown_actor_and_nonfinite_position_are_rejected(self) -> None:
        server = self.make_server()
        self.handshake(server)
        bad = envelope(
            "spatial_update",
            {
                "spatial_seq": 1,
                "updates": [
                    {"actor_id": "missing", "position_m": {"x": 1, "y": 2, "z": 3}}
                ],
            },
            message_id="spatial-bad",
        )
        server.handle_envelope(bad)
        self.assertEqual(server.world.spatial_sequence, 0)

        ilse_before = server.world.characters[CharIdStr("k0fb1")].position_m
        server.handle_envelope(
            envelope(
                "spatial_update",
                {
                    "spatial_seq": 1,
                    "updates": [
                        {
                            "actor_id": "k0fb1",
                            "position_m": {"x": 100, "y": 0.91, "z": 100},
                        }
                    ],
                },
                message_id="spatial-known-npc",
            )
        )
        self.assertIn(
            "only move the player",
            self.messages("status")[-1]["payload"]["message"],
        )
        self.assertEqual(
            server.world.characters[CharIdStr("k0fb1")].position_m,
            ilse_before,
        )

    def test_resync_always_returns_complete_current_snapshot(self) -> None:
        server = self.make_server()
        self.handshake(server)
        self.lines.clear()
        server.handle_envelope(
            envelope(
                "resync_request",
                {"last_world_revision": 0},
                message_id="resync-1",
            )
        )
        snapshots = self.messages("world_snapshot")
        self.assertEqual(len(snapshots), 1)
        self.assertEqual(len(snapshots[0]["payload"]["items"]), 2)

    def test_unknown_message_is_reported_and_ignored(self) -> None:
        server = self.make_server()
        self.handshake(server)
        server.handle_envelope(envelope("future_thing", {}, message_id="future-1"))
        self.assertEqual(
            self.messages("status")[-1]["payload"]["subsystem"], "protocol"
        )


class CommandTests(ServerTestCase):
    def offered_server(self) -> SmartActorServer:
        world = build_world()
        player = world.characters[CharIdStr("player")]
        ilse = world.characters[CharIdStr("k0fb1")]
        player.position_m = Vec3(0, 0.91, 111)
        apply_action(
            world,
            ilse,
            "offer_item",
            {"item_id": "c0prs", "target": "player"},
        )
        world.drain_events()
        return self.make_server(world=world)

    def test_player_accept_transfers_once_and_deduplicates_request(self) -> None:
        server = self.offered_server()
        self.handshake(server)
        payload = {
            "request_id": "accept-1",
            "item_id": "c0prs",
            "position_m": {"x": 0, "y": 0.91, "z": 111},
            "spatial_seq": 1,
        }
        server.handle_envelope(
            envelope("player_accept", payload, message_id="accept-msg-1")
        )
        revision = server.world.world_revision
        server.handle_envelope(
            envelope("player_accept", payload, message_id="accept-msg-2")
        )
        player = server.world.characters[CharIdStr("player")]
        self.assertEqual(player.holds.count(CharIdStr("c0prs")), 1)
        self.assertEqual(server.world.world_revision, revision)
        self.assertEqual(len(server.world.offers), 0)
        self.assertTrue(self.messages("command_result")[-1]["payload"]["success"])

    def test_failed_player_action_returns_code_without_mutation(self) -> None:
        server = self.make_server()
        self.handshake(server)
        server.handle_envelope(
            envelope(
                "player_offer",
                {
                    "request_id": "offer-bad",
                    "target_id": "k0fb1",
                    "item_id": "c0prs",
                    "position_m": {"x": 0, "y": 0.91, "z": 111},
                    "spatial_seq": 1,
                },
                message_id="offer-bad-msg",
            )
        )
        result = self.messages("command_result")[-1]["payload"]
        self.assertFalse(result["success"])
        self.assertEqual(result["error_code"], "not_owner")
        self.assertIn(
            CharIdStr("c0prs"), server.world.characters[CharIdStr("k0fb1")].holds
        )

    def test_debug_say_is_fake_only_and_uses_real_range_validation(self) -> None:
        server = self.make_server(fake_mode=False)
        self.handshake(server)
        payload = {
            "request_id": "debug-1",
            "text": "hello",
            "target_id": "k0fb1",
            "position_m": {"x": 0, "y": 0.91, "z": 111},
            "spatial_seq": 1,
        }
        server.handle_envelope(
            envelope("debug_player_say", payload, message_id="debug-msg")
        )
        self.assertEqual(
            self.messages("command_result")[-1]["payload"]["error_code"], "forbidden"
        )

        lines: list[str] = []
        fake = SmartActorServer(
            Path(self.temp.name),
            output=lines.append,
            fake_mode=True,
            turn_delay_seconds=100,
        )
        self.addCleanup(fake.close)
        fake.handle_envelope(hello())
        fake.handle_envelope(
            envelope("debug_player_say", payload, message_id="debug-msg-2")
        )
        messages = decode_output(lines)
        speech = [message for message in messages if message["type"] == "speech"][-1]
        self.assertIn("k0fb1", speech["payload"]["recipient_ids"])
        self.assertTrue(
            [message for message in messages if message["type"] == "command_result"][
                -1
            ]["payload"]["success"]
        )


class SpeechWorkerTests(ServerTestCase):
    def recording_payload(self, request_id: str, basename: str) -> dict:
        return {
            "request_id": request_id,
            "wav_basename": basename,
            "target_id": None,
            "position_m": {"x": 0, "y": 0.91, "z": 111},
            "spatial_seq": 1,
        }

    def test_fake_stt_success_applies_player_speech_and_deletes_recording(self) -> None:
        backend = FakeSpeech(transcript="  What's your name?  ")
        server = self.make_server(speech_backend=backend)
        self.handshake(server)
        wav = Path(self.temp.name) / "recording.wav"
        wav.write_bytes(b"RIFF")
        server.handle_envelope(
            envelope(
                "player_recording",
                self.recording_payload("record-1", wav.name),
                message_id="record-msg-1",
            )
        )
        wait_until(lambda: bool(self.messages("command_result")), server.poll)
        self.assertFalse(wav.exists())
        self.assertEqual(
            self.messages("transcription_result")[-1]["payload"]["text"],
            "What's your name?",
        )
        self.assertTrue(self.messages("command_result")[-1]["payload"]["success"])
        speech = self.messages("speech")[-1]["payload"]
        self.assertIsNone(speech["target_id"])
        self.assertEqual(
            set(speech["recipient_ids"]),
            {"sv3n1", "cb947", "k0fb1"},
        )

    def test_completed_recording_retry_deletes_its_new_wav(self) -> None:
        server = self.make_server(speech_backend=FakeSpeech(transcript="Only once"))
        self.handshake(server)
        original = Path(self.temp.name) / "completed-original.wav"
        original.write_bytes(b"RIFF-original")
        server.handle_envelope(
            envelope(
                "player_recording",
                self.recording_payload("completed-recording", original.name),
                message_id="completed-recording-original",
            )
        )
        wait_until(lambda: bool(self.messages("command_result")), server.poll)
        self.assertFalse(original.exists())

        same_message_retry = Path(self.temp.name) / "same-message-retry.wav"
        same_message_retry.write_bytes(b"RIFF-retry")
        server.handle_envelope(
            envelope(
                "player_recording",
                self.recording_payload("completed-recording", same_message_retry.name),
                message_id="completed-recording-original",
            )
        )
        self.assertFalse(same_message_retry.exists())
        self.assertEqual(len(self.messages("command_result")), 1)

        retry = Path(self.temp.name) / "completed-retry.wav"
        retry.write_bytes(b"RIFF-retry")
        server.handle_envelope(
            envelope(
                "player_recording",
                self.recording_payload("completed-recording", retry.name),
                message_id="completed-recording-retry",
            )
        )

        self.assertFalse(retry.exists())
        self.assertEqual(len(self.messages("transcription_result")), 1)
        self.assertEqual(len(self.messages("speech")), 1)
        self.assertEqual(len(self.messages("command_result")), 2)

        protected_recording = Path(self.temp.name) / "other-recording.wav"
        protected_tts = Path(self.temp.name) / "pending-speech.wav"
        generated_tts = Path(self.temp.name) / "generated-speech.wav"
        for path in (protected_recording, protected_tts, generated_tts):
            path.write_bytes(b"RIFF-owned")
        server._pending_recording_paths["other-request"] = protected_recording
        server._pending_tts_paths.add(protected_tts)
        server._generated_audio[("speech-event", generated_tts.name)] = generated_tts

        for index, protected in enumerate(
            (protected_recording, protected_tts, generated_tts)
        ):
            server.handle_envelope(
                envelope(
                    "player_recording",
                    self.recording_payload("completed-recording", protected.name),
                    message_id=f"completed-recording-protected-{index}",
                )
            )
            self.assertTrue(protected.exists())

    def test_pending_recording_retry_preserves_active_wav_and_deletes_distinct_wav(
        self,
    ) -> None:
        backend = BlockingSpeech(transcript="Only once")
        server = self.make_server(speech_backend=backend)
        self.handshake(server)
        active = Path(self.temp.name) / "pending-active.wav"
        active.write_bytes(b"RIFF-active")
        server.handle_envelope(
            envelope(
                "player_recording",
                self.recording_payload("pending-recording", active.name),
                message_id="pending-recording-original",
            )
        )
        self.assertTrue(backend.started.wait(timeout=1))

        malformed_retry = Path(self.temp.name) / "pending-malformed-retry.wav"
        malformed_retry.write_bytes(b"RIFF-retry")
        malformed_payload = self.recording_payload(
            "pending-recording", malformed_retry.name
        )
        malformed_payload["unexpected"] = True
        server.handle_envelope(
            envelope(
                "player_recording",
                malformed_payload,
                message_id="pending-recording-malformed",
            )
        )
        self.assertFalse(malformed_retry.exists())
        self.assertEqual(self.messages("command_result"), [])

        server.handle_envelope(
            envelope(
                "player_recording",
                self.recording_payload("pending-recording", active.name),
                message_id="pending-recording-same-path",
            )
        )
        self.assertTrue(active.exists())

        retry = Path(self.temp.name) / "pending-retry.wav"
        retry.write_bytes(b"RIFF-retry")
        server.handle_envelope(
            envelope(
                "player_recording",
                self.recording_payload("pending-recording", retry.name),
                message_id="pending-recording-distinct-path",
            )
        )
        self.assertFalse(retry.exists())
        self.assertTrue(active.exists())

        backend.release.set()
        wait_until(lambda: bool(self.messages("command_result")), server.poll)
        self.assertFalse(active.exists())
        self.assertEqual(len(self.messages("transcription_result")), 1)
        self.assertEqual(len(self.messages("speech")), 1)

    def test_malformed_recording_payload_cleans_only_unowned_audio(self) -> None:
        server = self.make_server(speech_backend=FakeSpeech())
        self.handshake(server)
        malformed = Path(self.temp.name) / "malformed-recording.wav"
        malformed.write_bytes(b"RIFF-malformed")
        payload = self.recording_payload("malformed-recording", malformed.name)
        payload["unexpected"] = True

        server.handle_envelope(
            envelope(
                "player_recording",
                payload,
                message_id="malformed-recording-message",
            )
        )
        self.assertFalse(malformed.exists())
        self.assertFalse(self.messages("command_result")[-1]["payload"]["success"])

        protected = Path(self.temp.name) / "protected-recording.wav"
        protected.write_bytes(b"RIFF-active")
        server._pending_recording_paths["active-request"] = protected
        payload = self.recording_payload("other-malformed", protected.name)
        payload["unexpected"] = True
        server.handle_envelope(
            envelope(
                "player_recording",
                payload,
                message_id="protected-malformed-message",
            )
        )
        self.assertTrue(protected.exists())

    def test_fresh_recording_request_cannot_claim_reserved_audio(self) -> None:
        server = self.make_server(speech_backend=FakeSpeech())
        self.handshake(server)
        active_recording = Path(self.temp.name) / "active-recording.wav"
        pending_tts = Path(self.temp.name) / "pending-tts.wav"
        generated_tts = Path(self.temp.name) / "generated-tts.wav"
        for path in (active_recording, pending_tts, generated_tts):
            path.write_bytes(b"RIFF-owned")
        server._pending_recording_paths["active"] = active_recording
        server._pending_tts_paths.add(pending_tts)
        server._generated_audio[("speech", generated_tts.name)] = generated_tts

        for index, protected in enumerate(
            (active_recording, pending_tts, generated_tts)
        ):
            payload = self.recording_payload(f"fresh-{index}", protected.name)
            if index == 0:
                payload["target_id"] = "k0fb1"
            server.handle_envelope(
                envelope(
                    "player_recording",
                    payload,
                    message_id=f"fresh-reserved-{index}",
                )
            )
            self.assertTrue(protected.exists())
            result = self.messages("command_result")[-1]["payload"]
            self.assertFalse(result["success"])
            self.assertEqual(result["error_code"], "audio_in_use")

    def test_tts_cannot_claim_a_reserved_recording_path(self) -> None:
        backend = FakeSpeech()
        server = self.make_server(speech_backend=backend)
        self.handshake(server)
        speaker = server.world.characters[CharIdStr("k0fb1")]
        event = server.world.emit(
            "speech",
            "say",
            speaker.id,
            text="This must remain text-only",
            position_m=speaker.position_m,
            recipient_ids=[server.player_id],
        )
        recording = Path(self.temp.name) / f"{event.event_id}.wav"
        recording.write_bytes(b"RIFF-active-recording")
        server._pending_recording_paths["active-recording"] = recording

        server._queue_tts(event)

        self.assertEqual(recording.read_bytes(), b"RIFF-active-recording")
        self.assertNotIn(recording, server._pending_tts_paths)
        self.assertEqual(backend.synthesized, [])
        status = self.messages("status")[-1]["payload"]
        self.assertEqual(status["subsystem"], "tts")
        self.assertIn("already in use", status["message"])

    def test_player_recording_rejects_non_null_target_and_deletes_audio(self) -> None:
        server = self.make_server(speech_backend=FakeSpeech())
        self.handshake(server)
        wav = Path(self.temp.name) / "targeted-recording.wav"
        wav.write_bytes(b"RIFF")
        payload = self.recording_payload("targeted-recording", wav.name)
        payload["target_id"] = "k0fb1"

        server.handle_envelope(
            envelope(
                "player_recording",
                payload,
                message_id="targeted-recording-msg",
            )
        )

        result = self.messages("command_result")[-1]["payload"]
        self.assertFalse(result["success"])
        self.assertEqual(result["error_code"], "invalid_target")
        self.assertFalse(wav.exists())
        self.assertFalse(self.messages("speech"))

    def test_recording_hearing_uses_utterance_position_while_player_moves(self) -> None:
        backend = BlockingSpeech(transcript="I spoke before walking away")
        server = self.make_server(speech_backend=backend)
        self.handshake(server)
        wav = Path(self.temp.name) / "moving-recording.wav"
        wav.write_bytes(b"RIFF")
        server.handle_envelope(
            envelope(
                "player_recording",
                self.recording_payload("moving-recording", wav.name),
                message_id="moving-recording-msg",
            )
        )
        self.assertTrue(backend.started.wait(timeout=1))

        server.handle_envelope(
            envelope(
                "spatial_update",
                {
                    "spatial_seq": 2,
                    "updates": [
                        {
                            "actor_id": "player",
                            "position_m": {"x": 0, "y": 0.91, "z": 200},
                        }
                    ],
                },
                message_id="walked-away-msg",
            )
        )
        backend.release.set()
        wait_until(lambda: bool(self.messages("command_result")), server.poll)

        speech = self.messages("speech")[-1]["payload"]
        self.assertEqual(set(speech["recipient_ids"]), {"sv3n1", "cb947", "k0fb1"})
        self.assertEqual(speech["speaker_position_m"]["z"], 111)
        self.assertEqual(
            server.world.characters[CharIdStr("player")].position_m,
            Vec3(0, 0.91, 200),
        )

    def test_stt_timeout_and_failure_degrade_without_crashing(self) -> None:
        for index, error in enumerate((TimeoutError("slow"), RuntimeError("failed"))):
            with self.subTest(error=type(error).__name__):
                self.lines.clear()
                backend = FakeSpeech(stt_error=error)
                server = self.make_server(speech_backend=backend)
                self.handshake(server)
                wav = Path(self.temp.name) / f"recording-{index}.wav"
                wav.write_bytes(b"RIFF")
                server.handle_envelope(
                    envelope(
                        "player_recording",
                        self.recording_payload(f"record-{index}", wav.name),
                        message_id=f"record-msg-{index}",
                    )
                )
                wait_until(lambda: bool(self.messages("command_result")), server.poll)
                self.assertTrue(server.running)
                self.assertFalse(
                    self.messages("command_result")[-1]["payload"]["success"]
                )
                self.assertEqual(
                    self.messages("status")[-1]["payload"]["state"], "degraded"
                )

    def test_invalid_unicode_transcription_is_rejected_without_protocol_output(
        self,
    ) -> None:
        server = self.make_server(speech_backend=FakeSpeech(transcript="bad\ud800text"))
        self.handshake(server)
        wav = Path(self.temp.name) / "recording-invalid-text.wav"
        wav.write_bytes(b"RIFF")
        server.handle_envelope(
            envelope(
                "player_recording",
                self.recording_payload("invalid-text-1", wav.name),
                message_id="invalid-text-msg",
            )
        )
        wait_until(lambda: bool(self.messages("command_result")), server.poll)

        result = self.messages("command_result")[-1]["payload"]
        self.assertFalse(result["success"])
        self.assertEqual(result["error_code"], "invalid_transcription")
        self.assertIsNone(self.messages("transcription_result")[-1]["payload"]["text"])
        self.assertFalse(wav.exists())

    def test_recording_path_traversal_is_rejected(self) -> None:
        server = self.make_server(speech_backend=FakeSpeech())
        self.handshake(server)
        payload = self.recording_payload("path-1", "../secret.wav")
        server.handle_envelope(
            envelope("player_recording", payload, message_id="path-msg")
        )
        self.assertEqual(
            self.messages("command_result")[-1]["payload"]["error_code"], "invalid_path"
        )

    def test_rejected_recording_is_deleted_before_returning_error(self) -> None:
        server = self.make_server(speech_backend=NoSpeech())
        self.handshake(server)
        wav = Path(self.temp.name) / "recording-unavailable.wav"
        wav.write_bytes(b"RIFF")
        server.handle_envelope(
            envelope(
                "player_recording",
                self.recording_payload("unavailable-1", wav.name),
                message_id="unavailable-msg",
            )
        )

        self.assertFalse(wav.exists())
        self.assertEqual(
            self.messages("command_result")[-1]["payload"]["error_code"],
            "stt_unavailable",
        )

    def test_tts_success_ready_acknowledgement_and_cleanup(self) -> None:
        backend = FakeSpeech()
        server = self.make_server(speech_backend=backend)
        self.handshake(server)
        ilse = server.world.characters[CharIdStr("k0fb1")]
        apply_action(
            server.world, ilse, "say", {"target": "player", "text": "Greetings"}
        )
        wait_until(lambda: bool(self.messages("tts_ready")), server.poll)
        ready = self.messages("tts_ready")[-1]["payload"]
        path = Path(self.temp.name) / ready["wav_basename"]
        self.assertTrue(path.exists())
        server.handle_envelope(
            envelope(
                "audio_consumed",
                ready,
                message_id="audio-consumed-1",
            )
        )
        self.assertFalse(path.exists())

    def test_tts_failure_preserves_text_event(self) -> None:
        server = self.make_server(
            speech_backend=FakeSpeech(tts_error=TimeoutError("slow"))
        )
        self.handshake(server)
        ilse = server.world.characters[CharIdStr("k0fb1")]
        apply_action(
            server.world, ilse, "say", {"target": "player", "text": "Still visible"}
        )
        wait_until(
            lambda: any(
                message["type"] == "status"
                and message["payload"]["subsystem"] == "tts"
                and message["payload"]["state"] == "degraded"
                for message in decode_output(self.lines)
            ),
            server.poll,
        )
        self.assertEqual(
            self.messages("speech")[-1]["payload"]["text"], "Still visible"
        )
        self.assertFalse(self.messages("tts_ready"))


if __name__ == "__main__":
    unittest.main()
