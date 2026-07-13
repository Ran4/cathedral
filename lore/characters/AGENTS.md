Characters in the game, with their character sheet which includes
their id, name, job, backstories and more.

Note: see `lore/core_lore/occupations.json`
and `lore/core_lore/occupation_descriptions.md`

## Folder structure

### Example:

```
lore/
    characters/
        candor_cleric/
            4bk3d_aubin_alder.json
            9cqbn_hamel_salter.json
            1nmpq_aldith_.json
        glazier/
            kcdg5_havise_wetalms.json
```

### Format:

```
lore/
    characters/
        {occupation}/
            {character_id}_{name_slug}.json
```

## Character spec

### Example character spec

```json
{
    "id": "kcdg5",
    "name": "Havise Wetalms",
    "age": 43,
    "gender": "f",
    "occupation_id": "glazier",
    "title": "Journeyman glazier",
    "rank": "journeyman",
    "faction_role": null,
    "illegal_activity": null,
    "district": "Cinder Row",
    "knows": ["4bk3d"],
    "father": "4bk3d",
    "mother": null,
    "children": [],
    "spawn_location": {"x": 43.19, "y": 0.91, "z": 0.5, "facing": 0.3},
    "conditions": ["crippled"],
    "memories": [],
    "core_character_description": "...",
    "extended_character_description": "..."
}
```

Notes:

* It's possible to have a father, mother and/or children that you don't know.
* `core_character_description` is a few sentences at most (pure text), it's fed to the character's llm every
  iteration, and should include stuff like how they act. It is written in the **second person**, matching
  `back_story` in `assets/world/seed.json` ("Born poor, you are now a blacksmith apprentice...").
  `extended_character_description` can be a LOT longer is a markdown text with subheadings etc;
  it will be available to the llm for introspection, but they don't go around thinking about it all the time.
* conditions isn't specified yet, but like... crippled, blind, alcoholic, fat etc.
Most don't really have any conditions.
* `occupation_id` joins a character to an occupation family in `lore/core_lore/occupations.json` by exact
  match on that file's `occupation_id`, and is also the folder name. The human-readable name of the family
  lives there as `occupation_display` ("Bell-ringer"), so nothing has to slugify or un-slugify to get from a
  character sheet to its trade. `title` is the specific `alternative_titles` entry for that person ("Master
  glazier", "Wick-priest", "Tapster") — the family stays a clean key, and the person keeps their own words.
* `rank`, `faction_role`, `illegal_activity` and `conditions` are the separate secondary fields that
  `lore/core_lore/occupation_descriptions.md` requires be layered onto an occupation rather than folded into
  it. `rank` is a guild rank (master / mistress / journeyman / apprentice / novice / warden / contractor).
  All four are null/empty for most people.
* `spawn_location` is Bevy's Y-up world space, in metres: `x`/`z` are the ground plane and `y` is the height,
  as in `position_m` in `assets/world/seed.json`. **The current values are placeholders** — the canonical
  in-game positions of the squares, streets and workshops have not been fixed yet, so the existing cast is
  scattered at random over the city footprint at a standing height of `y: 0.91`. `district` records where a
  person actually belongs, and is what a future pass should use to place them for real.
