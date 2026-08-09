# Obstruction as a live loop

**Status:** gameplay design, not implemented (2026-07-28). Point 3 of
`features/the_cut_improvements/index.md`.

The Cut's one civic rule is that the cartway stays clear. The city has a
sergeant who walks it twice a day, a notice system with a four-rung escalation
ladder, and a bell that tells the street to clear. None of them are connected
to anything, because nothing on the Cut can be in the wrong place.

This document is about giving the street something to get wrong, and then
listing — at length — what a player can *do* about it. The ideas in §4 are a
menu, not a plan; §6 assembles a subset into milestones.

Related, and deliberately kept separate:

- `features/the_cut_improvements/the_cut_kerb.md` — draws the line. **Hard
  prerequisite** for anything the player is expected to read as an offence.
- `features/the_cut_improvements/the_cut_dry_carry.md` — the convoy that
  actually needs the lane. Shares the ground-item cost (§3, tier 2).
- `features/design_the_cut_game.md` — the same mechanic pointed the other way:
  the game *is* an obstruction.
- `features/implemented/law_and_order.md` M3–M4 — the notice ladder this rides on.

---

## 1. What already ships

This is the part index.md gets right. The consequence half of the loop is
built, tested and idle.

| Piece | Where | State |
|---|---|---|
| `raise_notice` verb, law-cast only | `crates/cathedral-sim/src/actions.rs:125`, `:2750` | ships |
| Escalation ladder `Word → Summoned → Warranted` | `notices.rs:104` (`Rung`) | ships |
| `summon` with a named bell and a due time | `notices.rs:116` (`Summons`) | ships |
| `settle_notice`, restitution via `taken: Option<ItemId>` | `notices.rs:129`, `actions.rs:1955` | ships |
| Notice decay, carriers, oldest-out eviction that skips warrants | `notices.rs:211` | ships |
| Segwin Mott's south beat, `Maren's Green → Tallage → Maren's Green` | `assets/world/rounds.json:469` (`p00a3`, `leash_m: 16.0`, curfew-exempt) | ships, walks the Cut twice a day, does nothing there |
| A curfew bell with a real trigger | `clock.rs` — `Office::Snuffing`, 21:00, seven strokes | ships |
| Bulky items that cannot be pocketed | `item.rs:209` `ItemSize::Bulky`, refused at `actions.rs:1439` with `too_big` | ships; 11 kinds — barrels' worth of cloth bolts, hides, wool bales, firewood, baskets, buckets, cook pots, grain and flour sacks |

And the street is not as unpeopled as index.md implies. `rounds.json`'s
`workplaces` table lists **the Cut as a candidate workplace for six ambient
trades** — `cloth_worker`, `laundress`, `general_labourer`, `cooper`,
`sanitation_worker`, plus `roper` and `scavenger` at the ropewalk — bound
nearest-to-home. Only the *named* routes are thin (Tam Rud the fuller, `et7rd`,
two work legs).

That matters enormously for this feature, because every one of those trades is
inherently spread out. A cooper's work is barrels. A laundress's work is a line
of wet cloth. A fuller's is cloth on frames. A scavenger's is a heap of
rakings. **The obstruction does not need to be invented — it is the work these
people already do, given somewhere to overflow to.**

*Verify at implementation:* how many of those trades actually bind to the Cut
depends on where they live. Count the real ambient population on the Cut before
authoring around it.

---

## 2. The binding constraint: the judgement is an LLM's

`raise_notice` is a **verb**, not a rule. Segwin Mott raises a notice because a
model decided to, in a turn, having read his sheet. There is no rule engine
that will ever conclude "that stack is over the line" on its own, and adding
one would be the wrong shape for this codebase.

**Therefore the sim's job here is not to detect offences. It is to make the
encroachment perceivable, and let the officer judge.**

Concretely:

- The sim owns a fact: *this thing is N metres into the cartway, and has been
  since the Waning.*
- The prompt renders that fact to whoever can see it, through the existing
  perception and identification rules.
- Segwin may raise a notice, may tell them to shift it, may walk past, may take
  a bribe, may be talked round, may have a bad day about it. All of that is
  already free.

This is the whole reason the feature is cheap where it is cheap, and it also
means it degrades gracefully: with the fake backend, nobody raises anything and
the street still visibly breathes (§4.F). Nothing is *load-bearing* on
cognition.

The corollary trap: do not add an `obstruction` offence type, an offence
catalog entry, or a rule that auto-raises. `WardNotice.deed` is prose. An
obstruction notice is a notice whose deed happens to be about the cartway.

---

## 3. The missing primitive, in three tiers

Everything in §4 needs the same thing: *something with a position that can be
over the line*. There is a genuine fork here, and it is the main implementation
decision in this document.

### Tier 0 — people are the obstruction (free)

Actors already have positions. `|x + 213.5| < 5.0` is "in the cartway" (kerb
faces at `x = -218.5` / `-208.5`, per `the_cut_kerb.md` §3). A porter standing
in the lane, a knot of three arguing, a queue that has backed out of the
margin — all of this is *existing state*, needing only a percept.

No new world state. No verb. No item work. Enables §4.A3, A5, C1, C3, F1, F2.

### Tier 1 — the work post spills (small, recommended first)

Give the margin a handful of authored **work posts** — a cooper's pitch, a
laundress's line, a fuller's frames, the scavenger's heap — each owned by
whichever ambient actor binds there. A post carries one number: how far its
work has spilled toward the centreline.

The spill is driven in code by the office and by whether the owner is present
(work accumulates through the day, is pulled back at the beat and at the
Snuffing). Bevy projects barrels/lines/frames from that one number. **No ground
items exist; the props are presentation of a single sim float.**

This is the sweet spot. It is a small amount of new state, it makes the entire
street breathe on its own without a single LLM turn, and it gives the player
something to fix, report, exploit or be blamed for. Enables most of §4.B, C, E,
F.

### Tier 2 — real ground items (the actual cost)

A `put_down` verb, an item that has a world position rather than an owner, and
a way to pick it back up.

There is nothing like this today: the verb table (`actions.rs:100–131`) moves
items only actor→actor (`offer_item` / `accept_offered_item` / `decline_offer`
/ `retract_offer`) or into a body pocket (`pocket_item` / `retrieve_item`).
Nothing has ever rested on the ground.

`the_cut_dry_carry.md` §5 hedges around exactly this gap ("If safe ground items
would make the first milestone disproportionately large, the initial
interaction may omit voluntary put-down"). **The cost is shared between the two
features** — build it for whichever lands first.

Bulky items make it self-selecting and give the mechanic its teeth for free:
the 11 `ItemSize::Bulky` kinds already cannot be pocketed, so they are exactly
the things a player must either carry, hand to someone, or set down. Required
for the whole of §4.A and §4.D.

**Recommendation:** tier 1 alone, first. It is the cheapest thing that makes
the Cut a place where a rule is visibly kept and visibly bent. Add tier 2 when
either this feature's cause-half or dry_carry's burden needs it.

---

## 4. Gameplay

The menu. Each entry says what the player sees, what they do, and which tier it
needs.

### A. The player as the cause

**A1 — Set it down and be told to shift it.** *(tier 2)*
You are carrying something bulky down the Cut. You put it down — to rest, to
free a hand, to talk to someone. It is in the cartway. Whoever's business it
inconveniences tells you first; if you leave it, and Segwin's beat comes past,
it becomes a word on the ward's tongue. The full existing ladder is then
available to you as consequence: `Word` → `Summoned` to answer by a named bell
→ `Warranted`, and any law-cast actor may `seize` you anywhere in the city. The
notice system does not need to learn anything new to do this.

**A2 — The salt sack you were told not to put down.** *(tier 2, + dry_carry)*
The strongest single version, and it costs almost nothing on top of the two
features it joins. The dry_carry porter job's line is literally *"Do not put it
down."* The sack is bulky; the job disables sprint. So the game gives you a
burden, a distance, and a reason to rest — and the rest is the offence. You are
not choosing between good and evil, you are choosing between your shoulders and
a notice.

**A3 — Stand in the lane.** *(tier 0)*
The convoy is coming and you are in the cartway. Carters shout, the leader
swears, and if you do not move it is an ordinary discourtesy that an officer
may or may not care about. Free, and it teaches the line exists before any
notice ever fires.

**A4 — Buy something too big to carry home.** *(tier 2)*
A cook pot, a wool bale, a cloth bolt at the Tallage. The city's markets are
already on the Cut's ends. Getting a bulky purchase 240 m down the middle reach
is a small logistics problem, and every place you set it down is a decision.

**A5 — The favour that lands on someone else.** *(tier 1)*
You help a cooper shift barrels and stack them wrong. The pitch is *theirs*.
Segwin comes past and the notice names them, not you — because the notice
system records `accused: Option<ActorId>` from what the raiser believed, and
the raiser did not see you do it. You can let them wear it, or say something.
This is a genuine moral beat produced entirely by the existing witnessing
rules.

### B. The player as the fix

**B1 — Beat the bell.** *(tier 1)*
The gazetteer's rule for the squares is that stalls and goods "must clear for a
bell". The Snuffing already rings at 21:00, seven strokes, with a real trigger.
An overspilled pitch near the Tallage or Maren's Green at Lamplight is a timer
you can see and hear coming. Help clear it and you are owed something; the city
has a working `settle_notice` and a working spark economy to pay you in.

**B2 — Paid to move what someone cannot leave.** *(tier 1/2)*
A vendor cannot abandon their pitch and their goods are over the line. You can
walk. This is a porter job with no convoy attached — the smallest possible
employment loop on the Cut, and it works on any ordinary day.

**B3 — Clear ahead of the cart.** *(tier 1, + dry_carry)*
The convoy is at the Chain Bridge end and the middle reach is fouled. You get
200 m of straight sightline to see the problem before the cart does. Fix it and
the freight leader owes you; do not and the cart stops, which is its own scene.

**B4 — Argue it away.** *(tier 0/1)*
Segwin has raised a word about somebody. You know something he does not — the
goods were the fuller's, not the laundress's; they were inside the line until
the scavenger's heap pushed them out. `say` and the existing notice state are
the entire mechanic. The officer may settle it, may not.

**B5 — Settle someone else's word.** *(ships today)*
`settle_notice` already exists, and the wronged party may settle their own
notice whether or not they serve the law. Paying off a neighbour's obstruction
notice is a relationship move that needs no new code at all — it only needs
obstruction notices to exist in the first place.

### C. The player as witness

**C1 — Tell the sergeant.** *(tier 0/1)*
The important structural fact: **Segwin is not omniscient and does not live on
the Cut.** He walks it twice a day. Everything that happens between his passes
is invisible to him unless somebody carries it. That is an informer loop that
requires no new perception rules — it falls out of the existing
witnessed/identified percept model.

**C2 — Tell him wrong.** *(tier 1)*
Report goods that are, in fact, inside the line. He walks over and looks. The
notice system has no concept of a false report and does not need one — the
consequence is entirely social, and it is remembered by whoever heard you.

**C3 — Watch him miss it.** *(tier 1)*
The pitch pulls its goods back in as the beat approaches and puts them out
again once he has gone. You can see the whole 240 m reach do this. Nothing is
asked of you; it is characterisation of a city that polices by walking.

**C4 — Learn the beat.** *(tier 0)*
Segwin's route and hours are fixed data. A player who watches for a day knows
when the Cut is unwatched. Every other idea in §4.D depends on the player
having worked this out for themselves, and none of it needs to be told to them.

### D. The player as exploiter

**D1 — Foul the lane to stop a cart.** *(tier 2, + dry_carry)*
A stopped convoy is a convoy you can talk to, buy off, follow, or rob. The
obstruction is the tool, not the crime you happen to commit.

**D2 — Foul it to make someone late.** *(tier 2)*
Salt that misses the weigh-beam before the Tallage closes is salt that has to
sleep somewhere. Whoever benefits from that is a person you could have met
first.

**D3 — Move their goods, not yours.** *(tier 2)*
Pick up a neighbour's bulky item and set it in the cartway. The notice names
whoever the raiser thinks owns it. A frame-up built from two existing verbs and
one new one.

**D4 — Play the hoop game in the lane.** *(tier 1, + `design_the_cut_game.md`)*
This is why point 3 and point 4 of the index are the same feature seen twice:
the hoop game blocks the cartway *by design*. The bet is not only whether the
hoop runs — it is whether the run finishes before the beat arrives. Enforcement
and play become one system, which is exactly the thing a ward election can
swing.

**D5 — A spark to look the other way.** *(tier 1)*
Offering an officer money is an ordinary `offer_item` of coin. Whether it works
is a model's judgement, and it should be — Segwin refusing a bribe is as good a
scene as Segwin taking one, and neither needs new state.

### E. The street as politics

**E1 — Whose six metres?** *(tier 1)*
The margin is 6.7 m from kerb to façade. The householder behind it thinks it is
theirs; the Bench says it is the city's. That disagreement is already in the
lore, and with a drawn line the player can hear both sides while standing on
the contested ground.

**E2 — Selective enforcement.** *(tier 1)*
Segwin's beat covers Maren's Green and the Tallage. If notices cluster on one
end of the street, the player will notice, and can raise it with him, with the
victims, or with whoever runs the ward. No mechanism is needed to *implement*
bias — it will emerge from the model and from who happens to be present when he
passes. Something needs to make it *visible*, which is what a sheet of live
notices already does.

**E3 — The trades obstruct each other.** *(tier 1)*
Cooper against laundress against fuller against scavenger, all on the same 6.7 m
of margin, all with bulky work. The cart is not the only thing a spilled pitch
gets in the way of. Grievances between ambient neighbours are cheaper and more
frequent than grievances with the law, and they give the player somebody to
take a side with.

**E4 — The word follows you off the street.** *(ships today)*
A notice raised on the Cut is carried by gossips across the city, ages in
speech with a stamped `since`, and decays. An obstruction is not a local event;
it is something a stranger three squares away might already have heard about
you. This is entirely existing behaviour and it is the reason obstruction is
worth wiring into notices rather than into a bespoke system.

### F. What happens with no player at all

**F1 — The beat finally does something.** *(tier 1)*
Twice a day a sergeant walks 240 m of straight street and things move as he
comes. Watchable, repeatable, and the single clearest signal that this city has
a law in it rather than a law menu.

**F2 — The daily tide.** *(tier 1)*
Work creeps outward across the offices, snaps back at the beat and at the
Snuffing, and creeps again. A player who stands at the middle reach at the
Waning and again at Lamplight sees a different street. That is the answer to
"the Cut reads as empty" that costs the fewest tokens: **zero**, because it is
driven by the clock and not by cognition.

**F3 — A notice you had nothing to do with.** *(tier 1)*
You walk past an argument about a barrel. It is not your barrel. It resolves
without you. The city is more convincing for having business that is not the
player's.

---

## 5. Simulation ownership

`cathedral-sim` owns: work-post spill state and its clock-driven schedule
(tier 1); ground-item positions and the `put_down`/pick-up transitions if tier
2 lands; the encroachment fact as rendered into percepts and the prompt sheet;
notice state, exactly as today.

`cathedral-sim` does **not** own: the judgement that an encroachment is an
offence. That stays a verb.

Bevy projects the props from the spill number, the ground items from their
positions, and whatever HUD copy an active notice against the player deserves.
It never decides that something is obstructing.

`cathedral-backends` is unchanged.

---

## 6. Milestones

### M0 — the street breathes (tier 1, no player verbs)

Work posts on the margin, spill driven by office and owner presence, pulled
back at the beat and the Snuffing. Props projected from the spill. The
encroachment rendered as a percept to whoever can see it, so Segwin *may* act.

**Done when:** standing at the middle reach (`tp -213.5 1.7 -180 0`) at the
Waning and again at Lamplight shows a visibly different street, with the fake
backend and no cognition at all; and with a live provider, Segwin's beat
sometimes produces a notice whose deed is about the cartway.

Delivers §4.F entire, plus C3, C4, E3.

### M1 — the player in the loop, without new verbs

Reporting, arguing, settling, bribing — all of it is `say`, `offer_item`,
`settle_notice` against M0's state. Paid clearing jobs (B2) if the spill can be
reduced by a player action without full ground items.

**Done when:** a player can walk the Cut, find a spilled pitch, and change what
happens to it by talking to somebody; and the notice they cause or prevent
shows up on the sheet and in gossip elsewhere in the city.

Delivers §4.B4, B5, C1, C2, D5, E1, E2, E4.

### M2 — put it down (tier 2)

The `put_down` verb, ground items with world positions, pick-up. Bulky-only
first if that keeps it small. Coordinate with `the_cut_dry_carry.md` — whichever
feature reaches this first builds it for both.

**Done when:** an item set down in the cartway stays where it was left, is
perceivable to passers-by, can be picked up by anyone, and cannot be duplicated
or deleted by the notice, custody, or party-departure paths.

Delivers §4.A entire and §4.D1–D3.

### M3 — the bell and the beat as deadlines

The Snuffing clearing rule near the squares; the beat as a thing you can time.
Tuning pass on spill rates and stop positions.

**Done when:** a first-person run at Lamplight communicates a deadline without
a HUD timer.

Delivers §4.B1, B3.

---

## 7. Non-goals and traps

- **No offence catalog, no auto-raise, no obstruction rule engine.** §2.
- **No collider on the kerb.** `the_cut_kerb.md` §2.1 — a collided kerb severs
  the margin from the cartway for 748 m and the nav bake drops it. Anything
  this feature places in the margin inherits that constraint.
- **Do not make the cartway impassable.** The player must always be able to
  walk anywhere on the Cut. Obstruction is social, not physical.
- **Do not fill the street.** `the_cut_dry_carry.md` §1 is right: the Cut stays
  broad. Spill is an exception that reads as an exception.
- **Not the Cut game** (`design_the_cut_game.md`), though D4 is where they
  meet.
- **No new cognition budget.** M0 must run at zero provider calls. Turns are
  spent only when somebody chooses to act about an encroachment.
- **Ground items must not leak.** If M2 lands, every path that ends an actor's
  turn — custody, seizure, party departure, provider failure, conversation
  holds — must be audited for cargo duplication. dry_carry §11 invariants 8–11
  cover the same ground; reuse them.
- **Blocked stairs and mooring rings are not obstructions.** The kerb's margin
  furniture (`the_cut_kerb.md` §3) is permanent city fabric. Do not let the
  spill logic treat it as encroachment.

---

## 8. Acceptance

```sh
# the tide: same camera, four hours apart
CATHEDRAL_FAKE_BACKEND=1 CATHEDRAL_DRIVE='wait-online; \
  tp -213.5 1.7 -180 0; shot cut_waning; \
  key KeyT; key KeyT; sleep 20; \
  tp -213.5 1.7 -180 0; shot cut_lamplight; \
  tp -213.5 25 -180 0 -25; shot cut_reach_from_above; quit' cargo run
```

Compare `cut_waning` against `cut_lamplight` — if they are the same picture,
M0 did not land.

Invariants worth a test:

1. Spill is a pure function of clock, owner presence and authored post data —
   no wall-clock randomness, deterministic under the fake backend.
2. Spill never crosses the centreline, and never reaches a `the_cut` area box
   edge (`x -219.1 … -207.9`).
3. `collision_footprints.json` is unchanged — nothing here is a collider.
4. Zero provider calls are required for M0; a run with cognition unavailable
   still shows the tide.
5. A notice raised about an encroachment settles, decays and escalates through
   exactly the existing `Rung` ladder, with no new notice fields.
6. (M2) An item put down conserves: it exists in exactly one place, is not
   duplicated by pick-up, and is not deleted by notice settlement, custody, or
   a road party leaving the city.

---

*Filename note: this file is `obstruction_as_a_live_lop.md` — "lop" for "loop".
Left as-is because `index.md` and this document are the only references; worth
renaming if anything else comes to link it.*
