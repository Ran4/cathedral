# M3b2b3 owner handoff

Status: production integration and focused owner tests passed; source, Cargo and
executable ownership explicitly ceded to root on 2026-09-18. No owner command
remains live. Root requested ownership of independent review/tests and the full
workspace run at this checkpoint; those are not claimed passed here. No commit,
push, branch/worktree, additional agent, device/provider or renderer run occurred.

## Delivered source

- `src/session_log/bounded.rs`: actual preallocated JSONL/stderr pools, bounded
  submission/fence status, fixed counters, native worker, admitted constructor,
  and outer owner retaining its charge through actual join.
- `src/session_log/format.rs`: bounded typed tracing collector and JSON/console
  serialization. No producer Map/Value/String clones or Bevy formatter scratch.
- `src/session_log.rs`: real startup/global producer/normal join-owner wiring,
  diagnostic recovery summaries, drive/session evidence deadlines, sticky
  evidence failure and one bounded atexit fence. Existing path/meta/date code
  remains. The existing staged-file exact-once test uses the new real sink.
- `src/main.rs`: keeps the join guard, installs the fmt-layer override, routes
  worker stderr through bounded formatting, returns actual AppExit.
- `src/drive.rs`: bounded watchdog stderr; normal Quit reports an unsuccessful
  AppExit after evidence failure. Existing stdout remains synchronous evidence.
- `src/session_log/tests_bounded.rs`: seven new isolated native witnesses.
  `tests_public.rs` is an empty registered seam reserved for root ownership.
- `.claude/rules/LOGS_FOLDER.md`: actual bounded diagnostic/evidence contract;
  prompt archive format and required-archive policy remain unchanged.

Read [design.md](design.md) for exact limits, lifetime scopes and overload policy.
The two finite production sinks together have a real admitted 16 MiB persistent
Running seam; default startup still awaits shared-budget recipe wiring. Global
512 MiB Running, 1 GiB shared and 128 MiB typed/raw contracts are unchanged.

## Focused evidence

`focused-02` passed **15 tests, zero failed, zero ignored**: seven new bounded
witnesses and eight existing session tests. Cargo/build elapsed 219.0628725 s;
the test harness reported 0.08 s runtime. `final-format-01` and `final-diff-01`
both passed. Their source map is unchanged:

`f7f34722de385fb08d993fca563ff2f72bdd44400e0576f735f3e342c8db10bf`
(1,011 inputs; see each exact `*-sources.json`).

- Raw: `/tmp/alibi-m3b2b3-focused-02.log`
- Raw SHA-256: `cb18d2db8ce685612ffa0f4193b73c651866b098aac17ccfdcf88a3968d6320a`
- Gzip SHA-256: `96988d5d7a979db2d68e277cd4b558ecc4566873831e238e541b8aa68bf8bc95`

The adapted owner/run.py preserves command-start source/environment/helper
identity, exact raw logs and mtime-zero gzip. It removes inherited linker/Rust
wrapper flags and uses the accepted offline -j1 Cargo environment. Four command
records exist at cession. `focused-01` is a preserved compile failure from using
the sim-private CheckpointError::new constructor; public error fields fix it.
No failed check is counted as acceptance and no historical evidence was changed.

The new tests cover formatting/queued/active pool capacity and evidence reserve,
exact String pointer/capacity refusal, JSON escaping/Unicode, bounded Debug and
typed fields, actual native TLS termination retention, endpoints/drafts surviving
worker join, controlled startup-failure disposal and failed-write prefix fences.

## Remaining root review and successor scope

Root owns independent concurrent FIFO/prefix/overload/evidence-path review and
the full workspace. The retained BoundedSink.core field intentionally keeps its
allowance after native join and currently emits a never-read warning; it can be
renamed `_core` in the next source pass. The existing perf Probe warning remains.
Runtime pool admission is enforced by actual buffer availability; instantaneous
usage phase counters can briefly straddle transfer operations and are diagnostic
snapshots, not a separate allocation authority.

The 8 MiB per-sink scope includes 4.25 MiB fixed buffers and a 2 MiB requested
stack, with 1.75 MiB for controls/channel/formatting/TLS. No compiled layout or
whole-process heap census is claimed in this focused handoff. User Debug code can
still allocate internally; caller-owned inputs precede sink admission. Supplied
custom Write owners have their own external storage contract. Ordinary default
File/Stderr consumers do not retain unbounded sink-side jobs or formatting.

Required prompt archives were not reopened. Complete startup admission,
immutable installed recipe, disjoint whole-App accounting and adoption remain
successors. No synchronous frame-time, GPU, live provider or human acceptance is
claimed. Last read-only disk check showed 25 GiB free; no files/caches/executables
were deleted. Unrelated user paths remain preserved.
