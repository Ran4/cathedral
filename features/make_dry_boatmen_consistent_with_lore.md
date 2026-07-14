# Patch the rest of the lore for `the_dry_boatmen.md`

`lore/the_dry_boatmen.md` was written standalone and deliberately not merged into
the other files. It introduces material that other documents should now know
about, and it leans on a few facts that are currently *implied* by canon rather
than *stated*. This is the reconciliation list.

Nothing below is a contradiction with existing canon as far as I could tell —
these are additions, promotions of implied facts, and one or two places where an
existing dangling hook now has an obvious answer.

## New coinages that other files should learn

These are the reusable pieces. If they only live in one document they are wasted.

| Term | Meaning | Where it belongs |
|---|---|---|
| **the dry carry** | the doubled handling of every load since the diversion; the boat-families' name for the whole economic wound | `core_lore/trade_and_daily_life.md`, `core_lore/sayings_and_customs.md` |
| **to lift it twice** | to pay for something already paid for | `sayings_and_customs.md` |
| **come by barrow** | untolled; smuggled through the Reed Postern as handbarrow loads | `sayings_and_customs.md`, `secular_government.md` |
| **a dry drowning** | ruin by debt, quietly, on land | `sayings_and_customs.md` |
| **he poles the Cut** | works hard at something that goes nowhere | `sayings_and_customs.md` |
| **ask an Alder where the bed was** | said when a wall cracks or a scheme fails for an unseen old cause | `sayings_and_customs.md`, `wells_and_water.md` |
| **gate-caught** | shut outside at the Snuffing | `sayings_and_customs.md` |
| **born inside** | boat-family child who has never seen the water | `sayings_and_customs.md` |
| **there are more Hawsers than Alders** | most who had something have lost it | `sayings_and_customs.md`, `naming_language.md` |
| **the soundings** | the unwritten oral map of the old riverbed under the Cut | `wells_and_water.md`, `places/02_canonical_gazetteer.md` |
| **built on the bed** | a Cut house standing over the old channel, and therefore settling | `places/00_city_plan.md`, `wells_and_water.md` |

## Per-file patches

### `core_lore/sayings_and_customs.md`
Add the sayings above. The file already carries *“Boatmen insist that the water
knows one sun and state it with the same calm as a depth sounding”* — the new
material is the same register and should sit beside it. Also worth stating there:
**boat-families give directions by upstream/downstream on dry streets**, which is
the single fastest way to mark an NPC as Reed Ward.

### `core_lore/trade_and_daily_life.md`
The file already says the diversion *“redistributed rather than simply ended”*
prosperity, and that large carriers and toll interests profited. It does not name
the mechanism. Add **the dry carry** and the seven-step double lift as the concrete
form of that redistribution — it is the ward's core grievance and good prompt
fodder for any porter, carter, freight broker or gate clerk, not just boatmen.

### `core_lore/naming_language.md`
Promote an implied rule to a stated one: **a boat-family that loses its boat loses
its fixed family name and falls back to a byname of place or gear.** The existing
roster already demonstrates it perfectly (Cobb *Hawser* the rope-picker, Renn *of
the Slip*, Warin *Underbridge*), and the file's own rule — bynames are earned or
inherited, not self-selected — is what makes it bite. This is a general Ombreval
mechanism, not a Reed Ward one, and it should be stated in the naming file so it
can be used everywhere.

### `core_lore/calendar_and_history.md`
Three additions, all inside existing feasts:

- **Vhairestide** — the blessing of the first boats happens *outside the wall*, so
  add **the Walking Out**: the Reed Postern is opened wide for a crowd and the
  whole ward walks out to see the river, once a year. For many of them it is the
  only time.
- **Marenstide** — add **the Dry Race** (a retired hull on a runner-cradle, poled
  down the dry Cut with river poles; other wards enter; bets settled at the Hungry
  Ox) and the **roll of the drowned** dispute (see below).
- **Colm's Night** — note that the boat-families burn their candles *at the dry
  sluice grate* rather than at home, and that a Custody grey coat is generally on
  the sluice road that night. Rohese of the Sluice raking the stubs is already in
  the gazetteer; this just says who put them there.

### `core_lore/candor_and_churches.md`
The **roll of the drowned** at Saint Maren's: since the diversion almost nobody
drowns in Ombreval, so the roll grows only by boatmen lost outside the walls —
and after F.415 the parish began adding the ward's Hammering dead to it, because
they were denied name-knells and this was a bell that could still be rung. The
traditional objection is **“you cannot drown under slate.”** This is the
Unknelled argument fought on boat-family ground, and the wick-priest who has to
read the list, Renn Hobbe, is himself boat-family. Should sit next to the existing
*“one bell is not a name”* material.

Also: the **pole over the door** funeral custom (the dead man's pole laid across
the coffin up Maren's Slip, taken back at the churchyard gate, hung over the
house door). A doorway with three poles has buried three boatmen; boat-family with
*no* pole over the door has sold the boat, which is worse.

### `core_lore/secular_government.md`
Two things:

- **The barrow-toll gap.** Freight through the River Gate is tolled; handbarrows
  through the Reed Postern are tolled lightly or not at all. Therefore a boatload
  landed at dusk and walked up Maren's Slip as a dozen small trips at first light
  is untolled. This is the Reed Ward's standing petty crime and the real content of
  its postern politics. The Bench half-knows and votes for the postern anyway.
- **The wharf-born question.** Is a child born in a shed on the outer strip a Reed
  Ward resident? Does the household owe ward tax? May the father stand for bencher?
  Currently unanswerable, which is the point — but the file should note that ward
  residency is contested at the wall, because the boat-families are half-resident
  outside it.

### `features/lore_ward_politics.md`
That feature note already says *“Reed might fight to keep its postern open on
Lowmarket”* and mentions *“enforcement of the Cut game”* — **the Cut game is
defined nowhere in the lore corpus.** The Dry Race is the obvious candidate and I
would just claim the phrase: an inter-ward boat-on-a-cradle race down the dry Cut,
banned for obstruction every few years and never actually stopped. If the Cut game
was meant to be something else, say so and I will keep them separate.

The postern fight now also has a concrete, playable substance (the barrow-toll) and
a concrete Reed Ward platform (postern hours + the Serle tun).

### `wells_and_water.md`
The strongest cross-link in the whole document. That file already establishes made
ground, the filled Cut, and the ward grievance that *“the city made the Serle a
cart journey away, then treated the cost of carrying as a private inconvenience”* —
which the boatmen now repeat almost verbatim. Add:

- **The soundings** as a knowledge trade: well-diggers, cistern men, cellar-diggers
  and the Line-keeper quietly pay old boat-family members to say where the old
  channel ran, because a shaft or a foundation over the bed behaves differently
  from one on the bank.
- **Built on the bed** as the property consequence: Cut houses over the old channel
  settle; landlords deny the whole idea exists because it prices their houses.

Keep it strictly geological — no hidden channel, no secret water. The wells doc is
law on that and the boatmen doc defers to it explicitly.

### `places/02_canonical_gazetteer.md`
- **The Alder Moorings** — add that the yard's iron rings, forged for boats, are
  now used to tie handcarts and dogs, and rope for lanterns at the eel fair. Small,
  cheap, and it does the whole theme in one image.
- **The Old Sluice** — the gazetteer already has the true-arch secret and Rohese
  raking candle stubs after Colm's Night. It could note that the candles are put
  there chiefly by boat-families.
- **The Reed Postern** — currently listed for wharf hands, fish baskets, handcarts
  and funerals. Add its two ceremonial/economic roles: opened wide once a year for
  the Walking Out, and the barrow route that is not a customs house.
- **Maren's Slip** — the doc leans hard on an existing gazetteer fact: the final
  turn hides the river from inside the city. That means a boatman's last sight of
  the Serle is going out. Worth a line so nobody "fixes" the sightline later.

### `places/03_new_places_and_infrastructure.md`
The outer wharf strip needs **the outlodge**: sheds, half-lofts, unlicensed
drinking huts and beds outside the wall, used by gate-caught crews, which the Bench
cannot easily regulate *because it is outside*. This is where half the Reed Ward's
men sleep half the time and it explains the split-household shape of a boat-family.
The file already flags the strip as lore geography rather than playable space, so
this stays a horizon/through-gate fact.

### `lore/characters/` (no file edits required, but worth knowing)
The document reads several existing characters as a system and it would be good to
keep them coherent going forward:

- **Cobb Hawser** (63, rope-picker, Tanners' Slip) is treated as a *fallen boatman
  picking up rope for a living*, and as a man who still carries the soundings that
  nobody has thought to ask him for in twenty years. His character file does not
  say this. If that reading is wrong, this is the one thing in the document I would
  most want corrected.
- **Averil Vell** (badge attendant) is read as a fallen warehouse house.
- **Averil Skell** is the practical power of Maren's Green — smoke, fish, and
  lending at a spark on the bell.
- **Renn Hobbe** (wick-priest of Saint Maren's) is read as boat-family, which makes
  him the man who has to read the disputed roll of the drowned to his own kin.
- **Ewart Alder's** drunk threat to sell the Moorings now has a second and much
  worse version: selling *the soundings* to the masons' lodge.

## Open questions for you

1. **Is the Dry Race the "Cut game"?** I don't want it to be, let's figure out a new game together
2. **Cobb Hawser as a fallen boatman** — confirm or correct.
3. **The Reed Ward's two benchers** are deliberately left unnamed; the ward
   politics work should name them, and their platform is already written (postern
   hours, the tun).
4. **Family-name-loss** as a general Ombreval rule, or only a Reed Ward pattern? I
   wrote it as general and put it in the naming file's column, but it is the one
   claim with reach beyond this document.
