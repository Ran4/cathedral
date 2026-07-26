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
    "significance": "major",
    "planning_ward": "cinder",
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
    "circumstances": [],
    "conditions": ["crippled"],
    "memories": [],
    "core_character_description": "...",
    "extended_character_description": "...",
    "bespoke_appearance": null,
    "voice_key": "ilse",
    "holds": ["optional_item_id"],
    "goal": "optional initial goal"
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
  as in `position_m` in `assets/world/seed.json`. These are canonical in-game transforms, curated against
  `assets/world/areas.json` and the Ombreval city plan. Most street-level characters stand at `y: 0.91`;
  tower workers may use an authored elevated floor. `district` records where a person belongs, which may
  differ from a deliberately authored current location (notably the original Gradine trio).
* `planning_ward` is one of `fabric`, `wick`, `cloth`, `wallwright`, `cinder`,
  `weigh`, `reed`, or `bell_and_sluice`. It is authoring/spatial metadata and is
  not injected into the NPC prompt.
* The shipped roster is deliberately dispersed over the full walled city.
  Counting the subject, no 20 m neighbourhood may contain more than three
  NPCs, and no sliding axis-aligned 100 x 100 m region may contain more than
  ten. Major canonical scenes may retain groups of up to three; household and
  workplace relationships do not imply that everyone starts in one cluster.
* `bespoke_appearance`, `voice_key`, `holds`, and `goal` are optional runtime overrides. Most characters
  omit them and receive a deterministic voice from the existing three-voice pool plus a body *composed*
  from the sheet facts (gender, occupation, rank, circumstances → the structured `AppearanceSnapshot` in
  `crates/cathedral-sim/src/appearance.rs`). `bespoke_appearance` names a bespoke look on top of that
  composition; only the original demo trio (`sven`, `conny`, `ilse`) carries one. Held item ids must
  exist in `assets/world/seed.json`.

# Occupation and circumstance model

`occupation_id` remains a trade or livelihood family. These belong in separate
fields:

- `rank`: guild or institutional rank;
- `faction_role`: a special role in a faction;
- `illegal_activity`: prohibited conduct;
- `circumstances`: poverty, housing, family, residency and legal standing —
  authored, durable social/economic/legal tags (formerly named `statuses`);
- `conditions`: physical and health conditions;
- `significance`: canonical and computational importance.

Note: the field was renamed from `statuses` to `circumstances` so the word
`statuses` can be reused for the transient internal-drive/mood layer (hunger,
health, drunkenness, …) introduced by the movement work. See
`features/implemented/movement/03_the_ladder.md`.

A character who begs should usually retain a real former, occasional or
intermittent occupation. Examples include an injured porter, an unemployed
labourer, a widowed laundress, a retired watchman or an out-of-work servant.
Begging is then represented by circumstances such as `pauper`, `alms_dependent`,
`unhoused` and `begs_regularly`, plus the description and goal.

`occupation_id`, `title`, and `rank` may all be null only for genuine
dependants or people with no present or former trade. Those sheets live in
`no_fixed_trade/` and must explain their material support. Every other sheet
must live under its exact `occupation_id` and use a title registered in
`lore/core_lore/occupations.json`.


# Significance value of characters

### Major

- Individually required by canonical lore, a faction, an institution or a
  planned quest.
- May be referenced freely by other character sheets and lore documents.
- Receives the largest reasoning budget and richest persistent memory.
- Has a full authored description, consequential relationships and a clear
  place in the city.
- Removal requires a repository-wide lore and reference audit.

### Minor

- A stable named supporting character: a master tailor, ward officer, known
  beggar, brothel keeper, militia captain, gang organiser or recurring servant.
- May be mentioned in lore, but usually only locally or once or twice.
- Receives normal interaction compute and persistent memory.
- Has a compact but real backstory, several relationships and at least one
  continuing concern.
- Removal also requires a reference audit.

### Ambient

- Must not be individually named outside their own character sheet by core
  lore, second-sun lore, features presented as canon, major/minor character
  relationships, quests, canonical events or unique items.
- May have relationships with other ambient characters. Those relations are
  replaceable data, not canon.
- May point outward to a major/minor employer or public figure from their own
  sheet, but the stable character must not point back to the ambient character
  by id or name.
- Must have enough specificity to avoid becoming a generic crowd token:
  locality, livelihood/status, speech manner, current activity and one
  immediate material concern.
- Should normally have no unique faction office, quest item or world-changing
  secret.
- Is replaceable between authored versions. Once encountered in a save, their
  runtime identity and memories must remain stable for that save.
- Dynamic player interest may temporarily increase compute and memory without
  changing the character's canonical significance.

Do not include the words `major`, `minor` or `ambient` in the NPC's own prompt
as a statement about who they are. `significance` is host scheduling and
authoring metadata, not self-knowledge.

## Profile depth by significance

These are authoring targets, not hard byte limits:

| Significance | Core description | Extended description | Static relationships |
|---|---|---|---:|
| `major` | Roughly 150-300 words | As much as needed | Usually 5-15 meaningful links |
| `minor` | Roughly 80-180 words | Optional, roughly 600-2000 words | Usually 2-6 links |
| `ambient` | Roughly 40-90 words | Normally empty | Usually 0-2 links |

An ambient description should normally answer five things in a few sentences:

1. What are you doing here?
2. How do you materially survive?
3. How do you speak or react to strangers?
4. What small thing do you need today?
5. What immediately visible fact makes you distinct from the next person?

The immediate concern can be a wet blanket, a disputed penny, an employer who
is late, a missing chicken, sore feet, a place in the hiring line or fear of
losing a sleeping place. It does not need to be a secret or quest.
