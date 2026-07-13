"""The sound catalog: the single source of truth for three consumers.

The sim emits `sound` events and renders their percepts from these rows; the
generator (`scripts/generate_sounds.py`) synthesizes each row's asset from
`sfx_prompt`; Bevy resolves playback by convention from the id alone
(`assets/sounds/{sound_id}.mp3`), so no filename ever crosses the wire.

Full design: `../features/sounds.md` (and design/06 for the wider soundscape).
"""

from __future__ import annotations

import re
from dataclasses import dataclass

# Also the asset basename, so it must stay filesystem- and wire-safe.
SOUND_ID_RE = re.compile(r"^[a-z_]+$")


@dataclass(frozen=True, slots=True)
class Sound:
    sound_id: str  # [a-z_]+ — also the asset basename
    sound_class: str  # body | impact | bell   (design/06's `class`)
    audible_distance: float
    heard: str  # unattributed percept — everyone in radius
    seen: str | None  # attributed percept; None => not attributable
    sfx_prompt: str  # ElevenLabs generation prompt
    duration_seconds: float
    actor_emittable: bool  # may an LLM choose this via make_sound?

    def __post_init__(self) -> None:
        if not SOUND_ID_RE.fullmatch(self.sound_id):
            raise ValueError(f"invalid sound id {self.sound_id!r}")
        if self.seen is not None and "{actor}" not in self.seen:
            raise ValueError(f"attributable sound {self.sound_id!r} needs {{actor}}")


@dataclass(frozen=True, slots=True)
class AmbientSound:
    """A looping scene asset. Generated from this table, never simulated:
    ambient loops are plain Bevy `AudioPlayer`s and emit no events."""

    sound_id: str
    sfx_prompt: str
    duration_seconds: float

    def __post_init__(self) -> None:
        if not SOUND_ID_RE.fullmatch(self.sound_id):
            raise ValueError(f"invalid ambient sound id {self.sound_id!r}")


def _sound(sound_id: str, **kwargs: object) -> tuple[str, Sound]:
    return sound_id, Sound(sound_id=sound_id, **kwargs)  # type: ignore[arg-type]


SOUNDS: dict[str, Sound] = dict(
    (
        _sound(
            "fart",
            sound_class="body",
            audible_distance=20.0,
            actor_emittable=True,
            seen="{actor} farted.",
            heard="[You heard a big fart!]",
            sfx_prompt=(
                "A single loud comedic wet fart, one short burst, close and dry, "
                "no music, no voices, no reverb"
            ),
            duration_seconds=1.5,
        ),
        _sound(
            "glass_break",
            sound_class="impact",
            audible_distance=25.0,
            actor_emittable=True,
            seen="{actor} broke a beer glass.",
            heard="[You heard glass shatter nearby.]",
            sfx_prompt=(
                "A single beer glass shattering on a stone floor, sharp impact and "
                "scattering shards, medieval tavern room tone, no voices"
            ),
            duration_seconds=2.0,
        ),
        _sound(
            "town_bell",
            sound_class="bell",
            audible_distance=600.0,
            actor_emittable=False,
            seen=None,
            heard="[The town bell is ringing.]",
            sfx_prompt=(
                "A massive bronze cathedral bell tolling three slow strokes, deep "
                "and resonant, carrying over a medieval city, long natural decay, "
                "no music"
            ),
            duration_seconds=9.0,
        ),
    )
)

# Read only by the generator; the sim never sees these (an inbox line per
# fireplace crackle would be token suicide — design/06 §5).
AMBIENT: dict[str, AmbientSound] = {
    "fireplace": AmbientSound(
        sound_id="fireplace",
        sfx_prompt=(
            "A steady crackling fireplace, gentle wood pops over a warm even "
            "room tone, constant level, seamless loop, no voices, no music"
        ),
        duration_seconds=12.0,
    ),
}


def emittable_sound_ids() -> list[str]:
    return [sound_id for sound_id, sound in SOUNDS.items() if sound.actor_emittable]
