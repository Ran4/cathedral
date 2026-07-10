from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from support import MODULE_DIR, decode_output, envelope, hello, wait_until  # noqa: F401

from server import SmartActorServer
from sim import CharIdStr


class TextOnlySpeech:
    stt_available = False
    tts_available = False

    def transcribe(self, wav_path: Path) -> str:
        raise RuntimeError("unused")

    def synthesize(self, text: str, voice_key: str, output_wav: Path) -> None:
        raise RuntimeError("unused")


class FakeEndToEndTests(unittest.TestCase):
    def test_scripted_conversation_offer_accept_and_reoffer(self) -> None:
        with tempfile.TemporaryDirectory() as runtime:
            lines: list[str] = []
            server = SmartActorServer(
                Path(runtime),
                output=lines.append,
                fake_mode=True,
                speech_backend=TextOnlySpeech(),
                turn_delay_seconds=0,
            )
            self.addCleanup(server.close)
            server.handle_envelope(hello())

            server.handle_envelope(
                envelope(
                    "debug_player_say",
                    {
                        "request_id": "ask-name",
                        "text": "What's your name?",
                        "target_id": None,
                        "position_m": {"x": 0, "y": 0.91, "z": 111},
                        "spatial_seq": 1,
                    },
                    message_id="ask-name-msg",
                )
            )
            ilse = server.world.characters[CharIdStr("k0fb1")]
            conny = server.world.characters[CharIdStr("cb947")]
            sven = server.world.characters[CharIdStr("sv3n1")]
            # Open player speech reaches every nearby NPC without a gaze target.
            self.assertTrue(ilse.inbox)
            self.assertTrue(conny.inbox)
            self.assertTrue(sven.inbox)
            wait_until(
                lambda: any(
                    message["type"] == "speech"
                    and message["payload"]["speaker_id"] == "k0fb1"
                    and "My name is Ilse" in message["payload"]["text"]
                    for message in decode_output(lines)
                ),
                server.poll,
            )

            server.handle_envelope(
                envelope(
                    "debug_player_say",
                    {
                        "request_id": "ask-coin",
                        "text": "Please offer me your coin",
                        "target_id": None,
                        "position_m": {"x": 0, "y": 0.91, "z": 111},
                        "spatial_seq": 2,
                    },
                    message_id="ask-coin-msg",
                )
            )
            wait_until(
                lambda: CharIdStr("c0prs") in server.world.offers,
                server.poll,
            )
            offer = server.world.offers[CharIdStr("c0prs")]
            self.assertEqual(offer.giver_id, CharIdStr("k0fb1"))
            self.assertEqual(offer.target_id, CharIdStr("player"))
            # Offering never moved the item.
            self.assertIn(CharIdStr("c0prs"), ilse.holds)
            latest_snapshot = [
                message
                for message in decode_output(lines)
                if message["type"] == "world_snapshot"
            ][-1]["payload"]
            self.assertEqual(latest_snapshot["offers"][-1]["item_id"], "c0prs")

            server.handle_envelope(
                envelope(
                    "player_accept",
                    {
                        "request_id": "accept-coin",
                        "item_id": "c0prs",
                        "position_m": {"x": 0, "y": 0.91, "z": 111},
                        "spatial_seq": 3,
                    },
                    message_id="accept-coin-msg",
                )
            )
            player = server.world.characters[CharIdStr("player")]
            self.assertEqual(player.holds.count(CharIdStr("c0prs")), 1)
            self.assertNotIn(CharIdStr("c0prs"), ilse.holds)
            self.assertNotIn(CharIdStr("c0prs"), server.world.offers)

            server.handle_envelope(
                envelope(
                    "player_offer",
                    {
                        "request_id": "offer-conny",
                        "target_id": "cb947",
                        "item_id": "c0prs",
                        "position_m": {"x": 0, "y": 0.91, "z": 111},
                        "spatial_seq": 4,
                    },
                    message_id="offer-conny-msg",
                )
            )
            self.assertIn(CharIdStr("c0prs"), player.holds)
            self.assertEqual(
                server.world.offers[CharIdStr("c0prs")].target_id,
                CharIdStr("cb947"),
            )
            results = [
                message["payload"]
                for message in decode_output(lines)
                if message["type"] == "command_result"
            ]
            self.assertTrue(all(result["success"] for result in results))


if __name__ == "__main__":
    unittest.main()
