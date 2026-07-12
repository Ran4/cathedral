## File structure

```
lore
├── second_sun
│   │── ...
│   └── *.md
├── alternative_second_sun_created_by_codex_please_ignore/
│   └── second_sun
│       │── ...
│       └── *..md
├── AGENTS.md
└── CLAUDE.md
```

## Description

This folder includes lore for the game's world.

It could be markdown, html, wav files...

Lore files aren't just flavor, they're potential prompt fodder for the sidecar. A "what everyone in the city knows" doc, district reputations, or a rumor list can be injected straight into NPC context so actors gossip
consistently about the same events. That makes some lore types more valuable than others.

These are just some suggestions (be creative!):

Diegetic documents (things that exist in the world):
- Founding myth of the cathedral and the "impossible light" itself — what do citizens believe it is?
- A chronicle/annals: fires, plagues, sieges, miracles, the year the canal froze. Gives every NPC a shared timeline to reference.
- Saint hagiographies and a relics catalog; a guide to what each rose window panel depicts
- Guild charters, apprenticeship contracts, trade rivalries (mason vs. carpenter feuds write themselves in a city that's still arguably "under construction")
- Proclamations, edicts, curfew rules, wanted posters — things that could literally be posted on walls in the five squares
- Market price lists, merchant ledgers, bills of lading for canal traffic
- Letters, diaries, a confession someone shouldn't have written down
- Epitaphs, foundation stones, dedication plaques; street/bridge name etymologies
- Sermons, prayers, a heresy or schism brewing in one of the secondary churches
- Tavern menus, recipes, quack-medicine fliers, indulgence sales

Oral/folk culture (great as both text and wav):
- Bell peals and their meanings — curfew, alarm, mass, death knell. Doubles as an audio design doc.
- Street vendor cries, town crier announcements, boatmen's work songs, drinking songs
- Children's rhymes and skipping songs that encode old history nobody remembers the origin of
- Ghost stories pinned to specific alleys/bridges — good for making the procedural geometry feel authored
- Superstitions, omens, folk remedies, curses and blessings

Structural/meta lore (for you and the sidecar, not the player):
- Faction map with relationships and grudges
- Per-district "personality" briefs — what each square and quarter feels like, who lives there, what's sold there
- NPC seed bios: occupation, secret, allegiances, who they owe money to
- A rumor pool with truth values (some rumors false — NPCs spreading misinformation is very alive)
- Calendar: feast days, market days, processions — could eventually drive crowd behavior
- Naming conventions and a glossary, so generated content stays tonally consistent
- Coinage, weights, units of measure

Format-specific ideas: HTML works well for heraldry galleries and illuminated-manuscript-style pages;
wav for bells, chants, and street cries;
and maps (annotated pilgrim's guide vs. the "real" layout) could be images (so they can in the future
be read by the llm citizens themselves!).
