from __future__ import annotations

import contextlib
import io
import json
import base64
import struct
import tempfile
import threading
import unittest
import wave
from pathlib import Path

from support import MODULE_DIR, decode_output, envelope, hello, wait_until  # noqa: F401

from main import build_world
from protocol import ProtocolError, encode_message, parse_line, server_envelope
from server import (
    STT_STREAM_HELD_TRANSCRIPT_S,
    SmartActorServer,
    _wav_duration_seconds,
)
from sim import CharIdStr, Vec3, apply_action
from speech_client import RealtimeFailure, RealtimeTranscript


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
        self.transcribed: list[Path] = []
        self.synthesized: list[tuple[str, str]] = []

    def transcribe(self, wav_path: Path) -> str:
        self.transcribed.append(wav_path)
        if self.stt_error is not None:
            raise self.stt_error
        return self.transcript

    def synthesize(self, text: str, voice_key: str, output_wav: Path) -> None:
        if self.tts_error is not None:
            raise self.tts_error
        self.synthesized.append((text, voice_key))
        with wave.open(str(output_wav), "wb") as wav:
            wav.setnchannels(1)
            wav.setsampwidth(2)
            wav.setframerate(16_000)
            wav.writeframes(struct.pack("<800h", *([0] * 800)))


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


class BlockingTts(FakeSpeech):
    def __init__(self) -> None:
        super().__init__()
        self.started = threading.Event()
        self.release = threading.Event()

    def synthesize(self, text: str, voice_key: str, output_wav: Path) -> None:
        self.started.set()
        if not self.release.wait(timeout=1):
            raise TimeoutError("test did not release synthesis")
        super().synthesize(text, voice_key, output_wav)


class StreamingTts(FakeSpeech):
    def synthesize_stream(self, text, voice_key, on_chunk):
        self.synthesized.append((text, voice_key))
        encoded = base64.b64encode(struct.pack("<4h", 0, 100, -100, 0)).decode(
            "ascii"
        )
        on_chunk(0, 24_000, encoded)
        on_chunk(1, 24_000, encoded)
        return 2, 173


def player_recording_payload(
    request_id: str, basename: str, *, stt_backend: str = "cloud"
) -> dict:
    return {
        "request_id": request_id,
        "wav_basename": basename,
        "target_id": None,
        "position_m": {"x": 0, "y": 0.91, "z": 111},
        "spatial_seq": 1,
        "stt_backend": stt_backend,
    }


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
            ready["payload"]["capabilities"],
            {
                "llm": False,
                "stt": False,
                "stt_cloud": False,
                "stt_local": False,
                "tts": False,
                "tts_cloud": False,
                "tts_local": False,
                "tts_selected": "off",
            },
        )
        snapshot = ready["payload"]["snapshot"]
        self.assertEqual(snapshot["player_id"], "player")
        self.assertEqual(len(snapshot["actors"]), 4)
        self.assertNotIn("back_story", str(snapshot))

    def test_local_transcription_makes_stt_available_without_cloud_credentials(
        self,
    ) -> None:
        server = self.make_server(
            speech_backend=NoSpeech(),
            local_stt_backend=FakeSpeech(),
        )
        self.handshake(server)

        capabilities = self.messages("ready")[0]["payload"]["capabilities"]
        self.assertEqual(
            capabilities,
            {
                "llm": False,
                "stt": True,
                "stt_cloud": False,
                "stt_local": True,
                "tts": False,
                "tts_cloud": False,
                "tts_local": False,
                "tts_selected": "off",
            },
        )

    def test_local_tts_is_available_independently_from_cloud_speech(self) -> None:
        server = self.make_server(
            speech_backend=NoSpeech(),
            local_tts_backend=FakeSpeech(),
            tts_backend="local",
        )
        self.handshake(server)

        capabilities = self.messages("ready")[0]["payload"]["capabilities"]
        self.assertFalse(capabilities["stt"])
        self.assertFalse(capabilities["tts_cloud"])
        self.assertTrue(capabilities["tts_local"])
        self.assertTrue(capabilities["tts"])
        self.assertEqual(capabilities["tts_selected"], "local")

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

    def test_tts_backend_selection_is_strict_acknowledged_and_idempotent(self) -> None:
        cloud = FakeSpeech()
        server = self.make_server(
            cloud_tts_backend=cloud,
            tts_backend="off",
        )
        self.handshake(server)
        server.handle_envelope(
            envelope(
                "set_tts_backend",
                {"request_id": "tts-select-1", "backend": "cloud"},
                message_id="tts-command-1",
            )
        )
        result = self.messages("command_result")[-1]["payload"]
        self.assertTrue(result["success"])
        self.assertEqual(server.tts_backend, "cloud")

        server.handle_envelope(
            envelope(
                "set_tts_backend",
                {"request_id": "tts-select-1", "backend": "cloud"},
                message_id="tts-command-2",
            )
        )
        self.assertEqual(self.messages("command_result")[-1]["payload"], result)

        server.handle_envelope(
            envelope(
                "set_tts_backend",
                {"request_id": "tts-select-2", "backend": "automatic"},
                message_id="tts-command-3",
            )
        )
        self.assertFalse(self.messages("command_result")[-1]["payload"]["success"])
        self.assertEqual(server.tts_backend, "cloud")

    def test_selecting_unavailable_tts_does_not_change_server_state(self) -> None:
        server = self.make_server(tts_backend="off")
        self.handshake(server)
        server.handle_envelope(
            envelope(
                "set_tts_backend",
                {"request_id": "tts-local", "backend": "local"},
                message_id="tts-local-command",
            )
        )
        result = self.messages("command_result")[-1]["payload"]
        self.assertFalse(result["success"])
        self.assertEqual(result["error_code"], "tts_unavailable")
        self.assertEqual(server.tts_backend, "off")


class SpeechWorkerTests(ServerTestCase):
    def test_local_tts_forwards_pcm_before_stream_completion(self) -> None:
        backend = StreamingTts()
        server = self.make_server(
            local_tts_backend=backend,
            tts_backend="local",
        )
        self.handshake(server)
        ilse = server.world.characters[CharIdStr("k0fb1")]
        apply_action(
            server.world,
            ilse,
            "say",
            {"target": "player", "text": "Stream this"},
        )
        wait_until(lambda: bool(self.messages("tts_stream_end")), server.poll)

        chunks = self.messages("tts_chunk")
        self.assertEqual([item["payload"]["chunk_seq"] for item in chunks], [0, 1])
        self.assertTrue(all(item["payload"]["channels"] == 1 for item in chunks))
        end = self.messages("tts_stream_end")[-1]["payload"]
        self.assertEqual(end["chunk_count"], 2)
        self.assertEqual(end["first_chunk_ms"], 173)
        self.assertFalse(self.messages("tts_ready"))

    def test_tts_mode_is_captured_when_each_utterance_is_queued(self) -> None:
        local = BlockingTts()
        cloud = FakeSpeech()
        server = self.make_server(
            cloud_tts_backend=cloud,
            local_tts_backend=local,
            tts_backend="local",
        )
        self.handshake(server)
        ilse = server.world.characters[CharIdStr("k0fb1")]
        apply_action(
            server.world,
            ilse,
            "say",
            {"target": "player", "text": "Local line"},
        )
        server.poll()
        self.assertTrue(local.started.wait(timeout=1))

        server.handle_envelope(
            envelope(
                "set_tts_backend",
                {"request_id": "tts-cloud", "backend": "cloud"},
                message_id="tts-cloud-command",
            )
        )
        apply_action(
            server.world,
            ilse,
            "say",
            {"target": "player", "text": "Cloud line"},
        )
        server.poll()
        local.release.set()
        wait_until(lambda: bool(cloud.synthesized), server.poll)

        self.assertEqual([text for text, _ in local.synthesized], ["Local line"])
        self.assertEqual([text for text, _ in cloud.synthesized], ["Cloud line"])

    def test_off_mode_never_queues_synthesis(self) -> None:
        cloud = FakeSpeech()
        local = FakeSpeech()
        server = self.make_server(
            cloud_tts_backend=cloud,
            local_tts_backend=local,
            tts_backend="off",
        )
        self.handshake(server)
        ilse = server.world.characters[CharIdStr("k0fb1")]
        apply_action(
            server.world,
            ilse,
            "say",
            {"target": "player", "text": "Text only"},
        )
        server.poll()
        self.assertFalse(cloud.synthesized)
        self.assertFalse(local.synthesized)
        self.assertEqual(self.messages("speech")[-1]["payload"]["text"], "Text only")

    def recording_payload(
        self, request_id: str, basename: str, *, stt_backend: str = "cloud"
    ) -> dict:
        return {
            "request_id": request_id,
            "wav_basename": basename,
            "target_id": None,
            "position_m": {"x": 0, "y": 0.91, "z": 111},
            "spatial_seq": 1,
            "stt_backend": stt_backend,
        }

    def test_recording_routes_to_selected_local_backend(self) -> None:
        cloud = FakeSpeech(transcript="from cloud")
        local = FakeSpeech(transcript="from Canary")
        server = self.make_server(
            speech_backend=cloud,
            local_stt_backend=local,
        )
        self.handshake(server)
        wav = Path(self.temp.name) / "local-recording.wav"
        wav.write_bytes(b"RIFF")

        server.handle_envelope(
            envelope(
                "player_recording",
                self.recording_payload(
                    "local-recording", wav.name, stt_backend="local"
                ),
                message_id="local-recording-message",
            )
        )
        wait_until(lambda: bool(self.messages("command_result")), server.poll)

        self.assertEqual(
            self.messages("transcription_result")[-1]["payload"]["text"],
            "from Canary",
        )
        self.assertEqual(len(local.transcribed), 1)
        self.assertEqual(cloud.transcribed, [])

    def test_invalid_transcription_backend_is_rejected_and_cleans_audio(self) -> None:
        server = self.make_server(speech_backend=FakeSpeech())
        self.handshake(server)
        wav = Path(self.temp.name) / "invalid-backend.wav"
        wav.write_bytes(b"RIFF")

        server.handle_envelope(
            envelope(
                "player_recording",
                self.recording_payload(
                    "invalid-backend", wav.name, stt_backend="elsewhere"
                ),
                message_id="invalid-backend-message",
            )
        )

        self.assertFalse(wav.exists())
        self.assertEqual(
            self.messages("command_result")[-1]["payload"]["error_code"],
            "invalid_stt_backend",
        )

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
        failure = self.messages("tts_failed")[-1]["payload"]
        self.assertEqual(
            failure["speech_event_id"],
            self.messages("speech")[-1]["payload"]["event_id"],
        )
        self.assertIn("timed out", failure["reason"])


class TimingInstrumentationTests(ServerTestCase):
    def submit_recording(
        self, server: SmartActorServer, request_id: str, basename: str
    ) -> None:
        server.handle_envelope(
            envelope(
                "player_recording",
                player_recording_payload(request_id, basename),
                message_id=f"{request_id}-message",
            )
        )

    def timing_lines(self, stderr: io.StringIO) -> list[str]:
        return [
            line
            for line in stderr.getvalue().splitlines()
            if line.startswith("[smart actors/stt]")
        ]

    def test_batch_resolution_emits_one_timing_line(self) -> None:
        server = self.make_server(speech_backend=FakeSpeech(transcript="Hello"))
        self.handshake(server)
        wav = Path(self.temp.name) / "timing-batch.wav"
        wav.write_bytes(b"RIFF")
        self.submit_recording(server, "timing-batch", wav.name)
        stderr = io.StringIO()
        with contextlib.redirect_stderr(stderr):
            wait_until(lambda: bool(self.messages("command_result")), server.poll)
        lines = self.timing_lines(stderr)
        self.assertEqual(len(lines), 1)
        self.assertRegex(
            lines[0],
            r"^\[smart actors/stt\] timing-batch\.wav: "
            r"audio=\? path=batch endpoint->say=\d+ms$",
        )

    def test_failed_transcription_also_emits_one_timing_line(self) -> None:
        backend = FakeSpeech(stt_error=RuntimeError("provider offline"))
        server = self.make_server(speech_backend=backend)
        self.handshake(server)
        wav = Path(self.temp.name) / "timing-failed.wav"
        wav.write_bytes(b"RIFF")
        self.submit_recording(server, "timing-failed", wav.name)
        stderr = io.StringIO()
        with contextlib.redirect_stderr(stderr):
            wait_until(lambda: bool(self.messages("command_result")), server.poll)
        self.assertFalse(self.messages("command_result")[-1]["payload"]["success"])
        lines = self.timing_lines(stderr)
        self.assertEqual(len(lines), 1)
        self.assertIn("path=batch", lines[0])

    def test_wav_duration_reads_float32_riff_header(self) -> None:
        sample_rate = 48_000
        frames = sample_rate * 3 // 2
        data = b"\x00\x00\x00\x00" * frames
        fmt = struct.pack("<HHIIHH", 3, 1, sample_rate, sample_rate * 4, 4, 32)
        chunks = (
            b"fmt "
            + struct.pack("<I", len(fmt))
            + fmt
            + b"fact"
            + struct.pack("<I", 4)
            + struct.pack("<I", frames)
            + b"data"
            + struct.pack("<I", len(data))
            + data
        )
        wav = Path(self.temp.name) / "float32.wav"
        wav.write_bytes(b"RIFF" + struct.pack("<I", 4 + len(chunks)) + b"WAVE" + chunks)
        self.assertAlmostEqual(_wav_duration_seconds(wav), 1.5, places=3)

    def test_wav_duration_is_none_for_unparseable_files(self) -> None:
        garbage = Path(self.temp.name) / "garbage.wav"
        garbage.write_bytes(b"RIFF")
        self.assertIsNone(_wav_duration_seconds(garbage))
        missing = Path(self.temp.name) / "missing.wav"
        self.assertIsNone(_wav_duration_seconds(missing))


class StreamMessageMixin:
    def chunk_b64(self, samples: int = 480) -> str:
        return base64.b64encode(b"\x00\x00" * samples).decode("ascii")

    def send(
        self, server: SmartActorServer, message_type: str, payload: dict, message_id: str
    ) -> None:
        server.handle_envelope(envelope(message_type, payload, message_id=message_id))

    def begin(
        self,
        server: SmartActorServer,
        basename: str,
        *,
        message_id: str,
        sample_rate: int = 24_000,
        fmt: str = "pcm_s16le",
    ) -> None:
        self.send(
            server,
            "player_audio_begin",
            {"wav_basename": basename, "sample_rate": sample_rate, "format": fmt},
            message_id,
        )

    def chunk(
        self,
        server: SmartActorServer,
        basename: str,
        seq: int,
        *,
        message_id: str,
        encoded: str | None = None,
    ) -> None:
        self.send(
            server,
            "player_audio_chunk",
            {
                "wav_basename": basename,
                "seq": seq,
                "pcm_s16le_base64": encoded or self.chunk_b64(),
            },
            message_id,
        )

    def end(
        self,
        server: SmartActorServer,
        basename: str,
        chunk_count: int,
        *,
        message_id: str,
        silent: bool = False,
    ) -> None:
        self.send(
            server,
            "player_audio_end",
            {"wav_basename": basename, "chunk_count": chunk_count, "silent": silent},
            message_id,
        )

    def degraded_statuses(self) -> list[dict]:
        return [
            message["payload"]
            for message in self.messages("status")
            if message["payload"]["subsystem"] == "stt"
            and message["payload"]["state"] == "degraded"
        ]


class PlayerAudioStreamTests(StreamMessageMixin, ServerTestCase):
    def stream_server(self, **kwargs) -> tuple[SmartActorServer, FakeSpeech]:
        backend = kwargs.pop("speech_backend", FakeSpeech(transcript="Streamed words"))
        server = self.make_server(fake_mode=True, speech_backend=backend, **kwargs)
        self.handshake(server)
        return server, backend

    def test_streamed_utterance_resolves_through_result_pipeline(self) -> None:
        server, backend = self.stream_server()
        wav = Path(self.temp.name) / "player-recording-1.wav"
        wav.write_bytes(b"RIFF")
        self.begin(server, wav.name, message_id="s1-begin")
        self.chunk(server, wav.name, 0, message_id="s1-c0")
        self.chunk(server, wav.name, 1, message_id="s1-c1")
        self.end(server, wav.name, 2, message_id="s1-end")
        stderr = io.StringIO()
        with contextlib.redirect_stderr(stderr):
            server.handle_envelope(
                envelope(
                    "player_recording",
                    player_recording_payload("stream-1", wav.name),
                    message_id="s1-recording",
                )
            )
        self.assertFalse(wav.exists())
        self.assertEqual(
            self.messages("transcription_result")[-1]["payload"]["text"],
            "Streamed words",
        )
        self.assertTrue(self.messages("command_result")[-1]["payload"]["success"])
        self.assertEqual(len(backend.transcribed), 1)
        self.assertEqual(server._streams, {})
        self.assertEqual(self.degraded_statuses(), [])
        speech = self.messages("speech")[-1]["payload"]
        self.assertIsNone(speech["target_id"])
        timing = [
            line
            for line in stderr.getvalue().splitlines()
            if line.startswith("[smart actors/stt]")
        ]
        self.assertEqual(len(timing), 1)
        self.assertIn("path=stream", timing[0])
        self.assertIn("commit->transcript=", timing[0])

    def test_stream_messages_produce_no_command_result(self) -> None:
        server, backend = self.stream_server()
        self.begin(server, "player-recording-2.wav", message_id="s2-begin")
        self.chunk(server, "player-recording-2.wav", 0, message_id="s2-c0")
        self.end(server, "player-recording-2.wav", 1, message_id="s2-end")
        server.poll()
        self.assertEqual(self.messages("command_result"), [])
        self.assertEqual(self.messages("transcription_result"), [])
        self.assertEqual(self.messages("speech"), [])

    def test_begin_with_bad_format_degrades_to_batch_once(self) -> None:
        server, backend = self.stream_server()
        wav = Path(self.temp.name) / "player-recording-3.wav"
        wav.write_bytes(b"RIFF")
        self.begin(server, wav.name, message_id="s3-begin", sample_rate=48_000)
        self.chunk(server, wav.name, 0, message_id="s3-c0")
        self.end(server, wav.name, 1, message_id="s3-end")
        degraded = self.degraded_statuses()
        self.assertEqual(len(degraded), 1)
        self.assertIn("bad_format", degraded[0]["message"])
        server.handle_envelope(
            envelope(
                "player_recording",
                player_recording_payload("stream-3", wav.name),
                message_id="s3-recording",
            )
        )
        wait_until(lambda: bool(self.messages("command_result")), server.poll)
        self.assertTrue(self.messages("command_result")[-1]["payload"]["success"])
        self.assertEqual(len(backend.transcribed), 1)
        self.assertEqual(len(self.degraded_statuses()), 1)

    def test_stream_violations_degrade_exactly_once_each(self) -> None:
        server, _ = self.stream_server()
        self.begin(server, "player-recording-4a.wav", message_id="s4a-begin")
        self.chunk(server, "player-recording-4a.wav", 1, message_id="s4a-gap")
        self.chunk(server, "player-recording-4a.wav", 2, message_id="s4a-more")
        self.begin(server, "player-recording-4b.wav", message_id="s4b-begin")
        self.chunk(
            server,
            "player-recording-4b.wav",
            0,
            message_id="s4b-big",
            encoded="A" * 33_000,
        )
        self.begin(server, "player-recording-4c.wav", message_id="s4c-begin")
        self.chunk(server, "player-recording-4c.wav", 0, message_id="s4c-c0")
        self.end(server, "player-recording-4c.wav", 3, message_id="s4c-end")
        messages = [status["message"] for status in self.degraded_statuses()]
        self.assertEqual(len(messages), 3)
        self.assertIn("seq_gap", messages[0])
        self.assertIn("oversized_chunk", messages[1])
        self.assertIn("count_mismatch", messages[2])

    def test_chunk_after_completion_cannot_uncomplete_the_stream(self) -> None:
        server, backend = self.stream_server()
        wav = Path(self.temp.name) / "player-recording-4d.wav"
        wav.write_bytes(b"RIFF")
        self.begin(server, wav.name, message_id="s4d-begin")
        self.chunk(server, wav.name, 0, message_id="s4d-c0")
        self.end(server, wav.name, 1, message_id="s4d-end")
        self.chunk(server, wav.name, 1, message_id="s4d-late")
        server.handle_envelope(
            envelope(
                "player_recording",
                player_recording_payload("stream-4d", wav.name),
                message_id="s4d-recording",
            )
        )
        self.assertTrue(self.messages("command_result")[-1]["payload"]["success"])
        self.assertEqual(len(backend.transcribed), 1)
        self.assertEqual(self.degraded_statuses(), [])

    def test_silent_end_clears_stream_without_say(self) -> None:
        server, backend = self.stream_server()
        self.begin(server, "player-recording-5.wav", message_id="s5-begin")
        self.chunk(server, "player-recording-5.wav", 0, message_id="s5-c0")
        self.end(server, "player-recording-5.wav", 1, message_id="s5-end", silent=True)
        server.poll()
        self.assertEqual(server._streams, {})
        self.assertEqual(self.messages("transcription_result"), [])
        self.assertEqual(self.messages("speech"), [])
        self.assertEqual(backend.transcribed, [])
        self.assertEqual(self.degraded_statuses(), [])

    def test_abort_and_unknown_basenames_are_idempotent(self) -> None:
        server, _ = self.stream_server()
        self.send(
            server,
            "player_audio_abort",
            {"wav_basename": "player-recording-9.wav"},
            "s6-abort-unknown",
        )
        self.end(server, "player-recording-9.wav", 1, message_id="s6-end-unknown")
        self.chunk(server, "player-recording-9.wav", 0, message_id="s6-chunk-unknown")
        self.begin(server, "player-recording-6.wav", message_id="s6-begin")
        self.send(
            server,
            "player_audio_abort",
            {"wav_basename": "player-recording-6.wav"},
            "s6-abort",
        )
        self.send(
            server,
            "player_audio_abort",
            {"wav_basename": "player-recording-6.wav"},
            "s6-abort-again",
        )
        self.assertEqual(server._streams, {})
        self.assertEqual(self.degraded_statuses(), [])

    def test_begin_replaces_a_live_stream(self) -> None:
        server, backend = self.stream_server()
        wav = Path(self.temp.name) / "player-recording-7.wav"
        wav.write_bytes(b"RIFF")
        self.begin(server, wav.name, message_id="s7-begin-1")
        self.chunk(server, wav.name, 0, message_id="s7-c0")
        self.begin(server, wav.name, message_id="s7-begin-2")
        self.chunk(server, wav.name, 0, message_id="s7-c0-again")
        self.end(server, wav.name, 1, message_id="s7-end")
        server.handle_envelope(
            envelope(
                "player_recording",
                player_recording_payload("stream-7", wav.name),
                message_id="s7-recording",
            )
        )
        self.assertTrue(self.messages("command_result")[-1]["payload"]["success"])
        self.assertEqual(len(backend.transcribed), 1)
        self.assertEqual(self.degraded_statuses(), [])

    def test_completed_transcript_is_dropped_after_hold_window(self) -> None:
        self.now = 1_000.0
        server, backend = self.stream_server(clock=lambda: self.now)
        wav = Path(self.temp.name) / "player-recording-8.wav"
        wav.write_bytes(b"RIFF")
        self.begin(server, wav.name, message_id="s8-begin")
        self.chunk(server, wav.name, 0, message_id="s8-c0")
        self.end(server, wav.name, 1, message_id="s8-end")
        self.assertEqual(len(server._streams), 1)
        self.now += STT_STREAM_HELD_TRANSCRIPT_S + 0.1
        server.poll()
        self.assertEqual(server._streams, {})
        server.handle_envelope(
            envelope(
                "player_recording",
                player_recording_payload("stream-8", wav.name),
                message_id="s8-recording",
            )
        )
        wait_until(lambda: bool(self.messages("command_result")), server.poll)
        self.assertTrue(self.messages("command_result")[-1]["payload"]["success"])
        self.assertEqual(len(backend.transcribed), 2)

    def test_stream_messages_before_hello_are_dropped(self) -> None:
        server = self.make_server(fake_mode=True, speech_backend=FakeSpeech())
        stderr = io.StringIO()
        with contextlib.redirect_stderr(stderr):
            self.begin(server, "player-recording-11.wav", message_id="s9-begin")
        self.assertEqual(server._streams, {})
        self.assertEqual(self.messages("status"), [])


class FakeRealtimeSession:
    """Injected session double; tests drive its poll()/drain_status() output."""

    def __init__(self) -> None:
        self.begun: list[str] = []
        self.appended: list[str] = []
        self.committed: list[str] = []
        self.cleared: list[str] = []
        self.results: list = []
        self.statuses: list[tuple[str, str]] = []
        self.begin_ok = True
        self.append_ok = True
        self.commit_ok = True
        self.closed = False

    def begin(self, key: str) -> bool:
        self.begun.append(key)
        return self.begin_ok

    def append(self, key: str, pcm_s16le_base64: str) -> bool:
        self.appended.append(key)
        return self.append_ok

    def commit(self, key: str) -> bool:
        self.committed.append(key)
        return self.commit_ok

    def clear(self, key: str) -> None:
        self.cleared.append(key)

    def poll(self, now: float) -> list:
        drained, self.results = self.results, []
        return drained

    def drain_status(self) -> list[tuple[str, str]]:
        drained, self.statuses = self.statuses, []
        return drained

    def close(self) -> None:
        self.closed = True


class StreamJoinTests(StreamMessageMixin, ServerTestCase):
    def join_server(self, **kwargs):
        self.now = 1_000.0
        session = FakeRealtimeSession()
        backend = kwargs.pop("speech_backend", FakeSpeech(transcript="From batch"))
        server = self.make_server(
            speech_backend=backend,
            realtime_session=session,
            clock=lambda: self.now,
            **kwargs,
        )
        self.handshake(server)
        return server, session, backend

    def streamed_utterance(self, server, basename: str, *, prefix: str) -> None:
        self.begin(server, basename, message_id=f"{prefix}-begin")
        self.chunk(server, basename, 0, message_id=f"{prefix}-c0")
        self.end(server, basename, 1, message_id=f"{prefix}-end")

    def park_recording(self, server, request_id: str) -> Path:
        wav = Path(self.temp.name) / f"{request_id}.wav"
        wav.write_bytes(b"RIFF")
        self.streamed_utterance(server, wav.name, prefix=request_id)
        server.handle_envelope(
            envelope(
                "player_recording",
                player_recording_payload(request_id, wav.name),
                message_id=f"{request_id}-recording",
            )
        )
        return wav

    def test_committed_recording_parks_and_resolves_on_completion(self) -> None:
        server, session, backend = self.join_server()
        wav = self.park_recording(server, "join-1")
        self.assertEqual(session.begun, [wav.name])
        self.assertEqual(session.appended, [wav.name])
        self.assertEqual(session.committed, [wav.name])
        self.assertEqual(self.messages("transcription_result"), [])

        session.results.append(RealtimeTranscript(wav.name, "From the stream"))
        server.poll()
        self.assertEqual(
            self.messages("transcription_result")[-1]["payload"]["text"],
            "From the stream",
        )
        self.assertTrue(self.messages("command_result")[-1]["payload"]["success"])
        self.assertEqual(backend.transcribed, [])
        self.assertFalse(wav.exists())
        self.assertEqual(server._parked, {})

    def test_grace_expiry_batches_once_and_late_completion_is_discarded(self) -> None:
        server, session, backend = self.join_server()
        wav = self.park_recording(server, "join-2")

        self.now += server._stream_grace_seconds + 0.1
        wait_until(lambda: bool(self.messages("command_result")), server.poll)
        self.assertTrue(self.messages("command_result")[-1]["payload"]["success"])
        self.assertEqual(
            self.messages("transcription_result")[-1]["payload"]["text"],
            "From batch",
        )
        self.assertEqual(len(backend.transcribed), 1)
        self.assertIn(wav.name, session.cleared)

        session.results.append(RealtimeTranscript(wav.name, "Too late"))
        server.poll()
        self.assertEqual(len(self.messages("transcription_result")), 1)
        self.assertEqual(len(self.messages("speech")), 1)

    def test_session_failure_batches_parked_requests_immediately(self) -> None:
        server, session, backend = self.join_server()
        wav = self.park_recording(server, "join-3")

        session.statuses.append(("degraded", "socket lost"))
        session.results.append(RealtimeFailure(wav.name, "socket"))
        wait_until(lambda: bool(self.messages("command_result")), server.poll)
        self.assertTrue(self.messages("command_result")[-1]["payload"]["success"])
        self.assertEqual(len(backend.transcribed), 1)
        degraded = [
            status for status in self.degraded_statuses() if "socket" in status["message"]
        ]
        self.assertEqual(len(degraded), 1)

    def test_session_wide_failure_batches_every_parked_request(self) -> None:
        server, session, backend = self.join_server()
        self.park_recording(server, "join-4")

        session.results.append(RealtimeFailure(None, "socket"))
        wait_until(lambda: bool(self.messages("command_result")), server.poll)
        self.assertTrue(self.messages("command_result")[-1]["payload"]["success"])
        self.assertEqual(len(backend.transcribed), 1)
        self.assertEqual(server._parked, {})

    def test_commit_failure_degrades_quietly_to_batch(self) -> None:
        server, session, backend = self.join_server()
        session.commit_ok = False
        wav = Path(self.temp.name) / "join-5.wav"
        wav.write_bytes(b"RIFF")
        self.streamed_utterance(server, wav.name, prefix="join-5")
        server.handle_envelope(
            envelope(
                "player_recording",
                player_recording_payload("join-5", wav.name),
                message_id="join-5-recording",
            )
        )
        wait_until(lambda: bool(self.messages("command_result")), server.poll)
        self.assertTrue(self.messages("command_result")[-1]["payload"]["success"])
        self.assertEqual(len(backend.transcribed), 1)
        # Waiting for a session is expected; it never spams degraded statuses.
        self.assertEqual(self.degraded_statuses(), [])

    def test_local_backend_clears_the_session_and_never_streams(self) -> None:
        local = FakeSpeech(transcript="From Canary")
        server, session, backend = self.join_server(local_stt_backend=local)
        wav = Path(self.temp.name) / "join-6.wav"
        wav.write_bytes(b"RIFF")
        self.streamed_utterance(server, wav.name, prefix="join-6")
        server.handle_envelope(
            envelope(
                "player_recording",
                player_recording_payload("join-6", wav.name, stt_backend="local"),
                message_id="join-6-recording",
            )
        )
        wait_until(lambda: bool(self.messages("command_result")), server.poll)
        self.assertEqual(
            self.messages("transcription_result")[-1]["payload"]["text"],
            "From Canary",
        )
        self.assertIn(wav.name, session.cleared)
        self.assertEqual(backend.transcribed, [])
        self.assertEqual(len(local.transcribed), 1)

    def test_shutdown_closes_session_and_unlinks_parked_wav(self) -> None:
        server, session, backend = self.join_server()
        wav = self.park_recording(server, "join-7")
        server.close()
        self.assertTrue(session.closed)
        self.assertFalse(wav.exists())
        self.assertEqual(server._parked, {})


class TranscriptionPriorityTests(ServerTestCase):
    def transcribe(self, server: SmartActorServer, request_id: str, payload: dict) -> None:
        wav = Path(self.temp.name) / f"{request_id}.wav"
        wav.write_bytes(b"RIFF")
        payload = dict(payload, wav_basename=wav.name)
        server.handle_envelope(
            envelope("player_recording", payload, message_id=f"{request_id}-message")
        )
        wait_until(lambda: bool(self.messages("command_result")), server.poll)

    def test_transcribed_say_prioritizes_the_nearest_llm_recipient(self) -> None:
        server = self.make_server(speech_backend=FakeSpeech(transcript="Hello all"))
        self.handshake(server)
        self.transcribe(
            server, "priority-1", player_recording_payload("priority-1", "x.wav")
        )
        self.assertTrue(self.messages("command_result")[-1]["payload"]["success"])
        expected = next(
            character.id
            for character in server.world.characters_within(
                Vec3(0, 0.91, 111), 20.0, exclude=server.player_id
            )
            if character.control == "llm"
        )
        self.assertEqual(server.scheduler._priority_actor_id, expected)

    def test_prioritization_is_a_noop_without_llm_hearers(self) -> None:
        server = self.make_server(speech_backend=FakeSpeech(transcript="Anyone?"))
        self.handshake(server)
        payload = player_recording_payload("priority-2", "x.wav")
        payload["position_m"] = {"x": 500, "y": 0.91, "z": 500}
        payload["spatial_seq"] = 2
        self.transcribe(server, "priority-2", payload)
        self.assertTrue(self.messages("command_result")[-1]["payload"]["success"])
        self.assertIsNone(server.scheduler._priority_actor_id)


if __name__ == "__main__":
    unittest.main()
