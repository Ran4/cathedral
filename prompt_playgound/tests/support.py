from __future__ import annotations

import json
import sys
import time
from pathlib import Path
from typing import Callable

MODULE_DIR = Path(__file__).resolve().parents[1]
if str(MODULE_DIR) not in sys.path:
    sys.path.insert(0, str(MODULE_DIR))


def decode_output(lines: list[str]) -> list[dict]:
    return [json.loads(line) for line in lines]


def wait_until(
    predicate: Callable[[], bool],
    poll: Callable[[], None],
    timeout: float = 2.0,
) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        poll()
        if predicate():
            return
        time.sleep(0.002)
    raise AssertionError("condition did not become true before timeout")


def envelope(
    message_type: str,
    payload: dict,
    *,
    message_id: str,
    session_id: str = "test-session",
) -> dict:
    return {
        "protocol_version": 1,
        "session_id": session_id,
        "message_id": message_id,
        "type": message_type,
        "payload": payload,
    }


def hello(position: dict | None = None, spatial_seq: int = 0) -> dict:
    return envelope(
        "hello",
        {
            "supported_protocol_version": 1,
            "player_id": "player",
            "position_m": position or {"x": 0.0, "y": 0.91, "z": 111.0},
            "spatial_seq": spatial_seq,
        },
        message_id="hello-1",
    )
