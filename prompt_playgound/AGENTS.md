# prompt_playgound — the two local speech workers, and the secrets

Everything else that was here is gone. The LLM-driven character simulation used
to be a Python sidecar in this directory (`server.py`, `sim.py`, `prompt.py`,
`scheduler.py`, `protocol.py`, `llm_client.py`, `speech_client.py`, the terminal
prototype and their tests); it is now Rust, in `crates/cathedral-sim` (the pure
simulation) and `crates/cathedral-backends` (the provider client, the speech
backends, the prompt archive). Read `crates/cathedral-sim/AGENTS.md` for the
domain model, the action verbs and the prompt loop.

What survives here is what genuinely has to be Python, because the models are:

| File | What it is |
|---|---|
| `canary_qwen_worker.py` | local speech-to-text (NVIDIA Canary-Qwen 2.5B, FP16, CUDA, English-only). The Z key / the Esc menu selects it. |
| `pocket_tts_worker.py` | local NPC voices (Pocket TTS, CPU, streams PCM as it synthesizes). The X key / the Esc menu selects it. |
| `.env` | provider keys and speech settings (gitignored); `.example.env` is the template. |

Both workers are `uv` inline scripts, spawned as subprocesses by
`crates/cathedral-backends/src/worker.rs` and driven over a strict JSON-lines
protocol on stdin/stdout. They are the only subprocesses the game starts. Their
stderr is forwarded into the session log (`logs.jsonl`, sources `"stt"` and
`"tts"`).

`config.ron`'s `smart_actors.uv_binary` picks the `uv` that runs them; this
directory is `cathedral_backends::config::DEFAULT_WORKERS_DIR`, and `.env` here
is `DEFAULT_DOTENV_PATH` (real environment variables win over it). The directory
kept its name — typo and all — because that `.env` path and the "put it in
`prompt_playgound/.env`" error message are a user-facing contract a rename would
break for no gain.

A missing worker script, a missing `uv`, or a worker that dies takes down *only*
its own capability: cognition, transcription and voices degrade independently,
and the cathedral stays playable.
