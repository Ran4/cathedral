# Startup receipt lookup after completed jobs — 2026-10-01

Status: accepted after coordinator review, 18 passing controller tests, read-only
live receipt/cap verification and a successful idempotent live `start` check.
The repair author did not run any lifecycle operation, submit a job, restart
the worker, or execute Rust/game commands. The coordinator check is recorded below.

## Coordinator acceptance

The coordinator reviewed the exact controller/test diff and the live
reproduction, then ran the normal `control.py start` entry point after the
completed Market03 job. It returned `start: verified` without sudo. A subsequent
independent read verified the same invocation and host PID, live exact 40% quota,
zero burst and the current-invocation startup receipt. See
`RECEIPT_LOOKUP_LIVE_20261001.json`. No worker or unit restart was needed.
This accepts the narrow lifecycle lookup repair; game verification remains
independent. The author's validation above and historical setup records are
preserved.

## Actual reproduction

The installed journal is systemd 249 (`249.11-0ubuntu3.22`). The active worker
invocation was `1778ceb35d7b4050a2634f452edefe44`, with host MainPID `1141802`.
This exact controller query returned exit 1, empty stdout and empty stderr:

```sh
/usr/bin/journalctl --quiet --no-pager -o cat \
  '--grep="event": "worker_started"' -n 1 \
  _SYSTEMD_INVOCATION_ID=1778ceb35d7b4050a2634f452edefe44
```

A read-only `-o json -n 100` query for that same invocation returned two entries:

1. `worker_started`, UTC `2026-10-01T18:28:22.312665+00:00`, observed nice 15,
   worker SHA256 `b16603ff4ddd7a764c2d1191d15c548ad1c75574695024ff66528577725af38d`,
   cap `/system.slice/cathedral-alibi-build.service`, quota 40000, period 100000,
   burst 0.
2. `job_finished`, UTC `2026-10-01T19:19:09.196674+00:00`, job
   `cpu40-market-sale-20261001-03`, exit code 0.

Adding `--reverse` to the grep/one-entry query returned exit 0 and the startup
entry. The installed version's forward query limits the initial journal position
before matching; reverse traversal locates the matching startup even when newer
job receipts exist. The previous test only checked the presence of `--grep` and
mocked a successful response, so it did not detect this behavior.

## Repair and bounds

`started_receipt` now uses `--reverse --grep=... -n 1`. Output remains limited to
one matching journal entry. Its existing maximum three-second subprocess timeout
and enclosing forty-second lifecycle deadline still bound the lookup. It does not
depend on startup being within the last 100 entries. If journal retention removes
startup, verification still fails closed.

The query selects the exact current invocation and fixed main unit, requests JSON,
and verifies journal `_SYSTEMD_INVOCATION_ID` and `_SYSTEMD_UNIT` before decoding
`MESSAGE`. It still requires the current worker source SHA256, observed startup
nice 15, and exact startup quota/period/burst/cgroup. Existing live kernel quota,
zero burst, configured Nice=15, positive host PID and stable service checks remain.
Host PID visibility is not assumed. No worker, unit, installer, queue, privilege,
or job-admission policy changed.

## Validation

The following ran once after the code/test change: **18 tests, zero failures**.

```sh
uv --cache-dir /tmp/cathedral-uv-cache run --no-project --offline python -I -B \
  scripts/capped_verification/test_control.py
```

The regression preserves the observed systemd 249 responses: forward grep/one-entry
lookup after job completion has no match; reverse matching returns startup. It
checks the one-entry limit, exact invocation/unit filters, JSON output and timeout.
Additional checks reject missing/invalid current invocation, wrong/missing journal
invocation or unit, absent/malformed startup, wrong/missing nice or worker pin,
and wrong/missing receipt cap. Existing kernel quota/burst and PID namespace tests
continue to pass. A real journal failure still raises rather than being accepted.

A separate read-only Python probe imported the patched controller, called
`show`, `verify_running`, and `started_receipt`, then repeated `show` and
`verify_running`. Receipt validation returned true. Before/after invocation and
host PID matched the values above; both states were active/running with no pending
unit job. Live `cpu.max` was `40000 100000`, `cpu.max.burst` was `0`, and configured
Nice was `15`. No lifecycle entry point or marker writer was called.
`git diff --check` passed for the changed controller and test files.

Frozen SHA256:

- `control.py`: `134792f5e0eb3f2e7f32b9665bfaaf88cd4d08845b36067d3ec9749546dc7bc5`
- `test_control.py`: `d038b92d457294183a3b1d04cdfec958c214fcf888567e61658cedc33edf827d`
- unchanged `worker.py`: `b16603ff4ddd7a764c2d1191d15c548ad1c75574695024ff66528577725af38d`

The earlier lifecycle repair and setup-live records remain immutable historical
evidence. This note supersedes only the controller/test pins and the old claim
that the forward grep/one-entry query reliably finds an older startup receipt.
