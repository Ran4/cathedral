Status: Rewritten quest GDD delivered (2026-09-04); implementation plan delivered (2026-09-05). Game implementation pending.

# An Alibi in Stone

A **35-page GDD focused on one complete investigation quest**: reconstruct a theft and assault by
checking witnesses' movements, finding an upper route between apparently separate places, and
distinguishing opportunity from evidence that identifies the offender.

The document specifies the fixed crime, seven existing cast members, the incident chronology,
sixteen evidence entries, each investigative location, suspect responses, the packet's subsequent
movement, public/private resolutions, alternate playthroughs, partial outcomes and recovery cases.
Nine original diagrams explain the chronology, topology, timing, sightlines, evidence and state.

**Author spoilers begin on page 4.** Pages 3–30 focus on the quest and how it plays; the final pages
cover foundations, milestones, tests and sources.

The [implementation plan](implementation/plan/README.md) records the 2026-09-05 decisions and a
systems-first M0–M19 delivery. Its [decision record](implementation/plan/DECISIONS.md) and
[case contract](implementation/plan/CASE_CONTRACT.md) supersede conflicting GDD recommendations,
including pausing, save/load scope, dynamic enforcement and the repaired evidence/timing routes.
The original DOCX/PDF and model remain the earlier proposal.

| File | Purpose |
|---|---|
| [an_alibi_in_stone_gdd.pdf](an_alibi_in_stone_gdd.pdf) | Reading copy |
| [an_alibi_in_stone_gdd.docx](an_alibi_in_stone_gdd.docx) | Editable Word document |
| [manuscript.md](manuscript.md) | Complete source text, with explicit page boundaries |
| [generate_gdd.py](generate_gdd.py) | PEP 723 uv script: python-docx, PDF conversion and layout validation |
| [quest_figures.py](quest_figures.py) | Original code-generated diagrams |
| [quest_model.json](quest_model.json) | Inspectable proposed route distances, timing, itinerary and proof recipes |
| [validate_quest.py](validate_quest.py) | Checks design arithmetic and independent evidence scenarios |
| [quest_validation.json](quest_validation.json) | Results of those design checks |
| [source_manifest.json](source_manifest.json) | Source hashes, casting references and image provenance |
| [validation.json](validation.json) | PDF pagination, expected headings and text-bound checks |
| [figures/](figures/) | Nine diagrams in PNG/SVG plus the unchanged architectural reference |
| [previews/](previews/) | 35 rendered PDF pages and three contact sheets |
| [implementation/plan/](implementation/plan/README.md) | Milestones, shared protocols, 40 requirements, 78 planned scenarios and review evidence |

Regenerate from the repository root:

```sh
uv run --cache-dir /tmp/cathedral-gdd-uv \
  features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/generate_gdd.py --open
```

The script runs the design checks, generates the figures and DOCX with `python-docx`, then invokes
`soffice --headless --convert-to pdf --outdir <folder> <docx>` using an isolated LibreOffice profile.
Omit `--open` to regenerate without opening the PDF. Python dependencies are pinned inline for uv;
the host needs `soffice` and, for opening, `xdg-open`. Direct edits to the DOCX are overwritten by
regeneration; edit the manuscript instead.

Run only the written quest-model checks:

```sh
uv run --cache-dir /tmp/cathedral-gdd-uv \
  features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/validate_quest.py
```

The original model checks limited **design arithmetic and recipes**, not implemented gameplay.
The implementation review found gaps those checks did not cover; its
[design probe](implementation/plan/evidence/design_probe.json) reproduces them and the case contract
states the repairs. In particular, the proposed
576 m public return route and 48 m private circuit have not been measured in the game. The first
production milestone must prove that topology against real movement and collision. The chronology,
cast's knowledge boundaries and evidence rules are specified, but the crime and new rooms are not
existing canon. The shared knowledge/rumour feature remains a prerequisite for quest implementation.

No game run, human playtest, game-code change, lore edit, branch change or backlog reorder was
performed for the original GDD. The later planning pass's source checks and unsuccessful hidden-window
survey are recorded separately in its [review log](implementation/plan/REVIEW_LOG.md).
The market screenshot and general gathering material from the earlier
GDD are superseded by the investigation design.
