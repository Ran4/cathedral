# M5 owner handoff

Status: focused verification passed. Source/Cargo/executable ownership explicitly
ceded to the coordinator after host-02 exited (2026-09-21). No owner command
remains live; independent coordinator acceptance is pending.

The bounded foundation adds immutable sampled visual-presence receipts in
`crates/cathedral-sim/src/perception/observation.rs` and fixes actual player focus
through closed gate barriers. `DESIGN.md` records the ownership, supported
geometry, work limits, refusal policy, and remaining M5 gates.

Changed production files are `perception.rs`, `world.rs`, the new observation
module, and host `smart_actors/{targeting,interaction,mod}.rs`. Pure observation
tests live in the new `observation/tests.rs`; two minimal host tests live beside
targeting. No historic fixture bytes, configuration, save format, prompt,
hearing rule, simulation event production, or backend behavior changes.

The focus-hint system now runs immediately after target sampling and before
input collection. Closed gates clear actor/item focus and may show the constant
geometry clue. Reopening gates restores ordinary focus. Hidden actors do not
control whether the clue appears. Overload clears old focus/clues; it cannot
silently retain an actionable old target.

## Commands and evidence

Each command records exact START source hashes, environment, helper identity,
combined raw stdout/stderr, deterministic gzip, SHA-256, exit code, and final
source equality through `run.py`. The raw files remain under `/tmp/alibi-m5-*`.

| Command | Result | Source |
| --- | --- | --- |
| pure-01 | 7 passed, 0 failed, 0 ignored | initial receipt implementation; unchanged during run |
| pure-02 | 8 passed, 0 failed, 0 ignored | final pure identity/order source; unchanged during run |
| host-01 | compile failed: three test-only HUD type paths | same START source map as pure-02; unchanged during run |
| host-02 | 31 passed, 0 failed, 0 ignored | HUD fixture paths corrected to `smart_actors::hud::SmartActorHudState`; unchanged during run |

The pure-02 source map has 1,025 entries and SHA-256
`eee23a87c2940b69a36d2d0537678e348fa047a6e6c41b365f3580c6317e9fa7`.
Pure-02 includes stale generation, exact sample time and presence-epoch checks.
Host-01's original compile failure is retained verbatim; only the three fixture
paths changed afterward. Scoped formatting and `git diff --check` passed.
Root owns independent review and broader workspace verification.

`source_hashes.json` is an exact byte copy of host-02's START map: 1,025 entries,
SHA-256 `de6a99e4403ea1346dce7eb5f885f2615d126cf79ce8ba967cbfe4189fced55c`.
Only the three test HUD paths differ from pure-02; no pure production/test source
changed after its successful run. All four commands retain their original
START/result/source/archive metadata. Host-02 reports only the existing unused
`perf::Probe` variants warning. No full-workspace or frame result is claimed by
this owner handoff.

## Limits

Only explicit complete test-fixture geometry can currently certify a pure
receipt. The receipt proves visual presence at its supplied sample boundary,
associated with a source event; it does not prove the act, event-time coverage,
an uninterrupted interval, or private item identity. Receipts are not archived,
persisted, or installed in existing NPC/knowledge consumers. Existing prompts,
hearing, follow behavior, and witness semantics remain for the later coordinated
consumer migration. The actual host fix uses current collision/barrier geometry
and bounded scans, without claiming full structural spatial or frame acceptance.

All focused execution is offline/headless with fake backend and no renderer,
window, audio device, provider call, or GPU probe. No branch, commit, push, or
additional agent was created. Unrelated dirty/untracked paths were preserved.
