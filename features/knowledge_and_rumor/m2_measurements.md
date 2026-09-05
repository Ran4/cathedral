# M2 — what the city actually did

The **reported** half of `plan/02_numbers.md` §9: a record, not a guard. Everything
asserted lives in `crates/cathedral-backends/tests/pollen_cadence.rs`,
`crates/cathedral-sim/tests/knowledge_tests.rs` and the inline module tests; nothing
in this file fails a build.

Measured on 2026-09-05, `develop` with M2 in the tree, on this machine (20 cores,
RTX 4070, **shared** — load average 5–13 from other users throughout; `perf` cannot
profile here — `perf_event_paranoid = 4` and no passwordless sudo — so every cost
figure is wall/user time from `/usr/bin/time -v` on the **release** binary, and §3
says what that noise floor does to a ratio).

**Re-measured after the 2026-09-05 review**, which found three of the first
record's numbers were not what they claimed: the ward-level "cold scandal" run had
backdated the air and not the fact (its witnesses re-deposited at 1.0 on their
first poll); the slow end's expectation read 0.000 by sample resolution; and the
crossing tally counted bodies milling on their leash across a ward line. All three
instruments were fixed and everything below is from the fixed tree. What changed is
listed in §8; no constant moved.

**And re-taken once more on the final tree** (2026-09-05, 17:32 onwards): every headless
figure below is from the release binary built from the sources as they stand — the one
change after that build is a rustfmt hunk in `pollen.rs` — and every harness figure is
from the `cargo test --workspace` that is this milestone's gate on the same tree. Nothing
here is carried over from a run on an earlier binary.

**Every cadence run is at the shipped clock**: `--seconds-per-day 3600
--start-office dayspring`, and `--trace-pollen` caps the watch step at 0.4 s
(`MAX_MOVEMENT_CATCHUP_SLICES × MOVEMENT_TICK_SECONDS`, the most walking one poll can
realise, D22). At `--seconds-per-day 120` a game day is 120 sim-seconds, no 270 m leg
completes inside one, and nothing below would mean anything.

---

## 1. The band, both ends, at the shipped clock

Two sources, and the rows say which — they differ by a body or two because the test
parks the player in an empty corner and carries no weather or shelters, while the
headless binary runs the shipped configuration. Each is internally deterministic (§2).
Ages are game hours **from the mint**, which is at the Dayspring bell.

- **T** — `cargo test -p cathedral-backends --test pollen_cadence -- --nocapture`
  (two samples a game hour)
- **H** — the headless tracer, §6's second command (`--pollen-per-day 48`)

`expect` is the model's own `expected_crossings = warm_mint_ward_game_hours × X_W / 24`
with `X_W` the mint ward's **debounced** exit rate from the tally (§4) — a diagnostic,
never asserted (D54). `exits` are realised exits of the mint ward by anyone holding
the fact, warm or cold.

### The fast end — `pollen.bed` (Bed, s = 1.00), minted at the Wickmarket

| age gh | | wards reached | carriers | warm | expect | holder exits |
|---:|---|---:|---:|---:|---:|---:|
| 0.00 | T | 1 | 4 | 4 | 0.000 | 0 |
| 0.50 | T | **5** | 15 | 15 | 0.000 | 0 |
| 1.00 | T | 6 | 50 | 50 | 1.736 | 1 |
| 1.50 | T | 7 | 109 | 109 | 5.225 | 10 |
| 2.01 | T | **8** | 156 | 156 | 7.880 | 11 |
| 5.01 | T | 8 | 307 | 307 | 29.858 | 27 |
| **6.52** (F1, HighWick + 1.5) | T | **8** | 353 | 353 | 35.468 | 31 |
| **9.53** (F1, Waning + 1.5) | T | **8** | 392 | 392 | 59.804 | 50 |
| 11.03 | H | 8 | 400 | 400 | 72.007 | 62 |
| 23.56 | H | 8 | 446 | 446 | 121.883 | 95 |
| **24.06** (F2) | T | **8** | 446 | 446 | — | 105 |

**F1 asks `≥ 2` at 6.5 gh and `≥ 4` at 9.5 gh; both read 8.** **F2 asks `== 8` at
24 gh; it reads 8** (`wards 8/8, carriers 446, warm 446, mean hops 1.04, holder exits
105`). Both pass, and both are *over*-satisfied by a wide margin — the wave is in
five wards' air half a game hour after the mint and in all eight at **2.0 game hours
(five real minutes)**, not a game day.

**Why the plan's model was 5–10× slow, stated so nobody quotes it.** `02_numbers.md`
§4 put the first crossing at the HighWick bell (+5 gh) — "the first office after a
mint at the Dayspring bell has no bell in it" — and ≈ 21 holders walking out of Wick
then. It forgot that the mint **is** at the bell: everybody standing in Wick rolls
coin 0 on their first poll, within seconds of the mint, and the ≈ 11 who pick it up
are the Dayspring tide itself, already setting off for work in other wards. They
deposit at their next poll wherever they are then standing. That is the whole
mechanism of the fast end and it needs no bell: the mint ward's own standers are the
column. `STIRS_PER_GAME_HOUR` cannot move a coin-0 pickup, so this is transport —
the commute, the walk speed, the clock — and not a density, and §10.1's division was
**not** invoked. The correction is appended to `02_numbers.md` ("Measured
corrections").

**The fast side is now asserted too**, so a speed-up cannot pass silently: at the
first sample at or after **1.0 gh** the fact must be in **fewer than all eight**
wards' air (it reads **6**). The spec's own reason — "faster and nothing can ever be
outrun": the player walks the Wickmarket to the Shambles well (the nearest Weigh
place, 224 m) in 1.15 gh — and the run shows what "outrun" means here: a single
witness with a leg to walk (Jonet Kett, into Weigh at 0.30 gh) beats the player to
Weigh, and the wave beats them to two more wards, but not to all of them.

Carriers at one game day: **446 of ~515 present bodies (87%)** against the closed
form's `1 − exp(−2 × 0.1193 × 12.9843) = 0.955` at c̄ and 0.893 as the population
mean. Mean hops 1.04.

### The slow end — `pollen.craft` (Craft, off-affinity, s = 0.20 × 0.60 = 0.12)

Minted in the same breath, at the same place, by the same four mouths — none of whom
holds the subject's `revenue_worker` trade.

All **T** (the S1 test traces every sample of a full game day):

| age gh | wards reached | carriers | warm | expect | holder exits |
|---:|---:|---:|---:|---:|---:|
| 0.00 | 1 | 4 | **4** | 0.000 | 0 |
| 0.50 | 1 | 4 | **0** | 0.000 | 0 |
| 1.00 | 1 | 4 | 0 | 0.111 | 1 |
| 2.01 | 1 | 5 | 0 | 0.222 | 2 |
| 5.01 | 1 | 6 | 0 | 0.271 | 2 |
| **11.03** (S2) | **1** | 9 | 0 | **0.225** | 8 |
| **24.06** (S1) | **1** | 9 | 0 | **0.136** | 12 |

**S1 asks `0 < expected_crossings < 1.0` and `wards_reached == 1` at 24 gh: 0.136
and 1.** **S2 asks `exp(−expected) > 0.5` and `wards_reached == 1` at 11 gh: 0.799
and 1.** `holder_exits_same_trade` is **0** at every sample, as it must be — no
seeded mouth is a `revenue_worker`.

**The expectation is the closed form's own integrand now, at any cadence.** The
first record read `expect 0.000` for the whole day because the warm mouth-hours
were counted at the end of each 0.5 gh gap and the off-affinity witness is warm for
`12·log₂(0.12/0.119) = 0.145 gh` — the window closed between two samples and S1
was asserting `0 < 1`. `CrossingTally` now integrates each holder's warm window in
closed form: their warm life from now is `τ·log₂(heat(now) × salience /
VOLUNTEER_HEAT)` game hours, so the warm span inside a gap is
`[max(previous sample, learned_on), min(now, cold_at)]` clamped to the gap. Four
witnesses × 0.145 gh = **0.58 warm mouth-hours**, whatever the cadence:

| cadence | warm mouth-hours | X_Wick | expect at 11 gh | expect at 24 gh |
|---|---:|---:|---:|---:|
| T, 2 samples/gh | 0.58 | 9.43 (12 gh window) → settles | 0.225 | 0.136 |
| H, 384/day, 1.41 gh only | 0.58 | 21.1 — the Dayspring tide annualised at a 0.0625 gh gap | 0.51 at 1.41 gh (0.26 at 0.38 gh, the first sample with a believed exit) | — |

(The 384/day row's `expect` is the same 0.58 mouth-hours over a divisor that is the
tide itself: at a sixteenth of a game hour between samples the tally believes the
Dayspring column's every exit inside the first game hour and a half, and `X_Wick`
reads 21 a day. It is printed to show the integrand does not depend on the cadence;
the divisor does, which is why `X` is stated at two samples a game hour.)

The plan's **0.057** is the same integrand — `4 × 0.145 = 0.58` — divided by the
plan's *leg-derived* `X = 2.368`; the run divides by the tally's own X_Wick, which
also counts the water and food errands the round makes between legs (§4). Same
model, bigger divisor, still under one; and the realised backstop is the one that
carries the slow end either way: **no deposit outside Wick, at any sample, all
day** — the boundary solve of D53 (the Wickmarket is **75 m** from the nearest ground
of another ward, `the_standing_wards_are_a_patchwork`) holds with the witnesses cold
by 0.145 gh, before any leg reaches one.

The warm window itself, at `--pollen-per-day 384` (a sample every 0.0625 gh):

```
age 0.00 gh  warm 4      age 0.06 gh  warm 4      age 0.13 gh  warm 4      age 0.19 gh  warm 0
```

— `t_warm ∈ (0.125, 0.1875)` against the solved **0.145 gh**.

Nine carriers over a game day, every one at hops 1 and none of them ever warm enough
to pass it on. Four of the nine end the day standing outside Wick — carriers walk,
and the *air* still holds a row in Wick and nowhere else, which is exactly the
distinction D54 draws.

### The one line that is the whole point

| | pickup ∝ | deposit gate | measured over 12 gh |
|---|---|---|---|
| `Bed` at heat **0.300** — the fact minted 20.85 gh earlier, its four witnesses at 0.300 | 0.300 × 1.00 = 0.300 | 0.300 > 0.119 ✓ | **8/8 wards, 230 carriers (214 warm)** |
| `Craft` (off-trade) at heat **1.000** — minted fresh by the same four | 1.00 × 0.12 = 0.120 | 0.120 > 0.119 ✓ (barely) | **1/8 wards, 15 carriers (0 warm)** |

The cold scandal's own trajectory: 1 → 3 → 4 → 6 → 6 → 6 → 7 → **8** wards at
0.0 / 0.5 / 1.0 / 1.5 / 2.0 / 2.5 / 3.0 / 3.5 gh. A scandal three-quarters cooled
out-travels a fresh trade complaint by eight wards to one.
`a_cold_scandal_out_travels_a_fresh_squabble_across_the_city`.

**The first record's "407 carriers at air heat 0.300" was false**, and this is how:
the run backdated the *air* with one rewound sweep and left the four witnesses at
their mint stamp, so they stood at heat 1.0, re-deposited at 1.0 on the first poll
(`deposit` takes the maximum), and the Bed row was back at 0.9998 within 0.4 s —
a fresh scandal against a fresh squabble. The test now backdates the **fact**
(`plant_for_measurement(.., game_days = minted_at − 20.85/24)`), so the witnesses
derive 0.300 and re-deposit at 0.300; it asserts every witness stands at 0.300 and,
after the first round tick, that the row is **still ≤ 0.31** — the guard that would
have caught it.

### The same-trade ear — reported, never asserted

`02_numbers.md` §4: a `Craft` witness of the subject's own trade has
`s = 0.20 × 2.00 = 0.40` and a warm life of **21.0 gh**, so one such witness
contributes ≈ 2.1 warm crossings a game day. Bounding that would be bounding the
affinity table. `the_same_trade_ear_is_reported` plants one `Craft` fact about the
pack's own subject (`p0043`, `revenue_worker`) seeded to the three nearest
`revenue_worker`s, minted where the nearest of them stands, and prints:

```
[pollen] same-trade pack: trade revenue_worker, subject p0043, mouths fe2tn, p0044, fa8tn —
         all three standing in Weigh (the Tallage) — minted where fe2tn stands
```

The Wickmarket pack's own subject; the cast holds exactly four `revenue_worker`s, so the
three that are not Ede Kett are the mouths, and every one of them hears the fact at
`0.20 × 2.00 = 0.40` (asserted). Selected rows of the 49-sample trace (`same` is holder
exits of the mint ward by a same-trade holder; `warm-any` is the ear's own figure —
exits of **any** ward by a same-trade holder still warm on it):

| age gh | wards | carriers | warm | expect | holder exits | same | warm-any |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 0.00 | 1 | 3 | 3 | 0.000 | 0 | 0 | 0 |
| 1.00 | 1 | 5 | 3 | 0.081 | 0 | 0 | 0 |
| 1.50 | 3 | 12 | 3 | 0.455 | 2 | 1 | 1 |
| 3.01 | 3 | 26 | 3 | 2.675 | 9 | 3 | 3 |
| 5.01 | 3 | 46 | 4 | 3.080 | 12 | 3 | 3 |
| 6.52 | 6 | 55 | 4 | 3.067 | 14 | 3 | 5 |
| **11.03** (nightfall) | **6** | 76 | 8 | 5.536 | 26 | 5 | 9 |
| 13.03 | 6 | 85 | 3 | 5.699 | 28 | 5 | 12 |
| 20.55 | 6 | 118 | **3** | 9.202 | 39 | 5 | 12 |
| 21.06 | 6 | 119 | **0** | 9.230 | 39 | 5 | 12 |
| **24.06** | **6** | **128** | 0 | 8.762 | 51 | 5 | **12** |

Three things to read off it, all of them the affinity table and none of them a model change:

- **The warm life is the table's.** The three mouths stay warm from the mint until the sample
  at 21.06 gh, where `volunteering` drops from 3 to 0 — the warm-life table's **21.0 gh** for
  `Craft × own trade` (`12·log₂(0.40/0.119)`), to within a sample. The off-affinity pack's
  witnesses went cold at 0.145 gh. The extra warm mouths mid-run (8 at nightfall) are the
  **no-trade quarter**: a pauper hears `Craft` at ×1.4, so a hops-1 pauper carrier
  (`0.85 × λ^t × 0.28`) stays above the gate for the first ≈ 12 gh — the one other ear
  this topic has.
- **The reported term.** `02_numbers.md` §4 predicts ≈ 2.1 warm crossings a day per witness,
  computed at the plan's leg-count `X = 2.368`. Measured: **12 warm same-trade exits of any
  ward over a game day, 4.0 per witness** — between that prediction and the same arithmetic
  at the debounced exit rate of the ward the three stand in (the headless day's
  `X_Weigh` is 6.70: `21.0/24 × 6.70 ≈ 5.9`).
  `holder_exits_same_trade` (the mint-ward split) reads 5; the mint ward is Weigh, where the
  Tallage keeps them, so most of their crossings are out of the wards they walk home to.
- **A `Craft` fact heard by its own trade travels.** Six wards' air and 128 carriers after a
  game day, against the off-affinity pack's one ward and nine. "Nothing to anyone but a
  cooper, everything to a cooper" is a factor of 3.3 in salience and a factor of 145 in warm
  life, and this is what those two factors do on the shipped city. Bounding it would bound the
  table, so nothing here is asserted beyond the pack's shape and that a same-trade mouth is
  still warm at nightfall.

`the_same_trade_ear_is_reported`.

---

## 2. The flat-table identity — exact, not close

Two runs, byte-identical but for the lever:

```sh
run() { cargo run -q --release -p cathedral-backends --bin cathedral-headless -- \
    --fake --watch-clock 1 --seconds-per-day 3600 --start-office dayspring \
    --trace-pollen --pollen-per-day 48 --pollen-seed "The Wickmarket" "$@" \
    | grep '^\[pollen\]'; }
run --pollen-no-salience > /tmp/pollen_base.txt
run --pollen-flat        > /tmp/pollen_flat.txt
diff /tmp/pollen_base.txt /tmp/pollen_flat.txt
```

**Empty.** 0 differing lines out of **625** — 48 samples × 13 lines plus the plant
line: the header (store bytes, holdings, air rows, the roll bound, X), the eight
per-ward populations, mean curiosities and exit rates, and all eleven per-fact rows
(wards reached, carriers, warm, expected crossings, holder exits, mean hops, max heat,
age, carriers by ward). The salience-on run differs from both by **1,124** lines
(`diff … | grep -c '^[<>]'`), so the levers act.

`pollen_flat_reproduces_the_no_salience_run` asserts the same thing on the typed
`PollenCensus` — **field for field**, `assert_eq!` on the whole struct at every
sample, not a diff of a print. 13 samples over 6 gh; the last carries 2,765 holdings,
88 air rows and 371,974 store bytes, identical under both levers.

That is the identity in the form `02_numbers.md` §5 asks for: `--pollen-no-salience`
**deletes** the factor from the roll (`salience_for` returns a bare `1.0`), so what is
being reproduced is the model as it would be with no salience term at all — not a
baseline taken alongside the bands, which could not detect a band that was already
wrong. It holds because the self-subject rule sits in `may_carry`, outside the product
both levers touch (D51): the pack's subject `p0043` stands in the mint ward and never
rolls under either.

**Determinism, end to end** (Verification 10): the same command twice, `diff` of the
two `[pollen]` streams — **empty**, all 625 lines. Claimed for Layer 1 and for
headless runs only: in the game, Layer 2's real-seconds gate will make which transient
pairs get scanned vary run to run, and Layer 2 does not exist yet.

---

## 3. The cost guard — the layer measured in the guard's shape and in the game's

**What is measured, and in which shape.** ON is the release binary with the pack planted at the
Wickmarket and `--pollen-saturate` — nine rows in every ward's air at heat 1.0 and `live` filled to
`FACTS_MAX_LIVE = 256` with standing filler rows — so every poll rolls the full per-ward load and the
deposit walk runs at the length it is bounded at. OFF is the same command under
`CATHEDRAL_NO_KNOWLEDGE=1`. **Both arms carry `--trace-pollen --pollen-step`** (departure 1 in §8:
the plan's OFF arm had no step cap and would have run 200 polls against the ON arm's 1,200).

Two shapes, because the plan's guard and the game are not the same run:

- **the guard's shape** — `--pollen-step 3`, one `round::tick` per 3 s of sim time, 1/60th of the
  game's 20 Hz, so every tick catches up ~2,500 due person-polls in one batch with the store hot in
  cache. Per person-poll this is a **lower bound**; per *ratio* it is an upper one, because a 3 s
  step also cuts the pump's own work per game hour by about seven (a poll realises at most 0.4 s of
  walking) and so shrinks the denominator;
- **the game's shape** — `--pollen-step 0.05`, the 20 Hz tick, ~5 person-polls per tick interleaved
  with the round's whole-cast passes. This is the number risk 3 is about.

Each row is **three interleaved ON/OFF pairs** (ON-OFF, OFF-ON, ON-OFF), the median with the
min–max spread beside it, on a shared box whose one-minute load average ran from 17 (the gate's
compile had just finished) down to about 3 over the hour; the 20,000-body pairs in the game's shape
and at 6 gh are single pairs, at seven and fifteen minutes an arm. `user` seconds are the figure —
wall time adds the box's other tenants — and the RSS is `/usr/bin/time -v`'s maximum resident set.
The load figures (`holdings`, `store`, `rolls/gh (bound)`) are identical on every repeat of a row,
so the load is the same run every time and only the clock moves.

| bodies | shape | pairs | ON user s, median (min–max) | OFF user s, median (min–max) | ratio | ON wall / OFF wall | RSS ON / OFF (MB) | holdings | store | rolls/gh (bound) |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 0 | 1 game day, 3 s step (the plan's guard) | 3 | 2.83 (2.81–3.10) | 2.72 (2.56–2.76) | **+4.0%** | 2.9 / 2.8 | 34.7 / 33.6 (+1.1) | 2977 | 416.6KB | 10,640 |
| 1,000 | 1 game day, 3 s step | 3 | 12.73 (12.68–13.01) | 11.40 (11.40–11.43) | **+11.7%** | 12.8 / 11.4 | 61.4 / 58.9 (+2.5) | 8878 | 1016.1KB | 31,578 |
| 2,000 | 1.2 gh, 0.05 s step (the game's shape) | 3 | 14.09 (14.05–14.42) | 14.32 (14.23–14.62) | **-1.6%** | 14.1 / 14.3 | 57.0 / 57.0 (+0.0) | 4377 | 648.1KB | 51,128 |
| 2,000 | 1.2 gh, 0.05 s step, re-taken later on a quieter box | 1 | 13.65 (13.65–13.65) | 13.76 (13.76–13.76) | **-0.8%** | 13.7 / 13.8 | 57.4 / 57.0 (+0.4) | 4377 | 648.1KB | 51,128 |
| 20,000 | 1.2 gh, 3 s step | 3 | 182.06 (180.11–195.27) | 187.08 (174.15–195.57) | **-2.7%** | 182.8 / 187.4 | 366.6 / 367.4 (-0.9) | 39497 | 4854.8KB | 421,188 |
| 20,000 | 1.2 gh, 0.05 s step (the game's shape) | 1 | 429.65 (429.65–429.65) | 454.89 (454.89–454.89) | **-5.5%** | 429.8 / 455.1 | 549.5 / 540.4 (+9.1) | 38517 | 4770.6KB | 447,296 |
| 20,000 | 6 gh, 3 s step (the caps filled) | 1 | 919.29 (919.29–919.29) | 961.61 (961.61–961.61) | **-4.4%** | 919.8 / 962.1 | 367.2 / 367.7 (-0.5) | 100745 | 10569.1KB | 451,330 |
| 2,000 | 1.2 gh, 0.05 s step, re-taken later on a quieter box, **bare** (nothing planted, the two shipped rows) | 1 | 13.55 | — | -1.5% vs OFF | 13.6 | 57.1 | 330 | 65.0KB | 5,858 |
| 2,000 | 1.2 gh, 0.05 s step, re-taken later on a quieter box, **pack** (the pack, air not saturated) | 1 | 13.24 | — | -3.8% vs OFF | 13.3 | 57.4 | 2183 | 353.2KB | 41,120 |

One-minute load average at each arm's start, in order: 17.27 16.21 15.15 15.15 14.34 13.27 13.27 10.79 9.44 8.37 7.94 7.13 6.88 6.52 5.80 5.30 4.50 3.94 3.52 5.05 8.54 4.32 7.31 2.61 6.41 3.04 2.39 1.81 3.21 2.59 4.22 4.04 4.56 4.25

The first 2,000-body `bare` and `pack` arms landed under a load burst (wall 22.1 s against 18.1 s of
user time for `pack`) and are not in the table; both were re-taken with one more ON/OFF pair once the
box was quieter, and all four then sit within ±4% of each other — nothing planted, the pack alone, the
pack saturated, and the layer off are the same run at 2,000 bodies in the game's shape.

**The verdict against the 15% budget.** The layer's cost is **inside the run-to-run noise** wherever
the pump is large: at 2,000 bodies in the game's shape the ON median is *below* the OFF one
(−1.6%, spread ±2%), and at 20,000 in both shapes the same (the coarse guard −2.7% on a
±4% spread, the game's shape −5.5% and the six-game-hour pair −4.4%, single pairs whose sign is the noise's — the ON arm is not faster, the box was quieter). Where it is resolvable at all is the small pump: **+4.0% at 0 bodies and
+11.7% at 1,000, both in the guard's coarse shape** — the per-poll work is the same ~6 µs, but a
3 s step makes the denominator seven times smaller than the game's, so the ratio is a ceiling
for the game and still under 15%. The 1,000-body coarse figure is the one to quote as "at the
budget"; nothing in the game's own shape comes near it. Arithmetic behind the noise, so the
20,000 rows are not read as "free": 20,520 bodies × 6 polls per game hour × 1.2 gh ≈ 148k
person-polls at ~6 µs is ≈ 0.9 s, 0.5% of a 182 s pump — exactly what a ±4% spread cannot see.

**`02_numbers.md` §6's reverse-index decision** hangs on "step 20e's saturated wall-clock ratio
exceeds 15%": it does not, in either shape, so **`seeded_by_actor` is not added**. One caveat on
the number it was to be made on: the `live` filler rows are `decays: false`, which the deposit walk
skips at its first test, so the walk measured here is 256 iterations of the cheap branch. The
decaying branch (`may_carry`, a `seeded.contains` on an empty set, a binary search over ≤ 6 rows)
costs of the order of 20–30 ns more per filler; over the 148k person-polls of a 20,000 row that is
under a second on 182, and cannot move the verdict. A filler that is decaying but in no ward's air
needs an air-stripping entry that does not exist yet; if M3's Layer 2 measurement wants the walk at
its dearest branch, that is the one-line addition to `fill_live_for_measurement`.

**The deterministic half of the guard.** `the_store_stays_bounded_at_twenty_thousand` (the
ignored test, run by hand in release):

```
[pollen] 20520 bodies seeded in 5.4 s
[pollen] saturated: 9 rows in every ward's air, 245 filler facts to a live store of 256
[pollen] 300 polls over 6 game hours in 891.1 s wall            (2.97 s a poll)
[pollen] t=0.5417gd facts 256 holdings 100745 air 88 store 10569.1KB rolls/gh(bound) 451330.0
[pollen] footprint at 20000 extra ambient (20520 bodies): 10822762 bytes (10.3 MB);
         peak RSS of this process 365.8 MB
test the_store_stays_bounded_at_twenty_thousand ... ok   (891 s; every actor ≤ HOLDINGS_MAX)
```

and the synthetic bound, `the_store_footprint_is_bounded` (20,519 bodies × 6 rows, every row with a
`from`, then `live` at 256):

```
[pollen] store footprint at 20519 bodies × 6 holdings: 16131082 bytes (15.4 MB)
[pollen] Knowledge::clone() at 6 live facts: 0.0096 ms; the whole World::clone() (20,520 characters):
         18.527 ms — the store is 0.05% of what a catalog sale pays
[pollen] Knowledge::clone() at 256 live facts: 0.1434 ms; the whole World::clone() (20,520 characters):
         18.991 ms — the store is 0.76% of what a catalog sale pays
```

The first record's 11.9 MB for the synthetic store left `from`'s heap out of the formula
(`02_numbers.md` §6 had budgeted it, at 17.4 MB); counted, the synthetic store is **16,131,082 bytes (15.4 MB)** and
the real crowd's after six saturated game hours **10,822,762 bytes (10.3 MB)** over 100,745 holdings — fewer bytes a row than the synthetic store because every row picked up off a `debug_seed_air` row (`via: None`) carries `from: None` and no heap — both under the 32 MB bound. **The
peak-RSS delta cannot check that formula** at this size: the six-game-hour pair, with 100,745 holdings and a 10.3 MB store live, reads ON 367.2 MB against OFF 367.7 MB (**−0.5 MB**), and the game-shape pair +9.1 MB with a 4.8 MB store. A process whose high-water mark is set by a 20,520-body world's own transients — every catalog sale clones the whole `World`, 19 ms and tens of megabytes at that count — does not move by the size of a store that sits under it. It is recorded because
the plan asks for it, and the `footprint_bytes` arithmetic is what the bound is asserted on.
`Knowledge::clone()` at the cap (`live` and `by_id` are the only deep copies; `holdings` and `air`
are behind `Arc`) is **0.143 ms** against 19.0 ms for the whole `World` (0.76%; 0.010 ms at 6 facts) — M1's note 14, answered at 256 and not at 6: `live` does not need an
`Arc`.

---

## 4. The city's own inputs, re-confirmed

### c̄, over all 519 authored sheets

```
[pollen] c̄ over 519 authored sheets: mean 0.1193, median 0.082, p10 0.062,
         p90 0.192, min 0.030, max 0.392
[pollen] worst per-roll chance over the whole cast: 0.549 (p00au on Bed)
[pollen] the generated crowd's bound: CURIOSITY_CEILING 0.6 × the widest base × affinity 1.6 = 0.960
```

`02_numbers.md` §1 says **0.1193** (median 0.082, p10 0.062, p90 0.202, min 0.030,
max 0.392). Every figure reproduces; p90 reads 0.192 against 0.202, which is the
nearest-rank convention and not a cast change. **The clamp never binds**, as the fold
the plan specifies: `pickup_chance` — the roll's own function — over every authored
body × all nine of the pack's rows at heat 1.0; the worst is **0.549**, `p00au` (the
most curious no-trade body) on the `Bed` fact, exactly `0.392 × 1.00 × 1.40`. And for
a crowd nobody authored, whose curiosity is always the derived one: its ceiling
(0.60) times the widest `base × affinity` in the table (Bed × the ×1.6 ear = 1.6) is
**0.960 < 1**, so no generated citizen clamps either.

### The standing wards, at spawn

| ward | standing | c̄ |
|---|---:|---:|
| Bell and Sluice | 193 | 0.137 |
| Weigh | 63 | 0.105 |
| Reed | 63 | 0.126 |
| Wick | 53 | 0.115 |
| Fabric | 45 | 0.109 |
| Cloth | 45 | 0.113 |
| Wallwright | 27 | 0.089 |
| Cinder | 26 | 0.093 |

515 present bodies (the player included), **7.4 : 1** most to least. Against
`02_numbers.md` §1's Voronoi-over-`spawn_location` figures (192 / 66 / 62 / 53 / 45 /
46 / 29 / 26): Wick, Fabric and Cinder to the body, and three wards within three
people. The differences are `World::ward_at` after `Round::seed` against a Voronoi
over the sheet's `spawn_location`, not a cast edit. This is why every cadence figure
below is stated per ward.

### The ward grid, and the ground it describes

`cargo test -p cathedral-sim --lib the_ward_grid_is_the_size_it_says` →
**9,464 cells, 508 ambiguous (5.37%), 321 marks**, exactly `02_numbers.md` §3's
`WARD_CELL_M = 8.0` derivation. `the_ward_grid_matches_the_exact_search` agrees with
the exact 321-mark search at 10,000 pseudo-random points spanning x ∈ [-500, 500],
z ∈ [-600, 500] — **inside the box and outside it**, which is what makes the grid
exact rather than approximate.

`the_standing_wards_are_a_patchwork` (a diagnostic, named for the hypothesis it
tests and refutes): every ward is **one connected piece** (8 pieces in all,
4-connected over the settled cells), and the Wickmarket stands **75 m** from the
nearest ground that is not Wick's — the `d_b ≈ 82 m` the slow end was solved against
(D53), inside the 60–110 m the test allows before it would ask for a re-solve.

### X — measured, and why it is reported and never asserted

The tally is **debounced** (`CrossingTally`): a ward change is believed only when the
body has displaced at least 50 m from where it was last seen settled (twice the
widest authored idle leash, so a mill can never) **and** the new ward is still there
at the next sample. So it can only undercount — a transit shorter than about three
sample gaps is not resolved — and the raw flip count is printed beside it.

The city's own rate, off a game day with nothing planted (**H**):

| `--pollen-per-day` | sample gap | X, debounced, city mean | X_Wick |
|---:|---|---:|---:|
| 12 | 2.0 gh | **1.40** | 1.69 |
| 48 | 0.5 gh | **4.20** | **5.45** |
| 192 | 0.125 gh | **6.75** | 8.64 |

and the harness's own comparison over the twelve game hours from the Dayspring bell
(`the_crossing_tally_does_not_count_the_mill`, **T**), debounced (raw):

```
[pollen] X over 12 gh from the Dayspring bell, debounced (raw): 1.79 (5.69) at 0.5 samples/gh,
         5.85 (11.79) at 2, 9.81 (17.13) at 8; Wick 2.70 / 9.43 / 14.24
```

Two things are in those numbers, and they are different things:

1. **The mill, now removed.** Half of the raw count at every cadence was bodies
   milling on their idle leash across a ward line — 11.79 → 5.85 at the harness
   cadence. The first record's `X 8.21` (48/day) and `X_Wick 9.55` were that count.
2. **The errands, still there.** The debounced count still rises with the cadence
   because the round is more than its legs: between legs people fetch water and
   queue at a stall, and a 150 m errand across a boundary and back is a real
   crossing a 0.5 gh gap cannot resolve and a 0.125 gh gap can. The plan's
   `X = 2.368` counted `rounds.json` legs only; the city crosses more often than it
   commutes.

So X is stated **at the harness cadence** — two samples a game hour, the plan's
own choice — and `X_Wick = 5.45` (H) / 9.43 (T, the busier Dayspring-to-Lamplight
half only) is the divisor every expectation in §1 uses. The test asserts the
signature of an instrument that undercounts short walks and not one that counts
the mill: monotone in the cadence, at or under the raw count, and at eight samples
a game hour the debounce removes at least half a crossing per person per day.

It matters in exactly one place and in the safe direction: `expected_crossings`
divides by `X_W`, so a larger `X_W` makes the slow end's expectation **larger** and
S1 harder to pass. It passes.

### The pack `--pollen-seed "The Wickmarket"` chose

```
[pollen] planted 9 facts at The Wickmarket (-17.375, 248.375), ward wick;
  mouths bn5jk Jonet Kett (civic_officer), dv8ll Osanne Vell (chandler),
         p003t Tib Stott (pilgrim), p006f Betriss Sedge (garment_worker);
  subject p0043 Ede Kett (revenue_worker)
```

Exactly the four mouths and the subject `02_numbers.md` §4 computes both ends from.
None of the four is a `revenue_worker` (so the `Craft` fact is ×0.6 for all four) and
none is in the `Bed` ear (so the `Bed` fact is ×1.0 for all four);
`the_cadence_pack_seeds_four_off_affinity_mouths` pins all of it, plus that the
subject stands in the mint ward and never holds their own fact.

---

## 5. A standing fact, and a household

**A standing fact is answerable and never loud** (D52). A `decays: false` `Talk` row
seeded to three Wickmarket mouths:

```
[pollen] the standing fact after a game day: wards 0/8, carriers 3, warm 0
[pollen] one stir after the ask:             wards 1/8, carriers 4 (3 seeded), warm 0
[pollen] and 24 gh after one ask:            wards 1/8, carriers 8, warm 0,
                                             by {Fabric: 2, Wick: 5, Weigh: 1}
```

A game day of ticking adds **zero** non-seeded holders and puts **no** row in any
ward's air. One re-heat at `REHEAT_TO = 0.1309` puts a row in the one ward it was
asked in and gives it to **one** new mouth on the first stir (§4 computes ≈ 0.12
expected) and **five** over the row's whole 44 gh life (§4 computes ≈ 4). `wards_reached`
is 1 at every sample of both spans, and no holder ever `volunteers` it — every one of
them is at heat `0.85 × 0.1309 × 0.15 = 0.0167`, well under the gate. The five who
end up standing in Fabric and Weigh walked there; the air did not follow them.

**The household hears it after the city.** A `Bed` fact about the subject with the most
kin present in the shipped city (`a4anh` Ansel Hobbe, **4** kin), seeded to four mouths
outside the household:

```
[pollen] household 0.250 vs city 0.905 at 24.06 gh; integrated 0.173 vs 0.709 over 49
         samples; first household pickup 7.52 gh, first non-seeded city pickup 0.50 gh;
         0 sample(s) where the household led
[pollen] mean pickup chance at heat 1.0 over 49 samples: kin 0.0101 against the same
         wards' other standers 0.1423
```

One kin of four holds it after a game day against 91% of the city; the household's
first pickup is **fifteen times later** than the city's. The subject never holds it as
news at any sample — `may_carry`, outside the product. And the draw-free complement,
which cannot pass by luck: the kin's mean per-stir chance is **14× below** that of the
other people standing in the same wards, which is `HOUSEHOLD_DAMPING = 0.15` entering
the roll directly.

---

## 6. Reproducing all of it

```sh
cd /home/ran/src/rust/cathedralbevy

# The band, the identity, both invariants, the household, the inputs, the same-trade
# ear, the tally's debounce — everything asserted, with the per-topic tables printed.
cargo test -p cathedral-backends --test pollen_cadence -- --nocapture

# The city's own inputs, nothing planted (§4's X and per-ward table); 12 / 48 / 192
cargo run --release -p cathedral-backends --bin cathedral-headless -- \
    --fake --watch-clock 1 --seconds-per-day 3600 --start-office dayspring \
    --trace-pollen --pollen-per-day 48

# Both ends, per topic (§1)
cargo run --release -p cathedral-backends --bin cathedral-headless -- \
    --fake --watch-clock 1 --seconds-per-day 3600 --start-office dayspring \
    --trace-pollen --pollen-per-day 48 --pollen-seed "The Wickmarket"

# The slow end's warm window, resolved (§1)
cargo run --release -p cathedral-backends --bin cathedral-headless -- \
    --fake --watch-clock 0.06 --seconds-per-day 3600 --start-office dayspring \
    --trace-pollen --pollen-per-day 384 --pollen-seed "The Wickmarket"

# The identity, and determinism (§2) — the two `diff`s must be empty
# ... see §2

# The ground the wards stand on (§4)
cargo test -p cathedral-sim --lib the_standing_wards_are_a_patchwork -- --nocapture

# The footprint, and what the store costs a catalog sale at the cap (§3)
cargo test -p cathedral-sim --test knowledge_tests the_store_footprint_is_bounded \
    -- --nocapture

# The cost guard (§3): three interleaved pairs per row, medians reported
bash <the loop in §3>
cargo test -p cathedral-backends --release --test pollen_cadence \
    the_store_stays_bounded_at_twenty_thousand -- --ignored --nocapture
```

---

## 7. The city, in a real game

`CATHEDRAL_HEADLESS=1` throughout — the window is created and never mapped, so
nothing appeared on the desktop (D50). `config.ron` was backed up first and came back
byte-identical.

```sh
CATHEDRAL_HEADLESS=1 CATHEDRAL_FAKE_BACKEND=1 CATHEDRAL_DRIVE_TIMEOUT=180 \
  CATHEDRAL_DRIVE='wait-online; key KeyT; key KeyT; seize Ede Clove -> Grigor Ashe; \
    sleep 2; frame Grigor Ashe 4; commit Grigor Ashe; sleep 30; \
    shot after_the_arrest; quit' cargo run
```

`logs/session_780_2026-09-05_19_06_22` — re-run on the final tree, after the review fixes —
at the `T` key's 60× (so `sleep 30` is half a game day = 24 stirs, and nobody crosses a
ward — walking is real time, D22, so these are **in-ward** pickups). The game's window was
polled with `xwininfo` once a second for the whole run: 36 samples, every one `IsUnMapped`,
never in `xdotool search --onlyvisible`'s list and never the focus window; `config.ron` came
back byte-identical.

```
[drive] 2.1s seize Ede Clove -> Grigor Ashe
[smart actors] Ede Clove takes Grigor Ashe in charge for Tallage toll-house
[drive] 5.1s commit Grigor Ashe
[smart actors] fg2sh is committed to The Stone House
```

The mint fired on the commit and the word travelled. Of the session's 38 archived prompts,
six carry the line. Ede Clove, who was there, at hops 0:

> - **You saw this yourself:** a salt trader of the Weigh Ward (you don't know their
>   name) was taken in charge at In the Stone House, the civic gaol in the side court
>   behind the Bellstand today, for The Stone House

and five others at the hops-2 rung — `p007l` Ewart Lark, `bnrse` Rohese Sedge, and **three
generated citizens** with no authored sheet and outside the seeded earshot: `x00801` Betriss
Roper, `x00259` Corin of the Shambles and `x00518` Segwin Nineteen:

> - **The one who told you was not there either — they had it from the one who was:**
>   a salt trader of the Weigh Ward (you don't know their name) was taken in charge…

Coded mint → air → roll → merge → rung → mouth, in the shipped game, at two removes, to
people with no authored sheet. (Nobody happened to say it to the player this run; in the
earlier session 779, on the pre-review tree, Ewart Lark did. The fake backend's replies are
canned — the sheet is the evidence, the reply is the provider's.)

**One legibility wart, carried forward and not mine to fix.** "at **In the Stone
House, the civic gaol in the side court behind the Bellstand** today, for **The Stone
House**" — two of them in one line: `areas.json`'s labels are prepositional fragments
meant to follow "you are…", so `{place}` renders with its own leading preposition, and
`{station}` then repeats the place. `wickmarket`'s label is "The Wickmarket" and reads
fine, so the asset is inconsistent rather than uniformly wrong. The template and the
renderer are both stated by the plan (hook 1 takes the prisoner's position at the
commitment, so `{place}` *is* the gaol), so the fix belongs to M5's close-out (step
17): drop the "at " before `{place}`, give `render_line` a leading-preposition strip,
or add a short-name field to `areas.json`. The 2026-09-05 review asked for the
seizure point instead; that is a template decision the plan made the other way and
M5 owns the render, so it is recorded here and not changed.

---

## 8. Departures from the plan, and what the review changed

Neither of the first two moves a constant; none of the rest moves a number the
plan derived.

1. **The cost guard's off arm needs the same step as its on arm.** `plan/M2.md` step
   20e gives the `CATHEDRAL_NO_KNOWLEDGE=1` arm no `--trace-pollen`, so it runs at
   `--watch-clock`'s default `seconds_per_day / 200` = 18 s while the on arm is capped
   to `--pollen-step 3` — 200 polls against 1,200. The ratio would have been 6× and it
   would have been measuring the step. Both arms carry `--trace-pollen --pollen-step`
   in §3.
2. **The saturated 20,000 guard's window is six game hours, not twenty-four.**
   Measured: 3.0 s per poll at 20,520 bodies with nine rows in every ward's air (60
   polls in 180 s wall), so a game day at the 3 s step is 1,200 polls and an hour per
   arm. Nothing the guard asserts turns on the length — the air is saturated from the
   first instant, six game hours is 36 polls per body and ~2.4 million rolls, and every
   holdings cap fills long before the end.
3. **`the_roll_count_is_invariant_under_the_time_scale` does not compare
   `rolls_per_game_hour`.** The plan's table asks for it; it is
   `S × Σ over present bodies |air(their ward)|`, and where bodies stand is walking,
   which runs in sim-seconds (D22) — it cannot be clock-invariant. The test compares
   the sample count and every shared row's `stir`. The ladder-epoch salt of the poll
   gap is a second non-walking channel by which a person's *phase* on the grid differs
   by clock (coins are never skipped by it); recorded in the test's doc, not asserted.
4. **The measurement packs are planted in code, not read from
   `crates/cathedral-sim/tests/fixtures/pollen/*.json`.** `seed_pollen_pack` chooses
   the four mouths and the subject by the rule in `02_numbers.md` §7, and the cold
   scandal, household and same-trade scenarios re-plant with `plant_for_measurement`
   from that choice — so a cast edit that moves a mouth is legible in the printed
   line and not silent in a fixture. No `fixtures/pollen/` directory exists.
5. **`Round.pollen_due`, a due-ordered index beside `next_pollen`.** The plan's
   `tick_pollen` probed every enrolled body's deadline on every 20 Hz tick — O(N) per
   tick, 20,520 `BTreeMap<String, f64>` probes at the crowd knob's ceiling. The tick
   now pops the due people off a `BTreeSet<(deadline bits, ActorId)>` and walks
   nobody else; the deadline map stays the truth and a stale index entry (a
   re-enrolment) is dropped on pop. Order within a tick is `(deadline, id)` instead of
   `id` — still total, still deterministic; the census reproduces byte for byte (§2).
6. **`CrossingTally` is debounced** (§4) and integrates the warm window in closed form
   (§1); it carries its cached mint ward on every snapshot so `fill` never divides by
   a ward the census re-read off the air; it prints the raw flip count beside the
   believed one and a same-trade any-ward warm exit count beside the mint-ward split.
7. **The fast side of the band is asserted** — `wards_reached(bed) < 8` at the first
   sample ≥ 1.0 gh — which the plan's table did not have. Measured 6.
8. **`pickup_chance` is `pub`**, not `pub(crate)`, so the harness can fold the roll's
   own function over the real cast; gameplay reaches the roll only through
   `poll_person`, which uses the resolved-listener form (`Listener`,
   `pickup_chance_of`) — one `characters.get` per poll instead of four to six per
   (person, air row) pair.
9. **`--pollen-saturate` fills `live` to `FACTS_MAX_LIVE`** with standing filler rows
   nobody holds (`fill_live_for_measurement`), so the O(live) deposit walk the
   `seeded_by_actor` decision in `02_numbers.md` §6 hangs on is measured at the length
   it is bounded at, not at eleven rows.
10. **`sweep` mints one coin per grid edge elapsed**, not one per beat, so a stall
    longer than a stir window cannot lower the roll rate; `deposit` writes the spec's
    unquantised `max` and quantises only its change test; the headless tracer samples
    the mint's own instant before the first step, as the harness does;
    `footprint_bytes` counts the `from` and `view.subject` heap it had left out.
11. **Re-taken on the final binary.** The first re-measurement's headless runs were made
    on a binary built ten minutes before the last edit to `pollen.rs`; every figure in
    this file is now from the final tree (the header says so). One of them changed: the
    384/day row's `expect` had been read off the first seven samples of that earlier run
    ("0.263 peak, 0.228"); the final run reads 0.51 at 1.41 gh (§1).
12. **One rustfmt hunk** (`pollen.rs`, the `warm_same_trade_exits` assignment in `fill`)
    was the review's last new formatting departure and is applied; `crowd.rs`'s five
    hunks and `round/tests.rs`'s one are HEAD's own, byte-identical there.
13. **The ignored guard's doc comment** said the pump costs "~1.1 s" a poll in one place
    and 3.0 s in another; both say 3.0 s now (the run: 300 polls in 861 s).
14. **D22 carries an addendum** naming the ladder-epoch salt of the poll gap as one more
    non-walking quantity that is not clock-invariant (the phase, never the count), so
    M3's Layer 2 tests cannot assume it.
15. **`02_numbers.md` carries a `## Measured corrections` section** — the fast-end
    model, X, the slow end's divisor, the same-trade term, the footprint formula, the
    cost in the game's shape, the 37.4 → 36.9 doc figure and the standing wards — so
    nobody quotes the superseded predictions at close-out (§10.6); and `M2.md`'s notes
    for M3 carry six "as landed" seams (`Listener`, `pollen_due`, the coin per edge,
    the debounced instrument, the two-sided band's numbers, the cost shapes).

**No constant moved.** Both ends of the band pass, the fast side's ceiling holds, and
`02_numbers.md` §10's retune rule was never invoked — so there is no `## Retunes
taken` section; the "Measured corrections" appended to that file are corrections to
its *predictions*, not to a constant.
