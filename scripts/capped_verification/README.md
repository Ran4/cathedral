# Capped verification worker

Runs reviewed `m0_<name>` through `m19_<name>` owner evidence runners sequentially in one
long-lived system service. The fixed `cathedral-alibi-build` unit name excludes
overlap with the earlier one-shot service. All descendants run as `ran:ran` and
share **40% of one CPU core**, a 100 ms quota period, nice 15, and idle I/O.
The launcher uses the predecessor's systemd249-compatible properties. The worker
requires real cgroup-v2 `cpu.max <= 0.4` and `cpu.max.burst == 0` at startup and
before every job; unavailable controls fail closed. It never launches the game.

Install the reviewed permanent system service and its two fixed lifecycle paths
**once**, from a normal administrator terminal:

```sh
sudo sh /home/ran/src/rust/cathedralbevy/scripts/capped_verification/install_capped.sh
```

The installer refuses an active predecessor, queued work, unresolved claims,
STOP, differing existing units, overrides and symlink paths. It installs four
root-owned fixed units and enables the worker and paths at boot. Its root phase
uses isolated `/usr/bin/python3`, not user-owned `uv` or repository Python;
fixed control-file bootstrap runs after dropping to `ran`. Normal Python use
continues through `uv`. No Polkit grant or passwordless sudo rule is installed.

The worker remains idle between jobs, sleeping two seconds per queue check.
It starts after reboot and retries abnormal service failures every ten seconds.
Deliberate worker refusals (exit 2) and successful STOP exits do not auto-restart.
There is no start-rate cutoff; recurring other service failures remain visible
in the journal and may retry indefinitely. All descendants share the fixed cap;
unrelated processes are not covered. Do not start other builds outside it or
change the cgroup controls while running.

Routine lifecycle control needs no sudo:

```sh
uv --cache-dir /tmp/cathedral-uv-cache run --no-project --offline python -I -B \
  scripts/capped_verification/control.py status
# Replace status with start, stop or restart as needed.
sh scripts/capped_verification/start_capped.sh
```

`start_capped.sh` now uses that helper, with no transient-service fallback.
The helper writes only fixed start/stop request markers. The root stop broker
runs only `systemctl --no-block stop cathedral-alibi-build.service`; marker
contents never become root commands. `PathChanged` watches completed writes,
so old marker contents do not trigger service activity merely by existing.

Control operations serialize and hold the existing queue lock. They refuse
pending/staging work or unfinished claims. **Lifecycle stop/restart is idle-only**;
it is not the graceful queue-drain command below and will not interrupt an
active verification job. Restart acknowledges a new completed stop-broker
invocation, no remaining main-unit job, an empty/absent cgroup and rearmed paths
before requesting start. Start checks the current worker startup journal receipt,
exact live kernel quota/burst and the unit's configured `Nice=15`. The current
invocation startup receipt must report actual nice 15 and the current worker
source SHA256. MainPID is a host PID, so the client does not assume that PID is
visible in its own process namespace; startup nice is an observation at startup,
not a fresh process-priority measurement. Queue/pin review is still required before
submitting any job; a startup receipt is not verification acceptance.

Calls have a 40-second deadline, including systemctl/journalctl queries. On timeout
or client crash, `control/request.json` preserves the outstanding request; the
next call must reconcile it before issuing another. A crash after intent is
persisted but before its marker is written, or an unresolved request across
reboot, deliberately requires explicit evidence review. Inspect the request,
marker token, boot identity, journal, unit jobs and cgroup before archiving an
unresolved request and retrying. Never blindly delete it: a delayed stop can
otherwise kill a newly started worker. Control never deletes STOP or job receipts,
archives claims, or resubmits interrupted jobs.

After reviewing the next runner/helpers and freezing its game source/input map,
submit without sudo:

```sh
uv --cache-dir /tmp/cathedral-uv-cache run --no-project --offline python -I -B \
  scripts/capped_verification/worker.py submit \
  --runner features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence/m3_fact_admission/owner/run_capped.py \
  --run-name 20260926-facts-reviewed-01
```

Submission is authorization to execute that reviewed runner. Jobs contain only
`runner` and `run_name`; arbitrary commands, extra arguments, path traversal, and
symlinked runner/helpers are rejected. The accepted path is exactly the evidence
root shown above plus `m(?:[0-9]|1[0-9])_[a-z0-9_]+/owner/run_capped.py`. Future reviewed legs fit
this pattern without restarting the worker. Names are unique forever, up to 80
ASCII filename characters, beginning with a letter or digit. Queue order is
lexicographic by name; timestamp prefixes are useful. At most 256 jobs may wait.

Submission pins SHA-256 identities of `run_capped.py`, `run_tests.py`,
`run_build.py`, `reviewed-sources.json`, the shared source enumerator, and the
worker. The runner itself must enforce the reviewed source map and check it
between stages. Keep sources/helpers frozen until that job finishes; submit the
next leg only after reviewing and freezing it. This is a trusted-owner queue,
not a sandbox for hostile scripts or concurrent edits by the same Unix account.

Inspect `logs/capped_verification/claimed/<run-name>/` for `job.json`,
`review.json`, `claim.json`, `start.json`, `launched.json`, `output.log`, and
`result.json`. JSON receipts and queue transitions are fsynced; the output log is
fsynced on normal completion. The runner also writes its own evidence beside its
source. `result.json` records the runner exit code, quota, hashes and log digest;
it does not replace review of the runner's evidence. A nonzero runner exit leaves
the worker available for a separately reviewed next job. Service-level status:

```sh
systemctl status cathedral-alibi-build.service
journalctl -u cathedral-alibi-build.service --no-pager -n 30
```

When all work is submitted, drain the queue and stop cleanly:

```sh
uv --cache-dir /tmp/cathedral-uv-cache run --no-project --offline python -I -B \
  scripts/capped_verification/worker.py stop
```

`STOP` rejects further submission and exits after already queued work finishes.
For a later session, verify the old service is inactive, review and remove only
the `STOP` marker, and use `control.py start` again. Keep job directories and receipts intact.

A claim is durable **before launch** and is never automatically retried. A crash
between claiming and completion, malformed job, missing cap, or changed hash
stops the worker. An unresolved claimed directory or a pending `claim.json`
blocks subsequent work, even after restart. Inspect the evidence and confirm no
old child remains before manually archiving an interrupted directory outside
`pending/` or `claimed/`; preserve it and use a new run name for any authorized
retry. Do not remove just a claim marker or put claimed work back into pending.
Staging directories left by interrupted submission are likewise reserved and
require review. Editing worker.py requires a reviewed restart; runner changes
need a new submission after the prior job completes.

Lightweight tests (temporary fake runners, no Cargo, no service launch):

```sh
uv --cache-dir /tmp/cathedral-uv-cache run --no-project --offline python -I -B \
  scripts/capped_verification/test_worker.py
```

Lifecycle checks (fake state only, no service activation):

```sh
uv --cache-dir /tmp/cathedral-uv-cache run --no-project --offline python -I -B \
  scripts/capped_verification/test_control.py
sh -n scripts/capped_verification/install_capped.sh
sh -n scripts/capped_verification/start_capped.sh
systemd-analyze verify scripts/capped_verification/systemd/*
```

## Independently audited interrupted jobs

A job killed during verified service shutdown has no runner exit code. Never
write a synthetic `result.json`, delete/move its claimed directory, or reuse its
run name. The existing completed receipt format is unchanged.

After a complete independent failure audit and verified fixed-unit stop, the
reviewed controller can record one separate `abort.json`:

```sh
uv --cache-dir /tmp/cathedral-uv-cache run --no-project --offline python -I -B \
  scripts/capped_verification/control.py record-abort \
  --job-id <permanently-claimed-run-name> \
  --audit-authorization <repo-relative-abort-authorization.json> \
  --audit-sha256 <reviewed-authorization-sha256>
```

This action does not stop or start the service. It requires no pending/staging
work, precisely the specified unresolved claim, no unresolved lifecycle request,
both lifecycle/queue locks, and an inactive/failed main service with no systemd
job and an empty cgroup. Its inspection is capped at 4,096 historical claims and
a 40-second action deadline; every JSON/evidence read is bounded at 32 KiB.

The version-1 independent authorization must state `verified_interrupted`, the
matching `job_id`, `runner_exit_code: null`, a 32-hex `stopped_invocation_id`,
`cgroup_empty: true`, `source_freeze_released: true`, and a `stop_evidence`
repository-relative path/SHA256 pair. That pinned stop JSON must prove the same
job/prior invocation, `unit_stopped_cgroup_empty`, inactive/failed state with no
Job, and `cgroup_empty: true`. The resulting abort adds only the authorization
path/hash and recording timestamp to those fields. It never asserts test/build
success or a process exit that was not observed.

The worker owns the shared validator used by both future claims and lifecycle
idle checks. A malformed/unresolved claim, coexisting result+abort, stale or
tampered evidence, duplicate JSON keys, symlink or path escape refuses recovery.
Every original receipt/output remains immutable. An authorized abort permits a
separately reviewed future run with a fresh name; restart and submission still
require their ordinary reviewed lifecycle/source/cap gates.
