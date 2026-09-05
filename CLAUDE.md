# PICK THIS UP FIRST — the knowledge layer is mid-build (2026-09-05)

`features/knowledge_and_rumor/` is **being implemented right now, across sessions**. M0–M3 are in;
**M4 and M5 are not.** If you are a new session and the user says "keep going", this is the work.

**Read `features/knowledge_and_rumor/plan/README.md` before anything else.** The plan is finished,
reconciled and authoritative — 10 files, ~14k lines, every decision numbered (D1–D62). Do not re-plan,
do not re-open a numbered decision, and do not redesign from the spec: the spec states the requirement,
the plan states what actually ships and why it differs.

## Where it stands

| | State |
|---|---|
| **M0 / M0b** — the prompt prose | **Done, measured against a live provider.** 110 one-shot calls over three candidate wordings; `v6_both` won and is frozen. Evidence: `features/knowledge_and_rumor/m0_evidence/` (`NOTES.md` is the justification for every shipped string; `strings_draft.toml` and `ignorance_rule.txt` were M1's byte-for-byte inputs). **Append to NOTES.md, never edit it.** |
| **M1** — the store, `holds()`, the sheet block | **Committed** (`0504d29`), plus the one golden re-bless alone (`18bd26d`) |
| **M2** — the ward air, the roll, salience, two mints, the measured band | **Committed** (`44d717d`) |
| **M3** — garbling, the chain, stage-local hops | **Committed (`a1381b1`) but NOT REVIEWED, and its Q4 re-measurement is deferred.** Reviewing it is task 1 below |
| **M4** — the player's side, the journal, `raise_word` | **Not started.** One attempt was interrupted mid-edit and fully removed (`knowledge/mint.rs` is byte-identical with HEAD again); nothing host-side was ever begun |
| **M5** — consequence, the tune, the landing | **Not started** |

## Do these, in this order

1. **Review M3 before building on it** — `a1381b1` is committed unreviewed, and that is the one loose
   end in the tree. It is the milestone where a garbled subject could put a name in front of a reader
   who does not know that person, which a green suite will not catch. Read `plan/M3.md`, then attack the
   code: is every garble a pure function of `(fact sequence, carrier id, hops)`; does a swapped subject
   really come from the *holder's* own `knows`; is `chain()` bounded against a `from` cycle; does Layer 2
   ride the existing player-adjacent scan rather than adding a scan of its own (the spec's risk 3); and
   is topic invariant across every hop of a walked chain.
2. **Fire M3's Q4 re-measurement**, or decide out loud that the feature ships with that threshold failed.
   See "Two live-provider jobs are owed" below.
3. **M4**, then **M5**, one at a time, each reviewed before it is committed. `plan/M4.md` and
   `plan/M5.md` are step-by-step; M4's sim half and host half are separate crates and can be built in
   parallel if you pre-agree the `EngineMessage` variant from `plan/01_api.md`.
4. **The landing**, at the end of M5 — the checklist is at the bottom of this section.

## How to do a milestone

One milestone at a time, strictly serial (D45). For each: read `plan/M<n>.md` in full plus the
previous milestone's "Notes for the next milestone", implement its numbered steps, then **review the
result adversarially before committing** — an implementer's own report is the least reliable evidence
there is. In M1 and M2 the review found a genuine blocker each time, and in both cases the code and its
tests were green while a spec promise was quietly broken:

- M1: eviction ranked on heat-at-learn rather than derived heat, so "coldest first" was false for every
  aged row — the store's whole bound at 20,000 people.
- M2: the test for the spec's headline claim (*a cold scandal out-travels a fresh squabble*) backdated
  only the ward's air and not the fact, so its four witnesses re-heated it on the next poll. It passed
  while testing nothing. In the same milestone the slow end summed warm mouth-hours at each 0.5 gh
  right endpoint while the window being measured is 0.145 gh long, so it printed 0.000 all day and its
  `< 1.0` assertion would have passed for a warm life of infinity.

So: for every number a milestone reports, re-run it yourself; for every test, ask what would make it
fail. Do not accept "the suite is green" as evidence a promise is kept.

## The gate — run at every commit boundary

```sh
cd /home/ran/src/rust/cathedralbevy
rustfmt --check --edition 2024 <each file you touched>     # per file; must print nothing
cargo clippy --workspace --all-targets --offline 2>&1 | grep -E '^(warning|error)(\[|:)' | sort | uniq -c
cargo test --workspace --offline
git diff --stat crates/cathedral-sim/tests/fixtures/prompts/ assets/prompts/    # MUST be empty
```

Four things about that, each learned the hard way:

1. **Never `cargo fmt`, and never tree-wide.** It formats every crate root regardless of the file list,
   and this tree is 601 hunks red for reasons that predate the feature.
2. **Clippy is a delta, not an absolute.** `-D warnings` exits 101 on **nine** sites that all git-blame
   to 2026-07/08, behind ~60 further warnings. They live in `clock.rs`, `custody.rs` (×2), `engine.rs`
   (×2), `homes.rs`, `notices.rs`, `prompt/mod.rs` and `round.rs` — **line numbers drift as the feature
   adds code, so match on the file and the lint, not the line.** None is in `knowledge/`. Assert the site
   list is unchanged from HEAD: a new warning in your own code is a failure, inheriting those nine is
   not, and cleaning them up is a separate job nobody has asked for.
3. **The goldens are frozen.** M1 spent the feature's one re-bless (+23 lines / +1387 bytes / 0
   deletions on all 22 fixtures). From M2 on, a non-empty diff under `tests/fixtures/prompts/` or
   `assets/prompts/` means something renders unconditionally that should not — find it, do not re-bless.
4. **`grep` is a shell function here, not the binary.** `rtk proxy grep` gives raw output.

## Numbers you may not move

M2 measured the cadence band and **no constant moved**. `crates/cathedral-backends/tests/pollen_cadence.rs`
must keep passing **unmodified**; if a change of yours moves a number in it, that is a finding about your
change, not a baseline to re-bless. The measured baselines:

- fast end, `Bed`: wards reached 1 / 5 / 6 / 7 / 8 at 0 / 0.5 / 1.0 / 1.5 / 2.0 game hours; 446 carriers
  at a game day (87% of the present cast), mean hops 1.04
- slow end, off-affinity `Craft`: 1 ward at every one of the day's 48 samples, expected crossings 0.136,
  nine carriers, none ever warm
- the separating line: `Bed` at heat 0.300 reaches 8/8 wards and 230 carriers over 12 gh, while a fresh
  off-trade `Craft` at 1.000 reaches 1/8 and 15. **Salience is not heat.**
- the household: 0.250 of the subject's kin hold it at a day against 0.905 of their wards' other
  standers, and 0 of 49 samples had the household ahead
- the flat-table identity is **exact**: `SalienceTable::flat()` against the salience factor deleted from
  the expression, `assert_eq!` on the whole census at 13 samples, and 0 differing lines of 625 headless
- cost at `--extra-ambient 20000`, saturated: store 4.8 MB live, `footprint_bytes()` 15.4 MB against a
  32 MB bound with 20,519 bodies all at `HOLDINGS_MAX`

Only M5's tune step may move a free parameter, and only by the division in `plan/02_numbers.md` §10
(`VOLUNTEER_HEAT` for the slow end, `STIRS_PER_GAME_HOUR` for the fast). **Never move a salience band** —
the flat-table identity is asserted against them.

One known, recorded departure: the wave crosses all eight wards in about **two game hours** (five real
minutes), sooner than the spec's prose implies. It is recorded as a measured correction in
`plan/02_numbers.md` with a ceiling assertion, and it is **not** fixable by the free parameter — the
pickups responsible happen on the mint bell at stir 0. If the user wants the player able to outrun their
own story, that is a mechanism change in M5, not a tune.

## Two live-provider jobs are owed

Both cost fractions of a cent, both **append** their verdict to `m0_evidence/NOTES.md`:

- **M3 owes the parroting re-measurement** (deferred at M3's close; reasons appended to NOTES.md). M0b left that threshold failing (7/8 on moonshot, 3/8 on
  openai) and proved prose cannot close it: openai's replies are terser, so the shared fact-plus-hedge
  core is the whole reply. M3's garbling of the `said` text was supposed to close it. Re-fire the eight
  `q4_wick_*` sheets on both providers with garbling live and score them by NOTES.md's own unchanged
  rule.
- **M4 owes an uncontaminated topic-choice probe.** M0's with-occasion probe proved nothing: the verb
  fence's own example line was byte-for-byte the correct answer. Re-fire with an example whose topic and
  text are unrelated to the scenario (a bread example on a law occasion). This is the only evidence that
  `raise_word`'s closed-topic tag — the whole safety argument for the spec's risk 4 — actually works.

```sh
cargo build -q -p cathedral-backends --bin cathedral-headless
LLM_MODEL=kimi-k3 ./target/debug/cathedral-headless --provider moonshot --one-shot FILE
./target/debug/cathedral-headless --provider openai --one-shot FILE
```

`LLM_MODEL=kimi-k3` is not optional for moonshot… (see the note below on the default). `--fake` does
**not** divert `--one-shot`; every call is billed. Keys are in `prompt_playgound/.env`.

## Harness notes for whoever picks this up

- **Fable 5.1 has no usage credits** as of 2026-09-05 — agents pinned to it die with a 429 saying so.
  Anything model-pinned should be re-pointed at the session default before launching.
- **Session usage limits interrupted this work three times.** A long unattended chain will lose an agent
  mid-edit; that is how a half-built M4 came to be sitting in the tree. Prefer committing each milestone
  as it passes its gate over leaving several in the working tree, and never let a build phase start on
  an unreviewed base.
- The `m0_evidence/` and `plan/` directories are committed on purpose. `m0_evidence/` is required by the
  spec's own test contract as the only record of why the shipped prose is worded the way it is.

## When M5 lands — the landing checklist

`plan/M5.md` Group G, and `plan/README.md` restates it: the three spec files updated to what actually
shipped (every departure in a table, a `Status:` line per milestone, a `## Numbers` section);
`git mv features/knowledge_and_rumor features/implemented/` keeping `plan/` and `m0_evidence/` inside it;
`features/order.json` updated; and the three quest specs' `STALE WHERE IT TOUCHES KNOWLEDGE` banners
removed with their knowledge sections rewritten against the real API — `knowledge::holds` fully
qualified, `arm_actor(id, goal)` with no memories parameter, topic `talk` not `word`, a JSON pack not a
Rust type, `EngineMessage::Journal` not a casebook.

---

@AGENTS.md
