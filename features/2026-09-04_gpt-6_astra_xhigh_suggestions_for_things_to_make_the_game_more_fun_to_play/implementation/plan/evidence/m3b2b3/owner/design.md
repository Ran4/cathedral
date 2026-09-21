# M3b2b3 bounded diagnostic sinks

This sequential owner reuses the completed M2a15 agent because fresh-agent
dispatch exhausted the orchestrator thread limit. The current committed b2
source and accepted handoff, not earlier host APIs, define this cut. There are
no additional agents, branches, commits, providers, devices or renderer runs.

## Concrete storage and lifetime

`src/session_log/bounded.rs` is the actual production JSONL/stderr worker. Each
sink preallocates 64 ordinary and four evidence buffers. Every buffer contains
32 KiB encoded storage and a separate 32 KiB arena. A slot remains occupied
while formatting, queued or writing. No queue extension or post-write newline
growth is possible. Input UTF-8 is copied whole into bounded storage; JSON
escaping can refuse the complete record before enqueue. Owned stderr input
refusal returns the original String, including capacity and allocation identity.

The production writer has one explicit 2 MiB native stack per sink. Its admitted
constructor reserves an 8 MiB persistent Running child from the supplied existing
CheckpointBudget before creating buffers, channels or thread: 4.25 MiB buffers,
2 MiB stack and 1.75 MiB scoped control/channel/formatter/TLS allowance. Two sinks
therefore require 16 MiB, without changing any global cap. This is not a process
heap census. A supplied custom Write implementation owns its own external
storage contract; production uses File and Stderr. Arbitrary caller-owned
Strings and allocations performed inside user Debug implementations are not
silently claimed by this scope.

The native closure, queued jobs and formatting drafts never own a joining
handle. BoundedSink retains the core/allowance through actual native join;
surviving producer/draft owners retain it after that join. Draft destruction
recycles or destroys its buffers before releasing its final core. SessionLogGuard
closes both endpoints before normal off-frame destruction joins either worker.
No monitor/reaper thread or unbounded barrier queue is added.

Default startup uses these same finite workers, but has no shared application
budget yet. The admitted constructor is real and tested; complete startup recipe
wiring remains the explicitly separate M3b2b4/application allocation gate. No
independent private CheckpointBudget is invented at startup.

## Output and overload policy

Ordinary diagnostics refuse whole records on full, oversized or closed sinks.
Saturating counters remain observable through usage snapshots; subsequent calls
and exit attempt bounded recovery summaries. A stalled writer cannot grow
producer scratch or force a synchronous stderr/disk fallback on an ordinary
frame. Counters include refused summary attempts too; they are not a promise of
lossless diagnostics. Admitted records preserve FIFO commit order. JSONL stamps
are assigned under the short commit lock and never regress across concurrent
producers. Formatting occurs outside that lock; writer IO is native only.

Drive/session evidence has separate reserved slots and a single one-second
deadline for admission plus prefix completion. Success means its record and the
accepted prefix were written and flushed. Refusal, IO failure or timeout sets a
sticky evidence-failure flag. Normal drive Quit returns unsuccessful AppExit if
that flag is set, and main now returns the AppExit rather than discarding it.
The watchdog still exits 124 and uses bounded stderr. Existing drive stdout
remains synchronous evidence; this cut does not claim to remove that syscall.
Startup filesystem/counter warnings also remain explicit startup work.

Fences snapshot accepted sequence and wait for completed sequence through a
Condvar; later jobs do not extend that prefix. There are no allocated barrier
messages. Atexit uses one five-second deadline for both streams, without trying
to join a possibly stalled native writer. Process exit skips Rust owners; it
does not pretend their native storage was freed before process termination.
Normal off-frame teardown joins and can wait for a blocked OS writer while its
allowance remains retained.

The Bevy 0.19 default fmt layer has its own unbounded String before its writer.
`main.rs` therefore overrides it with an identity layer and uses the bounded
custom event layer for JSONL and console. The collector has 32 fixed field slots,
128-byte field names, bounded formatting arena, 256-byte target and no Map/Value
or owned String copies. Numeric/bool fields keep their JSON types. The console
still includes level, target, message and structured fields; color/timestamp
layout is no longer supplied by Bevy's default formatter.

Prompt archives, required cognition acceptance, archive filenames and screenshot
paths are unchanged. Their reviewed conservation policy remains independent of
diagnostic refusal. Session meta, directory/symlink naming and timestamp helpers
remain the existing source and tests.

## Ownership and verification

Owner files: session_log.rs, session_log/{bounded,format,tests_bounded}.rs,
main.rs and drive.rs. Root owns session_log/tests_public.rs and final independent
review/workspace verification after explicit cession. The empty public seam is
registered before focused compilation.

Owner witnesses use real isolated native workers and controlled Write owners:
slot retention, exact refusal, escaping/Unicode, bounded Debug with typed fields,
native TLS destruction, surviving endpoints/drafts, startup-failure disposal and
failed-write fences. The existing file exact-once/ordering witness remains.
No full host-frame or renderer acceptance is inferred.
