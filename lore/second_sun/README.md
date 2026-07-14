# The Second Sun

A lore & design corpus for **The Cathedral-City of Impossible Light** — 27 documents,
~85,000 words, set in the free fortified city of **Ombreval** in the year F.437.

> *"It moves against the sun; it stands in one pane for every eye; I have no theory.
> An impossible light, and the city at peace with it."*
> — Doctor Aubin Ferrant, in the margin of his notebook

## What this is

A second sun — cold, green-white, silent — is visible **only** through the glass of the
Great Rose, and **only** from inside the cathedral. Step outside and there is one sun, as
there has always been. It has done nothing, to anyone, for two hundred years. The dread is
that it does nothing — except be seen.

The corpus builds the city that has had to live with that: a Church that forbids the word
for it, a heresy that is wrong about it on purpose, a scholar whose theory is dead and who
keeps measuring anyway, and a street that speaks its dead's names into the light. It is
written to be **prompt fodder for the sidecar** as much as flavor — the truth-value ledger,
the rumor pool, the seed bios, and the "what everyone knows" doc are all shaped for direct
injection into NPC context (see `lore/AGENTS.md`).

Entry #1 of [`features/50_cool_suggestions.md`](../../features/50_cool_suggestions.md).

## The showcase

Open **`index.html`** in a browser — the reading room for the whole corpus: all 27
documents rendered in-page, full-text search, the Great Rose as an interactive window, the
Fabric-era timeline, and the fourteen people who each have something to hide. It is fully
self-contained (no network, no libraries) and works straight off disk via `file://`.

## 00_canon.md is law

**`00_canon.md` is the single source of truth.** Where any other document disagrees with it,
that document is wrong. Its three binding rules, in short:

1. **The phenomenon is a render-layer fact.** Purely visual, mediated by the Great Rose,
   verifiable by eye. No heat, no healing, no voices, no physical trace.
2. **The truth-value ledger (§10) is canonical.** Its 30 statements are 20 TRUE, 5
   FALSE-BUT-BELIEVED (someone sincerely spreads them), and 5 AMBIGUOUS-BY-DESIGN — and the
   ambiguous ones are *never* resolved, by any document, NPC, quest, or system.
3. **The rival-theory rule.** Add evidence for any theory only if it also fits at least one
   rival. Nothing may tip the designed ambiguity. The game never reveals what the Second Sun is.

## Reading order

Read the canon first; everything else assumes it.

| # | Start here | |
|---|---|---|
| 1 | [`00_canon.md`](00_canon.md) | The canon bible — names, the phenomenon, the layered truth, timeline, people, the ledger. **Law.** |
| 2 | [`07_what_everyone_knows.md`](07_what_everyone_knows.md) | The city's common knowledge — the shortest way to hear how Ombreval talks. |
| 3 | [`01_cosmology_and_doctrine.md`](01_cosmology_and_doctrine.md) | The four answers: church, heretics, scholars, street. |
| 4 | [`documents/the_sparr_deposition.md`](documents/the_sparr_deposition.md) | The oldest paper in the archive, as the leaves are sold. The corpus in one document. |

**The World** (01–12) — doctrine, the heretic cell, the Church and its Custody, the chronicle,
the dramatis personae, the rumor pool, common knowledge, folk culture, the rose window's
panel-by-panel iconography, the gazetteer, the glossary, and what lies beyond the walls.

**Documents (diegetic)** — things that physically exist in the world and can be found, bought,
stolen, or read aloud: the Edict, the heretics' catechism, the philosophers' letters, the
lesser legendary of the saints, a sermon, the Sparr deposition, the sealed trial record.

**Design Specs** — the feature as engineering: the vision, the light rules as testable
acceptance criteria, the whisper-at-the-grate voice design, the questlines, systems
integration with the existing sidecar, playtest and tuning, and the sound of the city.

## How it was made

Written by Claude, multi-agent, in four passes:

1. **Canon** — a judge panel of independent proposals for the phenomenon and the city,
   scored and synthesized into `00_canon.md`.
2. **Corpus** — 23 writers fanned out in parallel, each given the canon and one document.
3. **Audit** — 5 adversarial auditors hunted contradictions against canon; fixers applied
   the confirmed findings.
4. **Polish** — a completeness pass, then a cross-file consistency and tone pass.

Canon was frozen before step 2, which is why 27 documents by 23 authors agree on the color
of a light none of them could see.

## Rebuilding the showcase

`index.html` is generated. Edit the markdown (or `showcase_template.html`), then:

```sh
python3 build_showcase.py
```

The script (stdlib only) reads every `.md` file in this directory, `documents/`, and
`design/`, and bakes them into the template as JSON — the corpus is embedded rather than
fetched at runtime because `file://` pages cannot `fetch()` their own siblings. It reports
the document count, total words, ledger tallies, and output size, and fails loudly if a
placeholder is left unsubstituted or a document goes missing.
