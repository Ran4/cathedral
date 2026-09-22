# M6 owner handoff

Coordinator follow-up (2026-09-22): independent review corrected a reachable
release regression above 32 holders. Direct release retains existing authority;
only optional receipt capture is capped. Final corrected checks passed 10
focused tests and 26 custody integration tests with stable source maps. See
`../coordinator-review.md`; the owner results below remain historical evidence.

Status: focused checks passed. Source/Cargo/executable ownership explicitly ceded
to the coordinator after custody-01 exited (2026-09-21). No owner process remains
live; independent coordinator acceptance is pending.

Added pure `access.rs` and focused `access/tests.rs`, registered the module in
`lib.rs`, and integrated the common authority checks into `actions.rs`.
`DESIGN.md` records the exact supported scope and refusal policy.

The API exposes closed voluntary-travel and custody-release requests, immutable
fixed-size decision receipts, useful denial clues, and permission-bearing route
tickets. Current duty/custody, actor presence and pose, generation/time, public
revisions and navigation identity fence use. Route closure contents/revision
remain M4's physical authority. Direct action paths re-evaluate the same current
policy, and higher-priority duty geometry pricing retains its prior semantics.

The owner ran offline, pure Rust checks only. No host resources, renderer,
window, audio device, provider or GPU was started. No branches, commits, pushes
or additional agents were created, and unrelated dirty/untracked paths remain.

Each `run.py` command retains exact START source hashes and environment, original
combined stdout/stderr in `/tmp/alibi-m6-*`, deterministic mtime-zero gzip,
SHA-256 metadata, exit code and post-command source equality. Coordinator owns
independent review and broader workspace checks.

## Results

- `pure-01`: 5 passed, 1 failed, 0 ignored. The committed-work test omitted
  the kernel's required command-ledger admission and correctly received
  `operation_admission`. Original raw failure and unchanged source metadata are
  retained. The fixture now follows the existing begin/command/finish protocol;
  production code was unchanged by the correction.
- `pure-02`: rustc killed by signal 9/SIGKILL before any test ran; exit 101.
  No compiler diagnostic established a source error or the kill's cause. Exact
  raw/archive and unchanged-source metadata are retained.
- `pure-03`: same-source retry failed at link with undefined hidden LLVM/anon
  symbols across unchanged modules. This followed the SIGKILL and was treated
  as suspected incremental-artifact corruption, not a source diagnostic.
- `clean-01`: successful `cargo clean -p cathedral-sim --profile dev --offline`;
  Cargo reported removing 46,414 files / 73.5 GiB of package development build
  artifacts. Sources, release profile, original logs and evidence were preserved.
- `pure-04`: clean compilation succeeded; 5 passed, 1 failed, 0 ignored.
  The operation fixture reached the next guard and correctly rejected its
  invalid recovery horizon (5 seconds for 10 seconds of work). The fixture now
  uses a valid 20-second recovery budget; production code remains unchanged.
- `pure-05`: 6 passed, 0 failed, 0 ignored; unchanged source.
- `custody-01`: all 26 existing custody integration tests passed, 0 failed,
  0 ignored; same source as pure-05 and unchanged throughout.

`source_hashes.json` is an exact copy of custody-01's START map, also identical
to pure-05's map. SHA-256:
`332cb0326e056d1b904bc6488511885a87bef8a0a68084cd1c4ab11e05c0978c`.
All seven commands retain source/environment metadata and exact original logs.
The final two commands pass; earlier failures are development evidence, not
silently superseded passing checks. Scoped formatting and `git diff --check`
passed. Root owns full-workspace and independent acceptance checks.

## Remaining gates

This is the current-authority foundation, not full M6 completion. It adds no
mutable access state or checkpoint wire. General property grants, consent,
keys/loans, locks, scoped inspections, timed door operations, forced entry,
shared structural portal coverage and their persistence remain explicit gates.
Existing host gates, custody controls, raw geometry/debug queries, prompts and
hearing rules are unchanged. Receipts are prospective synchronous checks, not
archived legal or knowledge evidence.
