from __future__ import annotations

import json
import queue
import threading
import unittest
from unittest.mock import patch

from support import MODULE_DIR, wait_until  # noqa: F401

from speech_client import (
    RealtimeFailure,
    RealtimeTranscript,
    RealtimeTranscriptionSession,
)


class FakeTransport:
    """Scripted duplex transport: recv blocks on a queue, sends are recorded."""

    def __init__(self, *, fail_send_after: int | None = None) -> None:
        self.sent: list[dict] = []
        self.incoming: queue.Queue[str | None] = queue.Queue()
        self.closed = threading.Event()
        self._fail_send_after = fail_send_after

    def send(self, text: str) -> None:
        if self._fail_send_after is not None and len(self.sent) >= self._fail_send_after:
            raise ConnectionError("send failed")
        self.sent.append(json.loads(text))

    def recv(self) -> str:
        item = self.incoming.get()
        if item is None:
            raise ConnectionError("socket closed")
        return item

    def close(self) -> None:
        self.closed.set()
        self.incoming.put(None)

    def push(self, event: dict) -> None:
        self.incoming.put(json.dumps(event))

    def sent_types(self) -> list[str]:
        return [message["type"] for message in self.sent]


class FakeFactory:
    def __init__(self, *, fail_connects: int = 0) -> None:
        self.transports: list[FakeTransport] = []
        self.calls = 0
        self.fail_connects = fail_connects

    def __call__(self) -> FakeTransport:
        self.calls += 1
        if self.calls <= self.fail_connects:
            raise ConnectionError("connect refused")
        transport = FakeTransport()
        self.transports.append(transport)
        return transport


class RealtimeSessionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.now = 1_000.0
        self.sessions: list[RealtimeTranscriptionSession] = []

    def tearDown(self) -> None:
        for session in self.sessions:
            session.close()

    def make_session(self, factory, **kwargs) -> RealtimeTranscriptionSession:
        session = RealtimeTranscriptionSession(
            transport_factory=factory,
            clock=lambda: self.now,
            **kwargs,
        )
        self.sessions.append(session)
        return session

    def test_connect_sends_documented_transcription_session_config(self) -> None:
        factory = FakeFactory()
        session = self.make_session(factory)
        self.assertTrue(session.begin("player-recording-1.wav"))
        wait_until(lambda: len(factory.transports) == 1
                   and len(factory.transports[0].sent) >= 2, lambda: None)
        transport = factory.transports[0]
        config = transport.sent[0]
        self.assertEqual(config, session._session_config())
        self.assertEqual(config["type"], "session.update")
        self.assertEqual(config["session"]["type"], "transcription")
        audio_input = config["session"]["audio"]["input"]
        self.assertEqual(audio_input["format"], {"type": "audio/pcm", "rate": 24_000})
        self.assertEqual(
            audio_input["transcription"]["model"], "gpt-realtime-whisper"
        )
        self.assertIsNone(audio_input["turn_detection"])
        # A fresh utterance always starts from an empty provider buffer.
        self.assertEqual(transport.sent[1], {"type": "input_audio_buffer.clear"})

    def test_api_key_never_appears_in_statuses(self) -> None:
        secret = "sk-test-very-secret-value"

        def leaky_factory() -> FakeTransport:
            raise ConnectionError(f"handshake rejected for bearer {secret}")

        with patch.dict("os.environ", {"OPENAI_API_KEY": secret}):
            session = self.make_session(leaky_factory)
            self.assertTrue(session.begin("player-recording-1.wav"))
            wait_until(
                lambda: any(
                    state == "degraded" for state, _ in self._peek_statuses(session)
                ),
                lambda: None,
            )
        for _, message in self._collected_statuses:
            self.assertNotIn(secret, message)

    def _peek_statuses(self, session: RealtimeTranscriptionSession):
        drained = session.drain_status()
        if not hasattr(self, "_collected_statuses"):
            self._collected_statuses: list[tuple[str, str]] = []
        self._collected_statuses.extend(drained)
        return self._collected_statuses

    def test_commit_ack_binds_item_ids_and_out_of_order_completions_resolve(
        self,
    ) -> None:
        factory = FakeFactory()
        session = self.make_session(factory)
        self.assertTrue(session.begin("a.wav"))
        self.assertTrue(session.append("a.wav", "QUJD"))
        self.assertTrue(session.commit("a.wav"))
        self.assertTrue(session.begin("b.wav"))
        self.assertTrue(session.commit("b.wav"))
        wait_until(
            lambda: factory.transports
            and factory.transports[0].sent_types().count("input_audio_buffer.commit")
            == 2,
            lambda: None,
        )
        transport = factory.transports[0]
        transport.push({"type": "input_audio_buffer.committed", "item_id": "item-a"})
        transport.push({"type": "input_audio_buffer.committed", "item_id": "item-b"})
        transport.push(
            {
                "type": "conversation.item.input_audio_transcription.completed",
                "item_id": "item-b",
                "transcript": "second utterance",
            }
        )
        transport.push(
            {
                "type": "conversation.item.input_audio_transcription.completed",
                "item_id": "item-a",
                "transcript": "first utterance",
            }
        )
        results: list = []
        wait_until(
            lambda: len(results) == 2,
            lambda: results.extend(session.poll(self.now)),
        )
        self.assertEqual(results[0], RealtimeTranscript("b.wav", "second utterance"))
        self.assertEqual(results[1], RealtimeTranscript("a.wav", "first utterance"))

    def test_socket_death_fails_pending_keys_and_next_begin_reconnects(self) -> None:
        factory = FakeFactory()
        session = self.make_session(factory)
        self.assertTrue(session.begin("a.wav"))
        self.assertTrue(session.commit("a.wav"))
        wait_until(
            lambda: factory.transports
            and "input_audio_buffer.commit" in factory.transports[0].sent_types(),
            lambda: None,
        )
        factory.transports[0].push(
            {"type": "input_audio_buffer.committed", "item_id": "item-a"}
        )
        # Kill the socket: the reader must fail the bound key exactly once.
        factory.transports[0].incoming.put(None)
        results: list = []
        wait_until(
            lambda: bool(results),
            lambda: results.extend(session.poll(self.now)),
        )
        self.assertEqual(results, [RealtimeFailure("a.wav", "socket")])

        self.assertTrue(session.begin("b.wav"))
        wait_until(lambda: factory.calls == 2, lambda: None)
        wait_until(lambda: len(factory.transports[1].sent) >= 2, lambda: None)

    def test_connect_failure_backs_off_without_a_retry_storm(self) -> None:
        factory = FakeFactory(fail_connects=100)
        session = self.make_session(factory)
        self.assertTrue(session.begin("a.wav"))
        results: list = []
        wait_until(
            lambda: bool(results),
            lambda: results.extend(session.poll(self.now)),
        )
        self.assertEqual(results, [RealtimeFailure("a.wav", "connect_failed")])
        self.assertEqual(factory.calls, 1)

        # Still inside the backoff window: rejected without touching the net.
        self.assertFalse(session.begin("b.wav"))
        self.assertEqual(factory.calls, 1)

        self.now += 60.0
        self.assertTrue(session.begin("c.wav"))
        wait_until(lambda: factory.calls == 2, lambda: None)

    def test_in_flight_cap_rejects_the_newest_commit(self) -> None:
        factory = FakeFactory()
        session = self.make_session(factory, max_in_flight=2)
        for key in ("a.wav", "b.wav"):
            self.assertTrue(session.begin(key))
            self.assertTrue(session.commit(key))
        self.assertTrue(session.begin("c.wav"))
        self.assertFalse(session.commit("c.wav"))

    def test_clear_forgets_the_active_key_and_blocks_its_commit(self) -> None:
        factory = FakeFactory()
        session = self.make_session(factory)
        self.assertTrue(session.begin("a.wav"))
        self.assertTrue(session.append("a.wav", "QUJD"))
        session.clear("a.wav")
        self.assertFalse(session.commit("a.wav"))
        self.assertFalse(session.append("a.wav", "QUJD"))
        wait_until(
            lambda: factory.transports
            and factory.transports[0].sent_types().count("input_audio_buffer.clear")
            >= 2,
            lambda: None,
        )
        self.assertNotIn(
            "input_audio_buffer.commit", factory.transports[0].sent_types()
        )

    def test_idle_timeout_closes_and_next_begin_reconnects_quietly(self) -> None:
        factory = FakeFactory()
        with patch.dict("os.environ", {"STT_STREAM_IDLE_CLOSE_S": "5"}):
            session = self.make_session(factory)
        self.assertTrue(session.begin("a.wav"))
        self.assertTrue(session.commit("a.wav"))
        wait_until(
            lambda: factory.transports
            and "input_audio_buffer.commit" in factory.transports[0].sent_types(),
            lambda: None,
        )
        transport = factory.transports[0]
        transport.push({"type": "input_audio_buffer.committed", "item_id": "item-a"})
        transport.push(
            {
                "type": "conversation.item.input_audio_transcription.completed",
                "item_id": "item-a",
                "transcript": "done",
            }
        )
        results: list = []
        wait_until(
            lambda: bool(results),
            lambda: results.extend(session.poll(self.now)),
        )

        self.now += 6.0
        session.poll(self.now)
        wait_until(lambda: transport.closed.is_set(), lambda: None)
        # Deliberate idle close never surfaces failures.
        self.assertEqual(session.poll(self.now), [])

        self.assertTrue(session.begin("b.wav"))
        wait_until(lambda: factory.calls == 2, lambda: None)

    def test_close_is_bounded_with_a_wedged_transport(self) -> None:
        import time as real_time

        class WedgedTransport(FakeTransport):
            def close(self) -> None:  # never unblocks recv, never returns fast
                real_time.sleep(5.0)

        wedged = WedgedTransport()
        session = self.make_session(lambda: wedged)
        self.assertTrue(session.begin("a.wav"))
        wait_until(lambda: len(wedged.sent) >= 2, lambda: None)

        started = real_time.monotonic()
        session.close()
        self.assertLess(real_time.monotonic() - started, 2.0)


if __name__ == "__main__":
    unittest.main()
