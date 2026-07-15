# Patches to reconcile existing lore with `lore/families/`

Created alongside the new `lore/families/` folder (an author-facing family index
stubbed from the 500-character roster in `lore/characters/` plus existing lore).
These are **proposals only** — nothing outside `lore/families/` has been touched.
Each item is a small edit to another doc that would make the world consistent
with the family stubs. Ordered roughly by value.

## 1. Surface the **Sparr ↔ Crake glazier feud** in place lore

Dunstan Crake's bio (`characters/glazier/c3wnk_dunstan_crake.json`) holds a live
hook not reflected anywhere in place/faction lore: he keeps *"a deed-claim on the
very plot the Sparr furnace stands on, older than the fire of F.171,"* unspent so
it stays unlost. Combined with the **Glazier Rule** and the venerated **Idonea
Sparr** (`second_sun/02_the_heretic_cell.md`), Cinder Row has a three-way glass
story (Sparrs / Crakes / the cell) that the gazetteer doesn't mention.

**Suggested edit:** add a Cinder Row note to `places/02_canonical_gazetteer.md`
(the furnaces are numbered — "the Row's second furnace" — and the F.171 fire and
the Sparr/Crake plot dispute belong there). See `family_sparr.md` / `family_crake.md`.

## 2. Confirm which Reed Ward names are boat-blood

`the_dry_boatmen.md` names the five boat-houses (Alder, Skell, Hobbe, Tarn, Crake)
and the fallen bynames (Hawser, Underbridge, Vell). But the roster puts other
heavy Reed Ward names in the same streets — **Fitch** (Noll Fitch, sexton of Saint
Maren), **Rasp**, **Rud**, **Pell**, **Sark**. The family stubs treat these as
Reed-*resident* rather than boat-*blood*. **Suggested edit (optional):** a line in
`the_dry_boatmen.md` distinguishing the boat-families proper from the other
households of the ward, so the distinction is explicit rather than implied.
