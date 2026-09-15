# M3b2b2 — Retained native backend work

This cut extends accepted M3b2b1 `b37d8fcff58efcd219766a98fd0e1a83b731df7c`.
It closes finite native work and real termination ownership, before the later
immutable startup recipe, diagnostic sinks and complete application allocation
integration. It does not supply whole-App adoption or a second independent budget.

## Runtime and DNS

The locked runtime is Tokio 1.52.3; reqwest 0.12.28 has no hickory feature,
and its default resolver delegates to hyper-util 0.1.20. The latter's
`src/client/legacy/connect/dns.rs` spawns blocking system resolution. Tokio's
string `ToSocketAddrs` has the same cancellation-surviving native operation.
Limiting async workers alone therefore did not bound native threads or repeated
cancelled DNS jobs. Source files were inspected in the accepted offline registry;
no provider, live DNS experiment, device or GPU probe was used.

`BackendRuntime` explicitly configures two async workers, two additional blocking
workers and a 2 MiB stack for each. All production LLM/cloud STT/cloud TTS clients
receive a `NativeResolver` bound to the runtime's shared two-slot DNS pool and
their immutable generation endpoint. Realtime websocket connection uses the same
resolver and connects by `SocketAddr`, then performs TLS/websocket negotiation
against the original request URI (preserving Host and TLS server name). There is
no remaining production fallback to Tokio's string DNS path.

Two resolver slots include queued/running non-abortable work and retained answer
iterators. Admission refuses immediately with WouldBlock when exhausted; dropping
or aborting the async waiter cannot return the native closure's permit. Queued
native work is an ordered DnsWork aggregate: host and operation fields precede
its permit, and Drop prevents disjoint closure capture. Production operation
captures only the port; the injected arbitrary closure seam is for deterministic
tests. Host storage is capped at 253 bytes; address output is capped at 32 socket
addresses.
System resolver internal allocations are explicitly a trusted native allowance,
not a Rust heap census. Native errors/cancellation preserve the original endpoint
until actual owned data disposal. Old completion IDs never determine ownership.
Standalone client constructors use their separate finite two-slot fallback pool;
they are legacy standalone callers, not a new admitted application budget.

`start_admitted` takes the caller's existing CheckpointBudget and reserves a
separate persistent Running child of 16 MiB before construction: four explicit
2 MiB stacks, 4 MiB trusted system-resolution scratch and 4 MiB runtime/native
allocator/thread-local controls. This is a scoped native allowance, not measured
whole-process memory and not a claim about reqwest transport buffers or external
models. Default startup remains unadmitted until the later immutable recipe and
complete startup integration. The original aggregate 512 MiB Running minimum,
1 GiB shared ceiling and 128 MiB typed limits remain unchanged.

Final Runtime Drop now performs ordinary Tokio shutdown off-frame. It aborts
async futures and waits for running non-abortable work. The locked Tokio
`runtime/blocking/pool.rs::shutdown(None)` waits for shutdown notification and
then joins the actual native JoinHandles, including the final exited thread.
Consequently native closure Drop is not mistaken for native stack teardown.
A hung system resolver may occupy retirement and its charge indefinitely; new
admission may refuse while that real owner survives. It cannot free old charge
or block an active frame through a reserve, submit, cancel or fence operation.

Actual STT/TTS worker closures hold `BackendExecutor`, a raw Tokio Handle followed
by the shared retention core. That wrapper cannot own/drop Runtime. The outer
STT/TTS engine retains the Runtime Arc until after joining its worker. LLM and
realtime tasks capture clients/endpoints/state, not the runtime owner. Resolver,
executor and retained DNS answer owners keep the shared core after all native
workers join if their actual controls still survive. The core never owns Runtime
or its join owner, avoiding an ownership cycle. Arbitrary user-injected raw Tokio
API work is outside this finite production service inventory.

## Speech and child cleanup

STT batch and recording disposal retain their JoinHandles. TTS does likewise.
Off-frame final disposal interrupts the local child, closes its bounded queues,
and joins native workers while the outer endpoint and runtime remain alive.
The final join therefore covers the interval after a closure has returned but
before thread-local destructors and OS stack teardown complete. Accepted queued
jobs still produce their reserved terminals, or are inert after fencing; their
existing generation sender is not replaced. STT disposal retains every accepted
unlink until attempted, including a slow unlink after outer handle disposal.

The recording-disposal queue remains 64 entries, separate from the four-entry
batch queue. Both the incoming path capacity and full resolved path capacity are
bounded at 4,096 bytes; a large session prefix cannot bypass the job limit.
Owned disposal admission returns the exact original PathBuf allocation on full,
inactive or oversize refusal. The borrowed trait path keeps the established
fallback: private SessionDir cleanup owns full-queue deferred recordings. This
is distinct from required prompt archive admission. Idle configuration/session
storage remains part of the later complete installed-recipe inventory.

Each local Worker admits at most two child generations across its active process,
retired process, stderr reader and reaper. Restart refuses until a completed pair
of native logger/reaper handles has been joined. A process exiting does not free
its slot while a reader or reaper still survives. Poisoned streams still receive
SIGTERM, one-second grace, SIGKILL and wait on a native request/reaper worker.
NativeChild retains its original generation endpoint outside both closures and
joins before releasing it. Failed reaper spawn retains recoverable Child ownership
and reaps on the already-blocking request worker. Logger-spawn failure kills and
reaps the child before returning failure. close is terminal and prevents racing
queued requests from starting a replacement child during destruction.

One configured generation has exactly one batch worker, one discard worker and
one TTS worker. Its two local drivers can each retain two child generations,
each with one logger and one reaper. Thus the conservative simultaneous native
maximum is 11 threads, each with a 2 MiB explicit stack: 22 MiB of stack extent.
The directly managed child-process generations number at most four per generation
and stay owned through reap. This count does not census arbitrary descendants or
Python/model heap/GPU weights. An inherited stderr pipe that survives its direct
child keeps the original logger and child slot occupied. The shared runtime's
four threads are separate. Actual fixed control layouts are
reported by the final ELF inspection. This inventory and mailbox/world lifetime
pins do not prove an arbitrary previous world allowance covers these stacks;
later disjoint complete-application admission must incorporate them explicitly.

Realtime cleanup additionally has one detached close slot per SessionTask. The
close owns an ordered RetainedClose aggregate: the close future/transport field
precedes its immutable endpoint and then its slot. A Drop implementation prevents
disjoint async capture. This preserves disposal order before first poll, at await,
on timeout and on normal completion; normal-tail drop calls alone were insufficient.
An actual blocked transport-destructor witness exercises both Tokio cancellation
states and retains the original retirement lease until disposal completes.
A one-second best-effort timeout disposes a wedged close future. Further
reconnect/idle closes while this slot is occupied dispose their transport
immediately instead of accumulating
unbounded tasks. Existing fail_pending and accepted utterance terminal behavior
remain unchanged. These async tasks use the runtime's shared worker stacks.

## Actual outer-owner paths

The real LocalEngine retirement transport already moves Send services before its
BackendsHandle guard to the preparation worker. STT/TTS and final runtime joins
therefore run in that off-frame disposal. PreparedDelivery failure/cancellation
retains actual failed/bound services and returns them to its original preparation
slot; joins occur when that worker disposes them. Startup builds most assets
before native services, then parses SHELTERS_JSON after transcription/TTS
construction. Failure at that later parse has only idle queues, no submitted
speech/provider jobs, and realtime still awaits its first Action. Engine::new
failure destroys forwarding adapters while ForwardingServices retains the real
owners; LocalEngine::fail fences and retains its engine/services. In partial STT
construction, failure to spawn the second worker closes the first queue and joins
the first worker before unwinding its generation/runtime.
Normal process/app teardown and standalone CLI/test disposal are final destruction,
not an ordinary frame pump. Their final-runtime Drop contract now waits for actual
native completion. Whole-App M3b2c must keep using the retained disposal transport,
including startup/offline/dead host states; this cut exposes no World/Engine escape.

Verification command/source identities and limitations are recorded in HANDOFF.md.
