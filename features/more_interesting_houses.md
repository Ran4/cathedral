# More Interesting Houses

> Spec for turning anonymous burgage plots into places with functions, signs,
> sounds, schedules, and gossip. Idea-focused — implementation comes later.
> Companion to `interesting_houses.md` (the original prompt).

## The problem

The city has hundreds of houses and only about two dozen named places. Almost
every building is "for living in." Real medieval towns were the opposite:
nearly every street-facing ground floor *did something* — sold, brewed, cooked,
lodged, lent, washed, healed, or sinned — and the doing was legible from the
street via signs, smells, sounds, and open shutters.

The goal is not hundreds of new named landmarks. It's a middle tier between
"landmark" and "anonymous house": **typed houses**. A typed house has a
function, a visual tell, an audio signature, opening hours on the sim clock,
and one or two lines of prompt fodder so smart actors can use it and gossip
about it consistently.

## Design principles

1. **Most of them are still houses.** The medieval pattern is a dwelling that
   *also* does something, usually on the ground floor, usually run by the
   family living upstairs. Purpose-built commercial buildings should be rare
   (one fancy tavern, a couple of inns, the bath house). This keeps the
   existing burgage-plot generation valid — a typed house is a normal house
   plus dressing.
2. **Legible from the street.** No floating labels. Each type gets a
   sign-language element (see "Signboard language" below), an open/closed
   state, and a sound. The player should learn to *read* the city.
3. **Feed the smart actors.** Every type is a gossip venue, a daily-round
   destination, or both. The best return on this feature is not geometry, it's
   NPCs saying "Osanne's ale went sour this week" and being right.
4. **Rhythm over quantity.** A few places that open, close, fill, and empty on
   the Candor offices (Dayspring … Lamplight … the Snuffing) make the whole
   city feel alive; fifty static ones don't.
5. **Reuse what exists.** The Hungry Ox (boatmen's tavern, Maren's Green),
   Lise Copp's pawnshop, the masons' lodge, Malt Passage's malt-house, and
   Doctor Ferrant's house are already canon. New types should slot around
   them, not duplicate them.

---

## Catalog of house types

### 1. Alehouses — the rotating kind (the flagship idea)

Most medieval drinking happened in ordinary homes. A brewster (usually a
woman) brewed a batch, hung an **ale-stake** (a pole with a bundle of greenery)
over her door, and sold from her front room until the batch ran out — a week,
maybe two. Then the stake came down and it was a house again.

This is a gift to a procedural city: **alehouses are a system, not a set of
buildings.**

- Designate a pool of candidate houses (say 8–12, spread across wards). At any
  moment 2–4 are "up": ale-stake out, door open, benches visible, noise
  inside.
- The rotation runs on the sim calendar. A batch lasts ~a week; when it ends,
  the stake comes down and another candidate house goes up somewhere else.
  Regulars must *find out where the ale is this week* — which is exactly the
  kind of fact NPCs should trade in ("Sibbe's stake is out again, off
  Crookneck Lane").
- Each brewster has a name, a reputation, and a quality roll per batch:
  strong, fair, thin, or sour. Quality is prompt fodder and rumor fuel.
- **The ale-conner**: a civic official who must taste and approve each new
  batch before sale (real medieval office, inherently funny). He makes his
  rounds when a stake goes up. A brewster fined for bad ale is a week of
  gossip. Great recurring character.
- **Singing.** An alehouse at Lamplight leaks song. Even just 2–3 looping
  rowdy-chorus wavs (lore already has "The Water Knows One") emitted from
  whichever houses are currently "up" would transform the night city. Later:
  NPCs inside actually singing via TTS, off-key, together.
- Interior needs: nothing fancy. A front room, two benches, a barrel, fire,
  4–6 seated NPCs in the evening.

**Gameplay:** the freshest gossip in the city concentrates wherever a stake is
up. Visiting the current alehouses becomes the natural way to "check the news"
— the sim's rumor system gets a diegetic UI for free. Player can buy a mug
(1 spark) to sit and listen.

### 2. One fancy tavern — purpose-built, wine, doors that close

The Hungry Ox already covers the rough end (boatmen, Maren's Green). Add
exactly **one** establishment at the other pole, in a better neighborhood —
the eastern quarter near the Bellstand, or off the Draper's Reach among the
cloth money.

- Sells **imported wine** (a *bush* sign — the traditional vintner's mark —
  instead of an ale-stake), proper food, candles after dark.
- Prices gate the clientele: a cup of Ostrelle wine at 4–6 sparks when ale is
  1. Merchants, guild officers, freight brokers, a discreet canon.
- Signature feature: an **upstairs private room**. Deals, betrothals, and
  quiet arrangements happen there. NPCs can reference "what was agreed
  upstairs at the Brazen —" without the player having been in the room; if the
  player *is* in the room, they overhear a plot seed.
- Name in-register (practical, not saintly): **The Bell and Barrel**, **The
  Brazen Head**, or **The Ell** (after the cloth measure) — pick one later.
- Lore hook, light: the host is the kind of person who knows everything and
  says a tenth of it. Natural quest-giver / rumor broker archetype.

### 3. Inns — lodging, stabling, food, and news from outside

Two inns, sited where travelers actually arrive:

- **A gate inn** near the main land gate: big yard, stables, carts, carriers.
  Serves the salt and wool traffic. Loud, transactional, up before Dayspring.
- **A pilgrim inn** near the Gradine/Lanthorn precinct: dormitory beds, pious
  signage, a landlord fluent in relic-talk, badge sellers loitering outside.

Inns matter mechanically because **travelers are rumor injectors**. The sim's
gossip pool is otherwise closed; the inn is the tap where outside news enters
("the chain is down at the river mouth," "salt is dear in Salorge," "a
preacher is walking up from Brede"). Seed the inn with a slow trickle of
generated guests, each carrying one piece of outside news, and the whole
city's conversation gets a horizon beyond the walls.

**Gameplay:** the player can pay for a bed (sleep = pass time to a chosen
Candor office — useful once the daily round matters). The innkeeper remembers
who stayed and when — an alibi machine, or the opposite. The yard is a natural
stage for arrivals/departures the attention system can point idle cognition
at.

### 4. Brothels — the stews

Historically real, historically *regulated*, and best handled at door level:
what the game depicts is a house with a distinctive lantern by the door, a
firm doorkeeper, and people trying not to be recognized on the way in or out.

- One, maybe two, in the damp edge of the Reed Ward — off Tanners' Slip,
  where rents are low and the Watch's rounds are conveniently timed. Spoken
  name: **the stews** (generic) or a house name like **the Lantern with the
  Red Horn**.
- The interesting content is entirely social:
  - **Being seen.** NPCs entering/leaving is information. The sim's "who saw
    whom where" machinery gets its richest possible input. A churchwarden
    seen on Tanners' Slip at the Snuffing is a rumor with legs.
  - **The ledger of secrets.** The bawd knows more about the city's respectable
    men than anyone. She is untouchable in a way, and knows it.
  - **The Watch takes a cut**, which everyone knows and no one says — a
    standing example of tolerated illegality for the lore's politics.
- Depiction stays exterior/common-room only. The door policy, the doorkeeper,
  the lantern, the gossip. That's the game's entire interface to it.

### 5. The bath house

One public bath house — steam, tubs, gossip, and mild scandal. Medieval bath
houses were social institutions with a shady reputation the church grumbled
about.

- Needs water and fuel, which in Ombreval are both *plot-relevant* (the river
  left; wells matter — see `wells_and_water.md`). The bath house's water
  hauling and firewood bills are natural lore texture: it sits near a good
  well and burns a scandalous amount of wood.
- **Men's days and women's days** on the weekly calendar (e.g. men on
  Highmarket, women on Lowmarket, closed Bellday). The schedule is a fact NPCs
  know and plan around — and a mistake a foreign visitor can make.
- The barber-surgeon works an alcove: shaves, teeth, bleeding. Striped pole
  outside.
- **Gossip in undress is a social leveler**: in the steam, a mason and a
  freight broker talk as equals. Mechanically: conversations here can cross
  class lines that street encounters wouldn't, which makes the bath house a
  unique mixing chamber for the rumor system.
- Steam from a roof vent, splashing/murmur audio, a wet stone smell you can
  almost render.

### 6. Merchant houses

The top of the housing pyramid: 6–8 substantial houses for named trading
families — and the lore already has the families (Alder, Sparr, Vell, Copp,
Marle, Fitch…). A merchant house is a compound, not a room:

- Street front: a **shopfront with a drop-down shutter counter** (the classic
  medieval shop — the shutter hinges down to become the sales counter). Open
  by day, barred at night. Visually unmistakable and cheap to model.
- Behind/above: counting room (strongbox, ledgers, an apprentice asleep under
  the counter), hall, family quarters. A rear cart-yard or a claim on a
  warehouse (the Cut's old warehouses and the Tallage give sites).
- Factor's **house-marks** stenciled on crates and the doorpost — each family
  gets a simple geometric mark; the same mark appears on their goods around
  the city. Free world-coherence.
- **Gameplay:** merchant houses are where the city's *plots* live — marriage
  negotiations between families, a disputed inheritance (post-Hammering
  divided property is canon), a strongbox rumor, an apprentice who talks too
  much at the alehouse. Each merchant house should ship with one standing
  tension the smart actors can chew on.

### 7. The supporting cast (cheap, high-flavor types)

Each of these is one house + one sign + one sound + one line of prompt fodder.
Sprinkle by ward affinity:

- **Cookshop** — medieval fast food: pies, pottage, roast off a spit, mostly
  take-away out the window. Near the squares and the Tallage where working
  people can't cook. Smoke, sizzle, a queue at High Wick. (2–3 of them.)
- **Bakehouse / common oven** — most houses can't bake; households carry dough
  in and wait. A gossip node with a *schedule* (queue in the morning). The
  baker knows every household's business by what they bake.
- **Barber-surgeon** — if not folded into the bath house, a striped pole on a
  lane near Coswald's Yard (masons produce injuries).
- **Apothecary** — dried things in the window, a smell, a reputation for
  knowing what wives ask for quietly. Doctor Ferrant's respectable opposite
  number.
- **Scrivener / letter-writer** — writes and reads for the illiterate
  majority. Sees every secret in the ward pass across his desk in other
  people's words. Near the Tallage (contracts) or the Bellstand (petitions).
- **Dice house** — a back room that is *known about* rather than signed.
  Where wages go on Lowmarket eve. The Watch raids it when politics needs a
  gesture.
- **Wash-house / laundresses' yard** — linen on lines, pounding, and the
  city's most efficient information exchange. Laundresses know whose sheets
  say what. A well-adjacent court in the Reed Ward.
- **Almshouse** — a widow court formalized: six doors, a bell, a benefactor's
  plaque (a canon or a guild). Residents owe prayers for the donor's soul —
  gratitude with a rent attached, which is a mood.
- **Watch house** — where the Watch actually sits: a brazier, a bench, a cage
  for drunks to sleep it off before the fine. Near the Bellstand. Anchors
  curfew gameplay.
- **Smithy / farrier** — already implied by occupations, but deserves the
  audio treatment: rhythmic hammering is the best daytime sound-landmark a
  procedural city can have. Near the gate inn (horses).
- **Guild drinking hall** — the masons' lodge already exists; give one lesser
  trade (the glaziers of Cinder Row?) a hall that hosts feast-night dinners a
  few times a season. Scheduled, loud, exclusive — and resented by whoever
  isn't invited.

Deliberately **not** proposed: shops-as-inventory-systems, player crafting,
economy simulation. Every type above works as *theater plus prompt fodder*
with zero item mechanics.

---

## Cross-cutting systems (where the fun compounds)

### Signboard language

Pre-literate cities ran on pictorial signs, and a procedural city can too:

- ale-stake (pole + greenery) = ale here now; **stake absent = closed** —
  the sign *is* the open/closed state
- bush = wine; striped pole = barber; lantern by the door = the stews;
- painted board with an object (bell, barrel, ox, ell-rod) = tavern/inn names;
- house-marks = which family's goods/property;
- hung shutter down = shop open, shutter up = shut.

This is a small art/mesh budget (a dozen sign meshes + a stencil system) that
makes the *entire* city more readable and is fully in-period. Players learn
the iconography once and can then read any street the generator produces.

### The daily round integration

The movement work (M4) already gives NPCs homes, workplaces, market days and
curfew. Typed houses slot straight in as **third places**: after Lamplight,
an NPC's round can insert "alehouse (current one in my ward)" or "bath house
(my sex's day)" before home. Opening states run off the same clock. The city
then has a legible pulse: shutters down and smoke at Dayspring, cookshop
queues at High Wick, stakes and singing at Lamplight, the Watch and the last
lock-in after the Snuffing.

### Curfew and lock-ins

The Snuffing turns typed houses into gameplay: the alehouse that bars the
door and keeps pouring (a **lock-in**) with the Watch's brazier fifty yards
away is automatic drama. Light leaking through shutters after curfew is
information — for the player and for the Watch. Rumor output: who was locked
in where, and who climbed out a back window.

### Rumor venues for the smart actors

Tag each typed house for the sim as a **venue** with a gossip profile:
alehouse (fast, unreliable, cross-ward), bakehouse/wash-house (domestic,
accurate), bath house (cross-class), inn (outside news), stews (dangerous,
valuable), merchant house (private, plot-bearing). When NPCs share a venue,
the venue's profile shades what they trade. This is the cheapest way to make
the LLM conversations *place-flavored* — one line in the prompt: "You are in
the common room of the gate inn; carriers are talking about the road."

Venues also give the attention scheduler natural hotspots: a stage with six
NPCs in one room at Lamplight is exactly where idle cognition should be
spent, and exactly where the player will go to eavesdrop.

### Prices (anchor to canon coinage)

Post them diegetically on boards where literate, by crier where not:
mug of ale 1 spark; cookshop pie 1–2 sparks; bath 2 sparks (towel extra);
bed at the gate inn 3 sparks shared / 1 bell private; cup of Ostrelle wine
4–6 sparks; the stews — not posted. A journeyman's 3-bell day makes all of
this affordable-but-felt, which is the right texture.

---

## Suggested counts and siting (first pass)

| Type | Count | Where |
|---|---|---|
| Rotating alehouse pool | 8–12 candidates, 2–4 live | all wards |
| Fancy tavern | 1 | Bellstand / cloth quarter |
| Gate inn | 1 | main land gate |
| Pilgrim inn | 1 | near the Gradine |
| Stews | 1–2 | off Tanners' Slip, Reed Ward |
| Bath house | 1 | near a strong well |
| Merchant houses | 6–8 | the Cut, Tallage, cloth quarter, Maren's Green (Alder) |
| Cookshops | 2–3 | Tallage, Wickmarket, Coswald's Yard |
| Bakehouse/oven | 2 | residential wards |
| Wash-house | 1 | Reed Ward court |
| Watch house | 1 | Bellstand |
| Others (apothecary, scrivener, dice house, almshouse, smithy, guild hall) | 1 each | per notes above |

Total: roughly 30–40 typed houses out of hundreds — enough that every few
streets hold one, few enough that each stays a *place*.

## Prioritization sketch

1. **Ale-stake system** — biggest fun-per-effort: a handful of dressed houses,
   a rotation on the existing clock, singing audio at night, and instant
   rumor material. Everything else can follow its pattern.
2. **Signboard language** — small asset budget, city-wide readability gain.
3. **Inns + traveler news injection** — opens the rumor system to the world
   outside the walls.
4. **Fancy tavern + merchant houses** — where authored plots live.
5. **Bath house, stews, supporting cast** — texture and social-graph spice.

## Open questions

- Do typed houses get picked from existing generated plots (retrofit) or
  reserved during generation? (Retrofit is more in the spirit of "houses that
  intermittently become alehouses.")
- How much interior do we need? Proposal: alehouse front room only at first;
  most types can be door + threshold + sound until proven fun.
- Does the ale rotation seed from the lore families (brewsters get names from
  the family files) or generate fresh names from the name banks?
- Singing: looped wav choruses first, or hold for actual TTS ensemble?
- Should venue gossip profiles live in `cathedral-sim` (prompt-side) from day
  one, or start as pure set dressing and wire the sim in second?
