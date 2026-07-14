See `lore/characters/AGENTS.md`. The original 103-character cast introduced by
this feature is preserved inside the later 500-character authored population.

They are all loaded into the world by both the Bevy game and headless runner. Their full lore profiles remain
structured in `cathedral-sim`, while the extended descriptions are retained for future introspection rather
than sent on every LLM turn.

The Bevy engine loads initial NPC state directly from `lore/characters/**/*.json` (the files remain outside
`assets/`). Character transforms are curated against `assets/world/areas.json` and the authoritative map in
`lore/places/`.
