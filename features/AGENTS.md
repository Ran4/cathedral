This folder (`features/`) contain markdown files (and sometimes associated files) describing features
to implement.

Sometimes it's just a few words, sometimes it's entire detailed specifications.

Note: `features/implemented/` and the milestone records keep their original
pre-shrink coordinates — the city plan was shrunk 0.7× in 2026-07 (multiply
positions by ~0.7; building sizes and street widths were not scaled). Do not
edit those historical records.

## Working the backlog

- For larger features (with milestones), use `ESPFEIT` ("en subagent per feature, en i taget") = solve listed items with
  one subagent per feature, **sequentially**, so each agent gets an empty context.
- There is a file features/order.json which looks like

  ```json
  {
      "order": [
          {"path": "features/rats.md"}
      ],
      "finished": [
          {"path": "features/implemented/performance_improvements/", "when": "2026-07-26T0841"},
          {"path": "features/implemented/offer_lapse.md", "when": "2026-07-25T2326"},
      ]
  }
  ```
  which is a suggested order to complete features in, but the user chooses what features to implement.
- When features are fully implemented, `git mv` the feature file/folder to the features/implemented/ folder
  and in the order.json move the feature from order to the finished section (if a feature doesn't exit in
  the order field values, just write it to the finished section).
- Feel free to update a feature if you've implemented it differently.
- For partially updated features, update the features to clearly note what's been implemented.
  section.
- Before implementing from a spec, check the status line and
  `features/implemented/` first — the milestone you're about to build may
  already exist under a different name.

## Granularity — all sizes are legitimate

- **A few words / one paragraph**: an idea worth remembering. Mark unripe ones
  `NEEDS TO BE FLESHED OUT FIRST` on the first line (see `scents.md`).
- **A pitch**: a titled page with the fantasy and the mechanism sketched
  (`false_peals__ring_the_bells_manually.md`).
- **A full spec**: motivation (often a real playtest session with log paths),
  detailed design, and a milestone plan (`law_and_order.md`).
- **A folder**: when one file can't hold it — numbered chapters plus a
  `README.md` with a status line and file table, see feature folders section below.

## Feature folders

### Example folder structure:

```
features/
food_and_items/
      README.md
      M0_items_and_stacks.md
      M1_the_spark_standard.md
      M3_hunger.md
      M4_the_bread_round.md 
      M5_the_llm_seam.md
```

### Example README.md structure:

Multi-file features carry a `Status:` line at the top of `README.md`
(e.g. `Status: M0–M5 implemented (2026-07-20)`), including anything still
pending (a visual check, a handoff). Update it whenever milestone state
changes. Use absolute dates, never "yesterday".


```markdown
Status: M0–M4 implemented (2026-07-20)

# Milestones

* M0_items_and_stacks.md
* M1_the_spark_standard.md
* M3_hunger.md
* M4_the_bread_round.md 
* M5_the_llm_seam.md

Six milestones, each shippable and verifiable with the repo's existing tools
(`cathedral-headless`, `CATHEDRAL_DRIVE`, the golden fixtures). The
dependency shape: **M0 is the foundation and the risk** (every layer touches items, ~13 golden
fixtures pin the bytes); **M1 is pure content and fully parallel**; M2→M3→M4 stack on M0, and M5
replaces the deliberate M3/M4 supply and wallet cheats.

---

## M0 — Kinds & stacks

...

## M1 — The spark standard

...

## Risk ledger (what to watch while building)
```

## Milestones and status

- Big features (in a single file or feature folder) are cut into milestones `M0, M1, …`
  (sub-cut as `M5a–M5d` when a milestone splits). Milestones are the unit of implementation and review.
- Single-file specs note `Status: ` inline near the top the same way.
