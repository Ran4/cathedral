# M3b2b1 — Prompt archive admission and retention

This is the first coherent production prerequisite inside M3b2b, agreed with
the coordinator after source review. Accepted base is M3b2a
`30f8188301b8f1585df478e2e6df5f16bc1434cc`. Native backend lifetime, immutable
installed assets, diagnostic sinks and the complete application's disjoint
allocation/capacity proof remain separate pending cuts. No complete Engine or
private World extraction/adoption API is introduced here.

## Actual archive ownership

`Cognition::reserve_prompt_archive` is a synchronous nonblocking admission hook.
The production `PromptLog::cognition` decorator supplies it for both real and
fake LocalEngine cognition. The checkpoint forwarding adapter preserves it.
Standalone/headless cognition without a file archive returns an empty permit.
The scheduler and Night acquire a permit before cloning/submitting their
provider request. Refusal takes their existing worker-busy path, including
restored inbox/presented inputs and retained semantic retry/queued duties.

The permit is unique, opaque host allocation retention. It does not identify a
request by a reused execution integer, actor name or prompt text. Scheduler and
Night retain it separately from serialized authority through provider work and
held results. A matching stale-actor or failed result moves it into its actual
archive. Existing semantic replay/no-archive paths release their unused permit.
Generation retirement disposes the old domain's unused entitlements with that
domain; it does not turn fenced old callbacks into archive records.

`PromptExchange` owns private data followed by its permit. Read-only dereference
exposes semantic fields; there is no mutable dereference or extraction, even
when an external holder can uniquely unwrap the outer Arc. Scheduler/Engine
events carry `Arc<PromptExchange>`: event clones share strings and admission.
Semantic equality ignores the host permit. The actual permit fields are skipped
by checkpoint codecs and semantic copies; saved V1/V2 field schemas are unchanged.

Complete continuation service binding reserves both preserved held-result
permits before installing either or reporting services bound. If the second
refuses, the first temporary permit drops, no provider submission occurs, and
the original quarantined candidate plus actual failed service owners return
through the existing retained-failure/disposal contract. Archive association
therefore does not depend on saved RequestId rebasing. Complete App adoption
still owns execution/completion ID rebasing and fences.

## Finite queue and byte governor

Each writer has eight outstanding slots and a 16 MiB aggregate payload limit.
Slots include reserved provider/held work, queued jobs, the active writer, and
any external admitted event clone that survives a completed write. The channel
has eight fixed slots too; with a unique unqueued receipt at most seven other
admitted jobs can occupy it, so an admitted send never waits on disk/capacity.
The receipt prevents duplicate accepted writes from shared event clones.

For prompt capacity P, completion capacity C, combined actor-ID/name storage L,
directory capacity D and model capacity M, admission charges
`P + C + 8*L + 4*D + 2*M + 8192` with checked arithmetic before retaining a job.
Production pending work reserves C=401,024 bytes, the existing actual backend
LLM terminal allowance. The mailbox already replaces an oversized response or
error with its reserved bounded terminal. The scheduler also enforces the
100,000-Unicode-scalar reply policy before archive creation. Held checkpoint
errors use bounded 65,536-byte TextV1 fields. Standalone ingress charges its
actual answer/error capacities, rather than assuming provider bounds.

Labels cover the existing absent-actor fallback: ID length plus the maximum of
name and ID lengths. Receipt validation and standalone/shared ingress observe
ActorId::allocated_bytes and actual String capacities, so short strings with
large retained buffers cannot bypass admission. Metadata coefficients cover
sanitizer temporaries, formatted basename growth, joined-path old/new capacity,
record/Arc controls and timestamp formatting. These are scoped conservative
allocation charges, not RSS, wire-size or whole-world heap measurements.

The writer streams pretty JSON and then Markdown through separate 16 KiB
BufWriters. It does not hold the former markdown-fragment vector, joined
markdown and escaped complete JSON strings simultaneously. Source values and
their receipt survive both writes and all error cleanup. File names, schema,
field order, UTF-8/escaping and Markdown bytes retain their existing tests.
The existing policy of reporting an IO error without breaking the simulation
remains; accepted work is attempted, not silently dropped for queue pressure.

`record(owned input)` refuses before archive allocation and returns the original
input, including spare capacities. `record_shared` requires the record's own
admission and returns the same Arc on unadmitted/foreign/duplicate/size refusal.
Legacy `record_scheduler_event` reserves before cloning any fields into its own
distinct admitted record. Its original unadmitted caller graph remains the
caller's owner. Disabled ingress performs no exchange copy, and default
disabled constructors do not start a writer.

## Ordering, flush and actual disposal

Session forks share an ordered counter/progress owner; no global directory map
exists. `fork`/`fork_with_clock` preserve the same-second filename suffix across
old/replacement handles. A fresh constructor denotes a distinct session; future
load adoption must retain/fork the existing session rather than recreate it for
the same directory. Filenames are assigned only after admission. Counter
headroom is checked before accepting work.

Flush snapshots a per-session accepted counter and waits on a Condvar until
that prefix has completed. Flush barriers do not consume a queue slot or create
an unbounded secondary queue. Drop/flush remain explicitly off-frame; actual
LocalEngine retirement already transfers those owners to its disposal worker.
The ordinary reserve/record path performs no file operation, join or disk wait.
Normal teardown waits for accepted writes. The process-exit atexit hook keeps
the predecessor’s five-second best-effort deadline; hard abort and IO errors
do not acquire new durability guarantees in this cut.

WriteJob drops directory/model references, basename and timestamp before its
final exchange. The receipt drops its session metadata before PayloadCharge
debits the governor. The worker retains only a separate fixed completion
control after payload disposal, then signals flush. External event clones keep
their original reservation until their actual strings die.

An ArchiveWriter's clones declare their sender before a separate join owner.
The last sender therefore closes the channel before the last off-frame join
owner waits for the worker. The join owner retains Core through actual native
thread termination. Worker closures and receipts never retain the sender/join
owner; final permit Drop is admission-only. Receipts surviving all outer handles
continue retaining Core after the completed join.

`ArchiveWriter::start_admitted` reserves 20 MiB persistent Running storage before
worker/control creation: the 16 MiB payload governor, explicit 2 MiB native
stack, and 2 MiB fixed controls/IO buffers plus trusted native TLS/allocator
overhead. The latter is a trusted scoped native allowance, not a measured total
process heap. That reservation stays in Core through worker join and every
surviving permit. The default process writer is likewise finite, but its
complete startup/shared-budget wiring remains a successor responsibility; this
cut does not create a second independent CheckpointBudget as a workaround.

Idle caller-supplied log configuration and arbitrarily many independently
constructed standalone sessions are separate configuration/service ownership,
not inferred from the outstanding-job governor. Production has one startup
session and retained forks. The immutable startup recipe and complete service
inventory must admit that configuration before its construction. This boundary
must not be described as a whole-host allocation census.

## Verification

Owner tests exercise real scheduler/Night admission, floor-held and stale/error
archives, same-poll resubmission backpressure, exact resumed input/semantic
retention and complete held-result service binding. Provider-attempt counters
also prove both actual lanes reached provider refusal after successful archive
admission, released their temporary permits and retained the same retry roots. The coordinator owns the
separate backend review tests with isolated blocked writers and actual shared
budget/permit owners. Development command results are not acceptance results.
Final command/source/image identities and results will be recorded in HANDOFF.md.

## Successor boundaries

The default process archive still needs the actual shared startup budget and
immutable session recipe. `start_admitted` is the tested integration seam, not
a claim that the production complete App is admitted. The global session-log
JSONL staging and stderr queues remain separate real owners requiring their
own bounded diagnostic contract.

Native backend closure must include running DNS resolution and other transitive
blocking runtime work, not infer all native threads from the two async workers.
The coordinator's `/tmp/alibi-m3b2b2-handoff-draft.md` records the locked reqwest,
hyper-util and Tokio source paths and the detached recording-discard/child-reaper
owners. This cut makes no claim about those workers or external model/GPU memory.
The original 512 MiB aggregate Running, 1 GiB shared and 128 MiB typed limits
remain unchanged; a future disjoint allocation proof must justify save-again
while old actual owners remain in delayed retirement before whole-App adoption.
