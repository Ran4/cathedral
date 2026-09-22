# M6 partial-foundation review — 2026-09-22

Independent reviewer: fresh GPT-6 Astra High agent `m6_review`, read-only.
Coordinator also inspected the access API, direct reducer diff, custody holder
writer and release policy, focused fixtures and original evidence.

The original 1,027 source hashes matched the working tree before correction.
Decompressed `pure-05` and `custody-01` archives match both recorded raw SHA-256
and archive SHA-256: 6 and 26 passing tests respectively. These remain historical
evidence for the pre-review source, not proof of the corrected implementation.

Review found a reachable regression: `Custody::grab` admits distinct holders
beyond 32, but the new direct release policy refused all such records, including
an officer of record. Corrected direct release delegates to the existing keeper
policy; the optional receipt fingerprint alone retains the 32-holder cap. The
regression fixture creates 33 distinct, present actors through the real holder
writer and proves officer release succeeds while receipt capture refuses.

Receipt/route freshness is checked at projection time. Returned geometry is
cloneable, so future consequential consumers must revalidate after authority
changes. No new persistent grants, keys, portal state or whole-M6 acceptance is
claimed. Cached navigation fingerprints do not scan the graph on each capture.

`coordinator-release-01` passed 10 focused access/travel tests. Its global source
map changed during the command (documentation comment and isolated backend HTTP
work); it is a development pass. Final stable checks are recorded separately.
Cargo runs use one job and nice priority 15. No renderer/provider/device run or
full-workspace result is claimed by this review.

Final checks: `coordinator-final-01` passed 10 focused access/travel tests;
`coordinator-custody-01` passed all 26 custody integration tests. Both exited
zero with unchanged source maps. The coordinator checked all 1,029 input hashes
and verified the compressed/raw archive hashes for both commands. The global
map includes isolated, uncommitted backend HTTP changes; the M6 Cargo targets
are the pure simulator and do not compile those backend sources. Scoped
Rust formatting and `git diff --check` passed. This accepts the corrected
current-authority foundation only, with the full-M6 gates above still open.
