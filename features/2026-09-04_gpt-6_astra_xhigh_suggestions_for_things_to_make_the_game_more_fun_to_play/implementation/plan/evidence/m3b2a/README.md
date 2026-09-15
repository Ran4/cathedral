Status: M3b2a implemented and independently accepted (2026-09-15). Actual allocation/runtime accounting and complete App adoption remain pending.

# M3b2a — Stable allocation groups and atomic promotion

This prerequisite supplies the accounting transition needed by complete App
adoption. Reservations retain stable allocation identities, so existing child
leases follow their world from Running to Retiring, or LoadCandidate to
Running. Persistent service allocations stay Running. All admission, resize,
release and promotion operations resolve those identities under one mutex.

Preparation borrows the live root without removing it. Dropping a prepared
permit cancels the transition and leaves the original root, roles and charges
intact. Commit moves the old root into the actual retirement pin and changes
both roles without an allocation or fallible external operation. The opaque
candidate supports this operation without exposing its payload.

Retirement release waits for endpoint pins and surviving children in the
original transport and old-world groups. Observers retain metadata rather than
byte charges. Reusing a cohort cannot revive an old observer, and rewrapping a
surviving used retirement child cannot attach a second world.

## Reviewed scope and verification

The [owner implementation](owner/implementation.md) records the contract and
fixed six-group bound; the [owner handoff](owner/HANDOFF.md) records final commands
and successor obligations. The [independent review](coordinator/review.md) explains
the lifetime corrections and five coordinator witnesses. Three additional owner
tests exercise actual admitted payload destruction, child release observation
and reused-retirement refusal.

Frozen focused checks passed all eight new tests: the owner namespace passed
3 tests, and the complete checkpoint namespace passed 210 with 27 ignored.
Scoped formatting passed. The unchanged final workspace passed **2,280 tests,
zero failed, 47 ignored**, across 46 printed result groups. Compilation took
22 minutes 46 seconds; total runner wall time was 1,497.474 seconds.

The named real-file fresh-process preparation test passed. This workspace run
did not request `--nocapture`, so its passing child's stdout was suppressed;
the reported totals do not add an unprinted child group.

[All nine command records](coordinator/command-audit.json),
[lifetime boundaries](coordinator/boundary-audit.json) and
[final image/layout inspection](coordinator/image-audit.json) passed independent
audits. The measured control layouts give a conservative 60,712-byte bound
within the existing 65,536-byte fixed-control allowance. Both final test ELFs
were hashed in place and inspected without execution; no separate executable
copies were preserved for this cut.

The [source delta](coordinator/source-delta.json) compares accepted M3b1 commit
`72bc316e592a260344033689ae33b1ca7aa4c361` with the final source map
`04046178497e975d6942f76232ef3696fd5a4ad7674494ffc817e79d5b978032`:
1,003 inputs, one new independent-test file, two changed checkpoint files,
and 1,000 unchanged predecessor inputs. Command records retain start/result
metadata, source maps and deterministic lossless gzip alongside original raw
logs under `/tmp/alibi-m3b2a-*`.

The initial focused run is development feedback: the fifth independent test
was added after compilation began, and its source comparison records the change.
It is not evidence for the final frozen source. No failed or source-changing
record is rewritten as final acceptance.

## Remaining M3 work

The 512 MiB aggregate Running minimum, 1 GiB shared ceiling and complete
checkpoint limits are unchanged. The small-charge identity tests do not prove
actual two-world residency or save-again capacity. Fixed-layout inspection bounds
control metadata only; it cannot establish world, host or native heap
usage.

M3b2b must establish actual live/candidate/retired/persistent allocation scopes,
an immutable installed recipe, archive backpressure and detached runtime
ownership. M3b2c then supplies complete host staging/adoption. Player controls,
full frame acceptance and M4–M19 remain pending. Earlier renderer, populated
stress and live-provider limitations remain explicit.
