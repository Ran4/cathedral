"""Versioned JSON-lines protocol primitives for the Rust/Python bridge."""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping

PROTOCOL_VERSION = 1
MAX_PROTOCOL_LINE_CHARS = 1_000_000
MAX_ID_CHARS = 128


class ProtocolError(ValueError):
    def __init__(
        self, message: str, code: str = "malformed_message", *, fatal: bool = False
    ) -> None:
        super().__init__(message)
        self.code = code
        self.fatal = fatal


@dataclass(frozen=True, slots=True)
class IncomingEnvelope:
    protocol_version: int
    session_id: str
    message_id: str
    message_type: str
    payload: dict[str, Any]


def _reject_nonfinite(token: str) -> None:
    raise ProtocolError(
        f"non-finite JSON number {token} is not allowed", "invalid_number"
    )


def _validate_json_shape(
    value: object, *, max_depth: int = 64, max_nodes: int = 50_000
) -> None:
    stack = [(value, 0)]
    nodes = 0
    while stack:
        current, depth = stack.pop()
        nodes += 1
        if depth > max_depth or nodes > max_nodes:
            raise ProtocolError("JSON structure is too deeply nested or large")
        if isinstance(current, dict):
            stack.extend((child, depth + 1) for child in current.values())
        elif isinstance(current, list):
            stack.extend((child, depth + 1) for child in current)


def parse_line(line: str) -> IncomingEnvelope:
    if not isinstance(line, str):
        raise ProtocolError("protocol input must be UTF-8 text")
    if len(line) > MAX_PROTOCOL_LINE_CHARS:
        raise ProtocolError("protocol message exceeds size limit", "message_too_large")
    try:
        value = json.loads(line, parse_constant=_reject_nonfinite)
    except ProtocolError:
        raise
    except (json.JSONDecodeError, UnicodeError, RecursionError) as error:
        raise ProtocolError(f"invalid JSON: {error}") from error
    _validate_json_shape(value)
    return parse_envelope(value)


def parse_envelope(value: object) -> IncomingEnvelope:
    if not isinstance(value, Mapping):
        raise ProtocolError("protocol envelope must be a JSON object")
    required = {"protocol_version", "session_id", "message_id", "type", "payload"}
    missing = required - set(value)
    unknown = set(value) - required
    if missing:
        raise ProtocolError(f"missing envelope field: {sorted(missing)[0]}")
    if unknown:
        raise ProtocolError(f"unknown envelope field: {sorted(unknown)[0]}")
    version = value["protocol_version"]
    if isinstance(version, bool) or not isinstance(version, int):
        raise ProtocolError(
            "protocol_version must be an integer", "unsupported_version", fatal=True
        )
    if version != PROTOCOL_VERSION:
        raise ProtocolError(
            f"unsupported protocol version {version}", "unsupported_version", fatal=True
        )
    session_id = validated_id(value["session_id"], "session_id")
    message_id = validated_id(value["message_id"], "message_id")
    message_type = value["type"]
    if not isinstance(message_type, str) or not message_type or len(message_type) > 64:
        raise ProtocolError("type must be a non-empty string of at most 64 characters")
    if any(ord(character) < 0x20 for character in message_type):
        raise ProtocolError("type contains control characters")
    payload = value["payload"]
    if not isinstance(payload, dict):
        raise ProtocolError("payload must be a JSON object")
    return IncomingEnvelope(version, session_id, message_id, message_type, payload)


def validated_id(value: object, name: str) -> str:
    if not isinstance(value, str) or not value or len(value) > MAX_ID_CHARS:
        raise ProtocolError(
            f"{name} must be a non-empty string of at most {MAX_ID_CHARS} characters"
        )
    if any(ord(character) < 0x20 for character in value):
        raise ProtocolError(f"{name} contains control characters")
    return value


def request_id(payload: Mapping[str, object]) -> str:
    if "request_id" not in payload:
        raise ProtocolError("payload is missing request_id", "invalid_request")
    return validated_id(payload["request_id"], "request_id")


def server_envelope(
    session_id: str,
    event_seq: int,
    message_type: str,
    payload: Mapping[str, object],
) -> dict[str, object]:
    return {
        "protocol_version": PROTOCOL_VERSION,
        "session_id": validated_id(session_id, "session_id"),
        "message_id": f"python-{event_seq}",
        "type": message_type,
        "payload": dict(payload),
        "event_seq": event_seq,
    }


def encode_message(message: Mapping[str, object]) -> str:
    try:
        return json.dumps(
            message,
            ensure_ascii=False,
            separators=(",", ":"),
            allow_nan=False,
        )
    except (TypeError, ValueError) as error:
        raise ProtocolError(f"cannot encode protocol message: {error}") from error
