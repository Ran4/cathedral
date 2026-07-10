"""World state and action execution for the character simulation."""

from dataclasses import dataclass, field


@dataclass
class Item:
    id: str
    name: str


@dataclass
class Character:
    id: str
    name: str
    back_story: str
    location: str
    holds: list[str] = field(default_factory=list)  # item ids
    goal: str = "None"
    memories: list[str] = field(default_factory=list)
    inbox: list[str] = field(default_factory=list)  # events perceived since last turn
    knows: set[str] = field(default_factory=set)  # ids of people known by name


@dataclass
class World:
    characters: dict[str, Character] = field(default_factory=dict)
    items: dict[str, Item] = field(default_factory=dict)
    transcript: list[str] = field(default_factory=list)

    def add(self, entity: Character | Item) -> None:
        if isinstance(entity, Character):
            self.characters[entity.id] = entity
        else:
            self.items[entity.id] = entity

    def at_location(self, location: str, exclude: str | None = None) -> list[Character]:
        return [
            c
            for c in self.characters.values()
            if c.location == location and c.id != exclude
        ]


def identify(observer: Character, subject: Character) -> str:
    """How `observer` refers to `subject`: by name if known, else as a stranger."""
    if subject.id in observer.knows:
        return subject.name
    return f"a stranger (id {subject.id})"


def _cap(s: str) -> str:
    return s[0].upper() + s[1:]


def apply_action(world: World, actor: Character, verb: str, args: dict) -> str:
    """Apply one action to the world. Returns a transcript line.

    Raises KeyError on missing args and ValueError on unknown verbs; the
    caller turns those into system events in the actor's inbox.
    """
    if verb == "say":
        text = args["text"]
        target = world.characters.get(args.get("target", ""))
        bystanders = world.at_location(actor.location, exclude=actor.id)
        if target is not None and target.id != actor.id and target in bystanders:
            target.inbox.append(f'{_cap(identify(target, actor))} said to you: "{text}"')
            for other in bystanders:
                if other.id != target.id:
                    other.inbox.append(
                        f'{_cap(identify(other, actor))} said to '
                        f'{identify(other, target)}: "{text}"'
                    )
            return f'{actor.name} -> {target.name}: "{text}"'
        # No (valid) target: say it aloud to everyone nearby.
        for other in bystanders:
            other.inbox.append(f'{_cap(identify(other, actor))} said: "{text}"')
        return f'{actor.name} (aloud): "{text}"'

    if verb == "set_goal":
        actor.goal = args["goal"]
        return f"{actor.name} now wants: {actor.goal}"

    if verb == "remember":
        memory = args["memory"]
        if memory not in actor.memories:
            actor.memories.append(memory)
        return f"{actor.name} remembers: {memory}"

    if verb == "forget":
        memory = args["memory"]
        if memory in actor.memories:
            actor.memories.remove(memory)
            return f"{actor.name} forgets: {memory}"
        # Exact match failed; the model rarely reproduces a memory verbatim.
        for m in actor.memories:
            if memory in m or m in memory:
                actor.memories.remove(m)
                return f"{actor.name} forgets: {m}"
        return f"{actor.name} tried to forget something they never knew: {memory}"

    raise ValueError(f"unknown verb: {verb}")
