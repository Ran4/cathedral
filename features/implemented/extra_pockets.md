So, now we have an inventory system. Items are nondescript stored on the character's body.

But, humans already have places to put things.

It should be possible to have items in many places:

* Inventory (like today; nondescript "on the body")
* In the mouth (right click coin in inventory -> pick put in -> mouth)
* In the butt
* In the frontbutt (girls only!)

Of course, there should be actions associated with each place. Example:

* Right click ale that's in the mouth -> choose swallow
* Right click ale that's in the mouth -> choose spit
* Right click holy water that's in the mouth -> choose gargle
* Right click poop that's in the butt -> choose expel

Obviously poop should form in your butt some time after eating.

## Full design

We do this in turns. Start with a list of fun ideas to do, the actual implementation comes later.
Gargle things, make love -> semen in your butt/frontbutt/mouth etc.

### The fun list (unsorted, unfiltered)

* **Coin in the cheek** — the classic cutpurse defence. A spark in the mouth can't be
  lifted, but you can't talk properly either, and paying a vendor with a wet coin
  should cost you: vendors react, and a `condition=wet` metadata fork could carry a
  price discount (metadata already forks stacks, so wet sparks won't merge with dry).
  This works for all sorts of things - if poop in butt and you put bread in butt, the
  bread turns into poopstained bread, which... the npc:s won't like lol
  but you can always wash your poopstained bread to get wet bread (also not nice).
* **Two-stage drinking** — drinking becomes "take a mouthful" first, then choose:
  swallow (satiety/thirst/drunkenness apply), spit (item lands on the ground, or *at
  someone*), gargle (pure theatre + sound), or hold it and walk around like a wine
  taster. `eat` stays as the one-step verb; the mouth path is optional finesse.
* **Cheeking the draught** — pretend to swallow, spit it out later. Deception with real
  mechanics: an NPC told to drink something can *cheek* it. The LLM gets a genuinely
  interesting choice, and a watching character only sees "she drank it".
* **Spitting as a social act** — spit at someone: a targeted percept, an insult
  the whole square understands, and on the cathedral steps possibly a ward notice.
* **Gargling holy water** — does nothing. *Or does it?* At minimum the Sexton has
  opinions. (pure roleplay)
* **Swallow the evidence** — a palmable item can be swallowed whole and passes through
  on the digestion clock, returning with `condition=poopstained`. The gate search that
  checks your hands and your pack finds nothing. Patience as a smuggling mechanic.
* **The poop clock** — eating starts a gut timer on the sim clock; when it lapses, a
  poop item forms in the butt slot and an *urgency* body status ramps (same carriage
  seam as drunkenness/weariness, so the walk shows it). Expel to clear. Where you do
  it matters: chamberpot, gutter, the canal — witnessed in the wrong place, it's a
  notice.
* **Muffled speech** — a full mouth marks your `say` percepts as mumbled. Listeners'
  inbox says "said through a full mouth"; whether the *text* garbles is an open
  question below.
* **Hands-free carry** — the offer choreography needs hands; a mouth-carried item is
  the porter's third hand. Fits the existing hands/offer puppet work.

### The slot model

Body pockets per character: `mouth`, `butt`, `frontbutt` — with `frontbutt` availability
authored per character in their lore JSON (the cast is fixed and hand-authored, so
this is data, not detection). The player record in `seed.json` says which the player
has.

Items stay in `World.items`; possession stays in `holds`. A pocketed unit is a
**reservation** through the `inventory.rs` authority, exactly like offer promises and
transform reservations: pocketed units cannot be offered, sold, eaten, or counted by
a vendor restock while pocketed. Retrieval-first is the rule — one extra action of
friction is the price that buys concealment.

Capacity: **one palmable stack-unit per cavity.** This introduces a `size` field on
catalog kinds (`palmable` / `handheld` / `bulky`); only palmable fits. A halberd does
not go in your mouth, and the error for trying is `too_big`.

### Verbs

Same shape as everything else — `VERB {json}` for NPCs, right-click menus for the
player, both hitting the same `apply_action` paths:

* `pocket_item {"item_id": "<id>", "slot": "mouth"|"butt"|"frontbutt"}`
* `retrieve_item {"item_id": "<id>"}` — slot is derivable, don't make the model repeat it
* `swallow {"item_id": "<id>"}` — mouth only. Edible: effects apply. Inedible: enters
  the gut with a return date.
* `spit {"item_id": "<id>", "target": "<id>"}` — Target: the insult, with the percept fan-out `say` already
   has.
* `gargle {"item_id": "<id>"}` — drinkables only; the item survives; emits a sound.
* `expel {}` — butt/frontbutt slot; whatever is there comes out where you stand.

Errors follow house style and land as `system:` inbox lines the model can self-correct
from: `slot_full`, `not_pocketed`, `too_big`, `wrong_slot`, `nothing_to_expel`.

**Prompt cost discipline:** these verbs must not tax all 500 actors every turn. The
verb documentation and the slot lines render *conditionally* — an actor with empty
pockets and nothing palmable in hand gets zero extra tokens, the same way offers only
render when pending.

### What observers see

* Your own sheet: `you_hold` entries gain a suffix only when it applies —
  `- k3f9x spark (in your mouth)`.
* Others see **nothing** while an item is pocketed — that is the entire point. But the
  *transition* is visible: pocketing or retrieving within sight range is a percept
  ("slipped something into her mouth"), following the unknown-people rule
  (descriptions and places, never ids — same discipline as `notices.rs`).
* Register matters twice over: the prompt wording stays period-euphemistic ("carried
  privily", "in the cheek") — partly for tone, partly because crude anatomical prompt
  text is a real provider-compliance risk. The sheet should read like the century it
  happens in, and cloud models will play along with a euphemism far more reliably
  than with the blunt version.

### Digestion, determinism

Gut formation time is a pure function of (actor, meal, clock) — hash idiom, no RNG,
so the headless runner and the tests replay identically. The sim clock's 60× key
(`key KeyT` twice) reaches the timer without waiting out real hours, same as the
hour-gated soundscape beds. Urgency ramps as the timer overruns; it's a carriage
status like drunkenness, so it is visible in the walk and already has a drive-mode
stand-in seam (`status <name> urgency 0.8`) for eyeballing.

### Law and order interplay

The gate search (when law_and_order grows one) checks `holds` and finds body slots
never. Public expulsion, spitting at people, and paying with conspicuously wet coin
are exactly the shape `notices.rs` wants: a wrong on the ward's tongues, raised by
the law cast, cleared by restitution or laughed about for three days until the decay
clock takes it.

### Bodies, sounds

No new puppet morphs (a cheek-bulge blendshape is beneath our dignity, barely) —
reuse the gesture-verb choreography for spit/gargle/expel beats. New catalog rows
with `actor_emittable = true` and honest radii: `gulp` (2 m), `spit` (4 m), `gargle`
(6 m), and one discreet unnamed one (8 m, percept text tactfully vague).

### The intimacy seam (deferred)

Making love is its own feature with its own problems (relationships, consent,
privacy, what the camera does) and does not belong in a pockets document. What this
feature promises is only the **container side**: fluids are ordinary catalog kinds
that can sit in any slot like anything else, with provenance as metadata. Note the
tension to resolve there: provenance-as-id conflicts with the unknown-people rule —
notices solved this by keeping ids private on the record and prose public; the same
split applies. Park it as `features/intimacy.md`, not started.

### Milestones

* **M0 — the slot model.** Pockets on `CharacterState`, `size` on the catalog,
  reservation rules in `inventory.rs`, snapshot fields (serde-default, canary-checked).
  No verbs yet; a seeded world can simply *have* a coin in a cheek.
* **M1 — the mouth.** `pocket_item`/`retrieve_item`/`swallow`/`spit`/`gargle`,
  two-stage drinking, transition percepts, muffle flavor, player right-click menus,
  conditional prompt rendering. Golden prompts re-blessed once.
* **M2 — the lower pockets.** `butt`/`frontbutt` slots, authored availability,
  concealment vs the (future) search, `wet`/`poopstained` metadata economy.
* **M3 — the poop clock.** Gut timer, poop item, urgency status, `expel`, disposal
  percepts/notices, night-soil manifest line for an existing cast member.
* **M4 — NPC parity.** The verbs in NPC prompts (conditionally), cheeking as
  authored deception content, notices integration, headless transcript scenarios.
* **M5 — intimacy.** Separate feature doc. Not here.

### Open questions

* One unit per cavity, or a capacity number? (One is funnier *and* simpler. Two
  coins, one cheek each, is however objectively funnier still.)
  Hm, let's say two in total. Then bread inserted into [poop] turns into [poop, bread {condition=poopstained}]
  If we try to put more things in, we can't; if somethings created (e.g. poop or a key that you ate earlier),
  when you have two items there already, then one of those two item that's already there will (at random)
  be dropped.
* Muffled speech: flavor-only percept marking, or actually garble the text listeners
  receive? Garbling player STT output is a comedy goldmine and a usability hazard.
  flavor-only, the llm already tries to ignore stt errors, for good reason.
* Do ambient actors get these verbs at all, or majors/minors only? Conditional
  rendering makes the token cost near-zero, but ambient completion caps (350) are
  tight for a model that discovers it can gargle.
  yes, all get them
* Does gargled holy water do anything mechanical, or is the Sexton's disapproval the
  entire reward?
  Pure roleplay (which can lead to something mechanical, of course!)
* Night soil: a real priced buyer on a real round, or gutter flavor only until the
  supply chain (food_and_items M5) lands and shows how carrier manifests want to work?
  Nah, skip that for now.
* `spit` with no target: to hand (retrieve, but wetter) or to ground (item entity at
  your feet)? Ground needs items-on-the-floor to exist as a rendered thing.
  For no, target only. In future, when we have items on the floor, then it should drop on floor.

----------

## Implementation notes (M0–M4 shipped, 2026-07-24)

Everything above is implemented except the intimacy seam (parked as
`features/intimacy.md`). Where the letter of the sketch and the code differ:

* **Capacity** is two stack-units per cavity (`POCKET_SLOT_CAPACITY`), per the
  resolved open question. When the gut forms something into a full butt slot,
  the displaced unit is picked by the deterministic hash idiom and simply
  returns to plain `holds` (un-concealed) — nothing is destroyed.
* **A pocketed unit is a reservation**, exactly like an offer promise: the item
  stays in `World.items`/`holds`, `uncommitted_quantity` subtracts it, and the
  snapshot carries `ActorSnapshot.pockets: Vec<(BodySlot, ItemId)>`.
* **The metadata economy**: a mouth stamps `condition=wet` on non-drinks (via
  the new `World::restamp_metadata` stack-forking operation); sharing a lower
  slot with a stool stamps `condition=poopstained`. Both values are valid on
  *any* kind (`UNIVERSAL_CONDITIONS`) without per-kind declaration; a kind's own
  declared conditions (the apple's `bruised`) still work. Washing is not
  implemented — there is no verb for it yet.
  *(An unconditional butt stamp — an empty cavity fouling whatever entered it —
  was tried on 2026-07-25 and reverted the same day: what fouls a thing is the
  company it keeps. The stool-sharing rule is the intended one.)*
* **What witnesses get** is a two-ring fan-out (`deliver_pocket_percept`), added
  2026-07-25: everyone within `HEARING_RADIUS_M` still gets the item-blind line
  ("slipped something out of sight beneath their clothes"), but anyone inside
  `POCKET_PLAIN_SIGHT_RADIUS_M` (4 m, arm's length) is told what moved and which
  cavity took it — "hitched up their clothes and pushed an apple up their arse"
  / "up their cunt", "slipped a spark into their mouth", and the same on the way
  out ("pulled an apple out of their arse"). Concealment is now a matter of
  distance rather than magic; a mouthful of drink still reads as drinking at
  every range, so cheeking a draught survives. Ids still never appear — the
  observer's own `identify_ids` naming does the addressing. The turn explainer
  states the four-metre rule, so golden fixtures were re-blessed for it.
  **Note the register reversal**: the design above argued for period euphemism
  partly against provider-compliance risk. The blunt wording is a deliberate
  later call (the euphemism was not legible enough — NPCs read "slipped
  something away" and carried on haggling); if a provider ever starts refusing
  turns, these two strings are the first suspects.
* **The reaction is immediate** for the lower slots. The verb records its
  plain-sight watchers in `DomainEvent.witness_ids` (a mouth records none, so
  cheeking stays sly and cheap), and `Engine::nudge_pocket_witness` hands the
  nearest LLM one the priority slot — the same one-nudge-per-act shape
  `flush_sound` uses. Without it the percept sat in an inbox until the idle
  rotation came round, which is why the first version of this looked broken.
* **Drinks are palmable by fiat** (ale, milk, holy water): pocketing one unit is
  the "mouthful". A pail of water is deliberately not (`too_big` is honest).
* **`spit`**: target-only (as resolved). A spat solid *transfers to the target*
  (they now hold the wet thing); a spat mouthful of drink is consumed. When
  items-on-the-ground exist, untargeted spit should drop there instead.
* **`expel`**: voids both lower slots. Stools are consumed ("the gutter");
  anything else returns to plain `holds`. With a law-cast character within 20 m,
  a witnessed spit-at-someone or a voided stool mechanically raises a ward
  notice from the nearest officer (prose only, no ids — the notices split).
* **The gut**: meals coalesce into at most one brewing stool; a swallowed
  inedible always queues its own return (`condition=poopstained`). Lapse is
  `GUT_MIN_GAME_DAYS` (3 game hours) plus a hashed spread of up to 3 more; the
  engine's digest pass forms it, stains co-residents, and ramps the new
  `urgency` carriage status (0→1 over 2 game hours, quantized to 1/16 so the
  snapshot is not republished every poll). `expel` clears it on the next poll.
  Clock-less worlds (the frozen fixtures) have no gut at all.
* **Night soil round**: skipped, per the resolved question.
* **Sounds**: `gulp` (2 m), `spit` (4 m), `gargle` (6 m), `soft_report` (8 m)
  are catalog rows, all `actor_emittable`, with generated mp3 assets (the same
  run also filled in the long-missing `coin_clink` — its route comment says the
  standard playback supplements the balance-pan cue — and a `market_cry` that
  the soundscape route replaces, so that one sits unused). The spit *verb*
  renders its own targeted percepts and does not use the catalog row; the row
  is for `make_sound` theatre.
* **Muffled speech**: flavor-only, as resolved — a speaker with anything in the
  mouth slot gets "said through a full mouth" in listeners' inbox lines; the
  text is never garbled.
* **Prompt cost**: the verb docs and explainer render only when the actor has a
  pocketed unit or holds something palmable (`has_pockets`), and `frontbutt`
  is mentioned only for bodies that have one. Golden fixtures were re-blessed
  once as planned, and again in 2026-07 for the four-metre visibility line.
* **Frontbutt availability** is authored: `frontbutt` on a lore sheet or the
  player's seed record, defaulting from lore `gender == "f"`. The player record
  ships `false` — flip it in `assets/world/seed.json` to test.
* **Authored cheeking** (M4): Gude Quern (`p004q`, guide, pickpocket) spawns
  with a wet spark in her cheek and a memory explaining the habit.
* **Player UI**: `I` toggles the "Pack and pockets" screen
  (`src/smart_actors/inventory_ui.rs`): cursor released, movement/quickbar
  suppressed, carried + per-slot sections, right-click context menus (put in
  mouth/butt/frontbutt, eat, swallow, spit at the gazed target, gargle, take
  out, expel), errors surfaced in a feedback line, every control drive-mode
  clickable by `Name`. Fully-pocketed stacks are hidden from NPC hand props
  (concealment); `status <name> urgency 0.8` eyeballs the pressed walk.
