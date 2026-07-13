#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["openai", "python-dotenv", "websocket-client"]
# ///
"""Persistent JSON-lines smart-actor sidecar for the Bevy game."""

from __future__ import annotations

import argparse
import base64
import json
import math
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
    HEARING_RADIUS_M,
    PLAYER_SPEECH_MAX_CHARS,
    ActionError,
    CharIdStr,
    DomainEvent,
    SpatialUpdateError,
    Vec3,
    World,
    _cap,
    apply_action,
    emit_sound,
    identify,
)
from sounds import SOUNDS
from speech_client import (
    CanaryQwenSpeechBackend,
    OpenAITranscriptionBackend,
    OpenAITtsBackend,
    PocketTtsBackend,
    RealtimeFailure,
    RealtimeTranscript,
    RealtimeTranscriptionSession,
    SpeechBackend,
    SpeechUnavailable,
    TranscriptionBackend,
    TtsBackend,
)

MAX_REQUEST_HISTORY = 1_024
INPUT_QUEUE_CAPACITY = 256
SPEECH_QUEUE_CAPACITY = 32
MAX_UTTERANCE_TIMINGS = 64
MAX_ACTIVE_STREAMS = 8
STT_STREAM_MAX_CHUNKS = 256
STT_STREAM_MAX_CHUNK_B64 = 32_000
STT_STREAM_HELD_TRANSCRIPT_S = 5.0
FLOOR_POST_UTTERANCE_BEAT_SECONDS = 0.4
FLOOR_AUDIO_FAILSAFE_MAX_SECONDS = 45.0
MAX_FLOOR_AWAITING = 32
FLOOR_PLAYER_CHUNK_HOLD_SECONDS = 1.7
FLOOR_PLAYER_ENDPOINT_HOLD_SECONDS = 3.0
FLOOR_PLAYER_TRANSCRIBING_HOLD_SECONDS = 8.0


def _enabled(value: str | None) -> bool:
    return (value or "").strip().lower() in {"1", "true", "yes", "on"}


def _env_float(name: str, default: float, minimum: float, maximum: float) -> float:
    raw = os.environ.get(name)
    if raw is None:
        return default
    try:
        value = float(raw.strip())
    except ValueError:
        return default
    if not math.isfinite(value):
        return default
    return min(max(value, minimum), maximum)


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


def _elapsed_ms(start: float, end: float) -> int:
    return max(0, round((end - start) * 1000))


def _floor_audio_failsafe_seconds(text: str) -> float:
    """Ceiling on synthesis latency plus playback for one voiced utterance.

    Deliberately looser than the reading estimate: it only bounds how long a
    lost ``speech_presented`` acknowledgement can stall the conversation floor.
    """
    return min(8.0 + len(text) / 10.0, FLOOR_AUDIO_FAILSAFE_MAX_SECONDS)


def _speech_reading_seconds(text: str) -> float:
    """Mirror of Bevy's ``speech_text_seconds`` subtitle formula (speech.rs)."""
    return min(max(2.0 + len(text) / 15.0, 3.0), 10.0)


def _wav_duration_seconds(path: Path) -> float | None:
    """Duration of a RIFF WAV by header math alone.

    Player recordings are 32-bit float (format tag 3), which Python's
    ``wave`` module rejects, so the fmt/data chunks are walked by hand.
    """
    try:
        with path.open("rb") as stream:
            header = stream.read(12)
            if len(header) < 12 or header[:4] != b"RIFF" or header[8:12] != b"WAVE":
                return None
            byte_rate = 0
            while True:
                chunk_header = stream.read(8)
                if len(chunk_header) < 8:
                    return None
                chunk_id, chunk_size = struct.unpack("<4sI", chunk_header)
                padded = chunk_size + (chunk_size & 1)
                if chunk_id == b"fmt " and chunk_size >= 16:
                    fmt = stream.read(padded)
                    if len(fmt) < 16:
                        return None
                    _, _, sample_rate, rate, block_align, _ = struct.unpack(
                        "<HHIIHH", fmt[:16]
                    )
                    # block_align is bytes per frame across all channels.
                    byte_rate = rate or sample_rate * block_align
                elif chunk_id == b"data":
                    if byte_rate <= 0:
                        return None
                    return chunk_size / byte_rate
                else:
                    stream.seek(padded, 1)
    except (OSError, struct.error):
        return None


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
    backend: str


@dataclass(frozen=True, slots=True)
class _TtsTask:
    event_id: str
    text: str
    voice_key: str
    wav_path: Path
    backend_name: str
    backend: TtsBackend


@dataclass(frozen=True, slots=True)
class _TtsStreamChunk:
    event_id: str
    chunk_seq: int
    sample_rate: int
    pcm_s16le_base64: str


@dataclass(slots=True)
class _UtteranceTiming:
    """Latency probes for one player utterance, endpoint to applied say."""

    basename: str
    path: str
    endpoint_at: float
    audio_seconds: float | None = None
    commit_at: float | None = None
    completed_at: float | None = None


@dataclass(frozen=True, slots=True)
class _ParkedRecording:
    """A player_recording waiting briefly for its streamed transcript."""

    task: _TranscriptionTask
    stream: _StreamState
    deadline: float


@dataclass(slots=True)
class _StreamState:
    """One streamed player utterance, keyed by its recording basename.

    The stream is presentation-free plumbing: every failure quietly resolves
    through the batch WAV fallback at ``player_recording`` time, so degrade
    paths mark state (once) instead of raising protocol errors per chunk.
    """

    basename: str
    started_at: float
    state: str = "streaming"  # streaming | committed | completed | degraded
    next_seq: int = 0
    decoded_bytes: int = 0
    end_at: float | None = None
    commit_at: float | None = None
    completed_at: float | None = None
    transcript: str | None = None
    degrade_reason: str | None = None
    status_sent: bool = False


class FakeSpeechBackend:
    """Deterministic offline speech backend enabled only by ``--fake``."""

    name = "fake"

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

    def synthesize_stream(
        self,
        text: str,
        voice_key: str,
        on_chunk: Callable[[int, int, str], None],
    ) -> tuple[int, int]:
        sample_rate = 24_000
        sample_count = min(sample_rate // 4, max(800, len(text) * 40))
        pcm = struct.pack(f"<{sample_count}h", *([0] * sample_count))
        on_chunk(0, sample_rate, base64.b64encode(pcm).decode("ascii"))
        return 1, 1


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
        local_stt_backend: TranscriptionBackend | None = None,
        cloud_tts_backend: TtsBackend | None = None,
        local_tts_backend: TtsBackend | None = None,
        tts_backend: str | None = None,
        output: Callable[[str], None] | None = None,
        fake_mode: bool = False,
        turn_delay_seconds: float | None = None,
        clock: Callable[[], float] = time.monotonic,
        realtime_session: RealtimeTranscriptionSession | None = None,
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
        # Sound settings arrive from config.ron via the launcher's environment.
        sounds_enabled = os.environ.get("SMART_ACTORS_SOUNDS_ENABLED")
        if sounds_enabled is not None:
            self.world.sounds_enabled = _enabled(sounds_enabled)
        self.world.view_cone_degrees = _env_float(
            "SMART_ACTORS_VIEW_CONE_DEGREES", self.world.view_cone_degrees, 1.0, 360.0
        )
        self.sound_cooldown_seconds = _env_float(
            "SMART_ACTORS_SOUND_COOLDOWN_SECONDS", 2.0, 0.0, 3_600.0
        )
        self._last_player_sound_at = float("-inf")
        self.fake_mode = fake_mode
        completion_was_injected = llm_complete is not None
        if llm_complete is None:
            llm_complete = fake_llm_complete if fake_mode else llm_client.complete
        if llm_available is None:
            llm_available = (
                completion_was_injected or fake_mode or llm_client.is_available()
            )
        self.llm_available = bool(llm_available)
        speech_backend_was_injected = speech_backend is not None
        self.speech_backend = speech_backend or (
            FakeSpeechBackend() if fake_mode else OpenAITranscriptionBackend()
        )
        if local_stt_backend is not None:
            self.local_stt_backend = local_stt_backend
        elif fake_mode:
            self.local_stt_backend = self.speech_backend
        elif speech_backend_was_injected:
            # Tests and embedders that inject a complete speech service retain
            # their declared capabilities unless they also inject local STT.
            self.local_stt_backend = None
        else:
            self.local_stt_backend = CanaryQwenSpeechBackend()
        if cloud_tts_backend is not None:
            self.cloud_tts_backend = cloud_tts_backend
        elif fake_mode or speech_backend_was_injected:
            self.cloud_tts_backend = self.speech_backend
        else:
            self.cloud_tts_backend = OpenAITtsBackend()
        if local_tts_backend is not None:
            self.local_tts_backend = local_tts_backend
        elif fake_mode:
            self.local_tts_backend = self.speech_backend
        elif speech_backend_was_injected:
            self.local_tts_backend = None
        else:
            self.local_tts_backend = PocketTtsBackend()
        configured_tts = tts_backend
        if configured_tts is None:
            configured_tts = (
                "cloud"
                if speech_backend_was_injected
                else os.environ.get("SMART_ACTORS_TTS_BACKEND", "local")
            )
        configured_tts = configured_tts.strip().lower()
        if configured_tts not in {"cloud", "local", "off"}:
            configured_tts = "off"
            self._tts_startup_message = (
                "Configured NPC voice mode is invalid; voices are off"
            )
        elif configured_tts != "off" and not self._tts_backend_available(
            configured_tts
        ):
            self._tts_startup_message = f"Configured {configured_tts} NPC voice backend is unavailable; voices are off"
            configured_tts = "off"
        else:
            self._tts_startup_message = None
        self.tts_backend = configured_tts
        delay = (
            turn_delay_seconds
            if turn_delay_seconds is not None
            else float(os.environ.get("NPC_TURN_DELAY_SECONDS", "1.0"))
        )
        self._clock = clock
        # Conversation floor: NPC speech holds it until Bevy reports the
        # utterance presented (or an estimate elapses); the scheduler defers
        # applying finished turns while it is held (see _floor_busy).
        self._floor_awaiting: dict[str, float] = {}
        self._floor_until = 0.0
        # Player politeness hold: a rolling deadline bumped while microphone
        # chunks stream in and while an utterance's transcription is in
        # flight. Inherently failsafe — a dead client or hung STT worker
        # simply stops bumping it and it expires on its own, so player state
        # can never freeze NPC turns forever.
        self._player_hold_until = 0.0
        self.scheduler = NpcScheduler(
            self.world,
            llm_complete,
            minimum_delay_seconds=delay,
            clock=clock,
            verbose=_enabled(os.environ.get("SMART_ACTORS_VERBOSE")),
            floor_busy=self._floor_busy,
        )
        self._output = output or self._write_stdout
        self.session_id: str | None = None
        self.event_seq = 0
        self.running = True
        self.handshake_complete = False
        self._seen_message_ids: OrderedDict[str, None] = OrderedDict()
        self._pending_requests: set[str] = set()
        self._pending_recording_paths: dict[str, Path] = {}
        self._completed_requests: OrderedDict[str, dict[str, object]] = OrderedDict()
        self._last_snapshot_revision_sent = -1
        self._pending_tts_paths: set[Path] = set()
        self._generated_audio: dict[tuple[str, str], Path] = {}
        self._tts_stream_chunks: queue.Queue[_TtsStreamChunk] = queue.Queue(maxsize=64)
        self._tts_warm_errors: queue.SimpleQueue[Exception] = queue.SimpleQueue()
        self._utterance_timings: OrderedDict[str, _UtteranceTiming] = OrderedDict()
        self._streams: OrderedDict[str, _StreamState] = OrderedDict()
        self._parked: OrderedDict[str, _ParkedRecording] = OrderedDict()
        try:
            grace_ms = float(
                os.environ.get("STT_STREAM_COMPLETION_GRACE_MS", "").strip() or 2_000.0
            )
        except ValueError:
            grace_ms = 2_000.0
        self._stream_grace_seconds = max(0.2, grace_ms / 1_000.0)
        if realtime_session is not None:
            self._realtime: RealtimeTranscriptionSession | None = realtime_session
        elif fake_mode or speech_backend_was_injected:
            # Fake mode and injected test backends must never open sockets.
            self._realtime = None
        else:
            candidate = RealtimeTranscriptionSession(clock=clock)
            self._realtime = candidate if candidate.available else None
        self._stt_worker = _DaemonWorker(
            "smart-actor-stt", self._transcribe_task, capacity=4
        )
        self._tts_worker = _DaemonWorker(
            "smart-actor-tts", self._synthesize_task, capacity=SPEECH_QUEUE_CAPACITY
        )
        if self.tts_backend == "local":
            threading.Thread(
                target=self._warm_local_tts,
                name="smart-actor-tts-warmup",
                daemon=True,
            ).start()

    @staticmethod
    def _write_stdout(line: str) -> None:
        sys.stdout.write(line + "\n")
        sys.stdout.flush()

    def close(self) -> None:
        self.running = False
        self.scheduler.close()
        if (
            self.local_stt_backend is not None
            and self.local_stt_backend is not self.speech_backend
        ):
            close_local = getattr(self.local_stt_backend, "close", None)
            if callable(close_local):
                close_local()
        closed: set[int] = set()
        for backend in (self.cloud_tts_backend, self.local_tts_backend):
            if backend is None or id(backend) in closed:
                continue
            closed.add(id(backend))
            close_tts = getattr(backend, "close", None)
            if callable(close_tts):
                close_tts()
        self._stt_worker.close()
        self._tts_worker.close()
        if self._realtime is not None:
            self._realtime.close()
        self._streams.clear()
        self._parked.clear()
        self._floor_awaiting.clear()
        self._floor_until = 0.0
        self._player_hold_until = 0.0
        pending_audio = set(self._pending_recording_paths.values())
        pending_audio.update(self._pending_tts_paths)
        for path in pending_audio:
            try:
                path.unlink(missing_ok=True)
            except OSError:
                pass
        self._pending_recording_paths.clear()
        self._pending_tts_paths.clear()
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
        for path in self.runtime_dir.glob("*.wav.part"):
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
            if envelope.message_type == "player_recording":
                self._discard_recording_input(envelope.payload)
            return
        self._seen_message_ids[envelope.message_id] = None
        while len(self._seen_message_ids) > MAX_REQUEST_HISTORY:
            self._seen_message_ids.popitem(last=False)

        try:
            self._dispatch(envelope)
        except (ProtocolError, SpatialUpdateError, ActionError) as error:
            if envelope.message_type == "player_recording":
                # Schema/request validation can fail before the recording
                # handler takes ownership. Do not strand that unconsumed WAV.
                self._discard_recording_input(envelope.payload)
            print(
                f"[smart actors] invalid {envelope.message_type}: {error}",
                file=sys.stderr,
            )
            try:
                rid = request_id(envelope.payload)
            except ProtocolError:
                rid = None
            if rid is not None:
                completed = self._completed_requests.get(rid)
                if completed is not None:
                    self._send("command_result", completed)
                elif rid not in self._pending_requests:
                    self._pending_requests.add(rid)
                    self._finish_request(rid, False, _error_code(error), str(error))
            elif self.handshake_complete:
                self._send_status("protocol", "degraded", message=str(error)[:300])

    def poll(self) -> None:
        if not self.running:
            return
        for status in self.scheduler.poll(self._clock()):
            self._send("status", status.to_payload())
        self._poll_local_stt_status()
        self._poll_local_tts_status()
        self._poll_tts_warm_errors()
        self._poll_streaming()
        self._poll_transcriptions()
        self._poll_tts_stream_chunks()
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
        elif message_type == "player_audio_begin":
            self._handle_player_audio_begin(payload)
        elif message_type == "player_audio_chunk":
            self._handle_player_audio_chunk(payload)
        elif message_type == "player_audio_end":
            self._handle_player_audio_end(payload)
        elif message_type == "player_audio_abort":
            self._handle_player_audio_abort(payload)
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
        elif message_type == "player_sound":
            self._handle_player_sound(payload)
        elif message_type == "debug_sound":
            self._handle_debug_sound(payload)
        elif message_type == "audio_consumed":
            self._handle_audio_consumed(payload)
        elif message_type == "speech_presented":
            self._handle_speech_presented(payload)
        elif message_type == "set_tts_backend":
            self._handle_set_tts_backend(payload)
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
        cloud_stt = bool(self.speech_backend.stt_available)
        local_stt = bool(
            self.local_stt_backend is not None and self.local_stt_backend.stt_available
        )
        cloud_tts = self._tts_backend_available("cloud")
        local_tts = self._tts_backend_available("local")
        self._send(
            "ready",
            {
                "capabilities": {
                    "llm": self.llm_available,
                    "stt": cloud_stt or local_stt,
                    "stt_cloud": cloud_stt,
                    "stt_local": local_stt,
                    "tts": cloud_tts or local_tts,
                    "tts_cloud": cloud_tts,
                    "tts_local": local_tts,
                    "tts_selected": self.tts_backend,
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
        if self._tts_startup_message is not None:
            self._send_status("tts", "unavailable", message=self._tts_startup_message)

    def _tts_backend_for(self, name: str) -> TtsBackend | None:
        if name == "cloud":
            return self.cloud_tts_backend
        if name == "local":
            return self.local_tts_backend
        return None

    def _tts_backend_available(self, name: str) -> bool:
        backend = self._tts_backend_for(name)
        return backend is not None and bool(backend.tts_available)

    def _handle_set_tts_backend(self, payload: Mapping[str, object]) -> None:
        _exact_payload(payload, required={"request_id", "backend"})
        rid = self._begin_request(payload)
        if rid is None:
            return
        backend = payload["backend"]
        if backend not in {"cloud", "local", "off"}:
            self._finish_request(
                rid,
                False,
                "invalid_tts_backend",
                "TTS backend must be cloud, local, or off",
            )
            return
        assert isinstance(backend, str)
        if backend != "off" and not self._tts_backend_available(backend):
            self._finish_request(
                rid,
                False,
                "tts_unavailable",
                f"{backend} NPC voice backend is unavailable",
            )
            return
        self.tts_backend = backend
        if backend == "local":
            threading.Thread(
                target=self._warm_local_tts,
                name="smart-actor-tts-warmup",
                daemon=True,
            ).start()
        self._finish_request(rid, True, "ok", f"NPC voice backend set to {backend}")
        self._send_status("tts", "selected", message=backend, backend=backend)

    def _handle_spatial_update(self, payload: Mapping[str, object]) -> None:
        _exact_payload(payload, required={"spatial_seq", "updates"})
        sequence = _spatial_sequence(payload["spatial_seq"])
        raw_updates = payload["updates"]
        if not isinstance(raw_updates, list) or not raw_updates:
            raise ProtocolError("updates must be a non-empty array", "invalid_position")
        updates: list[tuple[CharIdStr, Vec3, float | None]] = []
        for raw_update in raw_updates:
            if not isinstance(raw_update, Mapping):
                raise ProtocolError(
                    "each spatial update must be an object", "invalid_position"
                )
            _exact_payload(
                raw_update,
                required={"actor_id", "position_m"},
                optional={"facing_yaw"},
            )
            actor_id = CharIdStr(validated_id(raw_update["actor_id"], "actor_id"))
            if actor_id != self.player_id:
                raise ProtocolError(
                    "protocol v1 spatial updates may only move the player",
                    "forbidden_actor",
                )
            facing_yaw = raw_update.get("facing_yaw")
            if facing_yaw is not None and (
                isinstance(facing_yaw, bool)
                or not isinstance(facing_yaw, (int, float))
                or not math.isfinite(float(facing_yaw))
            ):
                raise ProtocolError(
                    "facing_yaw must be a finite number", "invalid_position"
                )
            updates.append(
                (
                    actor_id,
                    Vec3.from_json(raw_update["position_m"]),
                    None if facing_yaw is None else float(facing_yaw),
                )
            )
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

    def _handle_player_sound(self, payload: Mapping[str, object]) -> None:
        """Fire-and-forget deliberate player noise (the F key).

        No ``command_result``: there is no failure the player can act on. The
        confirmation the player does get is the sound event itself, whose
        ``text_for_player`` names the player as the actor.
        """
        _exact_payload(payload, required={"sound_id"})
        sound_id = validated_id(payload["sound_id"], "sound_id")
        sound = SOUNDS.get(sound_id)
        if sound is None or not sound.actor_emittable:
            raise ProtocolError(
                f"there is no player-emittable sound {sound_id!r}", "unknown_sound"
            )
        if not self.world.sounds_enabled:
            return
        now = self._clock()
        if now - self._last_player_sound_at < self.sound_cooldown_seconds:
            # Dropped silently, not queued: percepts are prompt tokens, and
            # holding F must not become a denial-of-service on the LLM bill.
            return
        self._last_player_sound_at = now
        player = self.world.characters[self.player_id]
        line = emit_sound(self.world, player, sound)
        self.world.transcript.append(line)
        self._flush_domain_events()
        self._send_snapshot_if_changed()

    def _handle_debug_sound(self, payload: Mapping[str, object]) -> None:
        """CATHEDRAL_DRIVE stand-in for world causes the sim does not model.

        Nothing in the sim rings the town bell (no clock, no calendar, no
        weather), so drive scripts trigger world sounds directly. World sounds
        have no actor and are never attributed.
        """
        _exact_payload(payload, required={"sound_id", "position_m"})
        sound_id = validated_id(payload["sound_id"], "sound_id")
        sound = SOUNDS.get(sound_id)
        if sound is None:
            raise ProtocolError(f"there is no sound {sound_id!r}", "unknown_sound")
        if not self.world.sounds_enabled:
            return
        position = Vec3.from_json(payload["position_m"])
        line = emit_sound(self.world, None, sound, position_m=position)
        self.world.transcript.append(line)
        self._flush_domain_events()
        self._send_snapshot_if_changed()

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

    def _discard_recording_input(
        self,
        payload: Mapping[str, object],
    ) -> None:
        """Remove an unconsumed WAV without touching any owned audio path."""
        try:
            basename = _safe_basename(payload["wav_basename"])
            wav_path = self._runtime_input_path(basename)
        except (KeyError, ProtocolError):
            return
        if wav_path in self._reserved_audio_paths():
            return
        try:
            wav_path.unlink(missing_ok=True)
        except OSError:
            pass

    def _reserved_audio_paths(self) -> set[Path]:
        reserved = set(self._pending_recording_paths.values())
        reserved.update(self._pending_tts_paths)
        reserved.update(self._generated_audio.values())
        return reserved

    def _degrade_stream(self, stream: _StreamState, reason: str) -> None:
        if stream.state == "completed":
            # The utterance already resolved; late noise cannot un-complete it.
            return
        stream.state = "degraded"
        if stream.degrade_reason is None:
            stream.degrade_reason = reason
        if stream.status_sent:
            return
        stream.status_sent = True
        # Waiting for a session (or its reconnect backoff) is expected and the
        # session reports its own transitions; only genuine stream damage
        # warrants a per-utterance status.
        if reason not in {"no_session", "session_unavailable"}:
            self._send_status(
                "stt",
                "degraded",
                message=f"streamed audio fell back to batch ({reason})",
                backend="cloud",
            )

    def _bump_player_hold(self, seconds: float) -> None:
        """Extend the rolling player-speech hold; a bump never shortens it.

        The explicit releases (silent end, abort, resolved transcription) set
        ``_player_hold_until`` to 0.0 directly; they never call this.
        """
        self._player_hold_until = max(self._player_hold_until, self._clock() + seconds)

    def _handle_player_audio_begin(self, payload: Mapping[str, object]) -> None:
        _exact_payload(payload, required={"wav_basename", "sample_rate", "format"})
        basename = _safe_basename(payload["wav_basename"])
        if self._streams.pop(basename, None) is not None and self._realtime is not None:
            self._realtime.clear(basename)
        while len(self._streams) >= MAX_ACTIVE_STREAMS:
            evicted, _ = self._streams.popitem(last=False)
            if self._realtime is not None:
                self._realtime.clear(evicted)
        stream = _StreamState(basename=basename, started_at=self._clock())
        self._streams[basename] = stream
        # The player has started speaking; hold NPC turns while chunks flow.
        self._bump_player_hold(FLOOR_PLAYER_CHUNK_HOLD_SECONDS)
        sample_rate = payload["sample_rate"]
        if (
            isinstance(sample_rate, bool)
            or sample_rate != 24_000
            or payload["format"] != "pcm_s16le"
        ):
            self._degrade_stream(stream, "bad_format")
            return
        if self.fake_mode:
            return
        if self._realtime is None:
            # No realtime session is configured; the utterance quietly
            # resolves through the batch fallback at player_recording time.
            self._degrade_stream(stream, "no_session")
            return
        if not self._realtime.begin(basename):
            self._degrade_stream(stream, "session_unavailable")

    def _handle_player_audio_chunk(self, payload: Mapping[str, object]) -> None:
        _exact_payload(payload, required={"wav_basename", "seq", "pcm_s16le_base64"})
        basename = _safe_basename(payload["wav_basename"])
        stream = self._streams.get(basename)
        if stream is None:
            # Trailing chunks after an abort or silent end must never
            # resurrect a released player hold.
            return
        # Even chunks bound for a degraded stream mean the player is audibly
        # mid-utterance (the recording still lands via the batch fallback).
        self._bump_player_hold(FLOOR_PLAYER_CHUNK_HOLD_SECONDS)
        if stream.state == "degraded":
            # Rust cannot know a stream degraded; trailing chunks are expected.
            return
        if stream.state != "streaming":
            self._degrade_stream(stream, "chunk_after_end")
            return
        seq = payload["seq"]
        if isinstance(seq, bool) or not isinstance(seq, int) or seq != stream.next_seq:
            self._degrade_stream(stream, "seq_gap")
            return
        if stream.next_seq >= STT_STREAM_MAX_CHUNKS:
            self._degrade_stream(stream, "too_many_chunks")
            return
        encoded = payload["pcm_s16le_base64"]
        if (
            not isinstance(encoded, str)
            or not encoded
            or len(encoded) > STT_STREAM_MAX_CHUNK_B64
        ):
            self._degrade_stream(stream, "oversized_chunk")
            return
        try:
            decoded = base64.b64decode(encoded, validate=True)
        except ValueError:
            self._degrade_stream(stream, "bad_base64")
            return
        if not decoded or len(decoded) % 2:
            self._degrade_stream(stream, "bad_base64")
            return
        stream.next_seq += 1
        stream.decoded_bytes += len(decoded)
        if not self.fake_mode and self._realtime is not None:
            if not self._realtime.append(basename, encoded):
                self._degrade_stream(stream, "backpressure")
                self._realtime.clear(basename)

    def _handle_player_audio_end(self, payload: Mapping[str, object]) -> None:
        _exact_payload(payload, required={"wav_basename", "chunk_count", "silent"})
        basename = _safe_basename(payload["wav_basename"])
        stream = self._streams.get(basename)
        if stream is None:
            return
        silent = payload["silent"]
        if not isinstance(silent, bool):
            self._degrade_stream(stream, "bad_end")
            return
        if silent:
            # The worker discards sub-minimum utterances locally; nothing may
            # ever be committed or said for them.
            self._player_hold_until = 0.0
            del self._streams[basename]
            if not self.fake_mode and self._realtime is not None:
                self._realtime.clear(basename)
            return
        # Endpoint reached: the transcript and the resulting say normally
        # land within this window (player_recording extends it further).
        self._bump_player_hold(FLOOR_PLAYER_ENDPOINT_HOLD_SECONDS)
        if stream.state == "degraded":
            return
        if stream.state != "streaming":
            self._degrade_stream(stream, "bad_end")
            return
        chunk_count = payload["chunk_count"]
        if (
            isinstance(chunk_count, bool)
            or chunk_count != stream.next_seq
            or stream.next_seq == 0
        ):
            self._degrade_stream(stream, "count_mismatch")
            return
        stream.end_at = self._clock()
        if self.fake_mode:
            stream.commit_at = stream.end_at
            try:
                stream.transcript = self.speech_backend.transcribe(
                    self.runtime_dir / basename
                )
            except Exception:
                self._degrade_stream(stream, "fake_transcription_failed")
                return
            stream.state = "completed"
            stream.completed_at = self._clock()
            return
        if self._realtime is None:
            self._degrade_stream(stream, "no_session")
            return
        if self._realtime.commit(basename):
            stream.state = "committed"
            stream.commit_at = stream.end_at
        else:
            self._degrade_stream(stream, "session_unavailable")

    def _handle_player_audio_abort(self, payload: Mapping[str, object]) -> None:
        _exact_payload(payload, required={"wav_basename"})
        basename = _safe_basename(payload["wav_basename"])
        # A parked recording is deliberately untouched: it belongs to an
        # in-flight player_recording whose grace timer owns its resolution.
        if self._streams.pop(basename, None) is not None:
            # The utterance was discarded; nothing will ever be said for it.
            self._player_hold_until = 0.0
            if self._realtime is not None:
                self._realtime.clear(basename)

    def _poll_streaming(self) -> None:
        now = self._clock()
        session = self._realtime
        if session is not None:
            for state, message in session.drain_status():
                self._send_status("stt", state, message=message, backend="cloud")
            for result in session.poll(now):
                self._apply_realtime_result(result)
        overdue = [
            basename
            for basename, parked in self._parked.items()
            if now >= parked.deadline
        ]
        for basename in overdue:
            parked = self._parked.pop(basename)
            if session is not None:
                session.clear(basename)
            self._submit_parked_batch(parked, "grace")
        expired = [
            basename
            for basename, stream in self._streams.items()
            if stream.state == "completed"
            and stream.completed_at is not None
            and now - stream.completed_at > STT_STREAM_HELD_TRANSCRIPT_S
        ]
        for basename in expired:
            # The owning player_recording never arrived (Bevy died or a lost
            # abort); a held transcript must never turn into a late say.
            del self._streams[basename]

    def _apply_realtime_result(
        self, result: RealtimeTranscript | RealtimeFailure
    ) -> None:
        if isinstance(result, RealtimeTranscript):
            parked = self._parked.pop(result.key, None)
            if parked is not None:
                timing = self._utterance_timings.get(result.key)
                if timing is not None:
                    timing.completed_at = self._clock()
                self._handle_transcription_outcome(parked.task, result.text, None)
                return
            stream = self._streams.get(result.key)
            if stream is not None and stream.state == "committed":
                stream.state = "completed"
                stream.transcript = result.text
                stream.completed_at = self._clock()
            # An unknown key is a late completion after fallback: discarded.
            return
        if result.key is None:
            # Session-wide failure: every live streamed utterance falls back.
            for basename in list(self._parked):
                self._submit_parked_batch(self._parked.pop(basename), result.reason)
            for stream in self._streams.values():
                if stream.state in {"streaming", "committed"}:
                    self._degrade_stream(stream, result.reason)
            return
        parked = self._parked.pop(result.key, None)
        if parked is not None:
            self._submit_parked_batch(parked, result.reason)
            return
        stream = self._streams.get(result.key)
        if stream is not None:
            self._degrade_stream(stream, result.reason)

    def _submit_parked_batch(self, parked: _ParkedRecording, reason: str) -> None:
        task = parked.task
        timing = self._utterance_timings.get(task.wav_path.name)
        if timing is not None:
            timing.path = f"batch(fallback:{reason})"
        if not self._stt_worker.submit(task):
            self._handle_transcription_outcome(
                task,
                None,
                SpeechUnavailable("transcription queue is full"),
            )

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
            optional={"stt_backend"},
        )
        rid = self._begin_request(payload)
        if rid is None:
            self._discard_recording_input(payload)
            return
        wav_path: Path | None = None
        owns_wav = False
        try:
            basename = _safe_basename(payload["wav_basename"])
            wav_path = self._runtime_input_path(basename)
            if wav_path in self._reserved_audio_paths():
                raise ProtocolError(
                    "recording WAV is already owned by another audio task",
                    "audio_in_use",
                )
            owns_wav = True
            if payload["target_id"] is not None:
                raise ProtocolError(
                    "player microphone speech must have a null target_id",
                    "invalid_target",
                )
            backend = payload.get("stt_backend", "cloud")
            if backend not in {"cloud", "local"}:
                raise ProtocolError(
                    "stt_backend must be cloud or local", "invalid_stt_backend"
                )
            selected_backend = (
                self.speech_backend if backend == "cloud" else self.local_stt_backend
            )
            if selected_backend is None or not selected_backend.stt_available:
                raise ProtocolError(
                    f"{backend} speech transcription is unavailable", "stt_unavailable"
                )
            self._apply_player_position(payload)
            utterance_position = self.world.characters[self.player_id].position_m
            task = _TranscriptionTask(rid, wav_path, utterance_position, backend)
            stream = self._streams.pop(basename, None)
            if stream is not None and backend != "cloud":
                # The player switched to local transcription mid-utterance;
                # the streamed copy is irrelevant.
                if self._realtime is not None:
                    self._realtime.clear(basename)
                stream = None
            if stream is not None and stream.state == "completed":
                self._pending_recording_paths[rid] = wav_path
                self._begin_utterance_timing(
                    basename,
                    path="stream",
                    audio_seconds=stream.decoded_bytes / 2 / 24_000,
                    endpoint_at=stream.end_at,
                )
                timing = self._utterance_timings.get(basename)
                if timing is not None:
                    timing.commit_at = stream.commit_at
                    timing.completed_at = stream.completed_at
                self._handle_transcription_outcome(task, stream.transcript, None)
                return
            if stream is not None and stream.state == "committed":
                # The provider already holds all the audio; wait briefly for
                # its transcript instead of paying for a batch upload.
                self._parked[basename] = _ParkedRecording(
                    task=task,
                    stream=stream,
                    deadline=self._clock() + self._stream_grace_seconds,
                )
                self._pending_recording_paths[rid] = wav_path
                # The transcript is in flight (and may still fall back to a
                # batch round-trip); keep NPC turns held meanwhile.
                self._bump_player_hold(FLOOR_PLAYER_TRANSCRIBING_HOLD_SECONDS)
                self._begin_utterance_timing(
                    basename,
                    path="stream",
                    audio_seconds=stream.decoded_bytes / 2 / 24_000,
                    endpoint_at=stream.end_at,
                )
                timing = self._utterance_timings.get(basename)
                if timing is not None:
                    timing.commit_at = stream.commit_at
                self._send_status("stt", "transcribing", backend="cloud")
                return
            path = "batch"
            endpoint_at = None
            if stream is not None:
                path = f"batch(fallback:{stream.degrade_reason or 'incomplete_stream'})"
                endpoint_at = stream.end_at
            if not self._stt_worker.submit(task):
                raise ProtocolError("transcription queue is full", "overloaded")
            self._pending_recording_paths[rid] = wav_path
            # Batch STT round-trips can take several seconds; keep NPC turns
            # held until the transcription resolves (or this expires).
            self._bump_player_hold(FLOOR_PLAYER_TRANSCRIBING_HOLD_SECONDS)
            self._begin_utterance_timing(
                basename,
                path=path,
                audio_seconds=_wav_duration_seconds(wav_path),
                endpoint_at=endpoint_at,
            )
        except Exception as error:
            if owns_wav and wav_path is not None:
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
        if task.backend == "local":
            self._send_status(
                "stt",
                "loading",
                message="Loading local Canary-Qwen FP16; first use may download about 5 GB",
                backend="local",
            )
        else:
            self._send_status("stt", "transcribing", backend="cloud")

    def _transcribe_task(self, task: _TranscriptionTask) -> str:
        backend = (
            self.speech_backend if task.backend == "cloud" else self.local_stt_backend
        )
        if backend is None:
            raise SpeechUnavailable(f"{task.backend} transcription is unavailable")
        return backend.transcribe(task.wav_path)

    def _poll_local_stt_status(self) -> None:
        backend = self.local_stt_backend
        if backend is None:
            return
        drain_status = getattr(backend, "drain_status", None)
        if not callable(drain_status):
            return
        for state, message in drain_status():
            self._send_status("stt", state, message=message, backend="local")

    def _poll_local_tts_status(self) -> None:
        backend = self.local_tts_backend
        if backend is None:
            return
        drain_status = getattr(backend, "drain_status", None)
        if not callable(drain_status):
            return
        for state, message in drain_status():
            self._send_status("tts", state, message=message, backend="local")

    def _poll_transcriptions(self) -> None:
        for result in self._stt_worker.drain():
            self._handle_transcription_outcome(result.task, result.value, result.error)

    def _handle_transcription_outcome(
        self,
        task: _TranscriptionTask,
        value: str | None,
        error: Exception | None,
    ) -> None:
        """Resolve one utterance; every transcription path converges here."""
        try:
            self._resolve_transcription(task, value, error)
        finally:
            self._log_utterance_timing(task.wav_path.name)

    def _resolve_transcription(
        self,
        task: _TranscriptionTask,
        value: str | None,
        error: Exception | None,
    ) -> None:
        # Whatever the outcome, the player's utterance is no longer pending:
        # on success the applied say plus the NPC floor govern pacing from
        # here; on failure nothing will be said, so NPC turns may resume.
        self._player_hold_until = 0.0
        self._pending_recording_paths.pop(task.request_id, None)
        try:
            task.wav_path.unlink(missing_ok=True)
        except OSError as unlink_error:
            print(
                f"[smart actors] could not remove recording: {unlink_error}",
                file=sys.stderr,
            )
        if error is not None:
            print(
                f"[smart actors] transcription failed: {type(error).__name__}",
                file=sys.stderr,
            )
            error_message = (
                str(error)[:300]
                if isinstance(error, SpeechUnavailable)
                else "transcription failed"
            )
            self._send(
                "transcription_result",
                {
                    "request_id": task.request_id,
                    "text": None,
                    "error": error_message,
                },
            )
            self._send_status(
                "stt", "degraded", message=error_message, backend=task.backend
            )
            self._finish_request(
                task.request_id,
                False,
                "transcription_failed",
                error_message,
            )
            return
        text = value
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
            self._send_status(
                "stt",
                "idle",
                message="no speech detected",
                backend=task.backend,
            )
            self._finish_request(
                task.request_id, False, "empty_transcription", "no speech detected"
            )
            return
        if len(text) > PLAYER_SPEECH_MAX_CHARS:
            self._send(
                "transcription_result",
                {
                    "request_id": task.request_id,
                    "text": None,
                    "error": "transcription exceeds the 500 character limit",
                },
            )
            self._send_status("stt", "idle", backend=task.backend)
            self._finish_request(
                task.request_id,
                False,
                "text_too_long",
                "transcription exceeds the 500 character limit",
            )
            return
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
            self._send_status("stt", "idle", backend=task.backend)
            self._finish_request(
                task.request_id,
                False,
                "invalid_transcription",
                "transcription contains unsupported characters",
            )
            return
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
        except Exception as action_error:
            self._flush_domain_events()
            self._send_snapshot_if_changed()
            self._send_status("stt", "idle", backend=task.backend)
            self._finish_request(
                task.request_id,
                False,
                _error_code(action_error),
                _safe_message(
                    action_error, "the transcription was rejected by the world"
                ),
            )
            return
        # Being heard should be followed by the earliest possible reaction:
        # the nearest LLM listener takes the next turn without waiting out
        # the round-robin or the inter-turn delay.
        nearest = next(
            (
                character
                for character in self.world.characters_within(
                    task.position_m, HEARING_RADIUS_M, exclude=self.player_id
                )
                if character.control == "llm"
            ),
            None,
        )
        if nearest is not None:
            self.scheduler.prioritize(nearest.id, immediate=True)
        self._flush_domain_events()
        self._send_snapshot_if_changed()
        self._send_status("stt", "idle", backend=task.backend)
        self._finish_request(task.request_id, True, "ok", line)

    def _begin_utterance_timing(
        self,
        basename: str,
        *,
        path: str,
        audio_seconds: float | None,
        endpoint_at: float | None = None,
    ) -> None:
        self._utterance_timings[basename] = _UtteranceTiming(
            basename=basename,
            path=path,
            endpoint_at=self._clock() if endpoint_at is None else endpoint_at,
            audio_seconds=audio_seconds,
        )
        while len(self._utterance_timings) > MAX_UTTERANCE_TIMINGS:
            self._utterance_timings.popitem(last=False)

    def _log_utterance_timing(self, basename: str) -> None:
        timing = self._utterance_timings.pop(basename, None)
        if timing is None:
            return
        now = self._clock()
        audio = (
            f"{timing.audio_seconds:.2f}s" if timing.audio_seconds is not None else "?"
        )
        segments = [f"audio={audio}", f"path={timing.path}"]
        if timing.commit_at is not None:
            segments.append(
                f"endpoint->commit={_elapsed_ms(timing.endpoint_at, timing.commit_at)}ms"
            )
            if timing.completed_at is not None:
                segments.append(
                    "commit->transcript="
                    f"{_elapsed_ms(timing.commit_at, timing.completed_at)}ms"
                )
        if timing.completed_at is not None:
            segments.append(
                f"transcript->say={_elapsed_ms(timing.completed_at, now)}ms"
            )
        segments.append(f"endpoint->say={_elapsed_ms(timing.endpoint_at, now)}ms")
        print(
            f"[smart actors/stt] {basename}: {' '.join(segments)}",
            file=sys.stderr,
        )

    def _warm_local_tts(self) -> None:
        backend = self.local_tts_backend
        warm = getattr(backend, "warm", None)
        if not callable(warm):
            return
        try:
            warm()
        except Exception as error:
            self._tts_warm_errors.put(error)

    def _poll_tts_warm_errors(self) -> None:
        while True:
            try:
                error = self._tts_warm_errors.get_nowait()
            except queue.Empty:
                return
            message = (
                str(error)[:160]
                if isinstance(error, SpeechUnavailable)
                else "local Pocket TTS warmup failed"
            )
            self._send_status("tts", "degraded", message=message, backend="local")

    def _synthesize_task(self, task: _TtsTask) -> tuple[int, int] | None:
        synthesize_stream = getattr(task.backend, "synthesize_stream", None)
        if task.backend_name == "local" and callable(synthesize_stream):
            return synthesize_stream(
                task.text,
                task.voice_key,
                lambda chunk_seq, sample_rate, encoded: self._tts_stream_chunks.put(
                    _TtsStreamChunk(
                        task.event_id,
                        chunk_seq,
                        sample_rate,
                        encoded,
                    )
                ),
            )
        task.backend.synthesize(task.text, task.voice_key, task.wav_path)
        return None

    def _poll_tts_stream_chunks(self) -> None:
        while True:
            try:
                chunk = self._tts_stream_chunks.get_nowait()
            except queue.Empty:
                return
            self._send(
                "tts_chunk",
                {
                    "speech_event_id": chunk.event_id,
                    "chunk_seq": chunk.chunk_seq,
                    "sample_rate": chunk.sample_rate,
                    "channels": 1,
                    "pcm_s16le_base64": chunk.pcm_s16le_base64,
                },
            )

    def _poll_tts(self) -> None:
        for result in self._tts_worker.drain():
            task = result.task
            self._pending_tts_paths.discard(task.wav_path)
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
                if isinstance(result.error, SpeechUnavailable):
                    error_message = str(result.error)[:160]
                elif isinstance(result.error, TimeoutError):
                    error_message = f"{task.backend_name} speech provider timed out"
                elif isinstance(result.error, ValueError):
                    error_message = "NPC speech request was rejected before synthesis"
                else:
                    error_message = (
                        f"{task.backend_name} speech provider failed "
                        f"({type(result.error).__name__})"
                    )
                self._send_status(
                    "tts",
                    "degraded",
                    message=error_message,
                    backend=task.backend_name,
                )
                self._send_tts_failed(task.event_id, error_message)
                continue
            if task.backend_name == "local" and callable(
                getattr(task.backend, "synthesize_stream", None)
            ):
                completion = result.value
                if (
                    not isinstance(completion, tuple)
                    or len(completion) != 2
                    or not all(isinstance(value, int) for value in completion)
                ):
                    self._send_tts_failed(
                        task.event_id,
                        "local streaming synthesis returned no completion",
                    )
                    continue
                chunk_count, first_chunk_ms = completion
                self._send(
                    "tts_stream_end",
                    {
                        "speech_event_id": task.event_id,
                        "chunk_count": chunk_count,
                        "first_chunk_ms": first_chunk_ms,
                    },
                )
                self._send_status(
                    "tts",
                    "idle",
                    message=f"First local PCM in {first_chunk_ms} ms",
                    backend="local",
                )
                continue
            if not task.wav_path.is_file():
                error_message = f"{task.backend_name} synthesis made no WAV"
                self._send_status(
                    "tts",
                    "degraded",
                    message=error_message,
                    backend=task.backend_name,
                )
                self._send_tts_failed(task.event_id, error_message)
                continue
            try:
                if task.wav_path.stat().st_size > 16 * 1024 * 1024:
                    raise ValueError("generated WAV exceeds 16 MiB")
                with wave.open(str(task.wav_path), "rb") as wav:
                    if (
                        wav.getnchannels() not in {1, 2}
                        or wav.getsampwidth() not in {1, 2, 3, 4}
                        or wav.getframerate() < 8_000
                        or wav.getframerate() > 192_000
                        or wav.getnframes() < 1
                    ):
                        raise ValueError("generated WAV has unsupported parameters")
            except (OSError, EOFError, wave.Error, ValueError) as error:
                task.wav_path.unlink(missing_ok=True)
                error_message = str(error)[:160] or "generated WAV is invalid"
                self._send_status(
                    "tts",
                    "degraded",
                    message=error_message,
                    backend=task.backend_name,
                )
                self._send_tts_failed(task.event_id, error_message)
                continue
            basename = task.wav_path.name
            self._generated_audio[(task.event_id, basename)] = task.wav_path
            self._send(
                "tts_ready",
                {"speech_event_id": task.event_id, "wav_basename": basename},
            )
            self._send_status("tts", "idle", backend=task.backend_name)

    def _queue_tts(self, event: DomainEvent) -> bool:
        """Submit synthesis for a heard NPC line.

        Returns True only when a task was actually handed to the TTS worker,
        so callers can tell an awaited audio presentation from a text-only one.
        """
        speaker = self.world.characters.get(event.actor_id)
        if (
            speaker is None
            or speaker.control == "player"
            or speaker.voice_key is None
            or event.text is None
            or self.player_id not in event.recipient_ids
        ):
            return False
        selected = self.tts_backend
        if selected == "off":
            return False
        backend = self._tts_backend_for(selected)
        if backend is None or not backend.tts_available:
            error_message = f"{selected} NPC voice backend is unavailable"
            self._send_status(
                "tts",
                "unavailable",
                message=error_message,
                backend=selected,
            )
            self._send_tts_failed(event.event_id, error_message)
            return False
        basename = f"{event.event_id}.wav"
        wav_path = self.runtime_dir / basename
        if wav_path in self._reserved_audio_paths():
            error_message = "speech output path is already in use"
            self._send_status("tts", "degraded", message=error_message)
            self._send_tts_failed(event.event_id, error_message)
            return False
        task = _TtsTask(
            event.event_id,
            event.text,
            speaker.voice_key,
            wav_path,
            selected,
            backend,
        )
        if not self._tts_worker.submit(task):
            error_message = "speech queue is full"
            self._send_status("tts", "degraded", message=error_message)
            self._send_tts_failed(event.event_id, error_message)
            return False
        self._pending_tts_paths.add(wav_path)
        self._send_status("tts", "synthesizing", actor_id=speaker.id, backend=selected)
        return True

    def _send_tts_failed(self, event_id: str, reason: str) -> None:
        # This synthesis will never be presented as audio; Bevy keeps the text
        # visible for its own reading time, so only the awaited floor entry
        # (if this event ever acquired one) is released here.
        self._release_floor(event_id)
        self._send(
            "tts_failed",
            {"speech_event_id": event_id, "reason": reason[:160]},
        )

    def _floor_busy(self) -> bool:
        """True while a previous utterance is still being presented.

        Speech with queued TTS is awaited per event id until Bevy reports
        ``speech_presented`` (or its failsafe deadline passes); every other
        hold — text-only reading estimates and the post-utterance beat —
        extends ``_floor_until``. The player holds the floor too, through the
        rolling ``_player_hold_until`` deadline bumped while microphone
        chunks stream in and while a transcription is in flight.
        """
        now = self._clock()
        expired = [
            event_id
            for event_id, deadline in self._floor_awaiting.items()
            if deadline <= now
        ]
        for event_id in expired:
            # Failsafe: a lost speech_presented must never stall NPC turns.
            # An overdue presentation gets no post-utterance beat either.
            del self._floor_awaiting[event_id]
        return (
            bool(self._floor_awaiting)
            or now < self._floor_until
            or now < self._player_hold_until
        )

    def _acquire_floor(self, event: DomainEvent, tts_queued: bool) -> None:
        """Hold the next NPC turn application until this line was presented."""
        now = self._clock()
        text = event.text or ""
        if tts_queued:
            while len(self._floor_awaiting) >= MAX_FLOOR_AWAITING:
                # Insertion order matches deadline order, so this only trims
                # an already pathological backlog of unpresented lines.
                del self._floor_awaiting[next(iter(self._floor_awaiting))]
            self._floor_awaiting[event.event_id] = now + _floor_audio_failsafe_seconds(
                text
            )
        else:
            # No audio will ever be presented (player out of earshot, voices
            # off, or synthesis rejected). Pace the conversation at the same
            # reading speed Bevy uses for the on-screen text.
            self._floor_until = max(
                self._floor_until, now + _speech_reading_seconds(text)
            )

    def _release_floor(self, event_id: str) -> None:
        """Release one awaited utterance; late or unknown ids are no-ops."""
        if self._floor_awaiting.pop(event_id, None) is None:
            return
        if not self._floor_awaiting:
            # A short beat between utterances so consecutive voices breathe.
            self._floor_until = max(
                self._floor_until,
                self._clock() + FLOOR_POST_UTTERANCE_BEAT_SECONDS,
            )

    def _handle_speech_presented(self, payload: Mapping[str, object]) -> None:
        _exact_payload(payload, required={"speech_event_id"})
        event_id = validated_id(payload["speech_event_id"], "speech_event_id")
        # Fire-and-forget like spatial_update: no command_result, and ids whose
        # failsafe already expired (or duplicates) are legitimately unknown.
        self._release_floor(event_id)

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
                queued = self._queue_tts(event)
                if speaker.control != "player":
                    self._acquire_floor(event, queued)
            elif event.event_type == "sound":
                self._send_sound_event(event, player)
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

    def _send_sound_event(self, event: DomainEvent, player) -> None:
        """Project one sound event for Bevy and nudge the nearest reactor.

        ``text_for_player`` is the player's percept, rendered here so Bevy
        never decides what the player knows. Fail dark: unless the player
        witnessed the sound (or made it), ``actor_id`` is withheld — an
        unattributed sound must not leak its actor through the wire.
        """
        sound = SOUNDS.get(event.sound_id or "")
        if sound is None or event.position_m is None:
            return
        actor = (
            self.world.characters.get(event.actor_id)
            if event.actor_id is not None
            else None
        )
        player_is_actor = actor is not None and actor.id == player.id
        player_is_witness = player.id in event.witness_ids
        player_is_recipient = player.id in event.recipient_ids
        text_for_player = None
        if player_is_actor:
            # HUD confirmation even with nobody in range, or F feels broken.
            text_for_player = (
                sound.seen.format(actor="You")
                if sound.seen is not None
                else sound.heard
            )
        elif player_is_witness and actor is not None and sound.seen is not None:
            text_for_player = _cap(sound.seen.format(actor=identify(player, actor)))
        elif player_is_recipient:
            text_for_player = sound.heard
        reveal_actor = actor is not None and (player_is_actor or player_is_witness)
        self._send(
            "sound",
            {
                "event_id": event.event_id,
                "sound_id": sound.sound_id,
                "class": sound.sound_class,
                "actor_id": str(actor.id) if reveal_actor and actor else None,
                "position_m": event.position_m.to_json(),
                "audible_distance": sound.audible_distance,
                "recipient_ids": [str(actor_id) for actor_id in event.recipient_ids],
                "witness_ids": [str(actor_id) for actor_id in event.witness_ids],
                "text_for_player": text_for_player,
            },
        )
        # A percept sitting in an inbox does nothing until that actor's next
        # turn; hand the next slot to the nearest witness (falling back to the
        # nearest mere hearer) so the reaction lands promptly. One nudge per
        # sound: the turn stream is global and single.
        for id_group in (event.witness_ids, event.recipient_ids):
            for actor_id in id_group:
                candidate = self.world.characters.get(actor_id)
                if candidate is not None and candidate.control == "llm":
                    self.scheduler.prioritize(candidate.id, immediate=True)
                    return

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
        backend: str | None = None,
    ) -> None:
        payload: dict[str, object] = {"subsystem": subsystem, "state": state}
        if actor_id is not None:
            payload["actor_id"] = str(actor_id)
        if message is not None:
            payload["message"] = message[:300]
        if backend is not None:
            payload["backend"] = backend
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
