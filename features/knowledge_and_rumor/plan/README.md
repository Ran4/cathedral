# The build plan — index and build order

The executable plan for `features/knowledge_and_rumor/` (the spec is the three files one level up:
`../README.md`, `../01_facts.md`, `../02_rumor_pollen.md`). An implementation agent with an empty
context starts here, reads the four spine files, then the one milestone file it owns, and needs
nothing else. Anchors are `path:line (marker)` against `develop @ f56a2c3`; line numbers drift, the
markers do not. Every check in these files is `grep`, never `rg` (a shell function here, not a
binary).

## Files

| File | What it is | Read it when |
|---|---|---|
| `00_decisions.md` | Every design decision, resolved and numbered (D1–D62). No milestone re-opens one. D51–D62 are the 2026-09-04 reconciliation decisions | always, first |
| `01_api.md` | The paste-ready skeleton: every type, signature and constant, each block annotated with the milestone that lands it | always |
| `02_numbers.md` | Every constant's derivation, both ends of the cadence band, the flat-table identity, the footprint, the measurement commands | M2, M5, and any retune |
| `03_assets.md` | `facts.json`, `salience.json`, the 29 + 1 prompt strings (24 frozen by M0b), the `turn.j2` paragraph and where it goes, the fixture files | M1, M3, M4 |
| `M1.md` … `M5.md` | The milestones: steps, tests, verification, done-when, notes for the next | one at a time |
| `../m0_evidence/` | M0/M0b's measured record: `strings_draft.toml` and `ignorance_rule.txt` are M1's byte-for-byte inputs; `NOTES.md` is the justification. Append-only | M1 (inputs), M3 (the Q4 re-measurement), M4 (C1), M5 (the close-out pointer) |

## Build order — strictly serial, and why

**M1 → M2 → M3 → M4 → M5**, no overlap (D45). Each milestone's `Preconditions` section is the
contract with the one before it, listed by owner (D62), and its `Notes for the next milestone` is the
hand-off. Fan-out happens *inside* a milestone where its file says so (M4 Parts A/B after A16–A17),
never across one.

| # | Owns | Why it cannot start earlier |
|---|---|---|
| **M1** | the store, `holds()`, the 21-rung block, `arm_actor`, the assets, **the one golden re-bless** | nothing to depend on; it is the tree's first knowledge code |
| **M2** | the air, the roll, the mints, the ward grid, the instrument, **every cadence number** (`02_numbers.md`) | needs `holds()`, `learn`, `volunteers`, `may_carry` and the block to measure against |
| **M3** | garbling, the chain, Layer 2, `known_from`, the Q4 re-measurement | needs a working wave to garble and a baseline that must not move |
| **M4** | the player's side: receipts, the journal, `raise_word` and both occasion limbs, the re-heat | needs a chain to walk back and the coded mints to be the staple the verb is the spice of |
| **M5** | the rest of the whitelist, the bells, the four systemic readings, invalidation wired, the wave on the map, **the tune**, the landing | needs everything; its tune (step 16) runs **last and alone** because it re-measures against M2's checked-in baseline |

**Two things are owned once and never shared.** The **golden re-bless** is M1's, in a commit that
does nothing else (D40): +23 lines / +1387 bytes on all 22 fixtures, the ignorance paragraph before
`turn.j2:194` and nothing more. Every later milestone carries "goldens byte-unchanged from the M1
bless" in its done-when, so an unconditional render is caught by the milestone that introduced it.
**Every constant** lives in `knowledge/mod.rs`'s constants block with its derivation in
`02_numbers.md` §3; M1 lands the block, M2 adds `WARD_CELL_M`, M3 `GARBLE_SUBJECT_POOL_MAX`, M4 four
caps/cadences, M5 `KNELL_CARRY_M` and `DOOR_SHUT_REACH_M` — and only M5's step 16 may move a free
parameter, by the division in `02_numbers.md` §10, writing the substitution into that file and the
constant's doc comment. Nobody ever moves a salience band.

## The gate between milestones

Run at every commit boundary, in this order, and at the end of every milestone before the next one
starts. Nothing here is optional; there is no CI (D41, D42).

```sh
cd /home/ran/src/rust/cathedralbevy
# Per file, never `cargo fmt` and never tree-wide: `cargo fmt -- --check` formats every crate
# root regardless of the file list, and this tree is 601 hunks red at f56a2c3, before any of
# this feature existed. Each file you touched must print nothing:
rustfmt --check --edition 2024 <each file you touched>
# Clippy is a DELTA, not an absolute. `--workspace --all-targets -- -D warnings` exits 101 on
# this tree for nine sites that predate the feature (clock.rs:289, custody.rs:1092/:1101,
# engine.rs:2430, engine.rs:4157, homes.rs:98, notices.rs:241, prompt/mod.rs:1850,
# round.rs:6508 — all git-blamed to 2026-07-23…08-09), behind 68 further warnings. So: run it,
# and assert the site list is unchanged from HEAD. A new warning in your own code is a failure;
# inheriting those nine is not, and silencing them is a separate job nobody has asked for.
cargo clippy --workspace --all-targets --offline 2>&1 | grep -E '^(warning|error)(\[|:)' | sort | uniq -c
cargo test --workspace --offline        # the 64 KiB / 160 KiB canaries live in cathedral-backends
git diff --stat crates/cathedral-sim/tests/fixtures/prompts/ assets/prompts/   # empty from M2 on
```

**Why the gate is stated as a delta.** M1 discovered that the absolute form cannot pass and never
could: `-D warnings` promotes nine pre-existing lints to errors, so a milestone that ran the gate as
first written would either report a false failure or quietly adopt a ~59-site cleanup it did not
cause. The honest gate is *nothing new*, measured against HEAD.

Plus the milestone's own `Verification` block (headless runs, measurement runs, the drive scripts
with `CATHEDRAL_HEADLESS=1`, D50) and its `Done when` checklist — every item there is checkable by a
command in the file. M2's cadence tests (`crates/cathedral-backends/tests/pollen_cadence.rs`) are
part of `cargo test --workspace` from M2 on; M3/M4/M5 re-run them unmodified.

## Landing checklist (M5 Group G)

1. The three spec files updated to what shipped — every departure in M5 step 17's table, none left as
   a private correction; `Status:` line per milestone with dates; a `## Numbers` section.
2. `git mv features/knowledge_and_rumor features/implemented/`, keeping `plan/` and `m0_evidence/`
   inside it.
3. `features/order.json`: remove the `features/knowledge_and_rumor/` entry from `order`
   (`order.json:45-47`), add `{ "path": "features/implemented/knowledge_and_rumor/", "when": "<YYYY-MM-DDTHHMM>" }`
   at the top of `finished`.
4. The three quest READMEs: the `STALE WHERE IT TOUCHES KNOWLEDGE` banner (lines 3–7) removed and their
   knowledge sections rewritten against the real API (M5 step 19): `knowledge::holds` fully qualified,
   `arm_actor(id, goal)` with no memories parameter, topic `talk` not `word`, a JSON pack not a Rust
   type, `EngineMessage::Journal` not a casebook.
5. `m0_evidence/NOTES.md` carries the appended records: M1's optional re-observation (if fired), M3's
   Q4 re-measurement, M4's C1 verdict, M5's close-out pointer — appended, never edited.

## Reconciliation record (2026-09-04 / 05)

Three critiques were written against this plan and then applied in place — `critique_contract.md`
(the spec's test contract, bullet by bullet), `critique_executable.md` (executability by an
empty-context agent, ~180 anchors re-verified) and `critique_arithmetic.md` (every derivation
recomputed). They are deleted; what they changed is summarised here so the plan does not ship with
its own review notes beside it.

**Applied (blockers and majors):** the M0 inputs are M0b's `v6_both` — 24 keys / 21 measured rungs
under their own names, the paragraph before `turn.j2:194`, +1387 bytes per fixture (D13, D17, D18,
`03_assets.md`); M3 adds no rung, only `known_from`, and owns the Q4 re-measurement; the self-subject
rule moved out of the salience product into `may_carry` so the flat-table identity is exact (D51);
standing facts have no warm life (D52); `VOLUNTEER_HEAT` re-solved from the ward boundary to 0.119,
`REHEAT_TO` 0.1309, every warm-life cell −0.59 gh (D53, `02_numbers.md`); the band is defined at
`seconds_per_day` 3600 with a 0.4 s step, realised backstops read `wards_reached` (air rows), the fast
end is stated in bells and X is the mint ward's own exit rate (D22, D54); claims are templates —
`mint_claim` substitutes the subject's name with `{subject}`, garbles the subject only, and routes
through `install_fact` (D58); `arm_actor` lands in M1 with a test (D37); the occasion store is M4's
with an `offered` flag so a rendered verb survives its hour (D34, D62); the player's affinity is 1.0
(D26); `Held::seeded(fact)`, `MintKind.said`, `install_fact(own)`, `ItemWith`, `relevance_seated`,
one `volunteers`, one `stage_stir`, one `poll_player` call, `World::player_id()`, `WorldClock::new`,
`EngineCommand::PlayerSay` through `Engine::poll` — one name and one home per thing; the
projection-walking `fact_source_reaches_no_projection` (D60) and A-1's walk over all four
action-reachable mints (D61); the household statistic is a fraction at every sample with ≥ 2 kin
required; the 20,000 guard runs saturated and the RSS delta is recorded; `Holding` is 88 bytes; the
time-scale roll-count test exists; the seep knob is declined (D55); `DOOR_SHUT_REACH_M` is the 10 m
idle leash (D59); M5's door script uses `raise-word` and Jonet Kett's High Wick home leg; M5's Group E
collapsed to one appended pointer (M0b already scored it; `scripts/m0b/` is gone); `grep` everywhere;
every drifted anchor corrected in the table at the top of `00_decisions.md`.

**Rejected:** the arithmetic critique's "at 60× `sleep 30` is one stir, use `sleep 720`" — at 60× one
real second is 24 game minutes, so `sleep 30` *is* half a game day; the cross-ward half of that
finding (nobody walks a ward at 60×) is applied. M3's "hops 0 wins over cold" reversal — unmeasured;
the measured rig is cold-first, and only the own line outranks it (D18). Keeping 0.115 and
downgrading the slow end to an expectation — the boundary solve keeps the spec's promise for one
division. The contract critique's `stir` bump "on a genuine re-heat above the pre-sweep value" for a
deposit — D28 is sweep + `stir_up` only. A sale mint — still declined (D32).
