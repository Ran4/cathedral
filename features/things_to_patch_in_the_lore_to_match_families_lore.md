# Patches to reconcile existing lore with `lore/families/`

Created alongside the new `lore/families/` folder (an author-facing family index
stubbed from the 500-character roster in `lore/characters/` plus existing lore).
These are **proposals only** — nothing outside `lore/families/` has been touched.
Each item is a small edit to another doc that would make the world consistent
with the family stubs. Ordered roughly by value.

## 1. `core_lore/naming_language.md` — the surname bank is far too short

The doc's "established family names" list is:

> Sparr, Copp, Stott, Dorn, Alder, Ashe, Marle, Fitch, Pike, Brant, Vell, Rasp,
> Ferrant — approved: Crake, Hobbe, Skell, Tarn (17 total)

But the character roster actually uses **~42 recurring fixed surnames**. Names
carried by 5+ characters that the naming doc does not yet sanction:

> Rud, Quern, Sedge, Skep, Pell, Rusk, Wren, Dask, Clove, Thorn, Rook, Bram,
> Sark, Kern, Toll, Fenn, Mott, Lark, Mere, Kett, Nett, Rill, Husk, Wick, Dunn

**Suggested edit:** either fold these into the sanctioned bank, or (cleaner) add a
line to `naming_language.md` pointing at `lore/families/overview.md` as the
living registry of who actually carries which name, and keep the doc focused on
the *rule*. They already fit the stated aesthetic ("short and consonant-heavy").

## 2. Resolve the **Vell** double-status (family of standing vs. fallen byname)

Three docs currently pull the name in two directions:

- `naming_language.md` lists **Vell** among families *of standing*.
- `the_dry_boatmen.md` (line ~191) treats **Averil Vell** as *fallen*: "the Vells
  had a warehouse once… sells pilgrim badges in the shadow of somebody else's."
- `second_sun/05_dramatis_personae.md` makes **Osanne Vell** a Wickmarket chandler
  of clear standing (holds the Lanthorn candle contract; secretly the Tracer).

`family_vell.md` reconciles this as **one split house**: a risen Wick/Wickmarket
chandler branch and a fallen Reed-Ward branch. **Suggested edit:** add half a
sentence to `the_dry_boatmen.md` (and/or `naming_language.md`) confirming the
split is deliberate, so a reader doesn't take it for a contradiction.

## 3. Point `core_lore` at the faction canon in `second_sun/`

The family stubs lean on two shadow institutions — the **Custody of the Eye** and
the **Unwalled**. The Custody is summarised in `core_lore/candor_and_churches.md`,
but the **Unwalled** (Tracer / Namekeeper / Wicket, the lights-and-leads cell
structure, the pass) is only worked out inside the `second_sun/` tree
(`02_the_heretic_cell.md`, `05_dramatis_personae.md`). An author reading only
`core_lore/` gets half the picture.

**Suggested edit:** one pointer line in `core_lore/core_lore.md` (near the Custody
bullet) to `second_sun/02_the_heretic_cell.md` for the heretic-cell canon the
family docs assume. (Confirm first whether `second_sun/` is considered promotable
canon or a self-contained designed storyline — if the latter, the family docs may
need to hedge their faction claims instead.)

## 4. Surface the **Sparr ↔ Crake glazier feud** in place lore

Dunstan Crake's bio (`characters/glazier/c3wnk_dunstan_crake.json`) holds a live
hook not reflected anywhere in place/faction lore: he keeps *"a deed-claim on the
very plot the Sparr furnace stands on, older than the fire of F.171,"* unspent so
it stays unlost. Combined with the **Glazier Rule** and the venerated **Idonea
Sparr** (`second_sun/02_the_heretic_cell.md`), Cinder Row has a three-way glass
story (Sparrs / Crakes / the cell) that the gazetteer doesn't mention.

**Suggested edit:** add a Cinder Row note to `places/02_canonical_gazetteer.md`
(the furnaces are numbered — "the Row's second furnace" — and the F.171 fire and
the Sparr/Crake plot dispute belong there). See `family_sparr.md` / `family_crake.md`.

## 5. Confirm which Reed Ward names are boat-blood

`the_dry_boatmen.md` names the five boat-houses (Alder, Skell, Hobbe, Tarn, Crake)
and the fallen bynames (Hawser, Underbridge, Vell). But the roster puts other
heavy Reed Ward names in the same streets — **Fitch** (Noll Fitch, sexton of Saint
Maren), **Rasp**, **Rud**, **Pell**, **Sark**. The family stubs treat these as
Reed-*resident* rather than boat-*blood*. **Suggested edit (optional):** a line in
`the_dry_boatmen.md` distinguishing the boat-families proper from the other
households of the ward, so the distinction is explicit rather than implied.

## 6. Note the given-name collisions (documentation, not a contradiction)

The given-name bank is small and reused hard across families, producing many
same-first-name collisions: there is a **Gile** Skell, Hobbe, Pike, Skep, Rud,
Tarn *and* Crake; multiple Betriss, Osanne, Idonea, Corin, Renna. This is
in-register (medieval naming was shallow) and the stubs rely on trade/place to
disambiguate. **Suggested edit (optional):** one sentence in `naming_language.md`
acknowledging that given+surname can collide and that disambiguation is by trade,
place or feature — so it reads as intended, not as a generation bug.

---

### Also worth doing inside `lore/families/` later (no external patch needed)

- A **by-ward index** (families grouped by Reed / Fabric / Cinder / Weigh /
  Bell-and-Sluice / Wallwright / Cloth / Wick) — the current overview is by size.
- Promote the anchored houses (Alder, Vell, Rasp, Copp, Sparr, Marle, Ashe, Rud,
  Fitch) from stubs to full treatments on the `the_dry_boatmen.md` model.
- Add **heraldry / house-marks** for the families of standing (the AGENTS.md note
  suggests HTML heraldry galleries).
