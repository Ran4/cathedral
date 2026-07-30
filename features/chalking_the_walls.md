# Focus: Chalking the Walls

*Stigmergy made touchable: the environment becomes the database — deepened
against `notices.rs`, the prompt sheet, and the witness seam.*

The player (and possibly also NPCs) leave rule-triggered chalk marks —
a cross on a debtor's door, a tally at a well — and other NPCs' cheap rules
read them: refuse credit at a chalked door, avoid a marked lane. The
environment is the database, and the player can tamper with the medium
instead of the agents: scrub a cross at night and commerce resumes, forge one
on an enemy's door and watch the ward's rules turn on them. Zero LLM cost
until someone catches you at it.

Everything is chalk — one medium, many signs. The vocabulary lives in the
mark *kinds* (cross, tally, ward-sign), not in a spread of materials.

The player or an NPC needs to have a chalk pen (new item) in their inventory
to be able to write on the wall.

## Sketch

A `Mark` is sim state, not decoration: `World.marks: BTreeMap<MarkId, Mark>`
in cathedral-sim, where
`Mark { kind, anchor, about: Option<ActorId>, author, raised_game_days,
strength }` anchors to an existing handle — a household door, a
`PlaceRegistry` place (a well, a shrine), or an `areas.json` lane — and a
small `assets/world/marks.json` catalog owns each kind's prompt label ("a
chalk cross at knee height") and decay, exactly as `items.json` owns item
kinds.

**Writers** are cheap rules riding systems that already know the facts:
`notices.rs` chalks the accused's door when a restitution notice ages unpaid
past N game days (it already holds accused + place), the round ladder notches
the well tally when a carrier draws, and the Night Office ward batch may
spend one already-budgeted line chalking a ward-sign at the shrine.
**Readers** split by cost: non-LLM rungs consult marks at their anchor (the
unpaid-sale path in `actions.rs` refuses credit at a chalked household,
movers path-cost a chalk-warned lane), while on-stage NPCs get the mark
rendered into the prompt's location description — the mark IS the prompt
line, so LLM reactions cost zero extra calls.

The player sees every mark as an undeciphered described glyph until an NPC
explains that kind in dialogue or a lore page names it, flipping it into a
per-kind learned set that enables a HUD tooltip — **literacy as loot**.
Scrubbing and forging are two hold-interactions that call straight into the
sim as commands (`scrub_mark` / `draw_mark`, plus drive-mode actions for
scripted testing); each takes in-world seconds, and anyone inside the 4 m
radius gets the blunt inbox line plus the instant priority turn via the
existing witness seam, flowing into `raise_notice` unchanged. The whole loop
runs headless: `cathedral-headless --fake` ages a notice, chalks a door,
refuses a sale, and prints it — no Bevy, no tokens.

## Load-bearing risk

Authority collision. The sim already has an authoritative social database
(ward notices, memories) — if marks are merely a projection of it, scrubbing
changes nothing real and forging is placebo, and the feature's whole promise
collapses; but if marks are authoritative, an NPC's memory can contradict the
clean door. The design only works if each gated behavior reads exactly one
source — credit refusal reads the chalk and never the notice — and the
contradiction that remains is bounded and diegetic (the shopkeeper who
remembers you anyway is characterization; a sergeant re-chalking from a
still-live notice is the ward healing its database). Fudge this partition and
the drift bugs read as the world gaslighting the player.

## First step

One vertical slice, one kind, headless-provable: add `MarkKind::ChalkCross`,
`Mark`, and `World.marks` to cathedral-sim (new `marks.rs`, wired into
`world.rs` beside `notices`), write it from the existing unpaid-restitution
age check in `notices.rs` onto the accused's home anchor, render it into the
prompt's location description (updating `golden_prompts.rs` via the ignored
regenerate test), and gate the one existing unpaid-sale/credit path in
`actions.rs` on it — then verify the full loop with
`cargo run -p cathedral-backends --bin cathedral-headless -- --fake -t 6`
before any Bevy visual exists.

## Children

- **Weathering as gameplay** — chalk decays against the authoritative
  weather: rain half-lives a mark in hours, while a sheltered anchor (under
  an awning, inside a covered passage, on the lee side of a door) holds it
  for days — and a sergeant's beat rung re-draws any mark whose backing
  notice is still live. The ward visibly repairing its database after a storm
  (or after your midnight scrubbing) IS the answer to "which source wins,"
  and *where* you chalk becomes the forger's real choice: an exposed forgery
  washes away on its own, a sheltered one has to be caught.
- **The slate** — stall and well tallies become the physical credit ledger:
  the unpaid-sale line points at chalk strokes the player can walk up, read,
  and erase; discrepancies are caught not instantly but on a scheduled
  reckoning when the Night Office ward batch compares slate to memory. A slow,
  legible detection beat on a prompt already paid for.
- **Ward dialects** — the mark vocabulary differs per ward: a knee-height
  cross means debt in the Wickmarket and fever in a river ward, taught only
  by NPCs of that ward via a targeted `teach_mark` verb modeled on
  `tell_way`. Literacy with mistranslation stakes, and each ward's teacher is
  a specific hand-authored person you must find and befriend.
- **Shrine steering** — chalked petition strokes at shrines are cheap
  counters that weight the ambient cast's zero-token evening re-roll and the
  soundscape's place beds, so a heavily-chalked shrine literally draws
  tomorrow's crowd and hum — a god-game lever that is pure stigmergy, and it
  gives the off-stage city motion back without a single LLM call.
- **Marks as the mute witness** — when tampering had no live witness, a
  patrol rung can still read the scene: fresh scrub-streaks or an off-pattern
  forged cross raise a ward notice about "a person unknown" at that place,
  attaching the player's stranger-id only if someone later places them there.
  Crime-against-the-medium gets a slow-burn consequence channel, and a
  forgery good enough to pass inspection stays deliciously possible.
