# HTTP retention prerequisite review — 2026-09-22

The coordinator reviewed the shared builder and all three changed production
constructors (cognition, cloud STT and cloud TTS), `CompletionTask` poll/drop
ordering, the local socket tests and the retirement-pin lifecycle fixtures.

The builder retains each caller's DNS resolver and reqwest's locked redirect
policy. Zero idle capacity is supported by the inspected hyper-util source as
disabling the pool's map, rather than merely limiting entries per destination.
The extra TCP/TLS setup cost is explicit. Supported protocol/body/error paths
remain covered by the existing backend tests.

`CompletionTask` declares its owned request future before the terminal guard.
Normal completion also takes and drops that future before consuming the guard.
This establishes payload-before-guard disposal for its direct captures; it
does not establish termination of hyper's internally spawned drivers. The new
pinned allocation, active transport/TLS/config storage and complete application
accounting remain unfinished. No save/load permission follows from this cut.

The tests retain real retirement pins and deliberately retain payload inside
completed futures. Loopback servers offer keep-alive and check closure before
the client is destroyed, plus redirect body/credential and limit behavior.
One initial development test needed to construct its request inside Tokio;
that failure is retained.

Final recorded checks passed: three HTTP policy tests, five direct task disposal
tests, and the full backend library (210 passed, zero failed, three ignored).
The coordinator independently reran source/dependency manifest verification;
all hashes match. The full-suite log records both TEST_EXIT and IDENTITY_EXIT
as zero and identifies the executed binary. Scoped format and whitespace checks
passed. Whole-workspace checks follow separately; these backend results accept
only this narrow prerequisite, not complete M3 accounting or adoption.
