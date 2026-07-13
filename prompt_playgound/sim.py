"""Authoritative world state and action execution for smart actors.

The game client mirrors the public portion of this module's state, but never
commits an action itself.  Keep all validation here so LLM actions and player
commands have exactly the same semantics.
"""

from __future__ import annotations

import math
from dataclasses import dataclass, field
from typing import Any, Literal, Mapping, NewType, Sequence

from sounds import SOUNDS, Sound

HEARING_RADIUS_M = 20.0
ITEM_INTERACTION_RADIUS_M = 4.0
PLAYER_SPEECH_MAX_CHARS = 500
RECENT_CONVERSATION_MAX_ENTRIES = 16
# Total horizontal FOV for the sound witness test. 135° is a playtesting guess;
# config.ron `smart_actors.sounds.view_cone_degrees` overrides it per run.
DEFAULT_VIEW_CONE_DEGREES = 135.0

# Entity ids as they key world dictionaries and appear in JSON arguments.
ItemIdStr = NewType("ItemIdStr", str)
CharIdStr = NewType("CharIdStr", str)


class ActionError(ValueError):
    """An expected, player/LLM-safe action validation failure."""

    def __init__(self, message: str, code: str = "invalid_action") -> None:
        super().__init__(message)
        self.code = code


class SpatialUpdateError(ValueError):
    """An invalid or stale position update."""

    def __init__(self, message: str, code: str = "invalid_position") -> None:
        super().__init__(message)
        self.code = code


@dataclass(frozen=True, slots=True)
class Vec3:
    x: float
    y: float
    z: float

    def __post_init__(self) -> None:
        values: list[float] = []
        for name in ("x", "y", "z"):
            value = getattr(self, name)
            if isinstance(value, bool) or not isinstance(value, (int, float)):
                raise SpatialUpdateError(f"position {name} must be a finite number")
            value = float(value)
            if not math.isfinite(value):
                raise SpatialUpdateError(f"position {name} must be a finite number")
            values.append(value)
        object.__setattr__(self, "x", values[0])
        object.__setattr__(self, "y", values[1])
        object.__setattr__(self, "z", values[2])

    @classmethod
    def from_json(cls, value: object) -> Vec3:
        if not isinstance(value, Mapping):
            raise SpatialUpdateError("position_m must be an object with x, y, and z")
        if set(value) != {"x", "y", "z"}:
            raise SpatialUpdateError("position_m must contain exactly x, y, and z")
        return cls(value["x"], value["y"], value["z"])  # type: ignore[arg-type]

    def to_json(self) -> dict[str, float]:
        return {"x": self.x, "y": self.y, "z": self.z}

    def distance_squared(self, other: Vec3) -> float:
        dx = self.x - other.x
        dy = self.y - other.y
        dz = self.z - other.z
        return dx * dx + dy * dy + dz * dz


@dataclass(slots=True)
class Item:
    id: ItemIdStr
    name: str
    visual_key: str = "generic"


@dataclass(slots=True)
class Character:
    id: CharIdStr
    name: str
    control: Literal["llm", "player"]
    back_story: str
    location_description: str
    position_m: Vec3
    appearance_key: str
    voice_key: str | None
    # Compass bearing in radians, matching Bevy's yaw: the character faces
    # (-sin(yaw), -cos(yaw)) in the XZ plane. NPCs get a static seeded value;
    # the player's is updated live from `spatial_update`.
    facing_yaw: float = 0.0
    holds: list[ItemIdStr] = field(default_factory=list)
    goal: str = "None"
    memories: list[str] = field(default_factory=list)
    inbox: list[str] = field(default_factory=list)
    recent_conversation: list[str] = field(default_factory=list)
    knows: set[CharIdStr] = field(default_factory=set)

    def __post_init__(self) -> None:
        if self.control not in ("llm", "player"):
            raise ValueError("control must be 'llm' or 'player'")

    @property
    def location(self) -> str:
        """Compatibility alias for old terminal-prototype callers."""
        return self.location_description


@dataclass(frozen=True, slots=True)
class Offer:
    item_id: ItemIdStr
    giver_id: CharIdStr
    target_id: CharIdStr | None
    created_seq: int


@dataclass(frozen=True, slots=True)
class DomainEvent:
    """A structured historical event emitted by an authoritative action."""

    sequence: int
    event_type: Literal["speech", "world_event", "sound"]
    kind: str
    # Nullable only for sounds: a world sound (the town bell) has no actor.
    actor_id: CharIdStr | None
    target_id: CharIdStr | None = None
    item_id: ItemIdStr | None = None
    text: str | None = None
    position_m: Vec3 | None = None
    recipient_ids: tuple[CharIdStr, ...] = ()
    # Sound-only fields. ``witness_ids`` ⊆ ``recipient_ids``: the recipients
    # whose view cone contained the actor, in the same distance order.
    sound_id: str | None = None
    audible_distance: float | None = None
    witness_ids: tuple[CharIdStr, ...] = ()

    @property
    def event_id(self) -> str:
        prefix = {"speech": "speech", "sound": "sound"}.get(self.event_type, "world")
        return f"{prefix}-{self.sequence}"


@dataclass(slots=True)
class World:
    characters: dict[CharIdStr, Character] = field(default_factory=dict)
    items: dict[ItemIdStr, Item] = field(default_factory=dict)
    offers: dict[ItemIdStr, Offer] = field(default_factory=dict)
    transcript: list[str] = field(default_factory=list)
    world_revision: int = 0
    event_sequence: int = 0
    spatial_sequence: int = -1
    sounds_enabled: bool = True
    view_cone_degrees: float = DEFAULT_VIEW_CONE_DEGREES
    _events: list[DomainEvent] = field(default_factory=list, repr=False)

    def add(self, entity: Character | Item) -> None:
        if isinstance(entity, Character):
            if entity.id in self.characters:
                raise ValueError(f"duplicate character id {entity.id!r}")
            self.characters[entity.id] = entity
        else:
            if entity.id in self.items:
                raise ValueError(f"duplicate item id {entity.id!r}")
            self.items[entity.id] = entity
        self.touch_public_state()

    def touch_public_state(self) -> int:
        self.world_revision += 1
        return self.world_revision

    def characters_within(
        self,
        origin: Vec3 | Character,
        radius: float,
        exclude: CharIdStr | Character | None = None,
    ) -> list[Character]:
        """Return characters in inclusive range, ordered by distance then id."""
        if isinstance(origin, Character):
            position = origin.position_m
        elif isinstance(origin, Vec3):
            position = origin
        else:  # Defensive: callers are sometimes protocol-facing.
            raise TypeError("origin must be a Vec3 or Character")
        if isinstance(radius, bool) or not isinstance(radius, (int, float)):
            raise ValueError("radius must be a finite non-negative number")
        radius = float(radius)
        if not math.isfinite(radius) or radius < 0:
            raise ValueError("radius must be a finite non-negative number")
        exclude_id = exclude.id if isinstance(exclude, Character) else exclude
        radius_squared = radius * radius
        matches = [
            character
            for character in self.characters.values()
            if character.id != exclude_id
            and position.distance_squared(character.position_m) <= radius_squared
        ]
        matches.sort(key=lambda c: (position.distance_squared(c.position_m), str(c.id)))
        return matches

    def emit(
        self,
        event_type: Literal["speech", "world_event", "sound"],
        kind: str,
        actor_id: CharIdStr | None,
        *,
        target_id: CharIdStr | None = None,
        item_id: ItemIdStr | None = None,
        text: str | None = None,
        position_m: Vec3 | None = None,
        recipient_ids: Sequence[CharIdStr] = (),
        sound_id: str | None = None,
        audible_distance: float | None = None,
        witness_ids: Sequence[CharIdStr] = (),
    ) -> DomainEvent:
        self.event_sequence += 1
        event = DomainEvent(
            sequence=self.event_sequence,
            event_type=event_type,
            kind=kind,
            actor_id=actor_id,
            target_id=target_id,
            item_id=item_id,
            text=text,
            position_m=position_m,
            recipient_ids=tuple(recipient_ids),
            sound_id=sound_id,
            audible_distance=audible_distance,
            witness_ids=tuple(witness_ids),
        )
        self._events.append(event)
        return event

    def drain_events(self) -> list[DomainEvent]:
        events, self._events = self._events, []
        return events

    def update_positions(
        self,
        spatial_sequence: int,
        updates: Sequence[
            tuple[CharIdStr, Vec3] | tuple[CharIdStr, Vec3, float | None]
        ],
    ) -> bool:
        """Validate and atomically apply a spatial update.

        Equal sequences are accepted only as an idempotent repeat.  An equal
        sequence with different coordinates is rejected just like an older one.
        An update may carry an optional third ``facing_yaw`` element; facing
        changes are applied silently and never bump the public revision (the
        next snapshot, whatever triggers it, always reads current facing).
        """
        if isinstance(spatial_sequence, bool) or not isinstance(spatial_sequence, int):
            raise SpatialUpdateError("spatial_seq must be a non-negative integer")
        if spatial_sequence < 0:
            raise SpatialUpdateError("spatial_seq must be a non-negative integer")
        if spatial_sequence < self.spatial_sequence:
            raise SpatialUpdateError(
                "spatial update is older than current state", "stale_spatial_seq"
            )

        checked: list[tuple[Character, Vec3, float | None]] = []
        seen: set[CharIdStr] = set()
        for update in updates:
            actor_id, position = update[0], update[1]
            facing_yaw = update[2] if len(update) > 2 else None
            actor_id = _character_id(actor_id, "actor_id")
            if actor_id in seen:
                raise SpatialUpdateError(
                    f"duplicate actor_id {actor_id!r} in spatial update"
                )
            seen.add(actor_id)
            actor = self.characters.get(actor_id)
            if actor is None:
                raise SpatialUpdateError(
                    f"unknown actor id {actor_id!r}", "unknown_actor"
                )
            if not isinstance(position, Vec3):
                raise SpatialUpdateError("position_m must be a valid finite Vec3")
            if facing_yaw is not None:
                if (
                    isinstance(facing_yaw, bool)
                    or not isinstance(facing_yaw, (int, float))
                    or not math.isfinite(float(facing_yaw))
                ):
                    raise SpatialUpdateError("facing_yaw must be a finite number")
                facing_yaw = float(facing_yaw)
            checked.append((actor, position, facing_yaw))

        if spatial_sequence == self.spatial_sequence:
            if any(actor.position_m != position for actor, position, _ in checked):
                raise SpatialUpdateError(
                    "spatial sequence was reused with different coordinates",
                    "stale_spatial_seq",
                )
            return False

        changed = any(actor.position_m != position for actor, position, _ in checked)
        for actor, position, facing_yaw in checked:
            actor.position_m = position
            if facing_yaw is not None:
                actor.facing_yaw = facing_yaw
        self.spatial_sequence = spatial_sequence
        if changed:
            self.touch_public_state()
        return changed

    def public_snapshot(
        self, player_id: CharIdStr = CharIdStr("player")
    ) -> dict[str, Any]:
        player = self.characters.get(player_id)
        actors = []
        for actor in sorted(self.characters.values(), key=lambda c: str(c.id)):
            if player is None or actor.id == player.id or actor.id in player.knows:
                label = "You" if actor.id == player_id else actor.name
            else:
                label = f"a stranger (id {actor.id})"
            actors.append(
                {
                    "id": str(actor.id),
                    "name_for_player": label,
                    "control": actor.control,
                    "position_m": actor.position_m.to_json(),
                    "facing_yaw": actor.facing_yaw,
                    "appearance_key": actor.appearance_key,
                    "holds": [str(item_id) for item_id in actor.holds],
                }
            )
        items = [
            {"id": str(item.id), "name": item.name, "visual_key": item.visual_key}
            for item in sorted(self.items.values(), key=lambda item: str(item.id))
        ]
        offers = [
            {
                "item_id": str(offer.item_id),
                "giver_id": str(offer.giver_id),
                "target_id": str(offer.target_id)
                if offer.target_id is not None
                else None,
                "created_seq": offer.created_seq,
            }
            for offer in sorted(
                self.offers.values(),
                key=lambda offer: (offer.created_seq, str(offer.item_id)),
            )
        ]
        return {
            "world_revision": self.world_revision,
            "player_id": str(player_id),
            "actors": actors,
            "items": items,
            "offers": offers,
        }

    def assert_invariants(self) -> None:
        owners: dict[ItemIdStr, CharIdStr] = {}
        for actor in self.characters.values():
            for item_id in actor.holds:
                if item_id not in self.items:
                    raise AssertionError(
                        f"actor {actor.id} holds missing item {item_id}"
                    )
                if item_id in owners:
                    raise AssertionError(f"item {item_id} has multiple owners")
                owners[item_id] = actor.id
        for item_id, offer in self.offers.items():
            if offer.item_id != item_id:
                raise AssertionError("offer key does not match offer item_id")
            if owners.get(item_id) != offer.giver_id:
                raise AssertionError(f"offer giver does not hold item {item_id}")
            if offer.target_id == offer.giver_id:
                raise AssertionError("offer cannot target its giver")
            if offer.target_id is not None and offer.target_id not in self.characters:
                raise AssertionError("offer targets a missing character")


def identify(observer: Character, subject: Character) -> str:
    """How ``observer`` refers to ``subject`` from seeded knowledge."""
    if observer.id == subject.id or subject.id in observer.knows:
        return subject.name
    return f"a stranger (id {subject.id})"


def sees(observer: Character, subject: Character, view_cone_degrees: float) -> bool:
    """Whether ``subject`` is inside ``observer``'s horizontal view cone.

    The cone is a compass bearing only: facing is a single yaw, so there is
    nothing honest to test vertically. A subject directly above or below the
    observer has no horizontal bearing at all and fails dark (not seen), which
    keeps an undefined angle from ever attributing a sound.
    """
    dx = subject.position_m.x - observer.position_m.x
    dz = subject.position_m.z - observer.position_m.z
    horizontal = math.hypot(dx, dz)
    if horizontal < 1e-9:
        return False
    # Matches Bevy: yaw 0 faces -Z, and Quat::from_rotation_y(yaw) turns it.
    facing_x = -math.sin(observer.facing_yaw)
    facing_z = -math.cos(observer.facing_yaw)
    cosine = (facing_x * dx + facing_z * dz) / horizontal
    half_angle = math.radians(view_cone_degrees) / 2.0
    return cosine >= math.cos(half_angle) - 1e-9


def emit_sound(
    world: World,
    actor: Character | None,
    sound: Sound,
    *,
    position_m: Vec3 | None = None,
) -> str:
    """Emit one sound: everyone in radius hears it, witnesses see who did it.

    ``actor`` is None for world sounds (the town bell), which are never
    attributable regardless of the catalog row. Returns the transcript line.
    """
    if position_m is None:
        if actor is None:
            raise ValueError("a world sound needs an explicit position")
        position_m = actor.position_m
    recipients = world.characters_within(
        position_m,
        sound.audible_distance,
        exclude=None if actor is None else actor.id,
    )
    witnesses: list[Character] = []
    if actor is not None and sound.seen is not None:
        witnesses = [
            recipient
            for recipient in recipients
            if sees(recipient, actor, world.view_cone_degrees)
        ]
    witness_ids = {witness.id for witness in witnesses}
    for recipient in recipients:
        if recipient.id in witness_ids and actor is not None and sound.seen is not None:
            percept = _cap(sound.seen.format(actor=identify(recipient, actor)))
        else:
            # A percept you didn't see must not leak who it was — no id.
            percept = sound.heard
        _notify(recipient, percept)
    world.emit(
        "sound",
        sound.sound_class,
        actor.id if actor is not None else None,
        position_m=position_m,
        recipient_ids=[recipient.id for recipient in recipients],
        sound_id=sound.sound_id,
        audible_distance=sound.audible_distance,
        witness_ids=[witness.id for witness in witnesses],
    )
    if actor is not None and sound.seen is not None:
        return _cap(sound.seen.format(actor=actor.name))
    return sound.heard


def _cap(value: str) -> str:
    return value[:1].upper() + value[1:]


def _character_id(value: object, field_name: str) -> CharIdStr:
    if not isinstance(value, str) or not value or len(value) > 128:
        raise ActionError(
            f"{field_name} must be a non-empty character id", "invalid_arguments"
        )
    return CharIdStr(value)


def _item_id(value: object) -> ItemIdStr:
    if not isinstance(value, str) or not value or len(value) > 128:
        raise ActionError("item_id must be a non-empty item id", "invalid_arguments")
    return ItemIdStr(value)


def _text(value: object, field_name: str, max_chars: int) -> str:
    if not isinstance(value, str):
        raise ActionError(f"{field_name} must be a string", "invalid_arguments")
    value = value.strip()
    if not value:
        raise ActionError(f"{field_name} must not be empty", "invalid_arguments")
    if len(value) > max_chars:
        raise ActionError(
            f"{field_name} is too long (maximum {max_chars} characters)",
            "text_too_long",
        )
    try:
        value.encode("utf-8")
    except UnicodeEncodeError as error:
        raise ActionError(
            f"{field_name} contains invalid Unicode", "invalid_arguments"
        ) from error
    if any(
        (ord(character) < 0x20 and character not in "\n\t")
        or 0x7F <= ord(character) <= 0x9F
        for character in value
    ):
        raise ActionError(
            f"{field_name} contains control characters", "invalid_arguments"
        )
    return value


def _args(
    value: object,
    *,
    required: set[str],
    optional: set[str] = frozenset(),
) -> dict[str, object]:
    if not isinstance(value, dict):
        raise ActionError("action arguments must be a JSON object", "invalid_arguments")
    if any(not isinstance(key, str) for key in value):
        raise ActionError("action argument names must be strings", "invalid_arguments")
    unknown = set(value) - required - optional
    missing = required - set(value)
    if missing:
        raise ActionError(
            f"missing required argument: {sorted(missing)[0]}", "invalid_arguments"
        )
    if unknown:
        raise ActionError(
            f"unknown argument: {sorted(unknown)[0]}", "invalid_arguments"
        )
    return value


def _nearby(world: World, actor: Character, radius: float) -> list[Character]:
    return world.characters_within(actor, radius, exclude=actor.id)


def _require_interaction_range(actor: Character, other: Character) -> None:
    if (
        actor.position_m.distance_squared(other.position_m)
        > ITEM_INTERACTION_RADIUS_M**2
    ):
        raise ActionError(
            f"{identify(actor, other)} is more than {ITEM_INTERACTION_RADIUS_M:g} metres away",
            "out_of_range",
        )


def _world_event(
    world: World,
    kind: str,
    actor: Character,
    recipients: Sequence[Character],
    *,
    target_id: CharIdStr | None = None,
    item_id: ItemIdStr | None = None,
) -> DomainEvent:
    return world.emit(
        "world_event",
        kind,
        actor.id,
        target_id=target_id,
        item_id=item_id,
        position_m=actor.position_m,
        recipient_ids=[recipient.id for recipient in recipients],
    )


def _notify(recipient: Character, text: str) -> None:
    """Queue private prose only for actors whose scheduler consumes it."""
    if recipient.control == "llm":
        recipient.inbox.append(text)


def _remember_conversation(actor: Character, text: str) -> None:
    """Retain bounded, model-visible dialogue, including the actor's own lines."""
    if actor.control != "llm":
        return
    actor.recent_conversation.append(text)
    overflow = len(actor.recent_conversation) - RECENT_CONVERSATION_MAX_ENTRIES
    if overflow > 0:
        del actor.recent_conversation[:overflow]


def _notify_speech(recipient: Character, text: str) -> None:
    """Deliver new speech and also retain it as short-term conversation context."""
    _notify(recipient, text)
    _remember_conversation(recipient, text)


def apply_action(world: World, actor: Character, verb: str, args: object) -> str:
    """Validate and apply one action, returning its terminal transcript line.

    ``args`` is intentionally typed as ``object``: both model output and
    protocol data are untrusted, and malformed shapes must become ActionError
    rather than escaping as TypeError/KeyError.
    """
    if not isinstance(actor, Character) or world.characters.get(actor.id) is not actor:
        raise ActionError("acting character is not part of this world", "unknown_actor")
    if not isinstance(verb, str):
        raise ActionError("action verb must be a string", "invalid_action")

    if verb == "wait":
        _args(args, required=set())
        return f"{actor.name} waits"

    if verb == "say":
        parsed = _args(args, required={"text"}, optional={"target"})
        text = _text(parsed["text"], "text", PLAYER_SPEECH_MAX_CHARS)
        target_value = parsed.get("target")
        nearby = _nearby(world, actor, HEARING_RADIUS_M)
        target: Character | None = None
        if target_value is not None:
            target_id = _character_id(target_value, "target")
            target = world.characters.get(target_id)
            if target is None:
                raise ActionError(
                    f"there is nobody with id {target_id!r}", "unknown_target"
                )
            if target.id == actor.id:
                raise ActionError("you cannot speak to yourself", "self_target")
            if target not in nearby:
                raise ActionError(
                    f"{identify(actor, target)} is more than {HEARING_RADIUS_M:g} metres away",
                    "out_of_range",
                )

        if target is not None:
            _remember_conversation(
                actor,
                f'You said to {identify(actor, target)}: "{text}"',
            )
            for recipient in nearby:
                if recipient.id == target.id:
                    _notify_speech(
                        recipient,
                        f'{_cap(identify(recipient, actor))} said to you: "{text}"',
                    )
                else:
                    _notify_speech(
                        recipient,
                        f"{_cap(identify(recipient, actor))} said to "
                        f'{identify(recipient, target)}: "{text}"',
                    )
            target_id = target.id
            line = f'{actor.name} -> {target.name}: "{text}"'
        else:
            _remember_conversation(actor, f'You said aloud: "{text}"')
            for recipient in nearby:
                _notify_speech(
                    recipient,
                    f'{_cap(identify(recipient, actor))} said: "{text}"',
                )
            target_id = None
            line = f'{actor.name} (aloud): "{text}"'

        world.emit(
            "speech",
            "say",
            actor.id,
            target_id=target_id,
            text=text,
            position_m=actor.position_m,
            recipient_ids=[recipient.id for recipient in nearby],
        )
        return line

    if verb == "offer_item":
        parsed = _args(args, required={"item_id"}, optional={"target"})
        item_id = _item_id(parsed["item_id"])
        if item_id not in actor.holds:
            raise ActionError(
                f"you hold no item with id {item_id!r} (item_id takes an id, not a name)",
                "not_owner",
            )
        item = world.items.get(item_id)
        if item is None:
            raise ActionError(f"there is no item with id {item_id!r}", "unknown_item")
        target_value = parsed.get("target")
        target: Character | None = None
        if target_value is not None:
            target_id = _character_id(target_value, "target")
            target = world.characters.get(target_id)
            if target is None:
                raise ActionError(
                    f"there is nobody with id {target_id!r}", "unknown_target"
                )
            if target.id == actor.id:
                raise ActionError("you cannot offer an item to yourself", "self_target")
            _require_interaction_range(actor, target)

        old_offer = world.offers.get(item_id)
        new_target_id = target.id if target is not None else None
        nearby = _nearby(world, actor, HEARING_RADIUS_M)
        if (
            old_offer is not None
            and old_offer.target_id is not None
            and old_offer.target_id != new_target_id
        ):
            displaced = world.characters.get(old_offer.target_id)
            if displaced is not None and displaced in nearby:
                _notify(
                    displaced,
                    f"{_cap(identify(displaced, actor))} withdrew the offered "
                    f"{item.name} (id {item_id})",
                )
                _world_event(
                    world,
                    "retract_offer",
                    actor,
                    [displaced],
                    target_id=old_offer.target_id,
                    item_id=item_id,
                )

        if target is not None:
            for observer in nearby:
                if observer.id == target.id:
                    _notify(
                        observer,
                        f"{_cap(identify(observer, actor))} held out a {item.name} "
                        f"(id {item_id}) to you",
                    )
                else:
                    _notify(
                        observer,
                        f"{_cap(identify(observer, actor))} offered a {item.name} "
                        f"to {identify(observer, target)}",
                    )
            line = f"{actor.name} offers the {item.name} to {target.name}"
        else:
            for observer in nearby:
                _notify(
                    observer,
                    f"{_cap(identify(observer, actor))} held out a {item.name} "
                    f"(id {item_id}) to anyone who wanted it",
                )
            line = f"{actor.name} offers the {item.name} to anyone nearby"

        event = _world_event(
            world,
            "offer_item",
            actor,
            nearby,
            target_id=new_target_id,
            item_id=item_id,
        )
        world.offers[item_id] = Offer(item_id, actor.id, new_target_id, event.sequence)
        world.touch_public_state()
        world.assert_invariants()
        return line

    if verb == "accept_offered_item":
        parsed = _args(args, required={"item_id"})
        item_id = _item_id(parsed["item_id"])
        offer = world.offers.get(item_id)
        if offer is None:
            raise ActionError(
                f"nobody is offering you an item with id {item_id!r}", "no_offer"
            )
        if offer.giver_id == actor.id:
            raise ActionError(
                "that is your own offer (retract_offer to withdraw it)", "own_offer"
            )
        if offer.target_id is not None and offer.target_id != actor.id:
            raise ActionError(
                f"nobody is offering you an item with id {item_id!r}",
                "not_offer_target",
            )
        giver = world.characters.get(offer.giver_id)
        if giver is None:
            del world.offers[item_id]
            world.touch_public_state()
            raise ActionError("the person offering it no longer exists", "no_offer")
        _require_interaction_range(actor, giver)
        if item_id not in giver.holds:
            del world.offers[item_id]
            world.touch_public_state()
            raise ActionError(
                f"{identify(actor, giver)} no longer holds that item", "stale_offer"
            )
        item = world.items.get(item_id)
        if item is None:
            del world.offers[item_id]
            world.touch_public_state()
            raise ActionError("the offered item no longer exists", "stale_offer")

        del world.offers[item_id]
        giver.holds.remove(item_id)
        actor.holds.append(item_id)
        nearby = _nearby(world, actor, HEARING_RADIUS_M)
        for observer in nearby:
            if observer.id == giver.id:
                _notify(
                    observer,
                    f"{_cap(identify(observer, actor))} accepted the {item.name} "
                    f"(id {item_id}) you offered",
                )
            else:
                _notify(
                    observer,
                    f"{_cap(identify(observer, actor))} took a {item.name} "
                    f"from {identify(observer, giver)}",
                )
        _world_event(
            world,
            "accept_offered_item",
            actor,
            nearby,
            target_id=giver.id,
            item_id=item_id,
        )
        world.touch_public_state()
        world.assert_invariants()
        return f"{actor.name} takes the {item.name} from {giver.name}"

    if verb == "decline_offer":
        parsed = _args(args, required={"item_id"})
        item_id = _item_id(parsed["item_id"])
        offer = world.offers.get(item_id)
        if offer is not None and offer.target_id is None:
            raise ActionError(
                "that offer is open to anyone, not addressed to you - just ignore it",
                "broadcast_cannot_decline",
            )
        if offer is None or offer.target_id != actor.id:
            raise ActionError(
                f"nobody is offering you an item with id {item_id!r}", "no_offer"
            )
        giver = world.characters.get(offer.giver_id)
        if giver is None:
            del world.offers[item_id]
            world.touch_public_state()
            raise ActionError("the person offering it no longer exists", "no_offer")
        _require_interaction_range(actor, giver)
        item = world.items.get(item_id)
        if item is None:
            del world.offers[item_id]
            world.touch_public_state()
            raise ActionError("the offered item no longer exists", "stale_offer")

        del world.offers[item_id]
        nearby = _nearby(world, actor, HEARING_RADIUS_M)
        for observer in nearby:
            if observer.id == giver.id:
                _notify(
                    observer,
                    f"{_cap(identify(observer, actor))} declined the {item.name} "
                    f"(id {item_id}) you offered",
                )
            else:
                _notify(
                    observer,
                    f"{_cap(identify(observer, actor))} declined a {item.name} "
                    f"from {identify(observer, giver)}",
                )
        _world_event(
            world,
            "decline_offer",
            actor,
            nearby,
            target_id=giver.id,
            item_id=item_id,
        )
        world.touch_public_state()
        world.assert_invariants()
        return f"{actor.name} declines the {item.name} from {giver.name}"

    if verb == "retract_offer":
        parsed = _args(args, required={"item_id"})
        item_id = _item_id(parsed["item_id"])
        offer = world.offers.get(item_id)
        if offer is None or offer.giver_id != actor.id:
            raise ActionError(
                f"you have no pending offer of an item with id {item_id!r}", "no_offer"
            )
        item = world.items.get(item_id)
        if item is None:
            del world.offers[item_id]
            world.touch_public_state()
            raise ActionError("the offered item no longer exists", "stale_offer")

        del world.offers[item_id]
        recipients: list[Character] = []
        if offer.target_id is not None:
            target = world.characters.get(offer.target_id)
            if (
                target is not None
                and actor.position_m.distance_squared(target.position_m)
                <= HEARING_RADIUS_M**2
            ):
                _notify(
                    target,
                    f"{_cap(identify(target, actor))} withdrew the offered "
                    f"{item.name} (id {item_id})",
                )
                recipients.append(target)
        _world_event(
            world,
            "retract_offer",
            actor,
            recipients,
            target_id=offer.target_id,
            item_id=item_id,
        )
        world.touch_public_state()
        world.assert_invariants()
        return f"{actor.name} retracts the offer of the {item.name}"

    if verb == "eat":
        parsed = _args(args, required={"item_id"})
        item_id = _item_id(parsed["item_id"])
        if item_id not in actor.holds:
            raise ActionError(
                f"you hold no item with id {item_id!r} (item_id takes an id, not a name)",
                "not_owner",
            )
        item = world.items.get(item_id)
        if item is None:
            raise ActionError(f"there is no item with id {item_id!r}", "unknown_item")
        offer = world.offers.pop(item_id, None)
        nearby = _nearby(world, actor, HEARING_RADIUS_M)
        if offer is not None and offer.target_id is not None:
            target = world.characters.get(offer.target_id)
            if target is not None and target in nearby:
                _notify(
                    target,
                    f"{_cap(identify(target, actor))} withdrew the offered "
                    f"{item.name} (id {item_id})",
                )
        actor.holds.remove(item_id)
        del world.items[item_id]
        for observer in nearby:
            _notify(observer, f"{_cap(identify(observer, actor))} ate a {item.name}")
        _world_event(world, "eat", actor, nearby, item_id=item_id)
        world.touch_public_state()
        world.assert_invariants()
        return f"{actor.name} eats the {item.name}"

    if verb == "set_goal":
        parsed = _args(args, required={"goal"})
        goal = parsed["goal"]
        if goal is None:
            actor.goal = "None"
        else:
            actor.goal = _text(goal, "goal", 1_000)
        if actor.goal == "None":
            return f"{actor.name} drops their goal"
        return f"{actor.name} now wants: {actor.goal}"

    if verb == "make_sound":
        parsed = _args(args, required={"sound"})
        sound_value = parsed["sound"]
        # Ids only; no name-fallback, per the house rule.
        sound = SOUNDS.get(sound_value) if isinstance(sound_value, str) else None
        if sound is None or not sound.actor_emittable:
            raise ActionError(
                f"there is no sound {sound_value!r}", "unknown_sound"
            )
        if not world.sounds_enabled:
            raise ActionError("sounds are disabled in this world", "sounds_disabled")
        return emit_sound(world, actor, sound)

    if verb == "remember":
        parsed = _args(args, required={"memory"})
        memory = _text(parsed["memory"], "memory", 2_000)
        if memory not in actor.memories:
            actor.memories.append(memory)
        return f"{actor.name} remembers: {memory}"

    if verb == "forget":
        parsed = _args(args, required={"memory"})
        memory = _text(parsed["memory"], "memory", 2_000)
        if memory in actor.memories:
            actor.memories.remove(memory)
            return f"{actor.name} forgets: {memory}"
        for existing in actor.memories:
            if memory in existing or existing in memory:
                actor.memories.remove(existing)
                return f"{actor.name} forgets: {existing}"
        return f"{actor.name} tried to forget something they never knew: {memory}"

    raise ActionError(f"unknown verb: {verb}", "unknown_verb")
