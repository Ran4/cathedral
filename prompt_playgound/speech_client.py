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
