# L2 — The Ladder: "people have their own idea of what to do"

The brief says *"take some inspiration from `~/seagame` — people have their own idea of what to do."*
I read seagame. Here is what that sentence actually points at, and what to take from it.

---

## 1. What seagame actually does

**There is no LLM in seagame.** `package.json`'s only dev-dependencies are `sharp`, `typescript` and
`vite`; the OpenAI code is offline asset generation. Every crew decision is one function —
`updateIdleHuman()`, `~/seagame/src/crew/update.ts:1779-2126` — and it is a **flat priority ladder**:

```ts
if (flooding && morale >= 96 && haveWood)      { → nearest BREACH,  REPAIRING;  return }
if (kraken.attacking)                          { tryMonsterReaction();          return }
if (storm)                                     { tryStormReaction();            return }
if (injured)                                   { → BED,             SLEEPING;   return }
if (hungry || starving)                        { → STOVE,           EATING;     return }
if (lustful && cooldown <= 0)                  { trySeekLustPartner();          return }
if ((tired || exhausted) && (dark || exhausted)){ → BED,            SLEEPING;   return }
if (foodLow && rand < 0.02)                    { → FISHING_SPOT,    FISHING;    return }
if (brightness < 0.7)                          { → unlit LANTERN,   LIGHTING;   return }
if (brightness > 0.9)                          { → lit LANTERN,     EXTINGUISH; return }
…
wanderRandomly()
```

Seventeen rungs, first match wins, ~350 lines. It is not a utility system, not GOAP, not a behaviour
tree. There is no `goal`, no `score`, no planner. And it produces a crew who plainly have their own
ideas, because **eight people running the same ladder against different private state are on
different rungs.** Jack is hungry (rung 5). Anne is tired (rung 7). Mary is lustful (rung 6). Flint
falls all the way to the bottom and wanders. Same code, divergent lives, no coordination, no
assignment, no job board.

Three supporting mechanisms make it work, and they matter as much as the ladder:

**The decision-cadence throttle.** `updateIdle` (`update.ts:1751-1772`) opens with
`member.idleTimer -= dt; if (member.idleTimer > 0) return;`. An actor only re-runs the ladder when
its jittered timer expires — 1–6 seconds. **This is the entire performance story.** Nobody decides
every frame, and because the timers are jittered, decisions naturally stagger across the cast with no
scheduler at all.

**Random choice over candidates, plus claim checks.** `pickRandom()` over all candidate beds, not
*nearest* bed (`update.ts:251-254`). Deliberate: nearest makes eight people stampede one bed; random
spreads them across the ship. For scarce targets — lanterns, breaches, fishing spots — the candidate
list is additionally filtered by *"is anyone else already walking here with this same target?"*
(`update.ts:1971`). Two lines, and no crowd ever piles onto one object.

**Divergence keyed on an emergent scalar.** Morale (`update.ts:887-911`) is a lagging average of
hunger, energy and average friendship, drifting toward its target at 0.3/s. It is not a stat; it is a
*history*. And then the same external event splits the crew by it: in a storm
(`~/seagame/src/weather.ts:271-310`), `morale < 64` may flee below, `morale < 96` may drop and pray,
and everyone above that keeps working. Same storm, three different people. **That is where "their own
idea of what to do" actually comes from** — not from the ladder, from the fact that the ladder is
read against a scalar each of them earned separately.

And one more thing, which is small and transforms the game: **they can refuse you.** Below morale 64,
a crew member has a 50% chance to refuse any order, clearing their whole queued chain.

---

## 2. The trap: `statuses` and `conditions` already mean something else here

seagame's cleanest idea is a two-layer split: **`statuses`** is raw persisted state that you *write*,
**`conditions`** is a `Set<string>` **rebuilt from scratch every tick** that you *read*. Behaviour
code never touches a raw number; it asks `conditions.has('hungry')`.

**Do not import those two names.** Every one of the 500 character sheets already has both, meaning
something entirely different:

```json
"statuses":   ["alms_dependent", "begs_regularly", "insecure_lodging",
               "intermittently_employed", "pauper"],
"conditions": ["failing eyesight"]
```

Here, `statuses` is **authored social standing** (100 paupers, 18 `unhoused`, 20 `retired`, 8
`prisoner`) and `conditions` is **authored bodily condition** (*"deaf in the left ear"*, *"a pale scar
notches her lower lip"*). Both are static, both are content, and both are already load-bearing in the
prompt.

So take seagame's *structure* and give it a name that does not collide. Call the derived set **`Cues`**
— what is cueing this person's behaviour right now:

```rust
/// Rebuilt from scratch every movement tick. Read-only to the ladder.
/// Nothing else in the sim ever writes it.
pub struct Cues(BTreeSet<Cue>);
```

And now the payoff, which is the reason to do it this way rather than with a pile of `if`s: **the
authored `statuses` and `conditions` copy straight in.** seagame's `refreshConditions` begins by
copying every raw status key into the condition set, so a trait added anywhere becomes readable
everywhere for free. Do the same:

```rust
fn refresh_cues(actor: &Character, world: &World, clock: WorldTime) -> Cues {
    let mut cues = Cues::default();

    // 1. The authored sheet, verbatim. `pauper`, `unhoused`, `prisoner`,
    //    `enclosed_religious`, `begs_regularly` — 500 characters' worth of
    //    social truth, made behaviour-readable at zero authoring cost.
    for status in actor.lore().map(|l| &l.statuses).unwrap_or_default() {
        cues.insert(Cue::Status(status));
    }

    // 2. The needs, quantized. Continuous scalars never reach the ladder.
    if actor.needs.thirst  < 15 { cues.insert(Cue::Parched);  }
    else if actor.needs.thirst  < 70 { cues.insert(Cue::Thirsty); }
    if actor.needs.hunger  < 15 { cues.insert(Cue::Starving); }
    else if actor.needs.hunger  < 70 { cues.insert(Cue::Hungry);  }
    if actor.needs.fatigue < 25 { cues.insert(Cue::Exhausted); }
    else if actor.needs.fatigue < 60 { cues.insert(Cue::Tired);   }

    // 3. The world.
    if clock.office == Office::Snuffing || clock.office == Office::Watch {
        cues.insert(Cue::Curfew);
    }
    if world.brightness < 0.35 && !near_a_lit_lamp(world, actor) {
        cues.insert(Cue::InTheDark);
    }
    if actor.indoors.is_some() { cues.insert(Cue::Indoors); }
    if at(actor, actor.round.workplace) { cues.insert(Cue::AtWork); }
    if at(actor, actor.round.home)      { cues.insert(Cue::AtHome); }

    cues
}
```

**A hundred paupers now behave differently at curfew without anyone authoring a hundred behaviours.**
That is the whole argument for the pattern, and it is worth more here than it is in seagame, because
here the content already exists.

---

## 3. The needs

Small, and each one exists only because it makes somebody *walk somewhere that is already in the
city*. A need with no destination is a stat, not a behaviour.

| Need | 0–255, high = satisfied | Destination | Already built? |
|---|---|---|---|
| **thirst** | the fastest to decay | a well or cistern | **9 named sources, all areas; queue aprons already walkable; 4 sounds already in the catalog** |
| **hunger** | | home hearth, a cookshop, a tavern, a food stall | 77 taverns; 60 stall fixtures |
| **fatigue** | restores only in bed | home | 670 residential buildings; 2,565 doors |
| **duty** | *"am I where my round says I should be?"* | the workplace | the 20 authored routes; `occupation_id` |

That is four. Resist adding more. seagame has five and one of them is lust; the fifth thing you want
here is not a need, it is the **social pull** in rung 10 below — the thing that puts a body in front of
the player so the LLM has someone to talk to.

**Thirst first, and thirst fastest**, because `lore/wells_and_water.md` is 80 KB long, the wells all
exist, the queue rules are authored, and the sounds are sitting in `catalog.toml` under a comment that
says *"flip `actor_emittable` and a keeper can work the curb."* See [README §8](README.md#8-the-first-vertical-slice-should-be-water).

---

## 4. The ladder

Re-evaluated per actor on a **jittered 1–6 s cadence**, not per tick and never per frame. First match
wins; every rung ends by setting a `Route` with an `Arrival`, and returning.

| # | Rung | Fires when | Goes to |
|---|---|---|---|
| **0** | **the LLM's intent** | a `go_to` was issued and has not expired | wherever they said |
| **1** | **reflex** | the Ruin rings (fire, flood); violence within sight | away, or toward — by temper (§6) |
| **2** | **parched** | `thirst < 15` | the ward well with the shortest queue |
| **3** | **starving** | `hunger < 15` | food |
| **4** | **exhausted** | `fatigue < 25` | home, and to bed — *regardless of the hour* |
| **5** | **curfew** | `Curfew` && not a `watchman` && no night trade | home, **fast** |
| **6** | **thirsty** | `thirst < 70` | the well — but only if the queue is short |
| **7** | **hungry** | `hunger < 70` | food |
| **8** | **tired, and dark** | `fatigue < 60` && office ≥ Lamplight | home, and to bed |
| **9** | **the round** | the agenda's current leg says "be at X" and I am not at X | X |
| **10** | **the trade** | I am at my post | the idle act of my occupation — and it *makes noise* |
| **11** | **the social pull** | someone I `knows` is within 8 m and idle | drift toward them; turn to face them |
| **12** | **wander** | — | a random walkable point within a leash of my post |

**Rungs 4 vs 8 are seagame's sleep gate, and it is the whole daily rhythm in one line.** The tired go
to bed *only when it is dark*; the exhausted go to bed *whenever*. Nothing anywhere says "sleep at
21:00". The city beds down as it darkens because tiredness and darkness happen to coincide, and it
reads as a schedule. Steal it exactly.

**Rung 5 is the single most visible thing in this plan.** At the Snuffing the streets empty. You can
stand in the Wickmarket and watch it happen. It costs one rung.

**Rung 10 is where the city gets its voice.** The trade's idle act is not decorative: a market seller
cries their wares, a mason chisels, the windlass turns at the well, the lamplighter whistles the
wick-call at each corner (*"three notes, so folk know the shape on the ladder is lawful"* —
`08_folk_culture.md`). These are **sounds**, they go through the existing catalog, and they are heard
by the LLM layer — which means an ambient NPC's *work* becomes something an NPC standing next to them
can remark on, for free.

**Rung 11 is what makes the LLM layer work at all.** Today, NPCs are statues who happen to be near
each other. A weak social pull means the people the player walks up to are *already standing together*,
already facing each other, already a scene. It is three lines and it is worth more to the feel of the
game than most of the rest of this document.

### Ties

Exactly seagame's three, and no more:

1. **`pick_random` over candidates, not `nearest`.** Nearest makes forty people queue at one well.
   Random spreads them across the ward's four.
2. **Claim checks on scarce targets.** *"Is anyone else already walking here with this same
   `Arrival`?"* Two lines. No stampede, ever. Essential at the wells and mandatory in the Needle
   (which is 1.2 m wide and fits one person).
3. **Probability gates on the non-urgent rungs.** Even a settled ladder produces variety run to run.

### Determinism

`cathedral-sim` is deterministic and offline by decree, and its tests depend on it. **Do not introduce
an RNG.** The codebase already has the right idiom: `attention.rs:683-699` rolls curiosity as a *pure
hash* of `(salt, actor_id, context, visit)` rather than a fresh draw, precisely because *"the engine
polls at 60 Hz, and a re-drawn 20% is a certainty within a frame."* Every "random" choice in the
ladder is a hash of `(actor_id, decision_epoch, salt)`. Same inputs, same city, every time.

---

## 5. Cadence and cost

500 NPCs, ladder re-run every 1–6 s (mean 3.5), so ~140 evaluations per second across the whole cast.
Each is a few dozen comparisons against a `BTreeSet`. It rounds to zero.

**The important part is that the ladder does not run per tick.** It runs on a *decision epoch*, and
between epochs the actor is simply executing whatever it decided. That is what makes the number 140/s
instead of 10,000/s, and it is why seagame can afford a 350-line ladder with `findTilesOfType` scans
inside it.

Give the LOD tiers different cadences and it gets cheaper still:

| tier | distance | ladder cadence | steering |
|---|---|---|---|
| **stage** | < 32 m (the existing `attention.rs` radius) | 1–3 s | full: avoidance, corridor clamp, gait |
| **near** | < 150 m (the `VisibilityRange` fade) | 3–8 s | follow the route, no avoidance |
| **far** | ≥ 150 m | 10–30 s | advance along the route; eligible for the Long Errand fiat |

And **reuse `attention.rs`'s notion of "near the player"**, as `performance_improvements.md` item 6
insists: *"so there is one answer to 'is this actor near the player' and not three."*

---

## 6. Divergence — the thing the brief is actually asking for

The ladder alone does not give you people with their own ideas. It gives you people with the same
ideas at different times. What gives you *their own* ideas is an **emergent scalar** that the ladder is
read against — seagame's morale, which nobody sets and everybody earns.

Ours should be **temper**: a slow drift toward a target computed from the character's own private
history.

```rust
// Drifts toward `target` at ~0.3/s, like seagame's morale — a lagging
// indicator, never a snap. What it *is* is a history, not a stat.
let target = mean(needs.hunger, needs.fatigue, standing_in_ward)
           + if cues.has(InTheDark)  { -25 } else { 0 }
           + if cues.has(Status::Pauper) { -15 } else { 0 }
           + if cues.has(AtHome)     { +15 } else { 0 }
           + market_day_bonus
           + recent_kindness_from_the_player;
```

Then split the cast on it, at the rungs where it matters. The Scold rings curfew:

- **high temper** → home, unhurried, maybe one more word at the door;
- **middling** → home, briskly;
- **low temper** → *does not go home*. Lingers. Is in the street when the watch comes past. Which is
  a scene, and it happened without anybody writing it.

Same bell. Three different people. That is the mechanism, and it is why the brief points at seagame.

**And they may refuse the LLM.** An NPC who is `starving` and is told `go_to {"place": "outer_wharves"}`
— half a mile, out through a gate — may decline: the action fails with a real `ActionError`, the model
gets a `system:` line saying why, and can argue about it next turn. Three lines of code. It is the
difference between a cast and a set of puppets, and seagame proves it out.

---

## 7. What we take from seagame, and what we leave

**Take:**

- the two-layer split — raw state you write, a derived set you read, rebuilt every tick (renamed to
  `Cues`, §2);
- the flat first-match-wins ladder, over a utility system. It is debuggable (*"why is he doing that?"*
  → read down the list), it is authorable, and adding a behaviour is adding a rung, not retuning a
  weight matrix;
- the jittered decision-cadence throttle;
- `pick_random` + claim checks;
- one global brightness float compared against inline thresholds — **no `TimeOfDay` matching in
  behaviour code**;
- the sleep gate (dark *or* exhausted), which is the entire daily rhythm for free;
- **night-fear and lamps.** seagame drains morale in the dark unless you are near a lit lantern; the
  lanterns burn fuel; idle crew go and light them. One mechanic, and suddenly there is a candle
  economy, a reason to move at dusk, and a reason to fear the dark. Ombreval already has the
  lamplighters, the Wickmarket chandlers, and *Belwyn's lamp* — the one lantern left dark each night
  by rote (`08_folk_culture.md`). It is the same mechanic and it is already lore;
- divergent reactions to a shared event, keyed on an emergent scalar;
- the right to refuse.

**Leave:**

- **seagame's A\*.** Linear-scan open list, no heap, a 2,000-iteration cap, no crowd avoidance, and
  actors walk through each other. It is fine for 8 crew on a boat and it will die at 500. See
  [02_navigation.md](02_navigation.md).
- **the dense N×N relation array.** 500² is 250,000 records. Use `knows` (which already exists, is
  sparse, and is already the sim's model of who-knows-whom).
- **`updateIdleNPC`.** seagame's *town* NPCs — as opposed to its crew — are a home tile, a leash
  radius, and a table of flavour lines. They are set dressing and they are the weakest thing in the
  codebase. Ombreval's 350 ambients want the **crew** model, not the townsfolk model. They have names,
  trades, wards, families and secrets; do not reduce them to a wander radius.
- **the 2,462-line `update.ts`.** Rungs go in their own files.
