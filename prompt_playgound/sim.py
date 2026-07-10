"""World state and action execution for the character simulation."""

from dataclasses import dataclass, field
from typing import NewType

# Entity ids as they key the world dicts and appear in JSON args (`item_id`,
# `target`). Purely a type-checker distinction from other strings.
ItemIdStr = NewType("ItemIdStr", str)
CharIdStr = NewType("CharIdStr", str)


@dataclass
class Item:
    id: ItemIdStr
    name: str


@dataclass
class Character:
    id: CharIdStr
    name: str
    back_story: str
    location: str
    holds: list[ItemIdStr] = field(default_factory=list)
    goal: str = "None"
    memories: list[str] = field(default_factory=list)
    inbox: list[str] = field(default_factory=list)  # events perceived since last turn
    knows: set[CharIdStr] = field(default_factory=set)  # ids of people known by name


@dataclass
class World:
    characters: dict[CharIdStr, Character] = field(default_factory=dict)
    items: dict[ItemIdStr, Item] = field(default_factory=dict)
    # item id -> (giver id, target id or None for broadcast). The item stays
    # in the giver's holds until accepted, so "offer exists" => "giver holds it".
    offers: dict[ItemIdStr, tuple[CharIdStr, CharIdStr | None]] = field(default_factory=dict)
    transcript: list[str] = field(default_factory=list)

    def add(self, entity: Character | Item) -> None:
        if isinstance(entity, Character):
            self.characters[entity.id] = entity
        else:
            self.items[entity.id] = entity

    def at_location(self, location: str, exclude: CharIdStr | None = None) -> list[Character]:
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
        target = world.characters.get(CharIdStr(args.get("target", "")))
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

    if verb == "offer_item":
        item_id = ItemIdStr(args["item_id"])
        if item_id not in actor.holds:
            raise ValueError(f"you hold no item with id {item_id!r} (item_id takes an id, not a name)")
        item = world.items[item_id]
        # A bad target id is an error, NOT a fallback to broadcast — but a
        # null/omitted target IS the broadcast form.
        target_id = args.get("target")
        target = None
        if target_id is not None:
            target = world.characters.get(CharIdStr(target_id))
            if target is None:
                raise ValueError(f"there is nobody with id {target_id!r}")
            if target.id == actor.id:
                raise ValueError("you can't offer an item to yourself")
            if target.location != actor.location:
                raise ValueError(f"{identify(actor, target)} is not here")
        old = world.offers.get(item_id)
        world.offers[item_id] = (actor.id, target.id if target else None)
        if old is not None and old[1] is not None and old[1] != (target.id if target else None):
            jilted = world.characters.get(old[1])
            if jilted is not None:
                jilted.inbox.append(
                    f"{_cap(identify(jilted, actor))} withdrew the offered {item.name} (id {item_id})"
                )
        hint = f'(accept_offered_item {{"item_id": "{item_id}"}} to accept)'
        if target is not None:
            target.inbox.append(
                f"{_cap(identify(target, actor))} holds out a {item.name} (id {item_id}) to you. {hint}"
            )
            for other in world.at_location(actor.location, exclude=actor.id):
                if other.id != target.id:
                    other.inbox.append(
                        f"{_cap(identify(other, actor))} offered a {item.name} to {identify(other, target)}"
                    )
            return f"{actor.name} offers the {item.name} to {target.name}"
        for other in world.at_location(actor.location, exclude=actor.id):
            other.inbox.append(
                f"{_cap(identify(other, actor))} holds out a {item.name} (id {item_id}) "
                f"to anyone who wants it. {hint}"
            )
        return f"{actor.name} offers the {item.name} to anyone nearby"

    if verb == "accept_offered_item":
        item_id = ItemIdStr(args["item_id"])
        offer = world.offers.get(item_id)
        if offer is None:
            raise ValueError(f"nobody is offering you an item with id {item_id!r}")
        giver_id, target_id = offer
        if giver_id == actor.id:
            raise ValueError("that is your own offer (retract_offer to withdraw it)")
        if target_id is not None and target_id != actor.id:
            raise ValueError(f"nobody is offering you an item with id {item_id!r}")
        giver = world.characters.get(giver_id)
        if giver is None or giver.location != actor.location:
            raise ValueError("the person offering it is no longer here")
        if item_id not in giver.holds:
            # Can't happen while the offer invariant holds; clear the stale offer.
            del world.offers[item_id]
            raise ValueError(f"{identify(actor, giver)} no longer holds that item")
        item = world.items[item_id]
        del world.offers[item_id]
        giver.holds.remove(item_id)
        actor.holds.append(item_id)
        giver.inbox.append(
            f"{_cap(identify(giver, actor))} accepted the {item.name} (id {item_id}) you offered"
        )
        for other in world.at_location(actor.location, exclude=actor.id):
            if other.id != giver.id:
                other.inbox.append(
                    f"{_cap(identify(other, actor))} took a {item.name} from {identify(other, giver)}"
                )
        return f"{actor.name} takes the {item.name} from {giver.name}"

    if verb == "decline_offer":
        item_id = ItemIdStr(args["item_id"])
        offer = world.offers.get(item_id)
        if offer is not None and offer[1] is None:
            raise ValueError("that offer is open to anyone, not addressed to you — just ignore it")
        if offer is None or offer[1] != actor.id:
            raise ValueError(f"nobody is offering you an item with id {item_id!r}")
        giver = world.characters[offer[0]]
        del world.offers[item_id]
        item = world.items[item_id]
        giver.inbox.append(
            f"{_cap(identify(giver, actor))} declined the {item.name} (id {item_id}) you offered"
        )
        for other in world.at_location(actor.location, exclude=actor.id):
            if other.id != giver.id:
                other.inbox.append(
                    f"{_cap(identify(other, actor))} declined a {item.name} from {identify(other, giver)}"
                )
        return f"{actor.name} declines the {item.name} from {giver.name}"

    if verb == "retract_offer":
        item_id = ItemIdStr(args["item_id"])
        offer = world.offers.get(item_id)
        if offer is None or offer[0] != actor.id:
            raise ValueError(f"you have no pending offer of an item with id {item_id!r}")
        del world.offers[item_id]
        item = world.items[item_id]
        if offer[1] is not None:
            target = world.characters.get(offer[1])
            if target is not None:
                target.inbox.append(
                    f"{_cap(identify(target, actor))} withdrew the offered {item.name} (id {item_id})"
                )
        return f"{actor.name} retracts the offer of the {item.name}"

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
