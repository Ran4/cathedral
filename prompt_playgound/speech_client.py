"""OpenAI completed-file speech-to-text and text-to-speech adapter.

Text cognition is configured separately in :mod:`llm_client`. Missing speech
credentials therefore disable only STT/TTS and never prevent the server from
starting.
"""

from __future__ import annotations

import os
from pathlib import Path
from typing import Any, Protocol

try:
    from dotenv import load_dotenv
except ImportError:  # Offline fake backends do not require python-dotenv.

    def load_dotenv(path: Path) -> bool:
        return False


load_dotenv(Path(__file__).resolve().parent / ".env")

DEFAULT_STT_MODEL = "gpt-4o-mini-transcribe"
DEFAULT_TTS_MODEL = "tts-1"
DEFAULT_VOICES = {
    "sven": "onyx",
    "conny": "echo",
    "ilse": "nova",
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


class OpenAISpeechBackend:
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
        output_wav = Path(output_wav)
        if output_wav.suffix.lower() != ".wav":
            raise ValueError("speech output must be a WAV file")
        output_wav.parent.mkdir(parents=True, exist_ok=True)
        voice = self._resolve_voice(voice_key)
        with self._get_client().audio.speech.with_streaming_response.create(
            model=self.tts_model,
            voice=voice,
            input=text,
            response_format="wav",
        ) as response:
            response.stream_to_file(output_wav)
        if not output_wav.is_file():
            raise RuntimeError("speech service did not create a WAV file")

    @staticmethod
    def _resolve_voice(voice_key: str) -> str:
        if not isinstance(voice_key, str) or not voice_key.strip():
            raise ValueError("voice_key must be a non-empty string")
        logical_key = voice_key.strip().lower()
        configured = os.environ.get(f"TTS_VOICE_{logical_key.upper()}", "").strip()
        voice = configured or DEFAULT_VOICES.get(logical_key, logical_key)
        if len(voice) > 64 or not all(ch.isalnum() or ch in "_-" for ch in voice):
            raise ValueError("configured voice contains invalid characters")
        return voice


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
