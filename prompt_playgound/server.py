#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["openai", "python-dotenv"]
# ///
"""Persistent JSON-lines smart-actor sidecar for the Bevy game."""

from __future__ import annotations

import argparse
import json
import os
import queue
import struct
import sys
import threading
import time
import wave
from collections import OrderedDict
from collections.abc import Callable, Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import Generic, TypeVar

import llm_client
from main import build_world
from protocol import (
    PROTOCOL_VERSION,
    IncomingEnvelope,
    ProtocolError,
    encode_message,
    parse_envelope,
    parse_line,
    request_id,
    server_envelope,
    validated_id,
)
from scheduler import NpcScheduler
from sim import (
    PLAYER_SPEECH_MAX_CHARS,
    ActionError,
    CharIdStr,
    DomainEvent,
    SpatialUpdateError,
    Vec3,
    World,
    apply_action,
    identify,
)
from speech_client import OpenAISpeechBackend, SpeechBackend

MAX_REQUEST_HISTORY = 1_024
INPUT_QUEUE_CAPACITY = 256
SPEECH_QUEUE_CAPACITY = 32


def _enabled(value: str | None) -> bool:
    return (value or "").strip().lower() in {"1", "true", "yes", "on"}


def _safe_message(error: BaseException, fallback: str) -> str:
    if isinstance(error, (ActionError, SpatialUpdateError, ProtocolError)):
        text = str(error).strip()
        if text:
            return text[:300]
    return fallback


def _error_code(error: BaseException) -> str:
    return str(getattr(error, "code", "internal_error"))


def _exact_payload(
    payload: Mapping[str, object],
    *,
    required: set[str],
    optional: set[str] = frozenset(),
) -> None:
    missing = required - set(payload)
    unknown = set(payload) - required - optional
    if missing:
        raise ProtocolError(
            f"payload is missing {sorted(missing)[0]}", "invalid_request"
        )
    if unknown:
        raise ProtocolError(
            f"payload has unknown field {sorted(unknown)[0]}", "invalid_request"
        )


def _spatial_sequence(value: object) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise ProtocolError(
            "spatial_seq must be a non-negative integer", "invalid_position"
        )
    return value


def _item_id(value: object) -> str:
    return validated_id(value, "item_id")


def _target_id(value: object, *, nullable: bool = False) -> str | None:
    if nullable and value is None:
        return None
    return validated_id(value, "target_id")


def _safe_basename(value: object, *, suffix: str = ".wav") -> str:
    name = validated_id(value, "wav_basename")
    if name in {".", ".."} or "/" in name or "\\" in name or Path(name).name != name:
        raise ProtocolError(
            "WAV path must be a basename inside the runtime directory", "invalid_path"
        )
    if not name.lower().endswith(suffix):
        raise ProtocolError(f"audio basename must end in {suffix}", "invalid_path")
    return name


T = TypeVar("T")
R = TypeVar("R")


@dataclass(frozen=True, slots=True)
class _WorkerResult(Generic[T, R]):
    task: T
    value: R | None
    error: Exception | None


class _DaemonWorker(Generic[T, R]):
    def __init__(
        self,
        name: str,
        function: Callable[[T], R],
        *,
        capacity: int = SPEECH_QUEUE_CAPACITY,
    ) -> None:
        self._function = function
        self._closing = threading.Event()
        self._tasks: queue.Queue[T | None] = queue.Queue(maxsize=capacity)
        self._results: queue.SimpleQueue[_WorkerResult[T, R]] = queue.SimpleQueue()
        self._thread = threading.Thread(target=self._run, name=name, daemon=True)
        self._thread.start()

    def submit(self, task: T) -> bool:
        try:
            self._tasks.put_nowait(task)
        except queue.Full:
            return False
        return True

    def drain(self) -> list[_WorkerResult[T, R]]:
        results = []
        while True:
            try:
                results.append(self._results.get_nowait())
            except queue.Empty:
                return results

    def close(self) -> None:
        self._closing.set()
        try:
            self._tasks.put_nowait(None)
        except queue.Full:
            pass
        # Normal fake/local work exits immediately. A stuck provider never
        # blocks shutdown for more than this small grace period.
        self._thread.join(timeout=0.1)

    def _run(self) -> None:
        while True:
            task = self._tasks.get()
            if task is None:
                return
            try:
                value = self._function(task)
            except Exception as error:
                self._results.put(_WorkerResult(task, None, error))
            else:
                self._results.put(_WorkerResult(task, value, None))
            if self._closing.is_set():
                return


@dataclass(frozen=True, slots=True)
class _TranscriptionTask:
    request_id: str
    wav_path: Path
    position_m: Vec3


@dataclass(frozen=True, slots=True)
class _TtsTask:
    event_id: str
    text: str
    voice_key: str
    wav_path: Path


class FakeSpeechBackend:
    """Deterministic local WAV backend, enabled only by ``--fake``."""

    @property
    def stt_available(self) -> bool:
        return True

    @property
    def tts_available(self) -> bool:
        return True

    def transcribe(self, wav_path: Path) -> str:
        # Tests can choose a transcript without embedding text into an audio file.
        return os.environ.get("SMART_ACTORS_FAKE_TRANSCRIPT", "What's your name?")

    def synthesize(self, text: str, voice_key: str, output_wav: Path) -> None:
        output_wav.parent.mkdir(parents=True, exist_ok=True)
        sample_rate = 16_000
        sample_count = min(sample_rate // 4, max(800, len(text) * 40))
        with wave.open(str(output_wav), "wb") as wav:
            wav.setnchannels(1)
            wav.setsampwidth(2)
            wav.setframerate(sample_rate)
            wav.writeframes(struct.pack(f"<{sample_count}h", *([0] * sample_count)))


def fake_llm_complete(prompt: str) -> str:
    """Scripted cognition for offline integration tests, with no phrase hooks in prod."""
    try:
        sheet_text = prompt.split("```json\n", 1)[1].split("\n```", 1)[0]
        sheet = json.loads(sheet_text)
    except (IndexError, json.JSONDecodeError, TypeError):
        return 'set_goal {"goal": null}'
    name = sheet.get("name")
    history = "\n".join(str(event) for event in sheet.get("since_your_last_turn", []))
    history_lower = history.lower()
    if name == "Ilse" and "what's your name" in history_lower:
        return 'say {"target": "player", "text": "My name is Ilse. I am a pilgrim."}'
    if name == "Ilse" and "offer" in history_lower and "coin" in history_lower:
        return "\n".join(
            [
                'say {"target": "player", "text": "You may have my copper coin."}',
                'offer_item {"item_id": "c0prs", "target": "player"}',
            ]
        )
    return 'set_goal {"goal": null}'


class SmartActorServer:
    """State-thread implementation of the v1 sidecar protocol."""

    def __init__(
        self,
        runtime_dir: Path,
        *,
        world: World | None = None,
        llm_complete: Callable[[str], str] | None = None,
        llm_available: bool | None = None,
        speech_backend: SpeechBackend | None = None,
        output: Callable[[str], None] | None = None,
        fake_mode: bool = False,
        turn_delay_seconds: float | None = None,
        clock: Callable[[], float] = time.monotonic,
    ) -> None:
        runtime_dir = Path(runtime_dir)
        runtime_dir.mkdir(parents=True, exist_ok=True)
        self.runtime_dir = runtime_dir.resolve(strict=True)
        if not self.runtime_dir.is_dir():
            raise ValueError("runtime-dir must be a directory")
        self.world = world or build_world()
        self.player_id = CharIdStr("player")
        if self.world.characters.get(self.player_id) is None:
            raise ValueError("world has no stable player character")
        self.fake_mode = fake_mode
        completion_was_injected = llm_complete is not None
        if llm_complete is None:
            llm_complete = fake_llm_complete if fake_mode else llm_client.complete
        if llm_available is None:
            llm_available = (
                completion_was_injected or fake_mode or llm_client.is_available()
            )
        self.llm_available = bool(llm_available)
        self.speech_backend = speech_backend or (
            FakeSpeechBackend() if fake_mode else OpenAISpeechBackend()
        )
        delay = (
            turn_delay_seconds
            if turn_delay_seconds is not None
            else float(os.environ.get("NPC_TURN_DELAY_SECONDS", "1.0"))
        )
        self.scheduler = NpcScheduler(
            self.world,
            llm_complete,
            minimum_delay_seconds=delay,
            clock=clock,
            verbose=_enabled(os.environ.get("SMART_ACTORS_VERBOSE")),
        )
        self._clock = clock
        self._output = output or self._write_stdout
        self.session_id: str | None = None
        self.event_seq = 0
        self.running = True
        self.handshake_complete = False
        self._seen_message_ids: OrderedDict[str, None] = OrderedDict()
        self._pending_requests: set[str] = set()
        self._completed_requests: OrderedDict[str, dict[str, object]] = OrderedDict()
        self._last_snapshot_revision_sent = -1
        self._generated_audio: dict[tuple[str, str], Path] = {}
        self._stt_worker = _DaemonWorker(
            "smart-actor-stt", self._transcribe_task, capacity=4
        )
        self._tts_worker = _DaemonWorker(
            "smart-actor-tts", self._synthesize_task, capacity=SPEECH_QUEUE_CAPACITY
        )

    @staticmethod
    def _write_stdout(line: str) -> None:
        sys.stdout.write(line + "\n")
        sys.stdout.flush()

    def close(self) -> None:
        self.running = False
        self.scheduler.close()
        self._stt_worker.close()
        self._tts_worker.close()
        for path in list(self._generated_audio.values()):
            try:
                path.unlink(missing_ok=True)
            except OSError:
                pass
        self._generated_audio.clear()
        # Completed tasks may not yet have been polled into _generated_audio.
        for path in self.runtime_dir.glob("speech-*.wav"):
            try:
                path.unlink(missing_ok=True)
            except OSError:
                pass

    def handle_line(self, line: str) -> None:
        try:
            envelope = parse_line(line)
        except ProtocolError as error:
            print(f"[smart actors] protocol error: {error}", file=sys.stderr)
            if error.fatal:
                self.running = False
            elif self.handshake_complete:
                self._send_status("protocol", "degraded", message=str(error)[:300])
            return
        self.handle_envelope(envelope)

    def handle_envelope(
        self, envelope: IncomingEnvelope | Mapping[str, object]
    ) -> None:
        if not isinstance(envelope, IncomingEnvelope):
            try:
                envelope = parse_envelope(envelope)
            except ProtocolError as error:
                print(f"[smart actors] protocol error: {error}", file=sys.stderr)
                if error.fatal:
                    self.running = False
                elif self.handshake_complete:
                    self._send_status("protocol", "degraded", message=str(error)[:300])
                return
        if self.session_id is None:
            if envelope.message_type != "hello":
                print("[smart actors] ignored message before hello", file=sys.stderr)
                return
        elif envelope.session_id != self.session_id:
            print(
                "[smart actors] ignored message for a different session",
                file=sys.stderr,
            )
            return

        if envelope.message_id in self._seen_message_ids:
            return
        self._seen_message_ids[envelope.message_id] = None
        while len(self._seen_message_ids) > MAX_REQUEST_HISTORY:
            self._seen_message_ids.popitem(last=False)

        try:
            self._dispatch(envelope)
        except (ProtocolError, SpatialUpdateError, ActionError) as error:
            print(
                f"[smart actors] invalid {envelope.message_type}: {error}",
                file=sys.stderr,
            )
            rid = envelope.payload.get("request_id")
            if isinstance(rid, str) and rid and len(rid) <= 128:
                if (
                    rid not in self._pending_requests
                    and rid not in self._completed_requests
                ):
                    self._pending_requests.add(rid)
                self._finish_request(rid, False, _error_code(error), str(error))
            elif self.handshake_complete:
                self._send_status("protocol", "degraded", message=str(error)[:300])

    def poll(self) -> None:
        if not self.running:
            return
        for status in self.scheduler.poll(self._clock()):
            self._send("status", status.to_payload())
        self._poll_transcriptions()
        self._poll_tts()
        self._flush_domain_events()
        self._send_snapshot_if_changed()

    def _dispatch(self, envelope: IncomingEnvelope) -> None:
        message_type = envelope.message_type
        payload = envelope.payload
        if message_type == "hello":
            self._handle_hello(envelope)
        elif not self.handshake_complete:
            raise ProtocolError("hello handshake has not completed", "not_ready")
        elif message_type == "spatial_update":
            self._handle_spatial_update(payload)
        elif message_type == "player_recording":
            self._handle_player_recording(payload)
        elif message_type == "debug_player_say":
            self._handle_debug_player_say(payload)
        elif message_type == "player_offer":
            self._handle_player_action(
                payload, "offer_item", has_target=True, has_position=True
            )
        elif message_type == "player_accept":
            self._handle_player_action(
                payload, "accept_offered_item", has_target=False, has_position=True
            )
        elif message_type == "player_decline":
            self._handle_player_action(
                payload, "decline_offer", has_target=False, has_position=True
            )
        elif message_type == "player_retract":
            self._handle_player_action(
                payload, "retract_offer", has_target=False, has_position=False
            )
        elif message_type == "audio_consumed":
            self._handle_audio_consumed(payload)
        elif message_type == "resync_request":
            self._handle_resync(payload)
        elif message_type == "shutdown":
            _exact_payload(payload, required=set())
            self.running = False
        else:
            print(
                f"[smart actors] ignored unknown message type {message_type!r}",
                file=sys.stderr,
            )
            self._send_status(
                "protocol", "degraded", message=f"unknown message type {message_type!r}"
            )

    def _handle_hello(self, envelope: IncomingEnvelope) -> None:
        if self.handshake_complete:
            raise ProtocolError("hello was already received", "already_ready")
        payload = envelope.payload
        _exact_payload(
            payload,
            required={
                "supported_protocol_version",
                "player_id",
                "position_m",
                "spatial_seq",
            },
        )
        supported = payload["supported_protocol_version"]
        if isinstance(supported, bool) or supported != PROTOCOL_VERSION:
            self.running = False
            raise ProtocolError(
                "client does not support protocol version 1",
                "unsupported_version",
                fatal=True,
            )
        player_id = validated_id(payload["player_id"], "player_id")
        if player_id != self.player_id:
            raise ProtocolError("hello contains an unknown player id", "unknown_actor")
        position = Vec3.from_json(payload["position_m"])
        spatial_seq = _spatial_sequence(payload["spatial_seq"])
        self.world.update_positions(spatial_seq, [(self.player_id, position)])
        self.session_id = envelope.session_id
        self.handshake_complete = True
        self._send(
            "ready",
            {
                "capabilities": {
                    "llm": self.llm_available,
                    "stt": bool(self.speech_backend.stt_available),
                    "tts": bool(self.speech_backend.tts_available),
                },
                "snapshot": self.world.public_snapshot(self.player_id),
            },
        )
        self._last_snapshot_revision_sent = self.world.world_revision
        if self.llm_available:
            self.scheduler.start()
        else:
            self._send_status(
                "llm", "unavailable", message="text cognition is not configured"
            )

    def _handle_spatial_update(self, payload: Mapping[str, object]) -> None:
        _exact_payload(payload, required={"spatial_seq", "updates"})
        sequence = _spatial_sequence(payload["spatial_seq"])
        raw_updates = payload["updates"]
        if not isinstance(raw_updates, list) or not raw_updates:
            raise ProtocolError("updates must be a non-empty array", "invalid_position")
        updates: list[tuple[CharIdStr, Vec3]] = []
        for raw_update in raw_updates:
            if not isinstance(raw_update, Mapping):
                raise ProtocolError(
                    "each spatial update must be an object", "invalid_position"
                )
            _exact_payload(raw_update, required={"actor_id", "position_m"})
            actor_id = CharIdStr(validated_id(raw_update["actor_id"], "actor_id"))
            updates.append((actor_id, Vec3.from_json(raw_update["position_m"])))
        self.world.update_positions(sequence, updates)

    def _apply_player_position(self, payload: Mapping[str, object]) -> None:
        sequence = _spatial_sequence(payload["spatial_seq"])
        position = Vec3.from_json(payload["position_m"])
        self.world.update_positions(sequence, [(self.player_id, position)])

    def _begin_request(self, payload: Mapping[str, object]) -> str | None:
        rid = request_id(payload)
        completed = self._completed_requests.get(rid)
        if completed is not None:
            # Idempotent retry: resend the authoritative cached answer without
            # applying the command again.
            self._send("command_result", completed)
            return None
        if rid in self._pending_requests:
            return None
        self._pending_requests.add(rid)
        return rid

    def _finish_request(
        self,
        rid: str,
        success: bool,
        code: str,
        message: str,
    ) -> None:
        if rid in self._completed_requests:
            return
        self._pending_requests.discard(rid)
        payload: dict[str, object] = {
            "request_id": rid,
            "success": success,
            "error_code": None if success else code,
            "message": message[:300],
        }
        self._completed_requests[rid] = payload
        while len(self._completed_requests) > MAX_REQUEST_HISTORY:
            self._completed_requests.popitem(last=False)
        self._send("command_result", payload)

    def _handle_player_action(
        self,
        payload: Mapping[str, object],
        verb: str,
        *,
        has_target: bool,
        has_position: bool,
    ) -> None:
        required = {"request_id", "item_id"}
        if has_target:
            required.add("target_id")
        if has_position:
            required.update({"position_m", "spatial_seq"})
        _exact_payload(payload, required=required)
        rid = self._begin_request(payload)
        if rid is None:
            return
        try:
            if has_position:
                self._apply_player_position(payload)
            action_args: dict[str, object] = {"item_id": _item_id(payload["item_id"])}
            if has_target:
                action_args["target"] = _target_id(payload["target_id"])
            player = self.world.characters[self.player_id]
            line = apply_action(self.world, player, verb, action_args)
            self.world.transcript.append(line)
        except Exception as error:
            self._flush_domain_events()
            self._send_snapshot_if_changed()
            self._finish_request(
                rid,
                False,
                _error_code(error),
                _safe_message(error, "the action could not be completed"),
            )
            return
        self._flush_domain_events()
        self._send_snapshot_if_changed()
        self._finish_request(rid, True, "ok", line)

    def _handle_debug_player_say(self, payload: Mapping[str, object]) -> None:
        if not self.fake_mode:
            raise ProtocolError(
                "debug_player_say is available only in fake mode", "forbidden"
            )
        _exact_payload(
            payload,
            required={"request_id", "text", "target_id", "position_m", "spatial_seq"},
        )
        rid = self._begin_request(payload)
        if rid is None:
            return
        try:
            self._apply_player_position(payload)
            target_id = _target_id(payload["target_id"], nullable=True)
            player = self.world.characters[self.player_id]
            line = apply_action(
                self.world,
                player,
                "say",
                {"text": payload["text"], "target": target_id},
            )
            self.world.transcript.append(line)
            if target_id is not None:
                self.scheduler.prioritize(CharIdStr(target_id))
        except Exception as error:
            self._flush_domain_events()
            self._send_snapshot_if_changed()
            self._finish_request(
                rid,
                False,
                _error_code(error),
                _safe_message(error, "the injected utterance was rejected"),
            )
            return
        self._flush_domain_events()
        self._send_snapshot_if_changed()
        self._finish_request(rid, True, "ok", line)

    def _runtime_input_path(self, basename: str) -> Path:
        candidate = self.runtime_dir / basename
        try:
            resolved = candidate.resolve(strict=True)
        except OSError as error:
            raise ProtocolError(
                "recording WAV does not exist", "missing_audio"
            ) from error
        if resolved.parent != self.runtime_dir or not resolved.is_file():
            raise ProtocolError(
                "recording WAV is outside the runtime directory", "invalid_path"
            )
        return resolved

    def _handle_player_recording(self, payload: Mapping[str, object]) -> None:
        _exact_payload(
            payload,
            required={
                "request_id",
                "wav_basename",
                "target_id",
                "position_m",
                "spatial_seq",
            },
        )
        rid = self._begin_request(payload)
        if rid is None:
            return
        wav_path: Path | None = None
        try:
            basename = _safe_basename(payload["wav_basename"])
            wav_path = self._runtime_input_path(basename)
            if payload["target_id"] is not None:
                raise ProtocolError(
                    "player microphone speech must have a null target_id",
                    "invalid_target",
                )
            if not self.speech_backend.stt_available:
                raise ProtocolError(
                    "speech transcription is unavailable", "stt_unavailable"
                )
            self._apply_player_position(payload)
            utterance_position = self.world.characters[self.player_id].position_m
            task = _TranscriptionTask(rid, wav_path, utterance_position)
            if not self._stt_worker.submit(task):
                raise ProtocolError("transcription queue is full", "overloaded")
        except Exception as error:
            if wav_path is not None:
                try:
                    wav_path.unlink(missing_ok=True)
                except OSError:
                    pass
            self._send(
                "transcription_result",
                {
                    "request_id": rid,
                    "text": None,
                    "error": _safe_message(error, "transcription request was rejected"),
                },
            )
            self._send_snapshot_if_changed()
            self._finish_request(
                rid,
                False,
                _error_code(error),
                _safe_message(error, "transcription request was rejected"),
            )
            return
        self._send_status("stt", "transcribing")

    def _transcribe_task(self, task: _TranscriptionTask) -> str:
        return self.speech_backend.transcribe(task.wav_path)

    def _poll_transcriptions(self) -> None:
        for result in self._stt_worker.drain():
            task = result.task
            try:
                task.wav_path.unlink(missing_ok=True)
            except OSError as error:
                print(
                    f"[smart actors] could not remove recording: {error}",
                    file=sys.stderr,
                )
            if result.error is not None:
                print(
                    f"[smart actors] transcription failed: {type(result.error).__name__}",
                    file=sys.stderr,
                )
                self._send(
                    "transcription_result",
                    {
                        "request_id": task.request_id,
                        "text": None,
                        "error": "transcription failed",
                    },
                )
                self._send_status("stt", "degraded", message="transcription failed")
                self._finish_request(
                    task.request_id,
                    False,
                    "transcription_failed",
                    "transcription failed",
                )
                continue
            text = result.value
            if not isinstance(text, str):
                text = ""
            text = text.strip()
            if not text:
                self._send(
                    "transcription_result",
                    {
                        "request_id": task.request_id,
                        "text": None,
                        "error": "no speech detected",
                    },
                )
                self._send_status("stt", "idle", message="no speech detected")
                self._finish_request(
                    task.request_id, False, "empty_transcription", "no speech detected"
                )
                continue
            if len(text) > PLAYER_SPEECH_MAX_CHARS:
                self._send(
                    "transcription_result",
                    {
                        "request_id": task.request_id,
                        "text": None,
                        "error": "transcription exceeds the 500 character limit",
                    },
                )
                self._send_status("stt", "idle")
                self._finish_request(
                    task.request_id,
                    False,
                    "text_too_long",
                    "transcription exceeds the 500 character limit",
                )
                continue
            try:
                text.encode("utf-8")
                has_control = any(
                    (ord(character) < 0x20 and character not in "\n\t")
                    or 0x7F <= ord(character) <= 0x9F
                    for character in text
                )
            except UnicodeEncodeError:
                has_control = True
            if has_control:
                self._send(
                    "transcription_result",
                    {
                        "request_id": task.request_id,
                        "text": None,
                        "error": "transcription contains unsupported characters",
                    },
                )
                self._send_status("stt", "idle")
                self._finish_request(
                    task.request_id,
                    False,
                    "invalid_transcription",
                    "transcription contains unsupported characters",
                )
                continue
            self._send(
                "transcription_result",
                {"request_id": task.request_id, "text": text, "error": None},
            )
            try:
                player = self.world.characters[self.player_id]
                current_position = player.position_m
                player.position_m = task.position_m
                try:
                    line = apply_action(
                        self.world,
                        player,
                        "say",
                        {"text": text},
                    )
                finally:
                    # STT can finish after newer spatial updates. Freeze this
                    # utterance at its recorded action position without
                    # rewinding the authoritative current player position.
                    player.position_m = current_position
                self.world.transcript.append(line)
            except Exception as error:
                self._flush_domain_events()
                self._send_snapshot_if_changed()
                self._send_status("stt", "idle")
                self._finish_request(
                    task.request_id,
                    False,
                    _error_code(error),
                    _safe_message(error, "the transcription was rejected by the world"),
                )
                continue
            self._flush_domain_events()
            self._send_snapshot_if_changed()
            self._send_status("stt", "idle")
            self._finish_request(task.request_id, True, "ok", line)

    def _synthesize_task(self, task: _TtsTask) -> None:
        self.speech_backend.synthesize(task.text, task.voice_key, task.wav_path)

    def _poll_tts(self) -> None:
        for result in self._tts_worker.drain():
            task = result.task
            if result.error is not None:
                try:
                    task.wav_path.unlink(missing_ok=True)
                except OSError:
                    pass
                print(
                    f"[smart actors] speech synthesis failed: "
                    f"{type(result.error).__name__}",
                    file=sys.stderr,
                )
                self._send_status("tts", "degraded", message="speech synthesis failed")
                continue
            if not task.wav_path.is_file():
                self._send_status(
                    "tts", "degraded", message="speech synthesis made no WAV"
                )
                continue
            basename = task.wav_path.name
            self._generated_audio[(task.event_id, basename)] = task.wav_path
            self._send(
                "tts_ready",
                {"speech_event_id": task.event_id, "wav_basename": basename},
            )
            self._send_status("tts", "idle")

    def _queue_tts(self, event: DomainEvent) -> None:
        speaker = self.world.characters.get(event.actor_id)
        if (
            speaker is None
            or speaker.control == "player"
            or speaker.voice_key is None
            or event.text is None
            or self.player_id not in event.recipient_ids
        ):
            return
        if not self.speech_backend.tts_available:
            return
        basename = f"{event.event_id}.wav"
        wav_path = self.runtime_dir / basename
        task = _TtsTask(event.event_id, event.text, speaker.voice_key, wav_path)
        if not self._tts_worker.submit(task):
            self._send_status("tts", "degraded", message="speech queue is full")
            return
        self._send_status("tts", "synthesizing", actor_id=speaker.id)

    def _handle_audio_consumed(self, payload: Mapping[str, object]) -> None:
        _exact_payload(payload, required={"speech_event_id", "wav_basename"})
        event_id = validated_id(payload["speech_event_id"], "speech_event_id")
        basename = _safe_basename(payload["wav_basename"])
        path = self._generated_audio.pop((event_id, basename), None)
        if path is None:
            raise ProtocolError(
                "audio acknowledgement does not match a generated WAV", "unknown_audio"
            )
        if path.parent != self.runtime_dir:
            raise ProtocolError(
                "generated audio path escaped runtime directory", "invalid_path"
            )
        try:
            path.unlink(missing_ok=True)
        except OSError as error:
            print(
                f"[smart actors] could not remove generated speech: {error}",
                file=sys.stderr,
            )

    def _handle_resync(self, payload: Mapping[str, object]) -> None:
        _exact_payload(payload, required={"last_world_revision"})
        revision = payload["last_world_revision"]
        if isinstance(revision, bool) or not isinstance(revision, int) or revision < 0:
            raise ProtocolError("last_world_revision must be a non-negative integer")
        self._send("world_snapshot", self.world.public_snapshot(self.player_id))
        self._last_snapshot_revision_sent = self.world.world_revision

    def _flush_domain_events(self) -> None:
        if not self.handshake_complete:
            return
        player = self.world.characters[self.player_id]
        for event in self.world.drain_events():
            if event.event_type == "speech":
                speaker = self.world.characters.get(event.actor_id)
                if speaker is None or event.text is None or event.position_m is None:
                    continue
                label = (
                    "You" if speaker.id == self.player_id else identify(player, speaker)
                )
                self._send(
                    "speech",
                    {
                        "event_id": event.event_id,
                        "speaker_id": str(event.actor_id),
                        "target_id": str(event.target_id)
                        if event.target_id is not None
                        else None,
                        "text": event.text,
                        "speaker_position_m": event.position_m.to_json(),
                        "recipient_ids": [
                            str(actor_id) for actor_id in event.recipient_ids
                        ],
                        "speaker_name_for_player": label,
                    },
                )
                self._queue_tts(event)
            else:
                self._send(
                    "world_event",
                    {
                        "event_id": event.event_id,
                        "kind": event.kind,
                        "actor_id": str(event.actor_id),
                        "target_id": str(event.target_id)
                        if event.target_id is not None
                        else None,
                        "item_id": str(event.item_id)
                        if event.item_id is not None
                        else None,
                        "recipient_ids": [
                            str(actor_id) for actor_id in event.recipient_ids
                        ],
                    },
                )

    def _send_snapshot_if_changed(self) -> None:
        if (
            self.handshake_complete
            and self.world.world_revision > self._last_snapshot_revision_sent
        ):
            self._send("world_snapshot", self.world.public_snapshot(self.player_id))
            self._last_snapshot_revision_sent = self.world.world_revision

    def _send_status(
        self,
        subsystem: str,
        state: str,
        *,
        actor_id: CharIdStr | None = None,
        message: str | None = None,
    ) -> None:
        payload: dict[str, object] = {"subsystem": subsystem, "state": state}
        if actor_id is not None:
            payload["actor_id"] = str(actor_id)
        if message is not None:
            payload["message"] = message[:300]
        self._send("status", payload)

    def _send(self, message_type: str, payload: Mapping[str, object]) -> None:
        if self.session_id is None:
            return
        self.event_seq += 1
        message = server_envelope(
            self.session_id, self.event_seq, message_type, payload
        )
        self._output(encode_message(message))


def _reader_thread(lines: queue.Queue[str | None]) -> None:
    try:
        for line in sys.stdin:
            lines.put(line)
    finally:
        lines.put(None)


def run_stdio(server: SmartActorServer) -> int:
    lines: queue.Queue[str | None] = queue.Queue(maxsize=INPUT_QUEUE_CAPACITY)
    reader = threading.Thread(
        target=_reader_thread, args=(lines,), name="smart-actor-stdin", daemon=True
    )
    reader.start()
    try:
        while server.running:
            try:
                line = lines.get(timeout=0.02)
            except queue.Empty:
                line = ""
            if line is None:
                break
            if line:
                server.handle_line(line)
            server.poll()
    except KeyboardInterrupt:
        return 130
    finally:
        server.close()
    return 0


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--stdio", action="store_true", help="use JSON lines on stdin/stdout"
    )
    parser.add_argument("--runtime-dir", type=Path, required=True)
    parser.add_argument(
        "--fake",
        action="store_true",
        help="offline deterministic LLM/STT/TTS backends for tests and development",
    )
    args = parser.parse_args()
    if not args.stdio:
        parser.error("only --stdio transport is implemented")
    fake_mode = args.fake or _enabled(os.environ.get("SMART_ACTORS_FAKE_MODE"))
    try:
        server = SmartActorServer(args.runtime_dir, fake_mode=fake_mode)
    except Exception as error:
        print(f"[smart actors] startup failed: {error}", file=sys.stderr)
        raise SystemExit(2) from error
    raise SystemExit(run_stdio(server))


if __name__ == "__main__":
    main()
