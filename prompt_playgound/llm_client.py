"""Thin client for the configured chat-completion provider (Moonshot or OpenAI)."""

import os
from pathlib import Path
from typing import Any

try:
    from dotenv import load_dotenv
except ImportError:  # The offline test suite injects providers and needs no SDK.

    def load_dotenv(path: Path) -> bool:
        return False


load_dotenv(Path(__file__).resolve().parent / ".env")  # real env vars win over .env

PROVIDERS = {
    "moonshot": {
        "base_url": "https://api.moonshot.ai/v1",
        "key_env": "MOONSHOT_API_KEY",
        "default_model": "kimi-k2.5",
        # Instant mode: no reasoning pass.
        "extra": {"temperature": 0.6, "extra_body": {"thinking": {"type": "disabled"}}},
    },
    "openai": {
        "base_url": None,  # SDK default: api.openai.com
        "key_env": "OPENAI_API_KEY",
        "default_model": "gpt-5.6-luna",
        "extra": {},
    },
}

# USD per 1M tokens (input, output), as of July 2026. Cache-hit discounts are
# not modeled, so this is an upper bound.
PRICING = {
    "kimi-k2.5": (0.60, 3.00),
    "gpt-5.6-luna": (1.00, 6.00),
}

_client: Any | None = None
_usage: dict[str, list[int]] = {}  # model -> [prompt_tokens, completion_tokens]


class LLMConfigurationError(RuntimeError):
    """Configuration is unavailable, without terminating a long-lived server."""


def _config() -> tuple[str, dict, str]:
    provider = os.environ.get("LLM_PROVIDER", "moonshot").strip().lower()
    if provider not in PROVIDERS:
        raise LLMConfigurationError(
            f"unknown LLM_PROVIDER {provider!r} "
            f"(expected one of: {', '.join(PROVIDERS)})"
        )
    cfg = PROVIDERS[provider]
    model = os.environ.get("LLM_MODEL") or cfg["default_model"]
    return provider, cfg, model


def _get_client() -> Any:
    global _client
    if _client is None:
        from openai import OpenAI

        provider, cfg, _ = _config()
        api_key = os.environ.get(cfg["key_env"], "").strip()
        if not api_key and provider == "moonshot":
            # Legacy fallback from before .env existed.
            key_file = Path.home() / ".config" / "moonshot" / "key"
            if key_file.is_file():
                api_key = key_file.read_text().strip()
        if not api_key:
            raise LLMConfigurationError(
                f"{cfg['key_env']} not set - put it in "
                f"{Path(__file__).resolve().parent / '.env'} or the environment"
            )
        _client = OpenAI(
            api_key=api_key,
            base_url=cfg["base_url"],
            timeout=float(os.environ.get("LLM_TIMEOUT_SECONDS", "45")),
            max_retries=1,
        )
    return _client


def complete(prompt: str) -> str:
    _, cfg, model = _config()
    resp = _get_client().chat.completions.create(
        model=model,
        messages=[{"role": "user", "content": prompt}],
        **cfg["extra"],
    )
    if resp.usage is not None:
        tally = _usage.setdefault(model, [0, 0])
        tally[0] += resp.usage.prompt_tokens
        tally[1] += resp.usage.completion_tokens
    content = resp.choices[0].message.content
    if not isinstance(content, str):
        raise RuntimeError("LLM returned no text content")
    return content


def is_available() -> bool:
    """Whether cognition appears configured, without making a provider call."""
    try:
        provider, cfg, _ = _config()
    except LLMConfigurationError:
        return False
    if os.environ.get(cfg["key_env"], "").strip():
        return True
    return (
        provider == "moonshot"
        and (Path.home() / ".config" / "moonshot" / "key").is_file()
    )


def run_cost_usd() -> float | None:
    """Total cost of all complete() calls so far; None if a model lacks pricing."""
    if not _usage or any(model not in PRICING for model in _usage):
        return None
    return sum(
        (p * PRICING[m][0] + c * PRICING[m][1]) / 1e6 for m, (p, c) in _usage.items()
    )
