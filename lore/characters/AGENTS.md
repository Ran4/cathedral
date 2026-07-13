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
    "id": "kcdg5"
    "name": "Havise Wetalms",
    "age": 43,
    "gender": "f",
    "occupation": "glazier",
    "knows": ["4bk3d"]
    "father": "4bk3d",
    "mother": null,
    "children": [],
    "spawn_location": {"x": 43.19, "y": 19.3, "z": 0.5, "facing": 0.3},
    "conditions": ["crippled"];
    "memories": [],
    "core_character_description": "...",
    "extended_character_description": "..."
}
```

Notes:

* It's possible to have a father, mother and/or children that you don't know.
* `core_character_description` is a few sentences at most (pure text), it's fed to the character's llm every
  iteration, and should include stuff like how they act.
  `extended_character_description` can be a LOT longer is a markdown text with subheadings etc;
  it will be available to the llm for introspection, but they don't go around thinking about it all the time.
* conditions isn't specified yet, but like... crippled, blind, alcoholic, fat etc.
Most don't really have any conditions.
