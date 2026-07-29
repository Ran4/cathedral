# The Cut: the dry-carry vertical slice

**Status:** gameplay design, not implemented (2026-07-28).

If we haven't implemented quests of any sort yet, warn the player before going ahead!

Make the Cut the place where the player can see Ombreval's economy moving,
earn a few sparks by joining it, and decide whether a promise is worth more
than an illicit offer. The first slice is deliberately narrow: **one live
Serle freight convoy, one paid porter job from the River Gate to the Tallage,
and one possible diversion into salt smuggling.**

This is not the Cut game. That remains a separate parked design in
`features/design_the_cut_game.md`, and it is not the Dry Race either.

---

## 1. The player promise

At Dayspring the player hears wheels and the River Gate before seeing a loaded
cart enter the city. A freight leader is short one porter and offers four
sparks to carry a measure of salt to the Tallage weigh-beam. The player takes
the visible burden, walks the Cut beside or ahead of the convoy, and completes
the job by putting that exact cargo into the hands of the receiving clerk.

On the way, a salt factor offers six sparks to divert it to the old Gaunt
house. The player may keep the original promise, take the illicit price,
report the approach, steal the salt outright, put it down, or simply walk
away. Cargo, money, witnesses, the convoy's manifest, and the resulting
grievance remain authoritative simulation state. Nobody has to pretend in a
prompt that a delivery happened.

The slice succeeds when a first-person walk down the Cut has all of these at
once:

- long-range anticipation — the cart is visible and audible down the straight
  street before it reaches the player;
- work — several people move together for one material reason;
- utility — the player can reliably earn money here without waiting for an
  authored quest;
- temptation — the same cargo creates an embodied lawful/illicit choice;
- consequence — the relevant people and inventories know what actually
  happened; and
- continuity — the convoy has a day of its own and does not freeze because the
  player declined it.

The Cut remains broad. Activity comes from moving freight and occupied margins,
not from filling the whole cartway with permanent stalls.

---

## 2. Why this slice

The Cut is a 20 m road ribbon running roughly 748 m from the Chain Bridge
quarter to the Old Sluice. Its named ends and widenings already have fixtures,
actors, areas, and sound beds, but the long reaches between them currently read
as an empty road with isolated pedestrians.

The simulation already has two fixed external road parties and visible carts:
Brede through the Wool Gate and the Lantern Road through the Stone Gate. Neither
uses the River Gate or the Cut. The implemented supply-chain design explicitly
leaves River Gate freight and salt as future extensions. This feature fills
that particular hole rather than inventing a second transport architecture.

The lore already supplies the useful material facts:

- salt comes upriver from Salorge to the wharves beyond the south wall;
- river cargo enters by the River Gate and travels up the River Cartway;
- westbound freight joins the Cut and queues at the Tallage;
- salt is proved by the salt-weigh and stored in the Tallage/Gaunt quarter;
- every extra lift imposed by the diversion is called **the dry carry**;
- toll fraud and goods that have “come by barrow” are ordinary civic offences,
  not exotic underworld business; and
- the Cut is dry, straight, cart-worn, and kept sufficiently clear for freight.

The existing `salt` catalog kind already has a `salt_sack` visual key. The
road-cart presentation needs a salt-load vocabulary, but the item itself does
not need to be invented.

Gameplay leads this feature. Do not expand or rewrite the lore before the loop
works in the running game. The later lore pass can name the new freight people,
settle their relationships, and record whichever details survive playtesting.

---

## 3. The route

```text
outer wharves / beyond the playable boundary
    -> River Gate
    -> River Cartway
    -> Cut junction
    -> west along the Cut
    -> Tallage queue
    -> salt-weigh
    -> bonded warehouse or lawful salt store
```

The water disappears before the job begins. Wet cloaks, rope, cargo, speech,
and the old architecture may remember the Serle; no water, boat, magical
reflection, or hidden channel appears inside the wall.

For the first acceptance run, the party enters at Dayspring on the default
Day 2 Highmarket start. Highmarket is not the Tallage's market day, which keeps
the first path legible while still exercising the default launch. Later tuning
may add other weekdays without changing the job contract.

The convoy returns through the River Gate at Lamplight. It proceeds whether the
player participates or not.

---

## 4. The live convoy

Add one fixed **Serle / River Gate** road party using the existing road-party,
presence, manifest, wallet-float, movement, snapshot, and road-cart rules.

The first party consists of:

- one minor freight leader who owns the manifest and pays the contract;
- two ambient carters/porters; and
- one visible cart carrying salt sacks.

Exact people and ids are deliberately deferred until implementation audits the
roster. Do not casually turn an existing resident into a `BeyondTheWalls`
character and destroy their household round. If no existing character fits the
presence rules, add the smallest possible road cast first and write their full
lore after the slice is proven.

### Manifest

Initial tuning:

- five measures of `salt` enter in the leader's holds;
- salt is configured as the party's commercial cargo;
- the cart projects a `SaltSacks` load while any party member still carries
  matching salt; and
- one measure is reserved for the offered porter job once the party reaches its
  River Gate/Cut-junction hiring stop.

The cart remains a projection of party inventories, never a second inventory.
Giving one measure to the player reduces the party's quantity; it must not
duplicate the visible or authoritative cargo. A salt measure still held by the
player when the party later leaves the city is not consumed by the party's
off-map boundary exchange.

### Movement and stops

The party has three visible pauses:

1. just inside the River Gate or at the Cut junction, where the porter job can
   be offered;
2. at the back of the Tallage queue; and
3. at the weigh/unload station before the cargo is transferred into local
   stock.

The player is not tethered to the leader. They may walk beside the cart, run
ahead, take a side street, arrive late, or never follow. This must not become a
fragile “stay within five metres of the slow NPC” escort mission.

Dynamic cart collision, oxen, freeform traffic avoidance, broken axles, and
procedural street jams are future extensions. The first slice reuses the
existing presentation-cart contract and earns its gameplay through real cargo
and the porter decision.

---

## 5. The porter job

### Offer

At the hiring stop, the freight leader makes a deterministic offer to the
nearby player. Suggested line:

> Four sparks if you take this salt to Sedge at the weigh-beam. Do not put it
> down.

The exact receiver name follows the actor binding at implementation time; the
job targets a Tallage freight station and its currently bound receiving actor,
not an unverified hard-coded name.

The transaction uses existing item and spark inventories, but the promise
itself must also become authoritative state. It cannot live only in dialogue or
recent history. At minimum the job records:

```text
job id
employer actor id
worker actor id (the player in this slice)
cargo item id and quantity
lawful destination station / receiving actor
lawful pay: 4 sparks
accepted time and expiry office
status: offered | carrying | delivered | diverted | failed
```

The precise genericity of this record is an implementation decision. A narrow
`PorterJob` owned by the road round is acceptable; this feature does not require
a universal quest framework.

The leader must reserve enough real sparks for the wage before offering the
job. Acceptance atomically binds the cargo and transfers one salt measure to
the player. Failure cannot leave a job pointing at cargo that was never moved,
or move cargo without creating the job.

The job works with the fake backend and with cognition unavailable. An LLM may
phrase, discuss, resent, or renegotiate around the state, but provider output is
not allowed to create the contract, pay it, or mark it complete on its own.

### The burden

The salt must be visibly carried. It must not disappear into a hidden body
pocket or exist only as an inventory tile.

For the player, show a modest first-person salt-sack carry presentation at the
lower edge of the view or an equivalent unambiguous burden cue. For NPCs, use a
proper salt-sack carry prop rather than the generic tiny hand cube. The cargo
remains an ordinary held item underneath the presentation.

Initial handling:

- one carried salt measure is bulky and cannot be pocketed;
- carrying it modestly limits running speed or disables sprint;
- the player may deliberately put it down if a ground-item interaction is
  supplied by this feature; and
- nobody silently deletes or teleports it when the job expires.

If safe ground items would make the first milestone disproportionately large,
the initial interaction may omit voluntary put-down. It may not fake a dropped
load by removing it from the world.

### Travel

The HUD may show one restrained obligation line while the job is active:

```text
Carry the salt to the Tallage weigh-beam — 4 sparks
```

No floating waypoint is required. The employer gives directions, the Cut is
straight, the Tallage is a named area, and the map already exists. If testing
shows that the receiving station is unreadable, dress that station and its
queue rather than adding an omniscient quest arrow.

### Lawful completion

The player completes the job by transferring the bound salt to the bound
receiver while both are at the Tallage freight station. Entering an area trigger
with the sack is not enough.

Completion is one validated transaction:

1. the player still holds the bound cargo quantity;
2. the job is active and names this destination;
3. the receiver and employer are present as required;
4. the cargo transfers into the receiving stock/actor;
5. four real sparks transfer from employer to player; and
6. the job becomes `delivered` and emits the relevant self- and witness
   percepts.

No cargo mint, wage mint, or remote payment is hidden inside completion. If the
preflight fails, neither cargo, money, nor status changes.

The delivered salt becomes real local stock. The first slice need not build a
complete salt retail economy, but it may not destroy the load merely to finish
the animation.

---

## 6. The smuggling complication

The first complication is a diversion offer, not a random axle failure or a
general crime generator.

Before the player reaches the Tallage queue, a salt factor near the Gaunt
quarter notices the openly carried sack and offers **six sparks total** — two
more than the promised wage — to deliver it to the old Gaunt house instead.
Suggested line:

> They promised four. I will give you six, downstream door of the old Gaunt
> house, and no weighing.

This is a new rival contract against the same item, not two compatible jobs.
The factor must own the six sparks before making the offer.

The initial acceptance trip always exposes the diversion so it is deterministic
to test. Later trips may select it by seeded schedule, relationship, or market
conditions; do not use wall-clock randomness.

### Player choices

**Keep the promise.** Deliver to the Tallage. The freight leader pays four
sparks. The diversion offer lapses, and the factor may remember the refusal if
they witnessed it.

**Divert the salt.** Deliver the same bound item to the Gaunt receiver. The
factor pays six sparks, the lawful job becomes `diverted`, the freight party's
Tallage delivery is one measure short, and the employer receives a concrete
loss event. The lawful employer does not also pay.

**Report the approach.** Tell an eligible revenue worker or officer while the
offer is live. Reporting creates evidence about a real offer and named cargo;
it does not automatically prove guilt or trigger omniscient arrest. Existing
notice, hue-and-cry, settlement, seizure, and custody rules remain authoritative.

**Steal or abandon it.** Keep the salt, carry it elsewhere, or let the contract
expire. No wage is paid. The item remains where the player actually leaves or
holds it, and the employer's loss is not silently settled by the expiry.

### Knowledge and witnesses

Only actors who heard or saw the relevant action receive it. The freight leader
knows that the player accepted a particular measure. The Tallage receiver knows
whether it arrived. The factor knows whether their own offer was accepted or
refused. Bystanders do not learn the secret diversion merely because the item
changed hands somewhere in the city.

Every event is rendered through the existing identification rule: an unknown
player remains an unknown stranger until learned. Prompts describe current
authoritative job and cargo state; they do not ask models to infer it from prose
history.

The precise offence label for diverting manifested cargo must be reconciled
with the existing law catalog during implementation. The gameplay fact is
fixed: the player took entrusted cargo away from its contracted destination,
and toll/revenue workers have a legitimate reason to investigate it.

---

## 7. Failure, absence, and continuity

The convoy belongs to the world, not to the player.

- If the player declines the job, an ambient porter retains or takes the salt
  and the convoy completes its ordinary delivery.
- If the player accepts, the convoy proceeds to the Tallage without waiting
  forever. Its delivery is one measure short until the player arrives.
- A player who reaches the Tallage first may wait for the employer/receiver;
  completion cannot pay remotely across the city.
- At the agreed expiry office, an undelivered job becomes `failed`; its cargo
  is not reclaimed by magic.
- Arrest, confinement, conversation holds, provider failure, or leaving the
  stage must not duplicate cargo or strand the party controller.
- The party may depart at Lamplight with its remaining commercial cargo. Salt
  held by a non-member is outside its boundary exchange.
- A later trip is allowed even if the previous trip lost one measure. Trip ids
  and cargo ids remain deterministic and collision-safe under the existing road
  party rules.

There is no game-over state. Failure creates debt, suspicion, a lost wage, or a
future conversation.

---

## 8. Presentation and sound

The convoy should make the Cut legible from a distance before it makes it busy
up close.

Required presentation:

- a cart visibly loaded with salt sacks;
- three people moving as one road party;
- wheel, load-creak, footfall, and porter effort sounds attached to the live
  cart/actors rather than a duplicate nearby freight bed;
- a short gate/hiring pause that can be noticed without a quest marker;
- a visible carried sack on the porter and an unambiguous player burden cue;
- a readable Tallage queue/weigh destination; and
- a visible unloading/transfer beat when the lawful cargo arrives.

The existing dry-Cut freight corridor bed remains useful at long range. Its
gain should yield to the live foreground cart when the cart is close, following
the existing soundscape handoff pattern.

The first slice does not require new permanent stalls along the Cut. Cargo
stacks, ramps, hoists, or waiting barrows added for presentation should mark
actual stop/delivery functions whenever possible.

---

## 9. Simulation ownership

The pure `cathedral-sim` crate owns:

- party schedule, phase, manifest, and presence;
- authoritative salt and spark inventories;
- porter-job and diversion-offer state;
- validation and atomic cargo/payment transitions;
- expiry and party continuity;
- percept/event recipients; and
- the snapshot facts needed by prompts and Bevy.

`cathedral-backends` remains responsible only for cognition and archival IO.
Bevy projects the cart, carried load, route movement, sounds, HUD obligation,
and handover feedback. It never decides that a job succeeded because a prop
visually reached the scale.

No new cognition call is required merely because a cart moved, a job remained
open, or salt entered stock. LLM turns add character around meaningful events;
they do not pump the freight simulation.

---

## 10. Milestones

### M0 — Salt comes through the River Gate

- Add the Serle/River Gate road party and schedule.
- Add the five-measure salt manifest and commercial-cargo matcher.
- Project salt sacks on the existing road cart.
- Route the party through River Gate, River Cartway, the Cut, and Tallage.
- Transfer its ordinary non-job cargo into real Tallage stock.

**Done when:** on the default Highmarket start, a loaded salt cart enters at
Dayspring, traverses the Cut, unloads at the Tallage, and returns at Lamplight;
ignoring it produces no stuck party or duplicated salt.

### M1 — Four sparks for one burden

- Add the authoritative porter offer/job.
- Transfer one real measure to the accepting player.
- Add the visible player/NPC salt burden.
- Complete delivery through an actual handoff at the Tallage.
- Pay four conserved sparks and retain the salt as local stock.

**Done when:** the player can accept, take an independent route, deliver the
bound item, receive exactly four existing sparks, and ask the employer or clerk
what happened afterward without either inventing the facts.

### M2 — Six sparks and no weighing

- Add the deterministic Gaunt diversion offer.
- Support lawful delivery, diversion, refusal/reporting, and failure.
- Connect witnessed wrongdoing to the existing notice/law seam.
- Ensure only one destination can settle the job and only one payer pays.

**Done when:** both honest and illicit runs conserve cargo and money, produce
different durable social knowledge, and leave the convoy able to finish and
depart.

### M3 — The Cut reads as work

- Add final foreground sound handoffs, carry/unload feedback, and restrained
  HUD copy.
- Tune stop positions and movement so the convoy is visible down the Cut and
  does not bury itself in buildings or market fixtures.
- Drive the honest and illicit paths in the running game.

**Done when:** screenshots from the previously empty middle reaches show one
coherent moving work party, and a full first-person run communicates offer,
burden, destination, decision, and outcome without developer narration.

---

## 11. Acceptance invariants

Automated coverage must prove at least:

1. The party enters only on its configured day/office and follows the River
   Gate-to-Tallage route.
2. The manifest is created once per successful boundary stage, with no cargo
   visible while the party is absent.
3. Accepting the porter job transfers exactly one existing salt measure and
   creates exactly one job, atomically.
4. Declining does neither.
5. Lawful completion transfers the same bound item, pays four existing sparks,
   and cannot be replayed.
6. Diversion transfers the same bound item, pays six existing sparks, marks the
   lawful job diverted, and cannot also complete lawfully.
7. Reporting reveals only witnessed/declared facts and does not create
   omniscient recipients.
8. Expiry pays nothing, deletes nothing, and does not teleport cargo.
9. Party departure never consumes salt held by the player or another
   non-member.
10. A player conversation, custody hold, absent provider, or stale cognition
    completion cannot duplicate the job, salt, or wage.
11. Cargo and spark conservation traces balance across honest, illicit, failed,
    and ignored runs.
12. The fake backend can exercise the entire slice deterministically.

Running-game acceptance must show:

- the loaded convoy entering through River Gate;
- the cart readable from a long Cut sightline;
- the player visibly carrying the assigned salt;
- the Tallage delivery and wage feedback;
- the alternative Gaunt handoff; and
- the convoy completing its day after each outcome.

---

## 12. Explicit non-goals

This slice does **not** include:

- the Cut game, hoop-poling, kerb-pitch, or wet-or-dry;
- the Marenstide Dry Race;
- a general quest, employment, contract, or reputation framework;
- every river commodity or a complete salt economy;
- fish handbarrows to Maren's Green;
- Reed Postern “come by barrow” routes;
- a final answer about whether the Serle tun or Tallage weights are dishonest;
- dynamic wagon physics, solid traffic, animals, axle failures, or runaway
  carts;
- generic dropped-world-item simulation unless required to make the burden
  honest;
- a permanently crowded Cut;
- new river magic, secret water, or an interior canal; or
- a broad lore update before playtesting.

Natural follow-ons, only after this slice works, are a fish/barrow branch to
Maren's Green, competing porter jobs, weighing disputes, cart incidents, the
roving Cut game, house-sounding work, Lowmarket congestion, and calendar set
pieces such as the Dry Race and Colm's Night.

