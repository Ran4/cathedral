"""Thin client for the configured chat-completion provider (Moonshot or OpenAI)."""

import os
import sys
from pathlib import Path

from dotenv import load_dotenv
from openai import OpenAI

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

_client: OpenAI | None = None


def _config() -> tuple[str, dict, str]:
    provider = os.environ.get("LLM_PROVIDER", "moonshot").strip().lower()
    if provider not in PROVIDERS:
        print(
            f"error: unknown LLM_PROVIDER {provider!r} "
            f"(expected one of: {', '.join(PROVIDERS)})",
            file=sys.stderr,
        )
        sys.exit(1)
    cfg = PROVIDERS[provider]
    model = os.environ.get("LLM_MODEL") or cfg["default_model"]
    return provider, cfg, model


def _get_client() -> OpenAI:
    global _client
    if _client is None:
        provider, cfg, _ = _config()
        api_key = os.environ.get(cfg["key_env"])
        if not api_key and provider == "moonshot":
            # Legacy fallback from before .env existed.
            key_file = Path.home() / ".config" / "moonshot" / "key"
            if key_file.is_file():
                api_key = key_file.read_text().strip()
        if not api_key:
            print(
                f"error: {cfg['key_env']} not set — put it in "
                f"{Path(__file__).resolve().parent / '.env'} or the environment",
                file=sys.stderr,
            )
            sys.exit(1)
        _client = OpenAI(api_key=api_key, base_url=cfg["base_url"])
    return _client


def complete(prompt: str) -> str:
    _, cfg, model = _config()
    resp = _get_client().chat.completions.create(
        model=model,
        messages=[{"role": "user", "content": prompt}],
        **cfg["extra"],
    )
    return resp.choices[0].message.content
