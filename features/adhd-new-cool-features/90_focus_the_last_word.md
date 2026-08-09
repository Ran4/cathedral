# Focus: The Last Word

*Bedtime is the commit point for how a Major remembers the day — deepened
against the actual code in `crates/cathedral-sim/src/night.rs`.*

## Sketch

The mechanic is already half-real, which is why it's cheap to sharpen: the
night prompt tells a Major "recent_history is your day," `recent_history` is
a capped `Vec<String>` whose oldest lines scroll off, and
`NightOffice::bedtimes` is resolved once at seed from `Round::bedtime` — so a
line landed late in the day is guaranteed to still be in the buffer at
reflection while the morning may already be evicted. The deepening:

1. **State** — when `ring_bedtimes` crosses the office before a Major's
   bedtime, stamp a watermark index into their `recent_history`, so
   `render_night_prompt` (called from `NightOffice::submit`, which already
   holds `self.bedtimes`) can split the sheet into *the day* and a new
   *last_before_sleep* section.
2. **Prompt** — `assets/prompts/night.j2` gains one paragraph: "what happened
   last sits heaviest tonight; settle it first," biasing
   remember/forget/set_goal toward the tail.
3. **Telegraphy** — the existing once-an-evening yawn, home-window lamps, and
   neighbor small talk all key off the same bedtimes map: a Major within one
   office of bed yawns and dims their lamp, and a Minor can answer "she takes
   her thoughts to bed at the Snuffing."
4. **Receipt** — next morning the Major's first greeting draws on the settled
   memory, and a moved round leg is observable, so the player can verify the
   commit landed.

Player loop: learn the bedtime from lamps, yawns and gossip; land the
confession or lie inside the last office; watch tomorrow prove it took —
knowing a wrong done at that hour is written in first person, permanently.

## Load-bearing risk

The night gate: rule 2 makes the lane yield absolutely while *anyone* is on
stage with the player, and `submit()` silently drops any reflection whose
`due.day != today` — so the mechanic's core move (standing with a Major at
their bedtime to deliver the last word) is precisely the state in which their
Night Office refuses to run, and lingering past day rollover means the day
never commits at all. Unaddressed, the taught mastery is a lie; addressed
deliberately, "keeping them up" becomes a second mechanic — but that has to
be a decision, not an accident.

## First step

In `crates/cathedral-sim/src/night.rs`, thread `self.bedtimes[actor_id]` into
the `render_night_prompt` call in `submit()`, record a per-actor
`last_before_sleep` watermark on `recent_history` when `ring_bedtimes`
crosses the office before that bedtime, emit the tail as a separate sheet
section in `prompt/mod.rs`, and add the "sits heaviest" paragraph to
`assets/prompts/night.j2`. Verify by reading the archived night prompts from:

```sh
cargo run -p cathedral-backends --bin cathedral-headless -- \
    --fake --night-office --start-office waning --seconds-per-day 300
```

## Children

- **"Keeping them up" as a canon verb** — staying on stage with a Major past
  midnight already prevents their day committing (the gate yield and the
  `due.day != today` drop are both live code); surface it with escalating "I
  must sleep" lines and a weariness cost next day, so a player can
  deliberately filibuster a witness's memory of a crime they saw. Zero new
  sim machinery — and it defuses the load-bearing risk by making the gate's
  behavior legible.
- **Morning receipt — "she slept on it"** — the first greeting after a night
  in which a `remember` landed references the settled memory, and a
  `set_round` change is observable as her walking somewhere new. Mastery
  mechanics die without feedback; the prompts/ archive already proves what
  committed, it just needs one in-world surface.
- **Bedtime intelligence as a tradeable good** — authored rounds give Majors
  genuinely different bedtime offices, neighbors and paid gossip disclose
  them, and drunkenness/weariness lets a bedtime drift a bit — so the
  knowledge is real, soft, and worth buying.
- **The curfew bell as the commoners' public commit deadline** — Minors
  already commit as one ward batch at the Snuffing, so a scene seeded before
  the bell becomes tomorrow's ward mood for a whole ward; after the bell it
  waits a day. Teach it: the Scold's curfew peal IS the save bell. Scales the
  identical mechanic to ~120 people for zero extra provider calls.
- **Wrongs bake hardest** — an asymmetry in `night.j2`: injuries in
  `last_before_sleep` weigh double ("you go to bed angry") while kindnesses
  there need corroboration from earlier in the day to fully settle. A bedtime
  apology softens but cannot erase a day of wrongs — which directly kills the
  degenerate apology-sniping exploit, purely with prompt text.
