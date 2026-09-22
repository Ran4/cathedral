Status: narrow M3b2b HTTP retention prerequisite implemented and owner-verified (2026-09-22); coordinator acceptance pending. Complete M3 allocation and whole-App adoption remain pending.

# HTTP idle retention and cognition task disposal

This cut follows `/tmp/alibi-m3b2b4-allocation-followup-notes.md`. It changes
only provider HTTP client construction and the cognition task's direct ownership
order. It does not authorize whole-App load/save or close M3b2b accounting.

## Actual locked source and chosen policy

`Cargo.lock` selects reqwest **0.12.28** and hyper-util **0.1.20**. Audited source
is the locally installed crates.io source, not a recalled default or a live
provider observation. `source-sha256.txt` records the dependency and changed
production/test source identities.

In reqwest `src/async_impl/client.rs`, the defaults are a 90-second idle timeout
and `usize::MAX` idle connections per host (301–302). Its build path forwards
the pool values to hyper-util (975–976). In hyper-util
`src/client/legacy/pool.rs`, `Config::is_enabled()` requires a positive
`max_idle_per_host` (115–116); `Pool::new` uses `inner: None` for zero
(121–145). Thus zero omits the complete `PoolInner`, including its connecting,
idle and waiter maps. This is stronger than allowing one idle connection per
host: redirect destination churn cannot grow a retained map of idle hosts.
It is a structural source proof about this pool, not an RSS measurement.

`crates/cathedral-backends/src/http.rs::builder` applies this zero-idle policy
to `LlmClient`, `CloudTranscriber` and `CloudTts`, including their ordinary and
generation-resolver constructors. It keeps the existing DNS resolver bindings.
HTTP/1.1 and HTTP/2 remain supported. Every request requires its own connection;
connection reuse, including sharing an HTTP/2 connection across requests, is
lost. Extra TCP/TLS handshake latency and provider connection churn are the
explicit performance tradeoff. No latency improvement or production throughput
measurement is claimed.

The same builder explicitly sets `Policy::limited(10)`, preserving the locked
reqwest default. In `src/redirect.rs`, the limit rejects
`attempt.previous.len() > max` (132–134), and the default is `limited(10)`
(161–164). Ten hops may be followed: at most eleven request destinations per
attempt, with the initial URL included. The per-request redirect URL vector
may contain eleven previous URLs at refusal; this is a count bound, not a byte
bound on arbitrary configured URLs or Location headers. Existing cross-host
credential stripping and 307 body cloning remain reqwest behavior. Existing
provider retries, timeouts, body limits, speech streaming, wire formats and
error conversion are unchanged. A retry starts another bounded redirect chain.

Production cognition has two lanes (turn and Night Office), each of capacity
one. STT and TTS each process one native-worker job at a time. Queued terminals,
DNS jobs, alternate standalone callers, and multiple generations remain their
own owners; these lane counts do not prove total process allocation.

## Ordered task ownership

Previously `HttpCognition::submit_on` separately captured the client, prompt and
`LaneGuard` in an async closure. The guard conserved terminal/lane state, but
that alone did not specify payload-before-generation-release order.

`CompletionTask<F>` now holds the pinned request future before the guard in
struct declaration order. The request future owns the client, prompt and
cancellation endpoint, including suspended request/response locals. Dropping
the task before its first poll, while suspended, or during unwind drops that
future before the terminal guard. On Ready, the poll method explicitly destroys
the completed future before consuming the guard and sending the normal result
or cancellation failure. A completed future is allowed to retain captures, so
merely awaiting it would not establish this ordering. `LaneGuard` continues to
release its lane and spend exactly one terminal reservation.

The new pinned box is an additional finite task allocation, and the future's
concrete layout plus allocator overhead still belongs in the complete task
inventory. No new lease is manufactured or claimed to cover it here. Existing
generation-pinned mailboxes remain the owner; tests use an explicitly labeled
4 KiB fixture retirement reservation to observe release order, not to estimate
production heap usage.

Cloud STT/TTS HTTP work still runs on existing joined native workers. This cut
does not redesign their shutdown ownership or realtime WebSocket retention.

## Verification

All final checks passed against unchanged production/test source:

| Record | Result |
| --- | --- |
| `http-final-01.log` | 3 passed: idle connection disposal, 307/auth behavior, redirect limit. |
| `task-final-01.log` | 5 passed: direct payload/terminal/retirement-pin disposal order. |
| `backend-lib-final-01.log` | 210 passed, 3 ignored, 0 failed; 132.13 seconds. |

The full library suite covers existing LLM/cloud-speech requests, responses,
retries, error conversion, streaming, native retention and cancellation tests.
The ignored entries are two existing subprocess fixture entry points and the
explicit release storage timing probe; no new regression was excluded. Scoped
rustfmt and `git diff --check` also passed.

`verify-backends.sh` records real UTC start/end, command, HEAD, toolchain,
pre/post source hash checks and executable hashes. These runs use the default
dev/test profile and repository `target/`, default `/home/ran/.cargo`, no
CARGO_HOME/target/compiler/flag/wrapper overrides, offline locked dependencies,
one compilation job, `nice -n 15`, and one test thread. All three execute
`target/debug/deps/cathedral_backends-1a4b1aedf7bcfa01`, SHA-256
`95b7fa5544f538bddb592b8461a497f9237be82c3796919b613d9a19f80d5770`.
Other executable hashes in the directory listing are inventory only; the Cargo
`Running unittests` line identifies the binary actually used.

`http-dev-01.log` preserves the initial 2-pass/1-failure development run and its
original source hashes are in `http-dev-01-source-sha256.txt`. Its failed test
constructed `reqwest::send` outside Tokio before `block_on`; moving construction
inside the async block fixed that test setup. Production code was unchanged.
This development log has captured command/output/exit but no contemporaneous
START metadata; none was reconstructed. The final records above supersede it.

Tests use synthetic loopback servers only; no live network/provider, audio
device, GPU or Bevy window. No workspace run was performed by this owner.

New transport witnesses offer persistent HTTP/1.1 connections and require EOF
while the client remains alive, over repeated requests to four authorities;
preserve a POST body through a 307 redirect while stripping authorization on
authority change; accept ten redirects and refuse the eleventh.

Five task witnesses cover never-polled disposal, pending cancellation, Ready
success, Ready retirement and a panicking poll. Each runs with a retained
consumer to check exactly one terminal and again without that consumer so the
guard is the sole retirement-pin owner. Payload destruction checks that the
lane and actual retirement pin remain held, then final disposal must free both.

## Remaining M3 gates

This cut removes unbounded idle-pool retention from these configured clients.
It does **not** bound all HTTP transport, TLS, runtime, diagnostic, configuration
or world memory. In particular, hyper-util's `client.rs::connect_to` spawns
background HTTP/1 and HTTP/2 connection drivers (552 and 584); destroying a
request future is not a join barrier for those driver futures. Their supported
buffers, teardown lifetime and any TLS session/config retention need explicit
accounting. Active header/body storage and URL/config capacities, generation
construction failure/cancellation, and all simultaneous Running/candidate/
retired/shared owners also remain to be proved.

`src/installed_recipe/startup.rs::require_complete_admission` still returns
`UnprovedWholeAppAccounting`. The aggregate 512 MiB Running minimum, shared
1 GiB ceiling and 128 MiB typed limit are unchanged. M3b2c adoption, actual M3c
save/load controls and M3d complete-host acceptance remain unimplemented gates;
this source change and its passing backend tests cannot stand in for them.
