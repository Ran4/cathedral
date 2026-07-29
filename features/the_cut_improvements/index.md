(this is a collection document for other the cut features)

These are some suggestions for improvements to the cut.

The Cut is 730 m long, 11 m wide, dead straight, x ≈ -213.5, z from +325 to -406 — the longest single thing in the city. What's on it now (pre any of the new features described below):

- One actor has a leg at "The Cut" in rounds.json: Tam Rud, fuller. Everyone else is at the Tallage (z = 63) or Maren's Green (z = -255), which are squares that happen to sit on it.
- One prop cluster: build_ropewalk — six posts and three wires, and they're at x = -182, thirty metres off the street, behind the housefronts.
- A sound bed — CutFreightCorridor, doc-commented "Carts and porters along the old river bed."

That last one is the whole problem in one line. The soundscape is already lying. You stand there hearing freight on a street with no freight, no carts, no porters, no kerb, and two wandering strangers. The gazetteer promises "warehouse doors, blocked water stairs, cellar vents, hoists,
awnings, counting rooms" — none of it is built. What you get is a wide dirt road with generic gables on both sides for 200 m at a stretch.

Why it's empty, structurally

1. The sim puts people at named legs, and the Cut is a corridor, not a destination. Nothing in the round system generates presence along a line. Everyone who "uses" the Cut is modelled as being at either end of it.
2. There's no line drawn on the ground, so no rule can be broken there. The lore's central fact about the Cut is that the cartway must stay clear and the Bench issues obstruction notices. But there's no kerb, no margin, no visible cartway — so there's nothing to encroach on and nothing
to enforce.
3. Its one unique asset is unused. It's the only straight street in a city that bends. You can see 700 m down it. The game never once asks you to look.

## Features to implement

1. features/the_cut_improvements/the_cut_kerb.md Draw the kerb. Cheapest thing here and it unlocks two others. Kerbstones, raised bank thresholds, the marked stall margins the gazetteer already specifies. Right now the street is undifferentiated ground; with a kerb it becomes cartway + margin, and "keep the middle clear" is
suddenly a visible rule you can see people obeying and breaking.

2. features/the_cut_improvements/the_cut_dry_carry.md

3. features/the_cut_improvements/obstruction_as_a_live_lop.md Obstruction as a live loop. notices.rs ships, raise_notice is law-only, Segwin Mott's south beat already walks Maren's Green → Tallage → Maren's Green — i.e. he already walks the Cut and does nothing there. Give him something: goods creeping into the cartway, a rope across it, a stall over the margin. You can be the cause (drop something and be told to move it) or the fix (help a porter clear before the bell). Nearly free given what's built.

4. features/the_cut_improvements/differentiate_different_parts_of_the_cut.md

5. features/the_cut_improvements/poling_dry_game.md — resolves the parked features/design_the_cut_game.md. Walkthrough video: architecture/movies/poling_dry.mp4. The Cut game — and I'd pick hoop-poling, named "poling dry". features/design_the_cut_game.md is parked with four seeds. The gameplay argument settles it: hoop-poling is the only one that could not be played anywhere else in Ombreval. A barrel hoop rolled down the camber runs 100 m+ and the whole street
watches it the whole way — that's the wager. Every other street in the city bends and kills the run. And it lands exactly on top of idea 3: the game blocks the cartway, so the game and the obstruction notice are the same mechanic pointed two ways. That's precisely why "enforcement of
the Cut game" is the kind of thing a ward election swings. Player verbs: bet a spark, call a lane, roll. The Tarns supply the hoops and Gude Tarn already canonically runs an illegal game.

6. features/the_cut_improvements/later_ideas_for_the_cut.md - Later.
