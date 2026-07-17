# Hunger

The brief: *"Hunger that can be solved by eating."* Today "hungry" is a static string from Ilse's
lore sheet, rendered verbatim into her prompt forever (`prompt/mod.rs:787-789`); `eat` deletes an
item and changes nothing about the eater. This document adds the gauge, the decay, the meals, the
two reserved ladder rungs, and the one prompt change that lets an LLM actor actually stop being
hungry.

---

## 1. The gauge

```rust
// crates/cathedral-sim/src/character.rs — Needs grows its promised second field
pub struct Needs {
    /// Falls with time; refilled at a well. The fastest gauge to decay.
    pub thirst: f64,
    /// Falls with time; refilled by eating. Slower than thirst.
    pub hunger: f64,
}
```

Same convention as thirst (`character.rs:111-119`): runs `0..=255`, **high = satisfied**, never
rendered raw into the prompt. Constants alongside the thirst family in `lib.rs:141-153`:

```rust
pub const HUNGER_MAX: f64 = 255.0;
pub const HUNGER_FAMISHED: f64 = 15.0;   // rung 3: drop everything (the ladder spec's reserved number)
pub const HUNGER_HUNGRY: f64 = 70.0;     // rung 7: seek food when convenient (ditto)
pub const HUNGER_DECAY_PER_GAME_SECOND: f64 = HUNGER_MAX / (10.0 * 3600.0);
```

The thresholds 15/70 are not invented here — they are the numbers the ladder spec reserved
(`features/movement/03_the_ladder.md:160,164`).

**The rhythm the decay buys.** 255 over 10 game hours means someone who eats a full meal (satiety
150) at noon crosses `HUNGRY` (70) about 3 game hours later and would hit `FAMISHED` around
suppertime — two real meals a day, which is exactly the lore's day: dinner at High Wick (*"the main
meal and the market's peak"*, `features/movement/01_the_clock.md:118`) and supper toward Lamplight.
Thirst crosses its whole gauge in 4 hours; hunger is deliberately the slower, heavier need.

**Decay implementation:** one line added to the existing `decay_thirst` loop (`round.rs:1398-1414`,
renamed `decay_needs`), differenced over `clock.game_days` so the debug time-scale speeds it up
identically. Unlike thirst — which only decays for the bound water-drawers — **hunger decays for
every enrolled townsperson**: everyone eats. (README §8.1; the cost is one subtraction per person
per decision poll.)

**Seeding:** spread like thirst (`round.rs:877-878` idiom) so the city doesn't starve in lockstep:
`hunger = HUNGER_MAX * hash01("hunger_seed", id, 0)`, floored at 40 so nobody spawns mid-crisis.
Exception: an actor whose lore sheet declares the `hungry` condition seeds low — **Ilse seeds at
25**, hungry now, famished within the hour, which is her story made mechanical (§6).

## 2. Eating

`eat` ([01](01_items_and_stacks.md) §3.2) applies the kind's satiety:

```rust
eater.state.needs.hunger = (eater.state.needs.hunger + satiety).min(HUNGER_MAX);
```

A herring (70) buys the morning; a rye loaf (150) is dinner; tavern stew (170) is the best meal in the
city, as the Hungry Ox would want. Eating remains instant for LLM actors (a turn action); the
ladder's code-driven meal takes `EAT_SECONDS` at a bench so it reads visually
([04](04_the_bread_round.md) §5).

## 3. The two rungs

Into `decide()` (`round.rs:1804-1944`), in the ladder's established order — needs above the LLM's
`go_to` errand (rung 8), which stays above the daily round (rung 9):

| rung | fires when | does |
|---|---|---|
| **3 famished** (after parched, before thirsty) | `hunger < HUNGER_FAMISHED` and: holding edible food → **eat it now, standing**; else a food source is open and bound → go buy ([04](04_the_bread_round.md)); else (night, nothing open) → home to the hearth (§4) | injects `FAMISHED_PRESSURE` |
| **7 hungry** (after thirsty, before go_to) | `hunger < HUNGER_HUNGRY`, a bound food source is **open** and its queue is short (`FOOD_QUEUE_SHORT`, the `WELL_QUEUE_SHORT` twin), and the actor can afford list price | join the stall queue |

Details that matter:

- **Eat-what-you-hold comes first.** A famished actor holding a loaf eats it; the market is for
  people whose hands are empty. This single check is what makes bought-but-carried food meaningful.
- **Pressure percepts.** Like `PARCHED_PRESSURE`/`CURFEW_PRESSURE` (`round.rs:1794-1797`), rung 3
  injects a `system:` line — *"You are famished; your feet are taking you to food."* — giving an
  on-stage LLM actor one turn to excuse itself before the body walks (the `excused` flag,
  `round.rs:336-341`). Rung 7 is quiet, like thirsty.
- **Affordability is a rung predicate, not a market drama.** A hungry actor who cannot pay does not
  queue (rung 7 skips), and a famished one falls through to the hearth. Beggary, credit and the
  tavern slate are real lore (Renna's *"a tab is a leash"*) and real future features — but they are
  LLM-conversation features, not ladder features, and the ladder must not invent them silently.
- **`TooFar` finally earns its keep** (optional, small): a `go_to` errand whose route budget would
  cross `FAMISHED` before arrival can be refused with the reserved `too_far` error
  (`error.rs:39`) — *"that is farther than an empty stomach will carry you."* Flagged as a
  nice-to-have in M2; the variant has waited this long and can wait longer.

## 4. Meals without markets: the hearth

Most of a medieval city eats at home, and 500 NPCs buying three itemized meals a day would be
~1,500 conjured items nobody watches. So the honest cheat, symmetric with the magic restock:

**An actor at their home, or at a tavern, during a meal office (High Wick, or the supper span from
the Waning through Lamplight) refills hunger** at `HEARTH_REFILL_PER_GAME_SECOND` (full in ~20 game
minutes of sitting) — no items, no coins. The round already sends everyone home or to the tavern at
the right hours (`rounds.json` legs; the tavern archetype works through the evening); the hearth
just makes those legs *feed*.

What this buys: the market rungs fire mostly at **midday, on market days, in the squares** — which
is exactly where the player is watching and exactly what the lore says the market's peak is. The
visible economy concentrates where visibility exists; the invisible one is one multiplication.

## 5. Telling the LLM: the computed condition

The sheet line that today renders only static lore (`prompt/mod.rs:787-789`) becomes static **plus
computed**:

```
… Conditions: travel-worn, famished.
```

- `hunger < HUNGER_FAMISHED` → `famished`; `< HUNGER_HUNGRY` → `hungry`; else nothing. (Thirst
  deliberately stays un-surfaced — the water round runs fine without narrating it, and one new
  word-pair is enough for this regeneration. Revisit if thirsty dialogue is ever wanted.)
- The condition is recomputed per prompt, so **the loop closes**: famished Ilse eats a herring, and
  her next sheet simply does not say famished. No memory hygiene required, no "forget you were
  hungry" — the sheet is the truth, which is what the prompt already teaches ("you_hold … are the
  current truth").
- This is the golden-fixture regeneration the movement plan kept deferring
  (`character.rs:196-200`: *"Never rendered in M3, so the golden fixtures stay byte-identical"*).
  It lands in the same regeneration as M0's ×N rendering — **one bill, paid once**
  ([01](01_items_and_stacks.md) §7).

## 6. Ilse, specifically

The demo character finally works end to end:

- her lore sheet's `conditions: ["hungry", "travel-worn"]` drops `"hungry"` (now computed; keeping
  it would double-print) — `travel-worn` stays, static conditions remain the right place for
  states the sim doesn't model;
- her `memories` keep *"I am very hungry after the long road here"* — flavor, and now truthful;
- she seeds at hunger 25 (§1), holds her 1 copper, and a herring costs exactly 1
  ([02](02_the_copper_standard.md)) — the loaf she asked for is 2, which she cannot pay, and the
  fish stall feeds her anyway: the purchase the character was authored for is finally solvable,
  and *only just*, which is the drama;
- after eating: satiety 70 carries her to the Waning, and her sheet stops calling her hungry —
  the acceptance test of [06_milestones.md](06_milestones.md) M4.

## 7. Who does not hunger

- **The player.** No gauge, no `PlayerEat` (none exists today — `engine.rs` player commands stop
  at retract). The player's copper is for participating in the market, not surviving it.
  README §8.4 leaves the door open.
- **Un-enrolled actors** (no nav, headless fixture worlds): `Needs::default()` seeds both gauges
  full, and without round enrollment nothing decays — so every frozen test world stays byte-stable
  by construction, the same trick that kept thirst out of the fixtures.
