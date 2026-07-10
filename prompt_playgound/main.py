#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["openai", "python-dotenv"]
# ///
"""Tick-loop demo: characters take turns acting on a town square."""

import argparse
import sys

import llm_client
from prompt import parse_reply, render_prompt_and_drain
from sim import Character, CharIdStr, Item, ItemIdStr, Vec3, World, apply_action


def take_turn(world: World, actor: Character, verbose: bool = False) -> None:
    if actor.control != "llm":
        raise ValueError("the player is human-controlled and cannot take an LLM turn")
    prompt = render_prompt_and_drain(world, actor)
    if verbose:
        print(f"--- prompt for {actor.name} ---\n{prompt}", file=sys.stderr)
    reply = llm_client.complete(prompt)
    if verbose:
        print(f"--- reply from {actor.name} ---\n{reply}", file=sys.stderr)

    actions, errors = parse_reply(reply)
    for err in errors:
        actor.inbox.append(f"system: your last output was invalid: {err}")
        print(f"  [!] {actor.name}: {err}", file=sys.stderr)
    for verb, args in actions:
        try:
            line = apply_action(world, actor, verb, args)
        except Exception as e:
            actor.inbox.append(f'system: your action "{verb} {args}" failed: {e}')
            print(f"  [!] {actor.name}: {verb} failed: {e}", file=sys.stderr)
            continue
        world.transcript.append(line)
        print(f"  {line}")


def build_world() -> World:
    forecourt = "On the grand forecourt just outside the cathedral's west entrance"
    world = World()
    world.add(Item(id=ItemIdStr("fzbn9"), name="fish", visual_key="fish"))
    world.add(Item(id=ItemIdStr("c0prs"), name="copper coin", visual_key="copper_coin"))
    world.add(
        Character(
            id=CharIdStr("sv3n1"),
            name="Sven",
            control="llm",
            back_story=(
                "Born poor, you are now a blacksmith apprentice. You live in a "
                "large citystate surrounding a large cathedral, and you work in "
                "one of the back streets."
            ),
            location_description=forecourt,
            position_m=Vec3(-1.8, 0.91, 114.0),
            appearance_key="sven",
            voice_key="sven",
            holds=[ItemIdStr("fzbn9")],
            memories=["I'm going to get some fish"],
            knows={CharIdStr("cb947")},
        )
    )
    world.add(
        Character(
            id=CharIdStr("cb947"),
            name="Conny",
            control="llm",
            back_story=(
                "A fisherman who sells his catch on the town square. You know "
                "most faces in the quarter, including Sven, the blacksmith's "
                "apprentice."
            ),
            location_description=forecourt,
            position_m=Vec3(0.0, 0.91, 112.0),
            appearance_key="conny",
            voice_key="conny",
            memories=["Sven still owes me two coppers for that fish"],
            knows={CharIdStr("sv3n1")},
        )
    )
    world.add(
        Character(
            id=CharIdStr("k0fb1"),
            name="Ilse",
            control="llm",
            back_story=(
                "A pilgrim who arrived in the citystate this morning to see the "
                "great cathedral. You know nobody here"
            ),
            location_description=forecourt,
            position_m=Vec3(1.8, 0.91, 114.0),
            appearance_key="ilse",
            voice_key="ilse",
            holds=[ItemIdStr("c0prs")],
            memories=["I am very hungry after the long road here"],
        )
    )
    world.add(
        Character(
            id=CharIdStr("player"),
            name="Player",
            control="player",
            back_story="A human visitor exploring the cathedral city.",
            location_description=forecourt,
            # The hello message replaces this with the controller's true spawn.
            position_m=Vec3(0.0, 0.91, 68.0),
            appearance_key="player",
            voice_key=None,
            knows={
                CharIdStr("sv3n1"),
                CharIdStr("cb947"),
                CharIdStr("k0fb1"),
            },
        )
    )
    world.assert_invariants()
    return world


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "-t", "--ticks", type=int, default=6, help="number of turns to run (default 6)"
    )
    ap.add_argument(
        "-v",
        "--verbose",
        action="store_true",
        help="dump prompts and raw replies to stderr",
    )
    args = ap.parse_args()

    world = build_world()
    order = [actor for actor in world.characters.values() if actor.control == "llm"]
    for tick in range(args.ticks):
        actor = order[tick % len(order)]
        print(f"\n== tick {tick + 1}: {actor.name} ==")
        take_turn(world, actor, verbose=args.verbose)

    print("\n== final state ==")
    for c in order:
        known = [
            world.characters[i].name for i in sorted(c.knows) if i in world.characters
        ]
        holds = [f"{world.items[i].name} ({i})" for i in c.holds]
        print(f"{c.name}: goal={c.goal!r}, knows={known}, holds={holds}")
        for m in c.memories:
            print(f"  - {m}")
    for item_id, offer in world.offers.items():
        to = world.characters[offer.target_id].name if offer.target_id else "anyone"
        print(
            f"pending offer: {world.characters[offer.giver_id].name} offers "
            f"{world.items[item_id].name} ({item_id}) to {to}"
        )

    cost = llm_client.run_cost_usd()
    if cost is None:
        print("\nRun cost: unknown (no pricing entry for this model)")
    else:
        print(
            f"\nRun cost: {cost:.2f} USD"
            if cost >= 0.005
            else f"\nRun cost: {cost:.4f} USD"
        )


if __name__ == "__main__":
    main()
