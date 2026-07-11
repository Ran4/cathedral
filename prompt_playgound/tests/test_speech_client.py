from __future__ import annotations

import io
import json
import os
import sys
import tempfile
import unittest
from contextlib import nullcontext
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

from support import MODULE_DIR  # noqa: F401

import canary_qwen_worker
from speech_client import (
    CanaryQwenSpeechBackend,
    OpenAISpeechBackend,
    SpeechUnavailable,
)


class FakeStreamingResponse:
    def __init__(
        self, calls: list[dict], kwargs: dict, *, error: Exception | None = None
    ):
        self.calls = calls
        self.kwargs = kwargs
        self.error = error

    def __enter__(self):
        if self.error is not None:
            raise self.error
        self.calls.append(self.kwargs)
        return self

    def __exit__(self, exc_type, exc, traceback):
        return False

    def stream_to_file(self, path: Path) -> None:
        Path(path).write_bytes(b"RIFF-sdk-fake")


class FakeClient:
    def __init__(self, *, transcript: str = "hello", error: Exception | None = None):
        self.transcription_calls: list[dict] = []
        self.speech_calls: list[dict] = []
        self.transcript = transcript
        self.error = error
        self.audio = SimpleNamespace(
            transcriptions=SimpleNamespace(create=self._transcribe),
            speech=SimpleNamespace(
                with_streaming_response=SimpleNamespace(create=self._synthesize)
            ),
        )

    def _transcribe(self, **kwargs):
        if self.error is not None:
            raise self.error
        self.transcription_calls.append(kwargs)
        return SimpleNamespace(text=self.transcript)

    def _synthesize(self, **kwargs):
        return FakeStreamingResponse(self.speech_calls, kwargs, error=self.error)


class FakeCanaryWorker:
    def __init__(self) -> None:
        self.stdin = io.StringIO()
        self.stdout = io.StringIO(
            '\n'.join(
                [
                    json.dumps({"type": "ready"}),
                    json.dumps(
                        {"type": "result", "request_id": 1, "text": "first local"}
                    ),
                    json.dumps(
                        {"type": "result", "request_id": 2, "text": "second local"}
                    ),
                    "",
                ]
            )
        )
        self.stderr = io.StringIO()
        self.terminated = False

    def poll(self):
        return 0 if self.terminated else None

    def terminate(self) -> None:
        self.terminated = True

    def wait(self, timeout=None) -> int:
        return 0

    def kill(self) -> None:
        self.terminated = True


class SpeechAdapterTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.wav = Path(self.temp.name) / "input.wav"
        self.wav.write_bytes(b"RIFF")

    def test_transcribe_uses_configured_completed_file_endpoint(self) -> None:
        client = FakeClient(transcript="understood")
        with patch.dict(os.environ, {"STT_MODEL": "test-transcribe"}, clear=False):
            backend = OpenAISpeechBackend(client=client)
        self.assertEqual(backend.transcribe(self.wav), "understood")
        self.assertEqual(client.transcription_calls[0]["model"], "test-transcribe")
        self.assertTrue(client.transcription_calls[0]["file"].closed)

    def test_transcription_defaults_to_high_accuracy_model(self) -> None:
        client = FakeClient(transcript="understood")
        with patch.dict(os.environ, {"STT_MODEL": ""}, clear=False):
            backend = OpenAISpeechBackend(client=client)
        self.assertEqual(backend.transcribe(self.wav), "understood")
        self.assertEqual(client.transcription_calls[0]["model"], "gpt-4o-transcribe")

    def test_canary_worker_is_lazy_and_reused_between_transcriptions(self) -> None:
        worker = FakeCanaryWorker()
        worker_script = Path(self.temp.name) / "canary.py"
        worker_script.touch()
        with patch("speech_client.subprocess.Popen", return_value=worker) as popen:
            backend = CanaryQwenSpeechBackend(
                worker_script=worker_script,
                uv_binary="test-uv",
            )
            self.assertEqual(backend.transcribe(self.wav), "first local")
            self.assertEqual(backend.transcribe(self.wav), "second local")
            backend.close()

        popen.assert_called_once()
        self.assertEqual(
            popen.call_args.args[0][:11],
            [
                "test-uv",
                "run",
                "--python",
                "3.12",
                "--resolution",
                "highest",
                "--index",
                "https://download.pytorch.org/whl/cu124",
                "--index-strategy",
                "unsafe-best-match",
                "--script",
            ],
        )
        requests = [json.loads(line) for line in worker.stdin.getvalue().splitlines()]
        self.assertEqual([request["request_id"] for request in requests], [1, 2])
        self.assertTrue(worker.terminated)

    def test_canary_transcription_passes_explicit_mono_audio_to_salm(self) -> None:
        audio = object()
        audio_lens = object()

        class FakeAnswer:
            def cpu(self):
                return self

        class FakeModel:
            audio_locator_tag = "<audio>"
            tokenizer = SimpleNamespace(ids_to_text=lambda _answer: "understood")

            def __init__(self):
                self.calls = []

            def generate(self, **kwargs):
                self.calls.append(kwargs)
                return [FakeAnswer()]

        model = FakeModel()
        fake_torch = SimpleNamespace(inference_mode=nullcontext)
        with (
            patch.dict(sys.modules, {"torch": fake_torch}),
            patch.object(
                canary_qwen_worker,
                "load_mono_audio",
                return_value=(audio, audio_lens),
            ) as load_audio,
        ):
            text = canary_qwen_worker.transcribe(model, self.wav)

        self.assertEqual(text, "understood")
        load_audio.assert_called_once_with(model, self.wav)
        self.assertEqual(len(model.calls), 1)
        self.assertIs(model.calls[0]["audios"], audio)
        self.assertIs(model.calls[0]["audio_lens"], audio_lens)
        self.assertNotIn("audio", model.calls[0]["prompts"][0][0])

    def test_synthesis_requests_wav_and_distinct_default_voice(self) -> None:
        client = FakeClient()
        output = Path(self.temp.name) / "voice.wav"
        with patch.dict(
            os.environ,
            {"TTS_MODEL": "tts-1", "TTS_VOICE_ILSE": ""},
            clear=False,
        ):
            backend = OpenAISpeechBackend(client=client)
            backend.synthesize("Greetings", "ilse", output)
        self.assertTrue(output.exists())
        call = client.speech_calls[0]
        self.assertEqual(call["model"], "tts-1")
        self.assertEqual(call["voice"], "nova")
        self.assertEqual(call["response_format"], "wav")

    def test_configured_voice_is_used_without_real_person_imitation_logic(self) -> None:
        client = FakeClient()
        output = Path(self.temp.name) / "voice.wav"
        with patch.dict(os.environ, {"TTS_VOICE_SVEN": "alloy"}, clear=False):
            OpenAISpeechBackend(client=client).synthesize("Hello", "sven", output)
        self.assertEqual(client.speech_calls[0]["voice"], "alloy")

    def test_missing_credentials_are_independently_unavailable(self) -> None:
        with patch.dict(os.environ, {"OPENAI_API_KEY": ""}, clear=False):
            backend = OpenAISpeechBackend()
        self.assertFalse(backend.stt_available)
        self.assertFalse(backend.tts_available)
        with self.assertRaises(SpeechUnavailable):
            backend.transcribe(self.wav)

    def test_timeout_and_provider_failure_propagate_to_service_boundary(self) -> None:
        for error in (TimeoutError("slow"), RuntimeError("failed")):
            with self.subTest(error=type(error).__name__):
                backend = OpenAISpeechBackend(client=FakeClient(error=error))
                with self.assertRaises(type(error)):
                    backend.transcribe(self.wav)
                with self.assertRaises(type(error)):
                    backend.synthesize(
                        "Hello", "conny", Path(self.temp.name) / "out.wav"
                    )

    def test_invalid_files_text_and_voice_are_rejected_locally(self) -> None:
        backend = OpenAISpeechBackend(client=FakeClient())
        with self.assertRaises(ValueError):
            backend.transcribe(Path(self.temp.name) / "missing.wav")
        with self.assertRaises(ValueError):
            backend.synthesize("", "ilse", Path(self.temp.name) / "out.wav")
        with self.assertRaises(ValueError):
            backend.synthesize("bad\0speech", "ilse", Path(self.temp.name) / "out.wav")
        with self.assertRaises(ValueError):
            backend.synthesize(
                "bad\ud800speech", "ilse", Path(self.temp.name) / "out.wav"
            )
        with patch.dict(os.environ, {"TTS_VOICE_ILSE": "../../bad"}, clear=False):
            with self.assertRaises(ValueError):
                backend.synthesize("Hello", "ilse", Path(self.temp.name) / "out.wav")


if __name__ == "__main__":
    unittest.main()
