#!/usr/bin/env -S uv run --script
"""Review aid: compare authoritative owner fields with the M0 source inventory.

This is not a Rust parser or checkpoint serializer. It indexes named, one-line
field declarations in the selected simple structs; nested field/time policies
are in PERSISTENCE_INVENTORY.md. --write deliberately updates the review baseline.
"""
import argparse
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[5]
DEST = Path(__file__).with_name('m0_baseline') / 'owner_fields.json'
OWNERS = {
    'crates/cathedral-sim/src/world.rs': ['World'],
    'crates/cathedral-sim/src/character.rs': ['CharacterState'],
    'crates/cathedral-sim/src/engine.rs': ['Engine'],
    'crates/cathedral-sim/src/round.rs': ['Round', 'Townsperson'],
    'crates/cathedral-sim/src/round/residents.rs': ['Residents', 'Resident', 'ResidentWeather'],
    'crates/cathedral-sim/src/nav/residents.rs': ['SpotReservations', 'SpotClaims'],
    'crates/cathedral-sim/src/knowledge/mod.rs': ['Knowledge', 'Fact', 'Holding', 'LearnedHow', 'Occasion'],
    'crates/cathedral-sim/src/conversation.rs': ['Conversation', 'Engagement', 'CapturedAttention'],
    'crates/cathedral-sim/src/scheduler.rs': ['NpcScheduler', 'InFlight'],
    'crates/cathedral-sim/src/speech_router.rs': ['SpeechRouter', 'TranscriptionTask', 'StreamState', 'ParkedRecording'],
    'crates/cathedral-sim/src/night.rs': ['NightOffice'],
    'crates/cathedral-sim/src/floor.rs': ['ConversationFloor'],
}


def inventory():
    result = []
    for source, owners in OWNERS.items():
        lines = (ROOT / source).read_text().splitlines()
        for owner in owners:
            starts = [i for i, line in enumerate(lines)
                      if re.search(r'\bstruct ' + owner + r'\s*\{', line)]
            if len(starts) != 1:
                raise RuntimeError(f'expected exactly one {source}::{owner}')
            fields = []
            for line in lines[starts[0] + 1:]:
                if line == '}':
                    break
                match = re.match(r'^    (?:pub(?:\([^)]*\))? )?(\w+):\s*(.*)', line)
                if match:
                    fields.append({'field': match[1], 'type_declaration': match[2]})
            if not fields:
                raise RuntimeError(f'no fields in {owner}')
            result.append({'source': source, 'owner': owner, 'fields': fields})
    return {'schema': 1, 'scope': 'Selected private owner declarations; review against PERSISTENCE_INVENTORY.md, not serialization or automatic approval of new fields', 'owners': result}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write', action='store_true')
    args = parser.parse_args()
    result = inventory()
    if args.write:
        DEST.parent.mkdir(exist_ok=True)
        DEST.write_text(json.dumps(result, indent=2) + '\n')
    elif json.loads(DEST.read_text()) != result:
        raise SystemExit('Owner fields changed: review persistence/time policies before updating the inventory')
    print(f'{len(result["owners"])} owners, {sum(len(o["fields"]) for o in result["owners"])} fields indexed; private DTO implementation remains pending')


if __name__ == '__main__':
    main()
