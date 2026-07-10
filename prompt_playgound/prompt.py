"""The LLM text format: renders the character prompt and parses action replies."""

import json
import re

from sim import Character, World

HEADER = (
    "You are a character in a medieval 3d world, that can interact with "
    "the player as well as other characters."
)

FOOTER = """\
Take one or more actions.
Make SURE that what you're doing matches what you see, who you are, what you can think about/understand etc.

IMPORTANT: You have no memory besides stored_memories and current_goal. After this
turn you will only know what they contain, plus whatever you hear next. Use
remember for anything future-you should know (who you talked to, what was agreed),
and forget for memories that are no longer true.

People marked (unknown) are strangers: you don't know their name, but you can
still talk to them by id. If you want them to know your name, just say it — and
when someone tells you theirs, store it with remember (e.g.
remember {"memory": "The pilgrim with id k0fb1 is called Ilse"}).

Possible actions (format: `VERB ARGS`), examples:

```
say {"target": "4bfk4", "text": "Howdy, stranger!"}  # Say something to for example a person with id 4bfk4
say {"text": "Fresh fish for sale!"}                 # Without target: said aloud to everyone nearby
set_goal {"goal": "Eat fish"}
remember {"memory": "I like ships"}
forget {"memory": "I like ships"}
```

Output like this, and only like this (skip the backticks, and everything after # is a comment):

```
set_goal {"goal": "Eat fish"}  # We're hungry
say {"target": "4bfk4", "text": "Conny, do you like fish?"}
```"""


def render_prompt(world: World, actor: Character) -> str:
    people = [
        {
            "id": c.id,
            "name": (
                c.name
                if c.id in actor.knows
                else "(unknown - you don't know the name of this person)"
            ),
        }
        for c in world.at_location(actor.location, exclude=actor.id)
    ]
    sheet = {
        "name": actor.name,
        "back_story": actor.back_story,
        "you_are": actor.location,
        "you_hold": [
            {"id": item_id, "name": world.items[item_id].name}
            for item_id in actor.holds
        ],
        "you_see": {
            "description": "A few people that are nearby",
            "people": people,
        },
        "since_your_last_turn": actor.inbox or ["nothing"],
        "stored_memories": actor.memories,
        "the_only_languages_you_know": "English",
        "current_goal": actor.goal,
    }
    return f"{HEADER}\n\n```json\n{json.dumps(sheet, indent=4)}\n```\n\n{FOOTER}\n"


_ACTION_RE = re.compile(r"^([a-z_]\w*)\s*(\{.*)$", re.IGNORECASE)
_decoder = json.JSONDecoder()


def parse_reply(reply: str) -> tuple[list[tuple[str, dict]], list[str]]:
    """Parse `VERB {json}` lines into (actions, errors).

    Trailing `# comments` are handled by raw_decode stopping at the end of
    the JSON object, so `#` inside quoted strings is safe.
    """
    actions: list[tuple[str, dict]] = []
    errors: list[str] = []
    for line in reply.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#") or stripped.startswith("```"):
            continue
        m = _ACTION_RE.match(stripped)
        if not m:
            errors.append(f"not understood: {stripped}")
            continue
        verb = m.group(1).lower()
        try:
            args, _ = _decoder.raw_decode(m.group(2))
        except json.JSONDecodeError as e:
            errors.append(f"bad JSON in: {stripped} ({e})")
            continue
        if not isinstance(args, dict):
            errors.append(f"args must be a JSON object: {stripped}")
            continue
        actions.append((verb, args))
    return actions, errors
