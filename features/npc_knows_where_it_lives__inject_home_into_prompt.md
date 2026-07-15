# Let an NPC answer "Where do you live?"

The home-binding bake already exists — `scripts/bake_homes.py` writes
`assets/world/homes.json`, one residential door per non-homeless character, the
~132 `pauper`/`unhoused`/`insecure_lodging` deliberately left bedless
(`features/movement/04_the_round.md` §3). That bake is a **spatial** binding:
`{building, edge, door_node, point}`. It steers feet (walk home at the Snuffing,
leash/drift anchor for the round), not speech.

So today an NPC has a home but *cannot tell you about it*. Ask "Where do you live?"
and the model has nothing home-shaped in its prompt to answer from.
`04 §2b` even notes that `planning_ward` "is not injected into the NPC prompt"
— the home is, by design so far, invisible to the LLM.

This feature closes that gap: surface each character's home into the prompt as a
**readable place**, so the answer to "where do you live" is grounded and
consistent with where they actually walk home to.

## What's already there (don't rebuild it)

- `crates/cathedral-sim/src/prompt/mod.rs` — `PromptLoreProfile` already carries
  `district: &'a str` (a ward-level string like `"Cinder Row"`, from the
  character sheet's authored `district`, *not* from `homes.json`). And `YouAre`
  carries `location_description` for where they currently stand. There is a slot
  to add to; the machinery is not new.
- `homes.json` — the binding itself. But it stores only `building` (`omb_i0479`),
  `district`, and an XZ `point`. Nothing you'd want to put in a prompt verbatim.

## The two pieces to build

**1. A human-readable home descriptor (enrich the bake).** "You live at
`omb_i0479`, door node 6688" is useless to the model. `bake_homes.py` should also
resolve each home to something a person would say: the nearest named area /
landmark from `areas.json` (and/or a street, if the world has street names), plus
the ward. Aim for "a house in Cinder Row, near the Tallage" granularity, not raw
ids or coordinates. Add e.g. `"place_description": "..."` (and maybe a bare
`"landmark"`/`"ward"`) to each `homes.json` entry so the readable form is baked
once and deterministic, same as the rest of the file.

**2. Inject it into the prompt (the seam).** Load `homes.json` into the sim
(nothing reads it yet — `round.rs` still uses the spawn position as its "home"
anchor, so this load also unblocks M4's actual walk-home), and add a `home` field
to `PromptLoreProfile` (or a small `you_live` block). Populate it from the baked
`place_description`.

## The homeless are content here too

Don't leave the ~132 bedless people with an empty field the model silently
ignores — that's the most interesting case. Give them an explicit "no fixed bed"
framing so the LLM *knows* it and can play it: the woodstore of the Bell and
Ladle, a doorway, wherever `04 §3/§6` says. An NPC who answers "Vart bor du?"
with "nowhere the watch will leave me" is exactly the life the city is meant to
contain.

## Scope / caution

- Read-only into the prompt; no new movement behaviour required (though it pairs
  naturally with wiring `homes.json` into the round for M4).
- Keep it at place/landmark granularity, never coordinates — the model should
  speak like a resident, not read out a map pin.
- Adding a field to the prompt changes the golden fixtures; regenerate them and
  eyeball the diff.
- Related design: `04_the_round.md` §3 (the bake), `05_the_llm_seam.md` (what the
  LLM is and isn't allowed to touch).
