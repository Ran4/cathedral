Status: Planned (2026-10-07).

# M11 — Playtest and release

From: old M19, shrunk ([archive](archive/M19_acceptance_and_release.md)). R11 in [PLAN_REVIEW](PLAN_REVIEW.md).
Entry: M10.

Show that the finished quest is sound when played, and record what it costs.

## Scope

- **Developer playtests** across the routes in [CASE_CHECKLIST](CASE_CHECKLIST.md), by keyboard, typing and voice.
  [PLAYER_EXPERIENCE](PLAYER_EXPERIENCE.md) has the revision criteria.
- **One headless drive run of the main path** (`CATHEDRAL_HEADLESS=1`), with screenshots at the decisive moments.
- **A multi-day fake run** (`cathedral-headless --fake` with the case pack) over several game days.
- **Performance:** `scripts/perf_suite.sh` at the default city and at +2,000 residents, compared with the pre-quest baseline.
- **Lockstep scenarios** at the decisive boundaries all pass.
- **Bookkeeping.** Update the plan's status, move the feature to `features/implemented/` and update
  `features/order.json`.

## Not in scope

Frozen budgets, the 20,000-resident gate, an accessibility matrix, a release record.

## Acceptance (player-visible)

1. The developer plays the main path and at least two alternatives without help, and can explain what supports
   each finding.
2. The headless drive run of the main path reaches Corin's arrest, and its screenshots show it.
3. The multi-day fake run ends with a coherent city: no stuck guard, no duplicated item or payment, no reseeded case.
4. Frame cost at default and +2,000 residents is reported against the baseline, and any regression is explained.
5. Save → load at every decisive boundary continues correctly in a fresh process.
