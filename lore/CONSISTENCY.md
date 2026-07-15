# Protected inconsistencies — do NOT "fix"

Some apparent contradictions in `lore/` are **authored**: deliberate
misinformation, in-character unreliability, or mysteries that e.g.
`second_sun/00_canon.md` binds as never-resolved.
Collapsing them destroys the design.

This file is the canonical list (update it as you see fit!).
Each protected spot also carries an inline
`<!-- protected: … -->` comment pointing back here, so the guard travels with the
line. Find every guarded spot with:

```sh
grep -rn "protected:" lore/
```

**Rule for any consistency / copy-edit pass (human or LLM):** never edit a line
that sits under a `protected:` comment, and never reconcile a pair listed below.
`00_canon.md` is the source of truth ("Where a later document disagrees with this
one, it is wrong"); its truth-value ledger (§10) and designed-ambiguity clause
(§3f) adjudicate. See the fuller write-up in
`features/2026-07-15_fix_lore_discrepancies.md` §B.

## The protected pairs

1. **Great frost — "six weeks" (F.229) vs "nine weeks" (F.334).**
   `second_sun/04_chronicle_of_the_city.md`. The F.334 entry's later-hand gloss
   *("the entry of that year says six. Frost grows in the telling.")* is the
   chronicle-exaggeration device, explicitly cross-referencing the two figures.
   Both stay.

2. **River diversion — "seventy years" vs "sixty-eight years".**
   `the_dry_boatmen.md`. Ewart, in drink, rounds to "seventy years ago"; the
   narrator gives the precise "sixty-eight years" (F.437 − F.369). His line is
   bracketed as unreliable ("By morning he is ashamed"). Both stay.

3. **Night-storm open-sky sighting vs "never at night / only through the rose".**
   `second_sun/00_canon.md` rule 9 (bracketed, `AMBIGUOUS-BY-DESIGN`, ledgered
   §3f) vs rules 7 & 10 (hard render-enforced rules). The rules are fact; the
   sighting is an unresolved rumor. Both stay.

4. **Light "speaks no words" vs folk belief the dead hear names in its beam.**
   `core_lore/impossible_light.md:22` (observable: inert) vs `00_canon.md` §(e)
   (folk belief). Different registers; §3f binds "whether it hears anything spoken
   into its light" as never-confirm/never-deny. Both stay.

5. **Unwalled "old glass remembers the sky" vs the new-plain-glass test.**
   `second_sun/documents/heretic_catechism.md:58-59` (heretic doctrine) vs
   `00_canon.md` rule 2 + `core_lore/impossible_light.md:15-17` (new plain glass in
   the rose shows the disc → glass-memory is false). Canon heads the doctrine
   "Wrong on purpose." In-world misinformation; stays.

6. **"The Greensick cut throats" vs the Unwalled's bloodless door-rite.**
   Renna sincerely repeats a false rumor
   (`second_sun/05_dramatis_personae.md:339` `[FALSE-BUT-BELIEVED]`,
   `second_sun/design/03_questlines.md:55`) vs the cell's actual practice
   (`heretic_catechism.md:96-97`, "No knife has ever answered our door"). Ledger
   §10 row 27 = **F**. The gap is the authored payoff; stays.
