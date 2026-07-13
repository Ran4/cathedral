See lore/characters/AGENTS.md - in lore/characters we have 103 defined characters.

They are all loaded into the world by both the Bevy game and headless runner. Their full lore profiles remain
structured in `cathedral-sim`, while the extended descriptions are retained for future introspection rather
than sent on every LLM turn.

The Bevy engine loads initial NPC state directly from `lore/characters/**/*.json` (the files remain outside
`assets/`). Character transforms are curated against `assets/world/areas.json` and the authoritative map in
`lore/places/`.
