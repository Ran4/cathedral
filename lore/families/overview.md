# The Families of Ombreval

> Author-facing index of the city's family names: who carries them, where they
> cluster, what trades they hold, and what the name *means* socially. This is a
> **rough first pass** — a few sentences per family, stubbed from the 500-strong
> character roster in `lore/characters/` and the existing lore. Not all of it is
> hard canon yet; treat the individual files as seeds to expand, not law.
>
> Complements, and should stay consistent with:
> - `core_lore/naming_language.md` — the naming rule these families obey.
> - `the_dry_boatmen.md` — the Reed Ward boat-families worked out in full depth
>   (the model this folder imitates).
> - `second_sun/05_dramatis_personae.md` — the same people organised by person
>   rather than by lineage.
> - `second_sun/02_the_heretic_cell.md` — the Unwalled, who cut across families.

## The rule that makes a family

Ombreval's naming law (`naming_language.md`) draws a hard social line:

- **Families of standing hold a fixed surname** — Sparr, Alder, Copp — passed
  down and kept only as long as the house owns something worth naming.
- **The poor carry bynames** of trade, place, feature or event (Cobb Hawser, Ede
  of the Needle, Averil Wetalms). These are *earned or inherited, not
  self-selected*.
- **Women keep their own bynames at marriage.** A wife is a Skell in a house of
  Alders, and runs the ledger under her own name.
- **A family that loses its property loses its name.** *"There are more Hawsers
  than Alders."* The fallen keep a given name and whatever the street calls them.

So this is a register of the *named* — the houses solid enough to hold a surname
across three generations. The nameless poor are the water this list floats on.

## How to read the roster

A surname's spread is itself a story. A name found in one ward, in one trade, is
a tight house guarding a workshop; a name scattered across a dozen trades and
every quarter is a house that *broke and travelled* (see the Crakes). Where a
family runs to petty crime in the records, that is the ward's economy showing
through the name, not a moral verdict — the poor of Wick and Bell-and-Sluice
pilfer, clip and water ale because that is what poverty does there.

**Bell-and-Sluice is the mixing bowl.** Nearly every family has a branch on the
Bell-and-Sluice streets, the crowded central ward around the Bellstand. A family
with *only* a Bell-and-Sluice presence is usually poor and un-rooted; a family
with a Bell-and-Sluice branch *and* a heartland elsewhere has sent its overflow
into the middle of the city.

## The two shadows that cut across every family

Two institutions run *through* the families rather than beside them, and several
houses sit on opposite sides of the same quiet war:

- **The Custody of the Eye** — the Church's grey-coated licensing and
  surveillance office, run from the **Grey Press**. It watches the Emblem, stamps
  the pilgrim trade, and keeps lists of faces. Its master is **Segwin Rasp**
  (Provost-Custodian); its clerk **Sible Mott** fair-copies the funeral
  watch-lists; its "paid moths" (informers) include a Hobbe, a Pike and, without
  knowing it, the crier **Jos Brant**.
- **The Unwalled** — the heretic cell of about nineteen (`02_the_heretic_cell.md`)
  who believe a green sun was walled out of the sky. Their **Tracer** is the
  chandler **Osanne Vell**; their **Namekeeper**, the seamstress **Betriss
  Marle**; their **Wicket** (doorkeeper), the salt-merchant **Grigor Ashe**;
  their angry new intake, fullers like **Tam Rud**.

The whole conflict is personified by two families: the **Rasps**, who supply the
man who watches, and the **Vells**, who supply the woman who is watched — and
neither family, top to bottom, mostly knows it.

## Crests

Some — not all — of these families bear **arms**. Heraldry follows standing: a
house needs property, an office, or a workshop worth naming, so the houses of
standing get crests and the nameless poor do not. The five boat-families share a
**wavy base** (the diverted Serle) but keep their own charge; two jumped-up
ambient houses (Skep, Kern) bear crooked *assumed* arms nobody granted them. The
crests are drawn procedurally by
[`../../scripts/generate_family_crests.py`](../../scripts/generate_family_crests.py);
see [`crests/`](crests/) — [`crests/showcase.html`](crests/showcase.html) previews
them all and [`crests/README.md`](crests/README.md) lists who bears which and why.
Where a family has a crest it appears at the top of its file, linked from the
roster below.

## Roster

Standing: **S** = family of standing (fixed surname, canonical or well-rooted).
**A** = ambient house (attested across the roster, mostly minor/ambient folk).
*n* = characters carrying the name in the current roster.

| Family | n | Heartland | Trades | Standing | In a phrase |
|---|---|---|---|---|---|
| [Skell](family_skell.md) | 26 | Reed Ward | fish, eel, wharf, burial, drapery | S | boat-family that *let the boat go* — and prospered |
| [Hobbe](family_hobbe.md) | 23 | Fabric / Bell-and-Sluice | carting, milling, bells, clergy | S | boat-family that *went to the cart* |
| [Crake](family_crake.md) | 20 | scattered (Wallwright, Fabric) | every trade | S | boat-family that *went downriver*; the Long Departure |
| [Rud](family_rud.md) | 17 | Bell-and-Sluice / Wick | fulling, cloth, soldiering | A | the angry cloth-poor; raw timber of the Unwalled |
| [Quern](family_quern.md) | 17 | Wick / Bell-and-Sluice | baking, service, small crime | A | the light-fingered milling-poor |
| [Rasp](family_rasp.md) | 17 | Bell-and-Sluice / Wick | cooperage, scavenging — and the Grey Press | S | one grey eminence over a mass of poor |
| [Fitch](family_fitch.md) | 16 | Reed / Fabric | burial, parish, leather | S | the sexton's house; the register and the grave |
| [Sedge](family_sedge.md) | 16 | Weigh / Cloth | counter, grocer, tavern | A | the weighing-quarter's small-fraud family |
| [Skep](family_skep.md) | 15 | Bell-and-Sluice / Weigh | service, cooperage, muscle | A | a rougher house; protection and threat |
| [Vell](family_vell.md) | 15 | Wick (Wickmarket) | chandlery, wax, light | S | risen chandlers with a heretic at the head |
| [Stott](family_stott.md) | 15 | Wallwright | masonry, the lodge | S | the masons' warden and his plumb-line grudges |
| [Ashe](family_ashe.md) | 15 | Weigh / Reed | salt | S | the salt house whose cellar hides the grate |
| [Pike](family_pike.md) | 14 | Fabric (the Lanthorn) | vergers, keys, cathedral service | S | the keys of the cathedral and small door-graft |
| [Pell](family_pell.md) | 14 | Cloth / Reed | smallholding, victualling | A | a three-ward family of small gates and poaching |
| [Rusk](family_rusk.md) | 13 | Bell-and-Sluice / Reed | tavern, baking, hustle | A | the victualler-and-hustle house |
| [Brant](family_brant.md) | 13 | Bell-and-Sluice | crying, healing, trades | S | the city hears a Brant before its own bells |
| [Marle](family_marle.md) | 13 | Bell-and-Sluice / Fabric | cloth, masonry — and the bede-roll | S | a cloth-and-cathedral house with a heretic heart |
| [Tarn](family_tarn.md) | 12 | Reed (Tanners' Slip) | cooperage, tanning | S | boat-family that *went to barrel and hide* |
| [Wren](family_wren.md) | 12 | Weigh / Cloth | cooperage, instruments, fish | A | small makers of the weighing streets |
| [Dask](family_dask.md) | 12 | Bell-and-Sluice / Weigh | droving, boats, cobbling | A | a plain, hardworking, unremarkable house |
| [Clove](family_clove.md) | 12 | Weigh / Bell-and-Sluice | droving, brewing, muscle | A | sweet name, hard hands; drovers and debt-men |
| [Thorn](family_thorn.md) | 11 | Cloth / Weigh | leather, salt, portering | A | tanners and porters of the weigh-streets |
| [Rook](family_rook.md) | 11 | Bell-and-Sluice / Weigh | service, bell-ringing | A | a servant-and-belfry house round the towers |
| [Bram](family_bram.md) | 10 | Wick / Bell-and-Sluice | soldiering, milling, food | A | a soldiering family with a cook's sideline |
| [Sark](family_sark.md) | 9 | Reed / Cloth | washhouse, kitchen | A | washtub and cook-pot (a *sark* is a shirt) |
| [Kern](family_kern.md) | 8 | Cloth / Weigh | clerks, scholars, drapers | A | the most literate — and shabbiest — of the ambient houses |
| [Toll](family_toll.md) | 8 | Wallwright / Bell-and-Sluice | masonry, pottery, muscle | A | ironic name, hard edge, wall-quarter house |
| [Fenn](family_fenn.md) | 8 | Bell-and-Sluice | soldiering, chandlery | A | a marsh-name house, half in the watch |
| [Mott](family_mott.md) | 7 | Bell-and-Sluice / Fabric | butchery, bellfounding — and the Grey Press | S | a mixed house with a foot in the Custody |
| [Lark](family_lark.md) | 7 | Weigh / Bell-and-Sluice | carting, victualling | A | a carting-and-cook-pot house |
| [Mere](family_mere.md) | 6 | Cloth / Bell-and-Sluice | cooperage, victualling | A | victuallers with a false-manifest sideline |
| [Kett](family_kett.md) | 6 | Wick / Bell-and-Sluice | market food, water | A | a market-food house with a light finger |
| [Nett](family_nett.md) | 6 | Cloth / Weigh | garment, timber | A | a quiet cloth-and-carpentry house |
| [Rill](family_rill.md) | 6 | Bell-and-Sluice | rope, beasts, entertaining | A | a little water-name house |
| [Husk](family_husk.md) | 6 | Weigh | droving, carting, fish | A | drovers of the Tallage edge |
| [Wick](family_wick.md) | 6 | thin-spread | service, cobbling, labour | A | a nondescript house that shares the ward's name |
| [Dunn](family_dunn.md) | 5 | Weigh / Bell-and-Sluice | rope, tavern, beasts | A | a small humble house |
| [Alder](family_alder.md) | 3 | Reed (Alder Moorings) | the last boat of standing | S | boat-family that *kept the boat* — and the soundings |
| [Sparr](family_sparr.md) | 2 | Cinder Row | master glaziers | S | Idonea's line; the sealed deposition; watched glass |
| [Copp](family_copp.md) | 2 | Weigh (the Tallage) | forgery, pawn, paper | S | a tiny, dangerous house of two prices |
| [Dorn](family_dorn.md) | 1 | Fabric (the Lanthorn) | the Praelucent's office | S | one name, one throne of the Church |
| [Ferrant](family_ferrant.md) | 1 | Weigh (off the Tallage) | physic, astronomy | S | a learned singular name under the Rose |

## The families by ward

The roster above is ordered by size; this is the same houses ordered by *place*.
Each family is listed **Rooted** under the one ward that holds its weight, and as
a **Branch** wherever else it keeps a real presence — because *spread is the
story*, and few names sit in a single ward. This section is only a reordering of
the **Heartland** column above, which stays the single source of truth; a house
that is all branch and no root (the scattered Crakes, the thin-spread Wicks) is
one that *broke and travelled*, and that is the point, not an omission.

### Reed Ward — the water quarter
*Maren's Green, Tanners' Slip, Alder Moorings; the old boat-streets.*

- **Rooted:** Skell · Fitch · Tarn · Sark · Alder
- **Branches:** Ashe (salt cellar, from Weigh) · Pell (smallholding, from Cloth) ·
  Rusk (tavern-streets, from Bell-and-Sluice)

The boat-blood ward. Alder is the last boat *of standing*; Skell and Tarn are the
houses that gave the water up, and the fallen Hawsers and Underbridges the ones it
took the name from. Worked out in full in `../the_dry_boatmen.md`.

### Fabric — the cathedral quarter
*The Lanthorn and the streets under the works.*

- **Rooted:** Hobbe · Pike · Dorn
- **Branches:** Fitch (parish burial, from Reed) · Marle (the bede-roll, from
  Bell-and-Sluice) · Mott (bell-metal, from Bell-and-Sluice) · Crake (a thin
  strand of the scattered name)

The keys and the office: Pike holds the cathedral's doors, Dorn holds the
Praelucent's throne, Hobbe rings and serves. The Lanthorn is where the
church-service names cluster.

### Cinder Row — the glass quarter
*The furnace-streets; the numbered furnaces along the Row.*

- **Rooted:** Sparr
- **Branches:** Crake (Dunstan's furnace, and his deed-claim on the Sparr plot) ·
  Rud (the fulling stocks at the Cinder end of the Cut)

The smallest, hottest ward: two glazier houses over one plot. The Sparr ↔ Crake
feud and the venerated Idonea Sparr sit here — the three-way glass story
(Sparrs / Crakes / the Unwalled) that the gazetteer still doesn't name.

### Weigh — the weighing quarter
*The Tallage and the counting-streets.*

- **Rooted:** Sedge · Ashe · Wren · Clove · Lark · Dunn · Husk · Copp · Ferrant
- **Branches:** Skell (drapery, from Reed) · Skep (muscle) · Dask (droving) ·
  Thorn (portering) · Rook (service) · Kern (clerks) · Nett (timber) — most of
  them rooted across the Cloth or Bell-and-Sluice line

The small-fraud ward: false weights, watered ale, two prices (Sedge is its family
proper). Copp and Ferrant sit *off the Tallage* — the forger and the physician,
the quarter's two singular dangers.

### Bell-and-Sluice — the mixing bowl
*The crowded central ward around the Bellstand.*

- **Rooted:** Rud · Rasp · Skep · Rusk · Brant · Marle · Dask · Rook · Fenn ·
  Mott · Rill
- **Branches:** *nearly every house in the city.* Only rooted names are listed —
  a Bell-and-Sluice branch is the rule, not a distinction. A family with *only* a
  presence here is poor and un-rooted; one with a branch here **and** a heartland
  elsewhere has pushed its overflow into the middle of the city.

Both shadows run through this ward: the Grey Press (Rasp, Mott) and the Unwalled
(Marle, and the raw Ruds).

### Wallwright — the wall quarter
*The ramparts, Coswald's Yard, and the masons' lodge.*

- **Rooted:** Stott · Toll · Crake (thickest here)
- **Branches:** Skell (potters and painters, from Reed)

Stott is the masons' warden and his plumb-line grudges; Toll works stone and clay
and runs a shakedown along the wall. Crake is *thickest* here yet rooted nowhere —
the residue of the Long Departure.

### Cloth — the cloth streets
*Leather, garment, tannery.*

- **Rooted:** Pell · Thorn · Kern · Mere · Nett
- **Branches:** Sedge (grocer, from Weigh) · Wren (makers, from Weigh) ·
  Sark (washhouse, from Reed) · Rusk (a food branch, from Bell-and-Sluice)

Tanners, porters and small makers. Kern is the most literate — and shabbiest —
house on these streets.

### Wick — the light quarter
*Wickmarket; wax, tallow, and the milling-poor.*

- **Rooted:** Vell · Quern · Bram · Kett
- **Branches:** Rud (fulling) · Rasp (scavenging) — the Bell-and-Sluice poor spill
  west into Wick

The Vells rose on wax and light and put a heretic (Osanne, the Unwalled's Tracer)
at their head. The **Wick** family itself is thin-spread and merely shares the
ward's name — a house, not a landmark; do not confuse it with the ward.

## Not on this list

- **The bynamed poor** — Hawser, Underbridge, Threefinger, Halfbell, Bluehand,
  Tapster, and the *of-the-place* names (of the Needle, of the Sluice, of
  Ostrelle). These are people, not houses; some are fallen families (a Hawser was
  a boat-family once). See `the_dry_boatmen.md` on how a name is lost.
- **Church-office names** that are titles more than lineages (the Praelucent, the
  anchoress Aldith).
- Any family still to be invented — the roster is not closed.
