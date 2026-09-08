# Setup failures before Cargo commands — 2026-09-08

The initial uv code-generation command used the default cache and failed before writing source: `error: failed to open file /home/ran/.cache/uv/sdists-v7/.git: Read-only file system (os error 30)`. Retried with `UV_CACHE_DIR=/tmp/alibi-uv-cache`; no permission escalation was requested. This is transcribed tool output, not a native command-original archive.

The first measurement helper attempted to call read_bytes on a source key returned as str by sources(), and failed before spawning Cargo: `AttributeError: 'str' object has no attribute 'read_bytes'`. The run now uses the helper's direct path-to-hash map. No Cargo command or test result is inferred from this setup attempt.

The original command helper computed its own source-helper fingerprint after the child exited; root confirms component_input_sources.py remained unchanged from installation. Starting with private_complete the helper fingerprint is captured before subprocess invocation alongside the source manifest. All recorded helper hashes agree. Actual per-command source maps were always captured at START. Early check and test failures are preserved separately in commands.json and their exact original stdout/stderr archives.
