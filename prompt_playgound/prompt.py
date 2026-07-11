"""The LLM text format: renders the character prompt and parses action replies."""

import json
import re

from sim import ITEM_INTERACTION_RADIUS_M, Character, Offer, World

HEADER = (
    "You are a character in a medieval 3d world, that can interact with "
    "the player as well as other characters."
)

FOOTER = """\
Take one or more actions.
Make SURE that what you're doing matches what you see, who you are, what you can think about/understand etc.

IMPORTANT: recent_conversation is your short-term recollection of the latest
dialogue, including your own words. Read it before speaking: do not repeat a
question, answer, greeting, offer, or topic that is already there unless someone
has just given you a reason to revisit it. It is bounded and older lines will
disappear. Your only durable memory is stored_memories and current_goal. Use
remember for anything future-you should know after the recent conversation fades
— above all OUTCOMES: the moment a
trade, payment or agreement completes, record it in that same turn (e.g.
remember {"memory": "I bought the fish from Sven for one copper"}), or
future-you will not know it happened and will try to do it again. Record
half-done deals the same way: if you took payment but have not yet handed over
the goods — or paid and not yet received — remember that open obligation, and
forget it once settled. Write
memories in first person ("I ..."). Also re-read stored_memories every turn and
forget each one that is now false or finished (an item you no longer hold, a
debt now settled, a plan already carried out) — when you record an outcome,
forget the memories it replaces.

Keep current_goal current: when your goal is achieved (check you_hold and your
memories) or has become impossible, set_goal a new one — or clear it with
set_goal {"goal": null} — and remember how it ended.

People marked (unknown) are strangers: you don't know their name, but you can
still talk to them by id. If you want them to know your name, just say it — and
when someone tells you theirs, store it with remember (e.g.
remember {"memory": "The pilgrim with id k0fb1 is called Ilse"}).

Items change hands only by consent: offer_item holds an item out (you still
hold it), and it only moves when the other character accepts it on a later
turn. Pending offers stay on your sheet under you_offer / offered_to_you until
accepted, declined, or retracted. item_id always takes an id (like "fzbn9"),
never a name.

since_your_last_turn is history, in order — it can already be out of date (an
offer you heard about may be gone, taken by someone else). you_hold, you_offer
and offered_to_you are the current truth: you can only accept offers listed in
offered_to_you.

"Nearby" people and speech are within 20 metres. Item offers and their
acceptance require the people to be within 4 metres; offered_to_you already
contains only offers you can act on at this moment.

Speech in your history is what you could hear, not necessarily speech addressed
to you. Before speaking in response, decide from the wording (including names
and phrases such as "anyone"), your own identity, the nearby people, and the
recent conversation whether the speaker is talking to you, to the group, or to
somebody else. If a line is clearly for somebody else, normally use `wait {}`
alone. Do not answer merely to announce that you are not the named person.
Interject only when your character has a concrete reason to do so. If you are
the only plausible listener and the speaker asks for somebody who is not there,
asking for clarification or expressing confusion can be natural. Questions to
"anyone", "everyone", or the group are open to relevant answers.

Examples:
- In a group, if Ilse hears "Sven, do you have fish?", Ilse normally uses
  `wait {}` alone.
- If Sven hears "Sven, do you have fish?", Sven answers if he can.
- If Ilse is alone with the speaker and hears them ask for Sven, clarification
  or confusion is reasonable.
- After "Does anyone have fish?", answer only if you have a relevant answer;
  otherwise use `wait {}` alone.

Use ONLY the verbs listed below, spelled exactly as shown (lowercase English).
There are no other verbs: if what you want to do has no verb here (like walking
somewhere), express it in speech with say instead of inventing a verb.

Possible actions (format: `VERB ARGS`), examples:

```
say {"target": "4bfk4", "text": "Howdy, stranger!"}  # Say something to for example a person with id 4bfk4
say {"text": "Fresh fish for sale!"}                 # Without target: said aloud to everyone nearby
offer_item {"item_id": "fzbn9", "target": "4bfk4"}   # Hold out an item you hold to that person
offer_item {"item_id": "fzbn9"}                      # Without target: offered to anyone nearby, first to accept gets it
accept_offered_item {"item_id": "fzbn9"}             # Take an item currently offered to you
decline_offer {"item_id": "fzbn9"}                   # Turn down an item offered to you (the offerer keeps it)
retract_offer {"item_id": "fzbn9"}                   # Withdraw an offer you made
eat {"item_id": "fzbn9"}                             # Eat something you hold; it is gone for good
set_goal {"goal": "Eat fish"}
set_goal {"goal": null}                              # Clear your goal (achieved or given up)
remember {"memory": "I like ships"}
forget {"memory": "I like ships"}
wait {}                                              # Stay quiet when there is nothing useful and socially appropriate to do
```

Do not manufacture conversation merely because it is your turn. Use `wait {}`
alone whenever there is nothing useful and socially appropriate for you to do,
even if you just overheard something new.

Output like this, and only like this (skip the backticks, and everything after # is a comment):

```
set_goal {"goal": "Eat fish"}  # We're hungry
say {"target": "4bfk4", "text": "Conny, do you like fish?"}
```"""


def _person(actor: Character, c: Character, *, distance_m: float | None = None) -> dict:
    person = {
        "id": c.id,
        "name": (
            c.name
            if c.id in actor.knows
            else "(unknown - you don't know the name of this person)"
        ),
    }
    if distance_m is not None:
        person["distance_m"] = round(distance_m, 1)
    return person


def _distance_m(actor: Character, other: Character) -> float:
    return actor.position_m.distance_squared(other.position_m) ** 0.5


def _offer_sort_key(offer: Offer) -> tuple[int, str]:
    return offer.created_seq, str(offer.item_id)


def render_prompt(
    world: World,
    actor: Character,
    *,
    since_your_last_turn: list[str] | None = None,
) -> str:
    if actor.control != "llm":
        raise ValueError("the human-controlled player must never receive an LLM prompt")
    people = [
        _person(actor, person, distance_m=_distance_m(actor, person))
        for person in world.characters_within(actor, 20.0, exclude=actor.id)
    ]
    you_offer = []
    offered_to_you = []
    for offer in sorted(world.offers.values(), key=_offer_sort_key):
        item_id = offer.item_id
        item_entity = world.items.get(item_id)
        if item_entity is None:
            continue
        item = {"id": item_id, "name": item_entity.name}
        if offer.giver_id == actor.id:
            target = (
                None
                if offer.target_id is None
                else world.characters.get(offer.target_id)
            )
            you_offer.append(
                {
                    "item": item,
                    "to": ("anyone" if target is None else _person(actor, target)),
                }
            )
        elif offer.target_id is None or offer.target_id == actor.id:
            giver = world.characters.get(offer.giver_id)
            if giver is None:
                continue
            if (
                actor.position_m.distance_squared(giver.position_m)
                > ITEM_INTERACTION_RADIUS_M**2
            ):
                continue
            offered_to_you.append(
                {
                    "item": item,
                    "from": _person(actor, giver),
                    "accept_with": f'accept_offered_item {{"item_id": "{item_id}"}}',
                }
            )
    events = actor.inbox if since_your_last_turn is None else since_your_last_turn
    sheet = {
        "name": actor.name,
        "back_story": actor.back_story,
        "you_are": {
            "location_description": actor.location_description,
            "position_m": actor.position_m.to_json(),
        },
        "you_hold": [
            {"id": item_id, "name": world.items[item_id].name}
            for item_id in actor.holds
            if item_id in world.items
        ],
        **({"you_offer": you_offer} if you_offer else {}),
        **({"offered_to_you": offered_to_you} if offered_to_you else {}),
        "you_see": {
            "description": "People within 20 metres, nearest first",
            "people": people,
        },
        "since_your_last_turn": events or ["nothing"],
        "recent_conversation": actor.recent_conversation or ["nothing yet"],
        "stored_memories": actor.memories,
        "the_only_languages_you_know": "English",
        "current_goal": actor.goal,
    }
    return f"{HEADER}\n\n```json\n{json.dumps(sheet, indent=4)}\n```\n\n{FOOTER}\n"


def render_prompt_and_drain(world: World, actor: Character) -> str:
    """Move the current inbox into a prompt, leaving a fresh inbox behind."""
    drained = actor.inbox
    actor.inbox = []
    try:
        return render_prompt(world, actor, since_your_last_turn=drained)
    except Exception:
        actor.inbox = drained + actor.inbox
        raise


_ACTION_RE = re.compile(r"^([a-z_]\w*)\s*(\{.*)$", re.IGNORECASE)
_decoder = json.JSONDecoder()


def _safe_json_shape(
    value: object, *, max_depth: int = 64, max_nodes: int = 10_000
) -> bool:
    stack = [(value, 0)]
    nodes = 0
    while stack:
        current, depth = stack.pop()
        nodes += 1
        if depth > max_depth or nodes > max_nodes:
            return False
        if isinstance(current, dict):
            stack.extend((child, depth + 1) for child in current.values())
        elif isinstance(current, list):
            stack.extend((child, depth + 1) for child in current)
    return True


def parse_reply(reply: object) -> tuple[list[tuple[str, dict]], list[str]]:
    """Parse `VERB {json}` lines into (actions, errors).

    Trailing `# comments` are handled by raw_decode stopping at the end of
    the JSON object, so `#` inside quoted strings is safe.
    """
    if not isinstance(reply, str):
        return [], ["reply must be text"]
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
            args, end = _decoder.raw_decode(m.group(2))
        except (json.JSONDecodeError, RecursionError) as e:
            errors.append(f"bad JSON in: {stripped} ({e})")
            continue
        if not isinstance(args, dict):
            errors.append(f"args must be a JSON object: {stripped}")
            continue
        if not _safe_json_shape(args):
            errors.append(f"JSON structure is too deeply nested or large in: {verb}")
            continue
        trailing = m.group(2)[end:].strip()
        if trailing and not trailing.startswith("#"):
            errors.append(f"unexpected text after JSON in: {stripped}")
            continue
        actions.append((verb, args))
    return actions, errors
