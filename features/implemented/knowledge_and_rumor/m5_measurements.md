# M5 final measurements — 2026-09-06

**Pass; no retune.** The final source passed the workspace gate before measurement. The band,
identity, six full-day crowd runs and manual release tests then ran sequentially, with no source
edits, builds or other application runs alongside them. Documentation and landing followed.
`m5_evidence/RESULTS.json` contains parsed samples and exact commands; the adjacent compressed logs,
`*.pollen.gz` streams and `/usr/bin/time -v` records preserve the raw evidence.

The release headless binary remained SHA256
`771382ee0243cb4c826d28099e25f7a51b3dd0edf5bdc2687bfe83f0db86f331` throughout.
All runs were fake-backend, renderer-free and offline. No salience band, affinity, free parameter
or probability moved. The unchanged M2 cadence tests remain the behavioral baseline.

## Band and identity

The full-day band run used the shipped 3,600 seconds/day clock, Dayspring, a 0.4-second step,
48 samples/day and the Wickmarket pack. The existing headless tracer records 48 samples from age
0 to **23.56 game hours**; the run itself completes a day. The unchanged test harness records
through 24.06 hours. These are distinct sampling setups, as already recorded at M2.

| Reading | Final result |
|---|---|
| Bed first reaches all eight wards' air | **2.01 game hours**, 156 carriers, mean hops 1.07 |
| Bed at the last headless sample | Eight wards, **446 carriers**, mean hops 1.04, 95 holder exits from the mint ward |
| Bed guard | Fewer than eight wards at 1 hour; at least two by 6.5 and four by 9.5; eight within the day: passed |
| Off-affinity Craft | **One ward at every one of 48 samples** |
| Craft at nightfall sample, 11.03 hours | Nine carriers, none warm; expected crossings 0.225 |
| Nightfall confinement probability | `exp(-0.225) = 0.798516`, above 0.5 |
| Craft at the last sample, 23.56 hours | Eight carriers, none warm; expected crossings **0.132**, below one |
| Flat table versus salience term removed | **625 byte-identical pollen lines**, including headers |

The identity stream's SHA256 is
`4008e4c029af8da1ab7be08adfb79024e6d1836d312cc876ec3ea2892c220b6a`.
The M2 test also asserts equality of the complete census fields, beyond printed precision.
Its off-affinity Craft result is nine carriers / 0.136 expected crossings at 24.06 hours;
the headless result above is not a replacement baseline or a reason to retune.

Final Bed carriers by current ward: Fabric 57, Wick 52, Cloth 11, Wallwright 30, Cinder 16,
Weigh 73, Reed 35, BellAndSluice 172. Craft's cold holders are in Wick 5, Weigh 2 and
BellAndSluice 1; a cold person crossing a border does not deposit into that ward's air.
`RESULTS.json` preserves every topic's per-ward carrier counts rather than only a city mean.

The unchanged `the_same_trade_ear_is_reported` was also rerun with output captured. Its three
revenue-worker witnesses produce **12 warm same-trade crossings of any ward over the day**,
four per witness, versus the original prediction of about 2.1. At 24.06 hours the fact is in six
wards with 128 carriers; at nightfall there are 76 carriers and eight warm. These match M2's
measured correction. The crossing count is reported, never used to tune the affinity table.

## Full-day saturated crowd cost

Every row runs a full game day at a three-second step, with the same command and binary.
ON seeds nine topic rows into every ward's air and fills the live fact cap to 256. OFF changes
only `CATHEDRAL_NO_KNOWLEDGE=1`. Inherited knowledge ablations are removed by the runner.
Timing invokes the already-built executable directly, excluding Cargo/build time.

| Extra people | Knowledge | Wall seconds | User CPU seconds | System seconds | Peak RSS KiB | Final store KiB | Final rolls/game hour bound |
|---:|---|---:|---:|---:|---:|---:|---:|
| 0 | ON | 2.801 | 2.78 | 0.00 | 36,948 | 535.3 | 10,640 |
| 0 | OFF | 2.486 | 2.47 | 0.00 | 35,520 | 0.3 | 0 |
| 1,000 | ON | 12.041 | 12.00 | 0.02 | 63,552 | 1,366.1 | 31,578 |
| 1,000 | OFF | 10.934 | 10.91 | 0.02 | 61,216 | 0.3 | 0 |
| 20,000 | ON | 4,504.886 | 4,501.58 | 0.57 | 579,424 | 16,047.9 | 451,374 |
| 20,000 | OFF | 4,241.178 | 4,239.74 | 0.45 | 563,040 | 0.3 | 0 |

| Extra people | User CPU increase | Peak RSS increase |
|---:|---:|---:|
| 0 | 12.55% | 1.395 MiB |
| 1,000 | 9.99% | 2.281 MiB |
| 20,000 | **6.18%** | **16.000 MiB** |

All three observed CPU increases are below the 15% budget. These are **one sequential ON/OFF pair
per count**, not medians or confidence intervals. The coarse step batches work and does not measure
the rendered game's frame rate. M2 retains its separate repeated measurements in the game's pump
shape. M4's short 1.2-game-hour timings and older store estimator are not comparable to these
full-day store totals.

The final ON holdings are 2,977 / 8,878 / 120,990 at the three counts; air rows are 84 / 85 / 88.
The 20,000-person store peaks at **16,078.8 KiB (15.70 MiB)** among the 25 samples. Auxiliary
consequence/projection caches peak at **879 bytes** in every run, reported separately from
`PollenCensus.store_bytes`. Those lightly exercised caches are also covered by the pessimistic
allocation test below. The OFF store's 0.3 KiB is the empty struct's accounting, not retained facts.

## Allocation and prompt bounds

The unchanged ignored release test, `the_store_stays_bounded_at_twenty_thousand`, passed after
**300 polls / six game hours / 858.0 seconds** of simulation wall time. It checks all 20,520 bodies'
six-row caps and a nonzero roll bound, with the live store saturated from the beginning:

- 256 facts, **100,745 holdings**, 88 air rows, 451,330 rolls/game hour bound.
- Store **14,027,250 bytes (13.38 MiB)**, below 33,554,432 bytes (32 MiB).
- `/usr/bin/time -v`: 857.42 user seconds, 0.37 system seconds, 859.45 elapsed seconds including
  Cargo startup, peak RSS 375,044 KiB. This harness RSS has no paired OFF row; it is not an RSS delta.

The source's six-hour window is M2's deliberate manual guard, unchanged; the six full-day
headless rows above meet M5's separate one-day measurement requirement.

Root independently reran the synthetic allocation test with 20,519 bodies at six carried rows:

| Accounting stage | Bytes |
|---|---:|
| Initial six facts and every carried cap filled | 19,742,942 |
| 256 facts plus 64 receipt sets at 64 distinct mouths | 20,123,494 |
| All-resident refusal/door cache upper bound plus cached map projection | 5,746,199 |
| **Combined** | **25,869,693**, below 32 MiB |

M5 corrects omitted vector spare capacity, sealed-source strings, frozen household/craft data and
receipt bookkeeping. BTree node overhead remains an estimate. The higher report is partly corrected
accounting, not an equivalent allocation increase. The test drops its earlier clone timing probes
before saturation, so Arc copy-on-write cannot shrink the six-row vectors' original capacity eight.
At 256 facts, root measured a Knowledge clone at 0.1895 ms against the whole World's 11.663 ms.

The public snapshot remains exactly **137,179 bytes**. The longest base prompt is 14,812 bytes;
six held facts with three selected bullets produce **16,993 bytes**, an increment of 2,181 bytes
(727/bullet), below 64 KiB. The 100-mark snapshot is 147,189 bytes, below 160 KiB.

## Gate, visual evidence and reproduction

Final workspace gate: **1,711 passed, zero failed, seven ignored, 29 suites**. The crowd guard above
was then explicitly run despite its ignore marker. Explicit-file rustfmt passed for 27 Rust files;
clippy's 58 existing sites stayed unchanged, with no added or removed warnings. All 22 prompt goldens
and the M2 cadence test are byte-identical to the pre-session base. M5 changes no prompt asset;
M4's separately reviewed conditional `raise_word` verb-fence line remains the planned exception.

The knell, journal, map heat and door checks are in `m5_evidence/VISUALS.json` and the three PNGs.
There were **139 X11 samples, all unmapped and unfocused**. Actual accepted bell/mint and door
diagnostics were checked. The door fixture uses a temporary copy of the configuration with only
the unrelated novelty requirement disabled, so the door admission rule is reached; the user's
configuration is untouched. Only llvmpipe software rendering was available, so the UI checks use
real fonts/map assets with the documented texture/shader fallback. They establish UI behavior,
not full-textured or real-GPU rendering/performance.

From the repository root, with `CARGO_BUILD_JOBS=1`, reproduce the measurements using the archived
runner, sequentially, assessing the band before continuing:

```sh
cargo build --release -p cathedral-backends --bin cathedral-headless --offline
uv run --no-project features/implemented/knowledge_and_rumor/m5_evidence/run_measurements.py band
uv run --no-project features/implemented/knowledge_and_rumor/m5_evidence/run_measurements.py identity
uv run --no-project features/implemented/knowledge_and_rumor/m5_evidence/run_measurements.py crowd
cargo test --release -p cathedral-backends --test pollen_cadence --offline the_store_stays_bounded_at_twenty_thousand -- --ignored --nocapture
cargo test --release -p cathedral-backends --test pollen_cadence --offline the_same_trade_ear_is_reported -- --nocapture
```

To check the archived numerical evidence without rerunning the simulation:

```sh
uv run --no-project features/implemented/knowledge_and_rumor/m5_evidence/summarize_measurements.py --raw features/implemented/knowledge_and_rumor/m5_evidence --output /tmp/knowledge-m5-rechecked
```
