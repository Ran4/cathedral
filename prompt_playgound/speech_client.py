"""Independent cloud/local speech backends, including streaming local TTS.

Text cognition is configured separately in :mod:`llm_client`. Missing speech
credentials therefore disable only their cloud STT/TTS providers and never
prevent the server or local model workers from starting.
"""

from __future__ import annotations

import json
import os
import queue
import subprocess
import sys
import threading
import time
from collections import deque
from dataclasses import dataclass
from pathlib import Path
from collections.abc import Callable
from typing import Any, Protocol

try:
    from dotenv import load_dotenv
except ImportError:  # Offline fake backends do not require python-dotenv.

    def load_dotenv(path: Path) -> bool:
        return False


load_dotenv(Path(__file__).resolve().parent / ".env")

DEFAULT_STT_MODEL = "gpt-4o-transcribe"
DEFAULT_LOCAL_STT_MODEL = "nvidia/canary-qwen-2.5b"
DEFAULT_REALTIME_STT_MODEL = "gpt-realtime-whisper"
DEFAULT_REALTIME_URL = "wss://api.openai.com/v1/realtime?intent=transcription"
DEFAULT_TTS_MODEL = "tts-1"
DEFAULT_OPENAI_VOICES = {
    "sven": "onyx",
    "conny": "echo",
    "ilse": "nova",
}
LOGICAL_NPC_VOICES = frozenset(DEFAULT_OPENAI_VOICES)
DEFAULT_KOKORO_VOICES = {
    # These names are present in Kokoro-82M v1.0's published VOICES.md.
    "sven": "am_michael",
    "conny": "am_fenrir",
    "ilse": "af_heart",
}


class SpeechUnavailable(RuntimeError):
    pass


class SpeechBackend(Protocol):
    @property
    def stt_available(self) -> bool: ...

    @property
    def tts_available(self) -> bool: ...

    def transcribe(self, wav_path: Path) -> str: ...

    def synthesize(self, text: str, voice_key: str, output_wav: Path) -> None: ...


class TranscriptionBackend(Protocol):
    @property
    def stt_available(self) -> bool: ...

    def transcribe(self, wav_path: Path) -> str: ...


class TtsBackend(Protocol):
    name: str

    @property
    def tts_available(self) -> bool: ...

    def synthesize(self, text: str, voice_key: str, output_wav: Path) -> None: ...


def _validate_tts_request(text: str, output_wav: Path) -> Path:
    _validate_tts_text(text)
    output_wav = Path(output_wav)
    if output_wav.suffix.lower() != ".wav":
        raise ValueError("speech output must be a WAV file")
    return output_wav


def _validate_tts_text(text: str) -> None:
    if not isinstance(text, str) or not text.strip():
        raise ValueError("speech text must not be empty")
    if len(text) > 500:
        raise ValueError("speech text exceeds the 500 character limit")
    try:
        text.encode("utf-8")
    except UnicodeEncodeError as error:
        raise ValueError("speech text contains invalid Unicode") from error
    if any(
        (ord(character) < 0x20 and character not in "\n\t")
        or 0x7F <= ord(character) <= 0x9F
        for character in text
    ):
        raise ValueError("speech text contains control characters")
    return None


def _resolve_voice(
    voice_key: str,
    *,
    provider: str,
    defaults: dict[str, str],
) -> str:
    if not isinstance(voice_key, str) or not voice_key.strip():
        raise ValueError("voice_key must be a non-empty string")
    logical_key = voice_key.strip().lower()
    if logical_key not in defaults:
        raise ValueError("unknown logical NPC voice")
    qualified = os.environ.get(
        f"TTS_{provider.upper()}_VOICE_{logical_key.upper()}", ""
    ).strip()
    # Preserve the original cloud override while preferring provider-qualified
    # names so adding providers cannot accidentally cross-wire their voices.
    legacy = (
        os.environ.get(f"TTS_VOICE_{logical_key.upper()}", "").strip()
        if provider == "openai"
        else ""
    )
    voice = qualified or legacy or defaults[logical_key]
    if len(voice) > 64 or not all(ch.isalnum() or ch in "_-" for ch in voice):
        raise ValueError("configured voice contains invalid characters")
    return voice


class OpenAISpeechBackend:
    name = "cloud"

    def __init__(self, client: Any | None = None) -> None:
        self._api_key = os.environ.get("OPENAI_API_KEY", "").strip()
        self._client = client
        self.stt_model = os.environ.get("STT_MODEL", "").strip() or DEFAULT_STT_MODEL
        self.tts_model = os.environ.get("TTS_MODEL", "").strip() or DEFAULT_TTS_MODEL

    @property
    def stt_available(self) -> bool:
        return self._client is not None or bool(self._api_key)

    @property
    def tts_available(self) -> bool:
        return self._client is not None or bool(self._api_key)

    def _get_client(self) -> Any:
        if not self.stt_available:
            raise SpeechUnavailable(
                "OPENAI_API_KEY is not configured for speech services"
            )
        if self._client is None:
            from openai import OpenAI

            timeout = float(os.environ.get("SPEECH_TIMEOUT_SECONDS", "30"))
            self._client = OpenAI(api_key=self._api_key, timeout=timeout, max_retries=1)
        return self._client

    def transcribe(self, wav_path: Path) -> str:
        wav_path = Path(wav_path)
        if wav_path.suffix.lower() != ".wav" or not wav_path.is_file():
            raise ValueError("transcription input must be an existing WAV file")
        with wav_path.open("rb") as audio_file:
            response = self._get_client().audio.transcriptions.create(
                model=self.stt_model,
                file=audio_file,
            )
        text = getattr(response, "text", response)
        if not isinstance(text, str):
            raise RuntimeError("transcription service returned no text")
        return text

    def synthesize(self, text: str, voice_key: str, output_wav: Path) -> None:
        output_wav = _validate_tts_request(text, output_wav)
        output_wav.parent.mkdir(parents=True, exist_ok=True)
        voice = self._resolve_voice(voice_key)
        temporary = output_wav.with_suffix(".wav.part")
        temporary.unlink(missing_ok=True)
        try:
            with self._get_client().audio.speech.with_streaming_response.create(
                model=self.tts_model,
                voice=voice,
                input=text,
                response_format="wav",
            ) as response:
                response.stream_to_file(temporary)
            if not temporary.is_file():
                raise RuntimeError("speech service did not create a WAV file")
            temporary.replace(output_wav)
        except Exception:
            temporary.unlink(missing_ok=True)
            raise

    @staticmethod
    def _resolve_voice(voice_key: str) -> str:
        return _resolve_voice(
            voice_key, provider="openai", defaults=DEFAULT_OPENAI_VOICES
        )


class OpenAITranscriptionBackend:
    """Cloud transcription provider, configured independently from TTS."""

    def __init__(self, client: Any | None = None) -> None:
        self._service = OpenAISpeechBackend(client=client)

    @property
    def stt_available(self) -> bool:
        return self._service.stt_available

    def transcribe(self, wav_path: Path) -> str:
        return self._service.transcribe(wav_path)


class OpenAITtsBackend:
    """Cloud synthesis provider, configured independently from STT."""

    name = "cloud"

    def __init__(self, client: Any | None = None) -> None:
        self._service = OpenAISpeechBackend(client=client)

    @property
    def tts_available(self) -> bool:
        return self._service.tts_available

    def synthesize(self, text: str, voice_key: str, output_wav: Path) -> None:
        self._service.synthesize(text, voice_key, output_wav)


class PocketTtsBackend:
    """Persistent CPU-streaming Pocket TTS worker."""

    name = "local"

    def __init__(
        self,
        *,
        worker_script: Path | None = None,
        uv_binary: str | None = None,
        python_version: str | None = None,
    ) -> None:
        self.worker_script = (
            Path(worker_script)
            if worker_script is not None
            else Path(__file__).resolve().with_name("pocket_tts_worker.py")
        )
        self.uv_binary = (
            uv_binary or os.environ.get("SMART_ACTORS_UV_BINARY", "").strip() or "uv"
        )
        self.python_version = (
            python_version or os.environ.get("LOCAL_TTS_PYTHON", "").strip() or "3.12"
        )
        self._process: subprocess.Popen[str] | None = None
        self._next_request = 0
        self._lock = threading.Lock()
        self._statuses: queue.Queue[tuple[str, str]] = queue.Queue(maxsize=8)

    @property
    def tts_available(self) -> bool:
        return self.worker_script.is_file() and bool(self.uv_binary)

    def warm(self) -> None:
        with self._lock:
            self._ensure_process()

    def synthesize_stream(
        self,
        text: str,
        voice_key: str,
        on_chunk: Callable[[int, int, str], None],
    ) -> tuple[int, int]:
        _validate_tts_text(text)
        logical_voice = voice_key.strip().lower()
        if logical_voice not in LOGICAL_NPC_VOICES:
            raise ValueError("unknown logical NPC voice")
        with self._lock:
            process = self._ensure_process()
            self._next_request += 1
            request_id = self._next_request
            self._publish_status("synthesizing", "Streaming with local Pocket TTS")
            assert process.stdin is not None
            try:
                process.stdin.write(
                    json.dumps(
                        {
                            "request_id": request_id,
                            "text": text,
                            "voice_key": logical_voice,
                        },
                        separators=(",", ":"),
                    )
                    + "\n"
                )
                process.stdin.flush()
            except (BrokenPipeError, OSError) as error:
                self._forget_process(process)
                raise SpeechUnavailable("local Pocket TTS worker stopped") from error

            expected_seq = 0
            while True:
                try:
                    response = self._read_message(process)
                except SpeechUnavailable:
                    self._forget_process(process)
                    raise
                if response.get("request_id") != request_id:
                    self._forget_process(process)
                    raise SpeechUnavailable("local Pocket TTS returned an invalid response")
                message_type = response.get("type")
                if message_type == "chunk":
                    chunk_seq = response.get("chunk_seq")
                    sample_rate = response.get("sample_rate")
                    encoded = response.get("pcm_s16le_base64")
                    if (
                        chunk_seq != expected_seq
                        or isinstance(sample_rate, bool)
                        or not isinstance(sample_rate, int)
                        or not 8_000 <= sample_rate <= 48_000
                        or not isinstance(encoded, str)
                        or not encoded
                        or len(encoded) > 256_000
                    ):
                        self._forget_process(process)
                        raise SpeechUnavailable(
                            "local Pocket TTS returned an invalid audio chunk"
                        )
                    on_chunk(chunk_seq, sample_rate, encoded)
                    expected_seq += 1
                    continue
                if message_type == "result":
                    chunk_count = response.get("chunk_count")
                    first_chunk_ms = response.get("first_chunk_ms")
                    if (
                        chunk_count != expected_seq
                        or expected_seq == 0
                        or isinstance(first_chunk_ms, bool)
                        or not isinstance(first_chunk_ms, int)
                        or first_chunk_ms < 0
                    ):
                        raise SpeechUnavailable(
                            "local Pocket TTS returned an invalid completion"
                        )
                    return expected_seq, first_chunk_ms
                message = response.get("error")
                raise SpeechUnavailable(
                    str(message)[:160]
                    if isinstance(message, str) and message.strip()
                    else "local Pocket TTS synthesis failed"
                )

    def close(self) -> None:
        process = self._process
        self._process = None
        if process is None or process.poll() is not None:
            return
        process.terminate()
        try:
            process.wait(timeout=1)
        except subprocess.TimeoutExpired:
            process.kill()

    def drain_status(self) -> list[tuple[str, str]]:
        statuses = []
        while True:
            try:
                statuses.append(self._statuses.get_nowait())
            except queue.Empty:
                return statuses

    def _ensure_process(self) -> subprocess.Popen[str]:
        process = self._process
        if process is not None and process.poll() is None:
            return process
        if not self.tts_available:
            raise SpeechUnavailable("local Pocket TTS worker script is unavailable")
        self._publish_status(
            "loading", "Preparing local dependencies, model, and Pocket TTS voices"
        )
        try:
            process = subprocess.Popen(
                [
                    self.uv_binary,
                    "run",
                    "--python",
                    self.python_version,
                    "--script",
                    str(self.worker_script),
                ],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                bufsize=1,
                env=os.environ.copy(),
            )
        except OSError as error:
            raise SpeechUnavailable(
                "could not start local Pocket TTS; make sure uv is available"
            ) from error
        self._process = process
        assert process.stderr is not None
        threading.Thread(
            target=self._forward_stderr,
            args=(process.stderr,),
            name="pocket-tts-log-reader",
            daemon=True,
        ).start()
        try:
            response = self._read_message(process)
        except SpeechUnavailable:
            self._forget_process(process)
            raise
        if response.get("type") != "ready":
            self._forget_process(process)
            error = response.get("error")
            raise SpeechUnavailable(
                str(error)[:160]
                if isinstance(error, str) and error.strip()
                else "local Pocket TTS failed to load"
            )
        self._publish_status("ready", "Local Pocket TTS is loaded and streaming-ready")
        return process

    @staticmethod
    def _read_message(process: subprocess.Popen[str]) -> dict[str, object]:
        assert process.stdout is not None
        line = process.stdout.readline()
        if not line:
            raise SpeechUnavailable("local Pocket TTS worker exited; check the actor log")
        try:
            message = json.loads(line)
        except json.JSONDecodeError as error:
            raise SpeechUnavailable("local Pocket TTS returned invalid JSON") from error
        if not isinstance(message, dict):
            raise SpeechUnavailable("local Pocket TTS returned an invalid response")
        return message

    def _forget_process(self, process: subprocess.Popen[str]) -> None:
        if self._process is process:
            self._process = None
        if process.poll() is None:
            process.terminate()

    def _publish_status(self, state: str, message: str) -> None:
        try:
            self._statuses.put_nowait((state, message[:160]))
        except queue.Full:
            try:
                self._statuses.get_nowait()
            except queue.Empty:
                pass
            try:
                self._statuses.put_nowait((state, message[:160]))
            except queue.Full:
                pass

    def _forward_stderr(self, stderr: Any) -> None:
        for line in stderr:
            sys.stderr.write(line)
            sys.stderr.flush()


class KokoroTtsBackend:
    """Persistent local Kokoro-82M worker, started lazily on first synthesis."""

    name = "local"

    def __init__(
        self,
        runtime_dir: Path,
        *,
        worker_script: Path | None = None,
        uv_binary: str | None = None,
        python_version: str | None = None,
    ) -> None:
        self.runtime_dir = Path(runtime_dir).resolve()
        self.worker_script = (
            Path(worker_script)
            if worker_script is not None
            else Path(__file__).resolve().with_name("kokoro_worker.py")
        )
        self.uv_binary = (
            uv_binary or os.environ.get("SMART_ACTORS_UV_BINARY", "").strip() or "uv"
        )
        self.python_version = (
            python_version or os.environ.get("LOCAL_TTS_PYTHON", "").strip() or "3.12"
        )
        self._process: subprocess.Popen[str] | None = None
        self._next_request = 0
        self._lock = threading.Lock()
        self._statuses: queue.Queue[tuple[str, str]] = queue.Queue(maxsize=8)

    @property
    def tts_available(self) -> bool:
        return (
            self.runtime_dir.is_dir()
            and self.worker_script.is_file()
            and bool(self.uv_binary)
        )

    def synthesize(self, text: str, voice_key: str, output_wav: Path) -> None:
        output_wav = _validate_tts_request(text, output_wav)
        try:
            output_wav = output_wav.resolve(strict=False)
        except OSError as error:
            raise ValueError("speech output path is invalid") from error
        if output_wav.parent != self.runtime_dir:
            raise ValueError(
                "local speech output must stay inside the runtime directory"
            )
        voice = _resolve_voice(
            voice_key, provider="kokoro", defaults=DEFAULT_KOKORO_VOICES
        )
        with self._lock:
            process = self._ensure_process()
            self._next_request += 1
            request_id = self._next_request
            self._publish_status("synthesizing", "Synthesizing with local Kokoro-82M")
            assert process.stdin is not None
            try:
                process.stdin.write(
                    json.dumps(
                        {
                            "request_id": request_id,
                            "text": text,
                            "voice": voice,
                            "wav_basename": output_wav.name,
                        },
                        separators=(",", ":"),
                    )
                    + "\n"
                )
                process.stdin.flush()
            except (BrokenPipeError, OSError) as error:
                self._forget_process(process)
                raise SpeechUnavailable("local Kokoro worker stopped") from error
            try:
                response = self._read_message(process)
            except SpeechUnavailable:
                self._forget_process(process)
                raise
            if response.get("request_id") != request_id:
                self._forget_process(process)
                raise SpeechUnavailable("local Kokoro returned an invalid response")
            if response.get("type") != "result":
                message = response.get("error")
                raise SpeechUnavailable(
                    str(message)[:160]
                    if isinstance(message, str) and message.strip()
                    else "local Kokoro synthesis failed"
                )
            if (
                response.get("wav_basename") != output_wav.name
                or not output_wav.is_file()
            ):
                raise SpeechUnavailable("local Kokoro did not create the requested WAV")

    def close(self) -> None:
        process = self._process
        self._process = None
        if process is None or process.poll() is not None:
            return
        process.terminate()
        try:
            process.wait(timeout=1)
        except subprocess.TimeoutExpired:
            process.kill()

    def drain_status(self) -> list[tuple[str, str]]:
        statuses = []
        while True:
            try:
                statuses.append(self._statuses.get_nowait())
            except queue.Empty:
                return statuses

    def _ensure_process(self) -> subprocess.Popen[str]:
        process = self._process
        if process is not None and process.poll() is None:
            return process
        if not self.tts_available:
            raise SpeechUnavailable("local Kokoro worker script is unavailable")
        self._publish_status(
            "loading", "Preparing local dependencies and Kokoro-82M model"
        )
        try:
            process = subprocess.Popen(
                [
                    self.uv_binary,
                    "run",
                    "--python",
                    self.python_version,
                    "--script",
                    str(self.worker_script),
                ],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                bufsize=1,
                env={
                    **os.environ,
                    "KOKORO_RUNTIME_DIR": str(self.runtime_dir),
                },
            )
        except OSError as error:
            raise SpeechUnavailable(
                "could not start local Kokoro; make sure uv is available"
            ) from error
        self._process = process
        assert process.stderr is not None
        threading.Thread(
            target=self._forward_stderr,
            args=(process.stderr,),
            name="kokoro-log-reader",
            daemon=True,
        ).start()
        try:
            response = self._read_message(process)
        except SpeechUnavailable:
            self._forget_process(process)
            raise
        if response.get("type") != "ready":
            self._forget_process(process)
            error = response.get("error")
            raise SpeechUnavailable(
                str(error)[:160]
                if isinstance(error, str) and error.strip()
                else "local Kokoro failed to load"
            )
        self._publish_status("ready", "Local Kokoro-82M is loaded")
        return process

    @staticmethod
    def _read_message(process: subprocess.Popen[str]) -> dict[str, object]:
        assert process.stdout is not None
        line = process.stdout.readline()
        if not line:
            raise SpeechUnavailable("local Kokoro worker exited; check the actor log")
        try:
            message = json.loads(line)
        except json.JSONDecodeError as error:
            raise SpeechUnavailable(
                "local Kokoro returned an invalid response"
            ) from error
        if not isinstance(message, dict):
            raise SpeechUnavailable("local Kokoro returned an invalid response")
        return message

    def _forget_process(self, process: subprocess.Popen[str]) -> None:
        if self._process is process:
            self._process = None
        if process.poll() is None:
            process.terminate()

    def _publish_status(self, state: str, message: str) -> None:
        try:
            self._statuses.put_nowait((state, message[:160]))
        except queue.Full:
            try:
                self._statuses.get_nowait()
            except queue.Empty:
                pass
            try:
                self._statuses.put_nowait((state, message[:160]))
            except queue.Full:
                pass

    def _forward_stderr(self, stderr: Any) -> None:
        for line in stderr:
            sys.stderr.write(line)
            sys.stderr.flush()
            detail = " ".join(line.strip().split())
            if detail.startswith(("Downloading ", "Building ", "Installed ")):
                self._publish_status("loading", detail)


class CanaryQwenSpeechBackend:
    """Persistent local Canary-Qwen worker, started lazily on first use.

    NeMo's GPU dependency tree lives in a separate PEP-723 script so choosing
    cloud transcription does not install or import it. The worker keeps the
    FP16 model resident after its first transcription.
    """

    def __init__(
        self,
        *,
        worker_script: Path | None = None,
        uv_binary: str | None = None,
        python_version: str | None = None,
        torch_index: str | None = None,
        model: str | None = None,
    ) -> None:
        self.worker_script = (
            Path(worker_script)
            if worker_script is not None
            else Path(__file__).resolve().with_name("canary_qwen_worker.py")
        )
        self.uv_binary = (
            uv_binary or os.environ.get("SMART_ACTORS_UV_BINARY", "").strip() or "uv"
        )
        self.python_version = (
            python_version or os.environ.get("LOCAL_STT_PYTHON", "").strip() or "3.12"
        )
        self.torch_index = (
            torch_index
            or os.environ.get("LOCAL_STT_TORCH_INDEX", "").strip()
            or "https://download.pytorch.org/whl/cu124"
        )
        self.model = (
            model
            or os.environ.get("LOCAL_STT_MODEL", "").strip()
            or DEFAULT_LOCAL_STT_MODEL
        )
        self._process: subprocess.Popen[str] | None = None
        self._next_request = 0
        self._lock = threading.Lock()
        self._statuses: queue.Queue[tuple[str, str]] = queue.Queue(maxsize=1)

    @property
    def stt_available(self) -> bool:
        return self.worker_script.is_file() and bool(self.uv_binary)

    def transcribe(self, wav_path: Path) -> str:
        wav_path = Path(wav_path)
        if wav_path.suffix.lower() != ".wav" or not wav_path.is_file():
            raise ValueError("transcription input must be an existing WAV file")
        with self._lock:
            process = self._ensure_process()
            self._publish_status(
                "transcribing", "Transcribing with local Canary-Qwen FP16"
            )
            self._next_request += 1
            request_id = self._next_request
            assert process.stdin is not None
            try:
                process.stdin.write(
                    json.dumps(
                        {"request_id": request_id, "wav_path": str(wav_path.resolve())},
                        separators=(",", ":"),
                    )
                    + "\n"
                )
                process.stdin.flush()
            except (BrokenPipeError, OSError) as error:
                self._forget_process(process)
                raise SpeechUnavailable(
                    "local Canary-Qwen worker stopped; press Z to use cloud transcription"
                ) from error
            try:
                response = self._read_message(process)
            except SpeechUnavailable:
                self._forget_process(process)
                raise
            if response.get("request_id") != request_id:
                self._forget_process(process)
                raise SpeechUnavailable(
                    "local Canary-Qwen returned an invalid response"
                )
            text = response.get("text")
            if response.get("type") == "result" and isinstance(text, str):
                return text
            raise SpeechUnavailable(
                str(response.get("error") or "local Canary-Qwen transcription failed")
            )

    def close(self) -> None:
        # Do not wait for the request lock: app shutdown must also be able to
        # interrupt a worker that is downloading/loading the model.
        process = self._process
        self._process = None
        if process is None or process.poll() is not None:
            return
        process.terminate()
        try:
            process.wait(timeout=1)
        except subprocess.TimeoutExpired:
            process.kill()

    def _ensure_process(self) -> subprocess.Popen[str]:
        process = self._process
        if process is not None and process.poll() is None:
            return process
        if not self.stt_available:
            raise SpeechUnavailable("local Canary-Qwen worker script is unavailable")
        try:
            process = subprocess.Popen(
                [
                    self.uv_binary,
                    "run",
                    "--python",
                    self.python_version,
                    "--resolution",
                    "highest",
                    "--index",
                    self.torch_index,
                    "--index-strategy",
                    "unsafe-best-match",
                    "--script",
                    str(self.worker_script),
                ],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                bufsize=1,
                env={**os.environ, "LOCAL_STT_MODEL": self.model},
            )
        except OSError as error:
            raise SpeechUnavailable(
                "could not start local Canary-Qwen; make sure uv is available"
            ) from error
        self._process = process
        assert process.stderr is not None
        threading.Thread(
            target=self._forward_stderr,
            args=(process.stderr,),
            name="canary-qwen-log-reader",
            daemon=True,
        ).start()
        self._publish_status(
            "loading", "Preparing local dependencies and Canary-Qwen FP16"
        )
        try:
            response = self._read_message(process)
        except SpeechUnavailable:
            self._forget_process(process)
            raise
        if response.get("type") != "ready":
            self._forget_process(process)
            raise SpeechUnavailable(
                str(
                    response.get("error")
                    or "local Canary-Qwen failed to load; check CUDA and available VRAM"
                )
            )
        self._publish_status("ready", "Local Canary-Qwen FP16 is loaded")
        return process

    @staticmethod
    def _read_message(process: subprocess.Popen[str]) -> dict[str, object]:
        assert process.stdout is not None
        line = process.stdout.readline()
        if not line:
            raise SpeechUnavailable(
                "local Canary-Qwen worker exited; check the smart-actor log"
            )
        try:
            message = json.loads(line)
        except json.JSONDecodeError as error:
            raise SpeechUnavailable(
                "local Canary-Qwen returned an invalid response"
            ) from error
        if not isinstance(message, dict):
            raise SpeechUnavailable("local Canary-Qwen returned an invalid response")
        return message

    def _forget_process(self, process: subprocess.Popen[str]) -> None:
        if self._process is process:
            self._process = None
        if process.poll() is None:
            process.terminate()

    def drain_status(self) -> list[tuple[str, str]]:
        statuses = []
        while True:
            try:
                statuses.append(self._statuses.get_nowait())
            except queue.Empty:
                return statuses

    def _publish_status(self, state: str, message: str) -> None:
        while True:
            try:
                self._statuses.get_nowait()
            except queue.Empty:
                break
        try:
            self._statuses.put_nowait((state, message[:160]))
        except queue.Full:
            pass

    def _forward_stderr(self, stderr: Any) -> None:
        for line in stderr:
            sys.stderr.write(line)
            sys.stderr.flush()
            detail = " ".join(line.strip().split())
            if detail.startswith(("Downloading ", "Building ", "Installed ")):
                self._publish_status("loading", detail)


@dataclass(frozen=True, slots=True)
class RealtimeTranscript:
    """Final transcript for one streamed utterance key."""

    key: str
    text: str


@dataclass(frozen=True, slots=True)
class RealtimeFailure:
    """A streamed utterance (or, with ``key=None``, the whole session) that
    must fall back to batch transcription."""

    key: str | None
    reason: str


def _default_realtime_transport() -> Any:
    # Imported lazily: fake mode and the offline suite must never require
    # the websocket package or open sockets.
    import websocket

    api_key = os.environ.get("OPENAI_API_KEY", "").strip()
    if not api_key:
        raise SpeechUnavailable(
            "OPENAI_API_KEY is not configured for realtime transcription"
        )
    url = os.environ.get("STT_REALTIME_URL", "").strip() or DEFAULT_REALTIME_URL
    connection = websocket.create_connection(
        url,
        header=[f"Authorization: Bearer {api_key}"],
        timeout=30,
        enable_multithread=True,
    )

    class _Transport:
        """Normalizes the recv contract: '' means idle, exceptions mean dead."""

        def send(self, text: str) -> None:
            connection.send(text)

        def recv(self) -> str:
            try:
                received = connection.recv()
            except websocket.WebSocketTimeoutException:
                return ""
            return received if isinstance(received, str) else ""

        def close(self) -> None:
            try:
                connection.close()
            except Exception:
                pass

    return _Transport()


def _scrubbed_reason(error: BaseException, fallback: str) -> str:
    message = f"{type(error).__name__}: {str(error)[:120]}".strip(": ")
    api_key = os.environ.get("OPENAI_API_KEY", "").strip()
    if api_key and api_key in message:
        message = message.replace(api_key, "***")
    return message or fallback


class RealtimeTranscriptionSession:
    """One warm realtime transcription websocket shared by all utterances.

    ``begin``/``append``/``commit``/``clear`` only enqueue bounded work for a
    writer thread and can never block the protocol loop; a reader thread turns
    provider events into :class:`RealtimeTranscript`/:class:`RealtimeFailure`
    values drained by ``poll``. Completions are matched strictly by the
    ``item_id`` bound at the commit acknowledgement, never by arrival order.
    """

    def __init__(
        self,
        *,
        transport_factory: Callable[[], Any] | None = None,
        clock: Callable[[], float] = time.monotonic,
        max_in_flight: int = 4,
    ) -> None:
        self._transport_factory = transport_factory
        self._clock = clock
        self.max_in_flight = max_in_flight
        self.model = (
            os.environ.get("STT_REALTIME_MODEL", "").strip()
            or DEFAULT_REALTIME_STT_MODEL
        )
        self.delay = os.environ.get("STT_REALTIME_DELAY", "").strip() or "low"
        self.language = os.environ.get("STT_LANGUAGE", "").strip()
        try:
            self.idle_close_seconds = float(
                os.environ.get("STT_STREAM_IDLE_CLOSE_S", "").strip() or 300.0
            )
        except ValueError:
            self.idle_close_seconds = 300.0
        self._lock = threading.Lock()
        self._closing = threading.Event()
        self._actions: queue.Queue[tuple[str, str | None] | None] = queue.Queue(
            maxsize=512
        )
        self._results: queue.SimpleQueue[RealtimeTranscript | RealtimeFailure] = (
            queue.SimpleQueue()
        )
        self._statuses: queue.Queue[tuple[str, str]] = queue.Queue(maxsize=8)
        self._writer: threading.Thread | None = None
        self._reader: threading.Thread | None = None
        self._transport: Any | None = None
        self._active_key: str | None = None
        self._pending_commits: deque[str] = deque()
        self._items: dict[str, str] = {}
        self._failures = 0
        self._retry_at = 0.0
        self._last_used = 0.0

    @property
    def available(self) -> bool:
        return self._transport_factory is not None or bool(
            os.environ.get("OPENAI_API_KEY", "").strip()
        )

    def _session_config(self) -> dict[str, object]:
        """Single source of the session.update shape (documentation-volatile;
        adjust here and in its pinned unit test only)."""
        transcription: dict[str, object] = {"model": self.model, "delay": self.delay}
        if self.language:
            transcription["language"] = self.language
        return {
            "type": "session.update",
            "session": {
                "type": "transcription",
                "audio": {
                    "input": {
                        "format": {"type": "audio/pcm", "rate": 24_000},
                        "transcription": transcription,
                        "turn_detection": None,
                    }
                },
            },
        }

    def begin(self, key: str) -> bool:
        now = self._clock()
        with self._lock:
            if self._closing.is_set():
                return False
            if self._transport is None and now < self._retry_at:
                # A recent connect failure is still backing off; the caller
                # falls straight back to batch without touching the queue.
                return False
            self._active_key = key
            self._last_used = now
        # A fresh utterance always starts from an empty provider buffer.
        return self._enqueue_json({"type": "input_audio_buffer.clear"})

    def append(self, key: str, pcm_s16le_base64: str) -> bool:
        with self._lock:
            if self._closing.is_set() or self._active_key != key:
                return False
        return self._enqueue_json(
            {"type": "input_audio_buffer.append", "audio": pcm_s16le_base64}
        )

    def commit(self, key: str) -> bool:
        with self._lock:
            if self._closing.is_set() or self._active_key != key:
                return False
            self._active_key = None
            if len(self._pending_commits) + len(self._items) >= self.max_in_flight:
                return False
            self._pending_commits.append(key)
            self._last_used = self._clock()
        if self._enqueue_json({"type": "input_audio_buffer.commit"}):
            return True
        with self._lock:
            try:
                self._pending_commits.remove(key)
            except ValueError:
                pass
        return False

    def clear(self, key: str) -> None:
        with self._lock:
            was_active = self._active_key == key
            if was_active:
                self._active_key = None
            try:
                self._pending_commits.remove(key)
            except ValueError:
                pass
            for item_id, item_key in list(self._items.items()):
                if item_key == key:
                    del self._items[item_id]
        if was_active:
            self._enqueue_json({"type": "input_audio_buffer.clear"})

    def poll(self, now: float) -> list[RealtimeTranscript | RealtimeFailure]:
        results: list[RealtimeTranscript | RealtimeFailure] = []
        while True:
            try:
                results.append(self._results.get_nowait())
            except queue.Empty:
                break
        with self._lock:
            close_idle = (
                self._transport is not None
                and self._active_key is None
                and not self._pending_commits
                and not self._items
                and now - self._last_used > self.idle_close_seconds
            )
        if close_idle:
            try:
                self._actions.put_nowait(("close_idle", None))
            except queue.Full:
                pass
        return results

    def drain_status(self) -> list[tuple[str, str]]:
        statuses = []
        while True:
            try:
                statuses.append(self._statuses.get_nowait())
            except queue.Empty:
                return statuses

    def close(self) -> None:
        self._closing.set()
        try:
            self._actions.put_nowait(("close", None))
        except queue.Full:
            pass
        self._close_transport()
        writer = self._writer
        if writer is not None:
            writer.join(timeout=0.5)
        reader = self._reader
        if reader is not None:
            reader.join(timeout=0.5)

    def _enqueue_json(self, message: dict[str, object]) -> bool:
        self._ensure_writer()
        try:
            self._actions.put_nowait(
                ("send", json.dumps(message, separators=(",", ":")))
            )
        except queue.Full:
            return False
        return True

    def _ensure_writer(self) -> None:
        with self._lock:
            if self._writer is None:
                self._writer = threading.Thread(
                    target=self._writer_loop,
                    name="realtime-stt-writer",
                    daemon=True,
                )
                self._writer.start()

    def _writer_loop(self) -> None:
        while True:
            action = self._actions.get()
            if action is None:
                return
            kind, payload = action
            if kind == "close":
                self._close_transport()
                return
            if kind == "close_idle":
                self._close_transport()
                continue
            transport = self._ensure_connected()
            if transport is None:
                self._fail_pending("connect_failed")
                continue
            try:
                transport.send(payload)
            except Exception:
                self._close_transport()
                self._fail_pending("socket")

    def _ensure_connected(self) -> Any | None:
        with self._lock:
            if self._transport is not None:
                return self._transport
            if self._clock() < self._retry_at:
                return None
        self._publish_status("loading", "Connecting realtime transcription session")
        factory = self._transport_factory or _default_realtime_transport
        try:
            transport = factory()
            transport.send(json.dumps(self._session_config(), separators=(",", ":")))
        except Exception as error:
            with self._lock:
                self._failures += 1
                self._retry_at = self._clock() + min(30.0, 2.0**self._failures)
            self._publish_status(
                "degraded",
                _scrubbed_reason(error, "realtime transcription connect failed"),
            )
            return None
        with self._lock:
            self._transport = transport
            self._failures = 0
        reader = threading.Thread(
            target=self._reader_loop,
            args=(transport,),
            name="realtime-stt-reader",
            daemon=True,
        )
        self._reader = reader
        reader.start()
        self._publish_status("ready", "Realtime transcription session connected")
        return transport

    def _reader_loop(self, transport: Any) -> None:
        while not self._closing.is_set():
            with self._lock:
                if self._transport is not transport:
                    return
            try:
                raw = transport.recv()
            except Exception:
                break
            if not raw:
                continue
            self._handle_provider_event(raw)
        with self._lock:
            stale = self._transport is not transport
            if not stale:
                self._transport = None
        if not stale and not self._closing.is_set():
            self._fail_pending("socket")

    def _handle_provider_event(self, raw: str) -> None:
        try:
            event = json.loads(raw)
        except (json.JSONDecodeError, ValueError):
            return
        if not isinstance(event, dict):
            return
        event_type = event.get("type")
        if event_type == "input_audio_buffer.committed":
            item_id = event.get("item_id")
            with self._lock:
                if not self._pending_commits:
                    return
                key = self._pending_commits.popleft()
                if isinstance(item_id, str) and item_id:
                    self._items[item_id] = key
                    key = None
            if key is not None:
                self._results.put(RealtimeFailure(key, "protocol"))
        elif event_type == "conversation.item.input_audio_transcription.completed":
            item_id = event.get("item_id")
            transcript = event.get("transcript")
            with self._lock:
                key = self._items.pop(item_id, None) if isinstance(item_id, str) else None
            if key is None:
                # Unknown or already-abandoned item: a late completion after
                # batch fallback is discarded here, never a second say.
                return
            if isinstance(transcript, str):
                self._results.put(RealtimeTranscript(key, transcript[:2_000]))
            else:
                self._results.put(RealtimeFailure(key, "protocol"))
        elif event_type == "error":
            detail = event.get("error")
            message = (
                detail.get("message") if isinstance(detail, dict) else None
            ) or "realtime transcription provider error"
            self._publish_status("degraded", str(message)[:160])

    def _fail_pending(self, reason: str) -> None:
        with self._lock:
            keys: list[str] = []
            if self._active_key is not None:
                keys.append(self._active_key)
                self._active_key = None
            keys.extend(self._pending_commits)
            self._pending_commits.clear()
            keys.extend(self._items.values())
            self._items.clear()
        unique = list(dict.fromkeys(keys))
        for key in unique:
            self._results.put(RealtimeFailure(key, reason))
        if unique:
            self._publish_status(
                "degraded",
                f"realtime transcription dropped ({reason}); using batch fallback",
            )

    def _close_transport(self) -> None:
        with self._lock:
            transport, self._transport = self._transport, None
        if transport is None:
            return

        def _close() -> None:
            try:
                transport.close()
            except Exception:
                pass

        # Detached: a provider close that wedges must never hold up the
        # protocol loop or app shutdown.
        threading.Thread(target=_close, name="realtime-stt-closer", daemon=True).start()

    def _publish_status(self, state: str, message: str) -> None:
        try:
            self._statuses.put_nowait((state, message[:160]))
        except queue.Full:
            try:
                self._statuses.get_nowait()
            except queue.Empty:
                pass
            try:
                self._statuses.put_nowait((state, message[:160]))
            except queue.Full:
                pass


_backend: SpeechBackend | None = None


def backend() -> SpeechBackend:
    global _backend
    if _backend is None:
        _backend = OpenAISpeechBackend()
    return _backend


def transcribe(wav_path: Path) -> str:
    return backend().transcribe(wav_path)


def synthesize(text: str, voice_key: str, output_wav: Path) -> None:
    backend().synthesize(text, voice_key, output_wav)


def capabilities() -> tuple[bool, bool]:
    service = backend()
    return service.stt_available, service.tts_available
