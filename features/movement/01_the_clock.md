# L0 — The Clock

> *"Bells divide the day from the Watch and Kindling through Dayspring, High Wick, Waning,
> Lamplight, and the Snuffing curfew."* — `lore/core_lore/core_lore.md:93`

The clock is the smallest layer and the one everything else reads. It is also the only layer whose
content is entirely already written: we are implementing the lore, not inventing it.

---

## 1. What exists today: nothing

Grep-confirmed across `src/` and `crates/`: there is no `hour`, no `time_of_day`, no calendar, no
season, no weather. Two comments in the codebase say so out loud:

- `src/smart_actors/bridge.rs:143-145` — *"CATHEDRAL_DRIVE stand-in for world sounds the sim cannot
  cause yet (**nothing rings the town bell: no clock, no calendar**)."*
- `crates/cathedral-sim/Cargo.toml` — *"Deliberately frozen dependency set: no bevy, no tokio, no
  network, **no clock reads**, no file reads."*

The sun is a single `DirectionalLight` spawned once at `Startup` (`src/scene.rs:1096-1112`) and never
touched again:

```rust
DirectionalLight {
    color: Color::WHITE,
    illuminance: lux::RAW_SUNLIGHT,
    shadow_maps_enabled: true,
    ..default()
},
Transform::from_xyz(-420.0, 560.0, 300.0).looking_at(Vec3::new(0.0, 0.0, 40.0), Vec3::Y),
```

There is a real `Atmosphere::earth` (`scene.rs:33-42`) feeding an `AtmosphereEnvironmentMapLight` on
the camera. **That is a gift**: rotate the one light and the physical atmosphere gives you sunrise
and sunset colour for free, with no skybox asset and no gradient to author.

---

## 2. `WorldClock` — a pure function of `now`

The sim reads no clock; it is *handed* `now: f64` (monotonic seconds since app start) on every
`Engine::poll`. So the world clock is not a clock at all — it is a projection of `now`:

```rust
/// crates/cathedral-sim/src/clock.rs
///
/// The world's time. Pure: a projection of the `now` the host already passes to
/// `Engine::poll`, plus a scale and an epoch. No clock is read here (D22).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldClock {
    /// Real seconds per game day. From `config.ron`.
    seconds_per_day: f64,
    /// Where the world's clock stood at `now == 0`, in fractional days.
    /// Lets a run open at Dayspring instead of at midnight.
    epoch_days: f64,
    /// Extra multiplier on top of `seconds_per_day`. Debug only; the `T` key
    /// cycles 1× / 10× / 60× so a whole day can be watched in a minute.
    scale: f64,
}

/// A resolved instant. Cheap to compute, so it is computed rather than stored.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldTime {
    /// Days since the epoch. Day 0 is a Bellday.
    pub day: i64,
    /// [0, 1) through the day. 0.0 is midnight.
    pub fraction: f64,
    /// The last office whose bell has rung.
    pub office: Office,
    /// Which of the seven weekdays.
    pub weekday: Weekday,
}

impl WorldClock {
    pub fn at(&self, now: f64) -> WorldTime { /* … */ }

    /// 0.0 at the dead of night, 1.0 at High Wick. The single number the
    /// behaviour ladder reads for "is it dark".
    pub fn brightness(&self, now: f64) -> f64 { /* … */ }

    /// Offices crossed in `(previous, now]`. The bell-ringer's trigger; it is
    /// a *set*, so a frame drop or a debug time-scale jump can never skip a
    /// bell, and a paused game can never ring one twice.
    pub fn offices_crossed(&self, previous: f64, now: f64) -> Vec<Office> { /* … */ }
}
```

`offices_crossed` taking a *span* rather than testing an instant is the one non-obvious bit, and it
matters: at 60× debug speed a whole office can pass inside a single frame, and a bell that is tested
with `if hour == 12` will be missed. Ask *which bells did we cross*, and it is impossible to lose one.

---

## 3. The seven offices

The lore names them and gives them a rough time of day; it does not give them clock hours. These are
the hours I propose, and the reasoning is in the third column.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Office {
    Watch,      // 1 stroke
    Kindling,   // 2
    Dayspring,  // 3
    HighWick,   // 4
    Waning,     // 5
    Lamplight,  // 6
    Snuffing,   // 7
}
```

| # | Office | Rings at | The span it names | Why |
|---|---|---|---|---|
| 1 | **the Watch** | 02:00 | 02:00 – 05:00 | *"deep night"*. Streets empty but for watchmen, scavengers, and the desperate. |
| 2 | **the Kindling** | 05:00 | 05:00 – 07:00 | *"before light"*. The lore is emphatic that working people are already up: *"furnace lit before dawn"* (Sparr), *"the Moorings yard before dawn"* (Alder), *"fulling stocks from dawn"* (Rud). |
| 3 | **Dayspring** | 07:00 | 07:00 – 12:00 | *"sunrise"*. The gates open; the west doors are unbarred for the dawn-showing; the city's working morning. |
| 4 | **High Wick** | 12:00 | 12:00 – 15:00 | *"noon"*. *"Peace with the impossible, and **dinner at noon**"* — `08_folk_culture.md`. The main meal and the market's peak. |
| 5 | **the Waning** | 15:00 | 15:00 – 18:00 | *"mid-afternoon"*. Work resumes; deliveries; the strong hour in the nave. |
| 6 | **Lamplight** | 18:00 | 18:00 – 21:00 | *"sunset"*. The lamplighters walk. *"Cloth work ends at dusk."* The taverns fill. |
| 7 | **the Snuffing** | 21:00 | 21:00 – 02:00 | *"curfew"*. The gates shut. The streets clear. |

The Snuffing's span wraps midnight, which is correct — the night is one long office and the Watch
does not start until well into it.

**Each office rings its own ordinal at 3 s intervals**, as
`lore/second_sun/design/06_the_sound_of_the_city.md` §3 specifies: *"the Watch one stroke … the
Snuffing seven, at 3 s intervals. A player anywhere in the city learns the hour by counting."* That
is free to implement (a small stroke queue in the sim's sound emitter) and it is a genuinely lovely
thing to have in a game — a clock you read with your ears.

**The Scold rings after the seventh office.** The Bellstand's secular watch-bell rings the *legal*
Snuffing, following the Lanthorn's prayer:

> *"The office is prayer, the Scold is law, and the minutes between them are the city's **dusk
> grace**."* — `design/06` §3

So there are two curfew bells, minutes apart, and the gap between them is a real mechanic: you may be
in the street between the Lanthorn's Snuffing and the Scold's, and not be in breach. Watchmen enforce
after the Scold. That falls out of the clock for free and is worth having.

---

## 4. The week

```rust
pub enum Weekday { Bellday, Second, Highmarket, Fourth, Fifth, Lowmarket, Seventh }
```

`day % 7`, with day 0 a Bellday. From `lore/core_lore/trade_and_daily_life.md`:

| Day | | Effect on the round |
|---|---|---|
| 1 | **Bellday** | the weekly holy day. Trades close. The nave fills. Gravemouth rings the great mass. |
| 3 | **Highmarket** | *"chiefly at the Wickmarket and Coswald's Yard"* — the crowd moves there |
| 6 | **Lowmarket** | *"chiefly at the Tallage and Maren's Green"* — the crowd moves *there* |

Market day is the cheapest, most visible piece of life in the whole plan: **it changes where the
crowd is, and it costs one `match` in the agenda builder.** Walk into the Wickmarket on a Highmarket
and it is full; walk in on a Fourth and it is a square with some stalls. See
[04_the_round.md](04_the_round.md) §5.

The lore also warns us about it, in `lore/places/02_canonical_gazetteer.md:502`:

> On Lowmarket, a laden cart approaching the bridge stair can halt pedestrian movement for a moment.
> **This is a designed piece of congestion, not a collision bug.** People wait, trade remarks, read
> the chalk across the yard, or take a longer route.

Congestion is a feature. Do not "fix" it.

---

## 5. Brightness, and the one number the ladder reads

Steal seagame's shape exactly: a **single float**, computed once per tick, passed down as an ordinary
argument. seagame has no `TimeOfDay` enum in its behaviour code at all — every decision site compares
`brightness` against an inline threshold, and the result is a city that appears to keep a schedule
while nothing anywhere says so.

seagame's curve (`~/seagame/src/types.ts:6-27`) is a trapezoid: a 60 s dawn ramp, a flat day, a 60 s
dusk ramp, a flat night floor of 0.3. Ours is the same shape, pegged to the offices:

```
brightness
   1.0 ┤              ╭──────────────╮
       │             ╱                ╲
       │            ╱                  ╲
   0.5 ┤           ╱                    ╲
       │          ╱                      ╲
  0.05 ┼─────────╯                        ╰────────────
       └────┬────┬────┬────┬────┬────┬────┬────┬────
          02   05   07   12   15   18   21   00
         Watch Kndl Dayspr HighWick Waning Lamplt Snuffing
```

`brightness` drives three separate consumers, and it is important that they are the *same* number:

1. **The sun.** One `Transform` on the existing `DirectionalLight`, plus `illuminance` and
   `GlobalAmbientLight::brightness`. The `Atmosphere` does the rest.
2. **The ladder.** `in_the_dark`, the sleep gate, the lamplighters' round, the tavern's evening.
3. **The prompt.** Not directly — the prompt gets the *office name*, not a float (§7).

**Night is dark.** seagame floors at 0.3 because a top-down 2D game must stay legible. A
first-person medieval city at night should be genuinely, frighteningly dark — a floor of about
**0.05**, lifted only by lamps, windows, and the moon. That is what makes the lamplighter matter, and
it is what `lore/places/04_routes_and_sightlines.md` §"Night and bells" already assumes:

> At night, street navigation relies on sparse lamps, windows, known silhouettes, **and bells**.

---

## 6. The numbers: how long is a day?

This is the one hard decision in the clock, and there is no free lunch. Fix NPC walking speed at the
lore's **1.2 m/s** and let `R` be the real seconds in a game day, so compression `C = 86400 / R`:

| 1 game day = | C | 60 m — to the ward well | 200 m — across your ward | 500 m — across town | 1 km — wall to wall |
|---|---|---|---|---|---|
| 24 real min | 60× | 50 game min | 2 h 47 m | 6 h 56 m | 13 h 53 m |
| **60 real min** | **24×** | **20 game min** | **1 h 07 m** | **2 h 46 m** | 5 h 33 m |
| 120 real min | 12× | 10 game min | 33 game min | 1 h 23 m | 2 h 46 m |
| 240 real min | 6× | 5 game min | 17 game min | 42 game min | 1 h 23 m |

Read the bottom-right corner of each row and ask whether a person could have that day.

- **24× (1 real hour per day)** — the Skyrim number, and the one I recommend. Local errands read
  perfectly. A cross-town commute reads as absurd, and needs the mitigations below.
- **12× (2 real hours per day)** — everything reads correctly, with no mitigation whatsoever. The
  cost is that seeing a full day/night cycle takes a two-hour sitting.

Two mitigations make 24× honest, and they should both exist regardless:

**(a) Content: people live where they work.** This is not a hack; it is how medieval cities worked,
and *the data already says so*. Every character has a `planning_ward` (8 values), and 2,566 buildings
have a `district`. A ward is 200–400 m across. Bind home, work, well, church and tavern **inside the
ward** for the great majority, and a full day's walking is a few hundred metres. The twenty authored
routes are, almost without exception, ward-local — the one systematic exception being the boat
families, whose commute *through a gate* is itself a canon plot point (see
[04_the_round.md](04_the_round.md) §7).

**(b) The Long Errand rule.** For the rare genuinely-cross-city trip:

> An actor whose route exceeds `long_errand_m` (250 m) and who stays further than `fiat_radius_m`
> (150 m — beyond the render's `VisibilityRange` fade) from the player for the whole leg **may be
> advanced by the clock rather than by their feet**: their position is placed where the office-span
> says they should be along the route.

It is Skyrim's off-screen warp, made explicit and bounded. The guard is what makes it invisible: an
actor inside the visibility range is *never* fiat-advanced, so you can never watch someone teleport.
It is also strictly a *performance* rule — nothing about the behaviour changes, only who bothers to
integrate.

**Debug time-scale is not optional.** A day/night cycle you can only test in real time is a day/night
cycle nobody tests. Bind `T` to cycle 1× / 10× / 60×, print the office in the HUD, and mirror both
into `logs.jsonl` so a `CATHEDRAL_DRIVE` script can assert on them.

---

## 7. How the clock reaches the LLM — the decision that saves the budget

**Not as percepts.** This is settled in the lore, at `lore/second_sun/design/06_the_sound_of_the_city.md` §5:

> **The offices are a clock, not events.** Seven percepts per actor per day would be token waste:
> Evenblow instead updates the **scene-header time-of-day** every actor already receives (*"the last
> office rung was the Waning"*). **Only *deviations* from the daily round are events.**

Concretely, the sheet's `you_are` block gains one field:

```json
"you_are": {
    "location_description": "The Wickmarket",
    "the_hour": "Lamplight — the lamps are being lit; the market is closing",
    "position_m": { "x": -37.75, "y": 0.91, "z": 379.3 }
}
```

That is one line of prompt, rendered from the clock every turn, costing nothing and queueing nothing.
The model always knows what time it is and never gets a turn *because* of what time it is.

Compare the alternative honestly. `town_bell` is audible at 600 m — most of the city. Emitting it as a
sound percept would put a line in ~500 inboxes seven times a game day. And `CharacterState::inbox` is
an **unbounded `Vec<String>`** (`character.rs:87`; only `recent_history` is bounded, at 32). An
ambient NPC in a far ward who never takes a turn under the stage gate would accumulate bell lines for
the entire session.

(It would *not*, at least, cause 500 LLM turns. `engine.rs:1314-1327` nudges exactly one actor per
sound — *"Exactly one nudge per sound: the turn stream is global and single."* But 500 inbox lines is
bad enough, and **the unbounded inbox is a real latent bug that movement would expose.** Bound it
while you are here.)

**What the bell still is:** a *sound*, for the player, played from the Lanthorn, seven ordinals at 3 s
intervals. The player counts the strokes. The NPCs read the clock. Nobody spends a token.

**What is still an event:** the Ruin (the ring rung backward — fire or flood, drop everything), the
name-knell, the Scold's summons before a proclamation. Those are deviations, they are rare, and they
*should* cost a turn.

---

## 8. The countergait — a debt the rose window is owed

If you add a day/night cycle you have implicitly promised the second sun, because the lore says it
moves *backwards* across the day. From `lore/second_sun/07_what_everyone_knows.md`:

> It walks against the true sun — the countergait: **huge in the west at dawn (the dawn-showing),
> sharing the window late in the day (the strong hour), crossing once daily — the Passing, or the
> Kiss.**

This is visible only from inside the Lanthorn, through the Great Rose. It is a *second* directional
light (or a rose-window emissive) that runs the clock in reverse, and it is one of the game's central
images.

Two of the twenty authored routes already peg themselves to it — Praelucent Dorn's *"the crossing at
the strong hour"*, Doctor Ferrant's *"the nave at the strong hour, by arrangement with Verger Pike"*.
So the clock does not merely enable the countergait; **two NPCs' days already depend on it.**

Out of scope for M0. Note it, do not forget it, and do not let the clock's design make it awkward:
keep the sun's angle a pure function of `WorldTime` so a second one is trivial to add.

**One caution, from `features/50_cool_suggestions.md`:** suggestion #6, *The Rose Meridian* — a
walkable sundial that *"actors schedule their day around"* — is in the **NoWay** tier. Light-driven
scheduling has been explicitly rejected. Bell-driven scheduling has not. Schedule off the clock and
the bells; let the light be beautiful and mean nothing mechanically.

---

## 9. Config

Additive under `smart_actors`, matching the existing style:

```ron
smart_actors: (
    // …existing…
    clock: (
        // Real seconds in a game day. 3600 = one game day per real hour.
        seconds_per_day: 3600.0,
        // Which office the run opens on. "dayspring" puts you in a working morning.
        start_office: "dayspring",
        // Day 0 is a Bellday; 2 opens on a Highmarket.
        start_day: 0,
        // Night's brightness floor. 0.05 is genuinely dark; raise it if the city
        // becomes unnavigable rather than atmospheric.
        night_brightness: 0.05,
        // The seven ordinal strokes, 3 s apart (design/06 §3).
        ring_the_offices: true,
    ),
)
```

`EngineConfig` gains a `clock: WorldClock`. The headless runner gets `--seconds-per-day` and
`--start-office`, so `cathedral-headless --fake --seconds-per-day 60 -t 120` plays two whole game days
in two minutes and prints every office as it rings. That is the loop you will actually iterate the
schedule in.
