# The Light Rules: A Testable Specification of the Second Sun

> Use: design spec for developers — the observable rules of the Second Sun stated as acceptance criteria a graphics programmer can implement from and a tester can verify by eye and F5 screenshot; nothing in this document is implemented.
> Canon: 00_canon.md

## 0. Status and scope

**This is a proposal. Nothing below exists in the game.** No render layer, no second light, no interior gate, no Concurrence clock. This document turns the observable canon (00_canon.md §2) into numbered, testable rules with player-visible proofs. Where it decides something canon is silent on, the decision is marked **[spec decision]** and binds later documents unless canon is amended.

Two principles govern everything:

1. **The phenomenon is identical forever.** Two hundred years of in-world records describe it without variance; therefore no randomness, no drift, no per-save variation. Same hour, same pane, same colour, every playthrough. Determinism is the horror.
2. **Fail dark.** Wherever the implementation must choose between wrongly showing and wrongly hiding — threshold straddles, reflection probes, portal edges — it hides. An absence inside the Lanthorn is a defect; a presence outside it breaks canon §2.5 ("one sun, one shadow, always") and cannot be walked back.

## 1. Definitions and datum points

- **The Lanthorn interior volume** — the enclosed air of the Great Church of Saint Ambrelle: nave, aisles, crossing, transepts, choir, triforium and clerestory walks. Excludes the crypt, the porches, and the Gradine.
- **The sill** — the threshold plane of each exterior door. The interior volume ends here, exactly.
- **Rose-UV** — the two-dimensional coordinate space of the Great Rose, pane by pane. **The eye** is its central round light (canon §1).
- **The mid-stone** — a worn octagonal slab at the centre of the crossing floor **[spec decision — minted]**: the viewing datum for the Concurrence and all "seen from the crossing" criteria. Four centuries of feet have dished it.
- **The paying bench** — verger furniture against the north arcade, third bay from the west doors **[spec decision — minted]**: angled so a seated adult's sightline passes through the lower lights of the rose to open sky. Verger Dunstan Pike positions it; pilgrims pay; the placement is discovery choreography (§9).
- **Hours.** *Dawn* (sunrise), *the dawn-showing* (first hour after sunrise), *forenoon*, *noon*, *the strong hour* (late afternoon, both suns in the rose), *the Passing* (the daily crossing; its time walks through the year), *last light* (sunset), *night*.
- **The grave-light** — the implementation's name for the second, interior-gated illumination source; its shadow is the **grave-shadow** (canon §9.3). Never surfaced in UI or dialogue; NPCs use the canon vocabulary (Emblem / Green Sun).

## 2. Rule L1 — Visibility

**L1.1 The one condition.** The disc renders when and only when all of the following hold: (a) the camera is inside the Lanthorn interior volume; (b) the sightline from the camera passes through glass currently leaded into the Great Rose; (c) beyond that glass lies sky — clear or clouded, but sky, not scaffold planking, canvas, or masonry; (d) the hour is between dawn and last light.

**L1.2 Any pane.** Every pane of the rose transmits it equally: coloured, grisaille, or plain; original Sparr glass or new glass leaded in yesterday (canon §2.2, truth #10). No panel is privileged. The eye matters only as the Concurrence target (L6), not as a transmission condition.

**L1.3 Gaps show nothing.** A missing pane, an open light, a hole in the glass: the gap shows one ordinary sky. The disc never bridges a gap; its edge clips exactly to the leading **[spec decision: hard clip, no feathering]**. This is the broken-pane fact that makes witnesses go quiet, and it must survive close inspection from the triforium walk.

**L1.4 Sky occlusion.** Canvas or scaffold planking hung outside a pane blanks that pane's share of the disc while the pane stays visible. This is the Custody's only off-switch (the Fabric pretext, canon §6.5) and must work per-pane, since the F.419–421 Re-leading proceeded light by light.

**L1.5 Other windows.** All other glass in the world shows one sky. A rose pane carried elsewhere shows nothing (canon §2.2). Replica glass shows nothing anywhere.

**L1.6 Nothing else doubles.** One moon, single stars, single flames (canon §2.10). The grave-light pairs only with the sun; candlelight, torchlight, and moonlight never gain a green twin.

**Proof.**
- Standing on the mid-stone at forenoon on a clear day, the player sees a green-white disc in one pane of the rose, and one ordinary sun through any side window they turn to.
- Standing on the triforium walk beside the rose at forenoon, with one pane removed for "repair", the player sees the disc clipped dead against the leading and plain sky through the hole.
- Standing on the Gradine at the same minute, looking up at the rose from outside, the player sees ordinary glass and one sun.

## 3. Rule L2 — Colour and intensity

**L2.1 The colour.** A cold green-white — canon's swatch is "dawn seen through river ice". One authored chromaticity, fixed forever; it must read as green-white against every tint of rose glass and against gold nave light. No time-of-day tinting, no atmosphere scattering applied to the disc itself.

**L2.2 It ignores the pane.** Sky and cloud seen through a red pane redden; the disc in that same pane stays green-white (canon §2.3, truth #2). The disc is composited after glass tinting, not before. This is the first wrongness witnesses name and the single most important pixel-level criterion in this spec.

**L2.3 It permits the eye.** Looking straight at the disc costs nothing: no bloom flare, no exposure crush, no eye-adaptation punishment; its luminance clamps near paper-white **[spec decision]**. Looking at the true sun through a clear pane must still punish — full bloom and adaptation — so the contrast is felt in the player's own flinch. The permission unsettles more than the colour (truth #3).

**L2.4 Size.** At the dawn-showing the disc subtends roughly twice the true sun's diameter, low in the western lights; it tapers as it climbs, and by the Passing it matches the true sun closely enough that at the Concurrence the two discs superpose edge to edge **[spec decision]**. The taper is the same every day of a given season.

**Proof.**
- Standing on the paying bench at forenoon, the player sees the disc hold its green-white while the clouds around it take the red of the pane it stands in.
- Standing anywhere in the nave, staring at the disc costs nothing; turning to the true sun through a clear side pane punishes at once.

## 4. Rule L3 — Anchoring and the countergait

**L3.1 No parallax.** At any instant the disc occupies one position in rose-UV, identical for every eye in the building (canon §2.6, truth #6). Implementation shape: the disc is anchored to the glass, not to a sky direction — a rose-space element, not a skybox element. A walker crossing the nave watches the true sun slide from pane to pane; the disc does not move by one lead-width.

**L3.2 The countergait.** The disc's rose-UV path is authored per season and runs against the true sun's course (canon §2.7):

- **Dawn.** As the true sun crests at the city's back (east, behind the choir), the disc brightens with the dawn sky — no pop-in — already low and huge in the western rose: the dawn-showing, the most crowded hour.
- **Day.** It climbs the lights against the sun's course while the true sun crosses the southern sky.
- **The strong hour.** Late afternoon, the true sun comes round into the west; both stand in the rose.
- **The Passing.** Once daily their apparent positions cross, as judged from the mid-stone. The crossing point walks through the year; only on the Concurrence does it land in the eye.
- **Last light.** The disc fades with the failing sky. Never seen at night (L5.1).

**L3.3 Beams follow a virtual source.** Though the visible disc is pane-anchored, its beams enter the nave as if cast from the disc's stated position and sweep the floor with the hour (canon §2.6). The beam direction at every hour must contradict the true sun's: at the dawn-showing the green beams run west to east down the whole nave axis while true dawn light enters from the choir end; at the strong hour both enter from the west at differing pitch and skew, which is what draws the X (L6.2).

**Proof.**
- Standing on the mid-stone at forenoon and then walking to the west doors, the player sees the true sun cross three panes and the disc stay fixed in its one pane the whole walk.
- Standing at the dawn-showing anywhere on the nave axis, the player sees a huge low disc in the western rose behind them and its long green beams running toward the altar, against the true dawn.
- Standing at the strong hour on the mid-stone, the player sees both suns in the rose at once.

## 5. Rule L4 — The light indoors

**L4.1 What doubles a shadow.** A doubled shadow is cast only by the pair {true sun, grave-light}. Any shadow-caster lit by both — pillar, NPC, dropped item, the player's body or shadow proxy — casts one warm true shadow and one faint, cold, differently aimed grave-shadow. Objects lit by neither, or by candles alone, cast singles (L1.6).

**L4.2 Where it reaches.** The grave-light illuminates exactly the interior points with an unobstructed line to the rose glass — shadow-mapped from the virtual source of L3.3. Side chapels around corners get none; the crypt gets none; step fully into a pier's own grave-shadow and your second shadow vanishes while your true one stays. Doubling is strongest inside the direct beam footprints on the nave floor; a weak green fill within line-of-sight of the rose keeps second shadows faint but findable elsewhere **[spec decision]**; falloff to zero outside line-of-sight, hard.

**L4.3 No mixing.** Where a green pool crosses a true sunbeam pool, the two overlap without blending into a third colour: each pool keeps its edge and colour inside the other's light, and each shadow keeps its edge and colour inside the other's pool (canon §2.4, truth #4). Implementation latitude is total; the acceptance is pixel-blunt: nowhere in the overlap may a yellow-green mongrel tone appear.

**L4.4 No warmth.** The grave-light couples to nothing physical: no heat, no wax-softening, no gameplay stat, ever. If a candle or wax simulation is added, green beams are excluded from it by rule. The dread is that it does nothing — except be seen.

**Proof.**
- Standing in a green beam footprint at forenoon, third bay of the nave, the player sees two shadows of the nearest pillar diverging on the floor, one warm, one cold.
- Standing at the strong hour where the beams cross, the player sees gold and green pools overlapped, each shadow keeping its own edge inside the other's light, and no mixed tone anywhere along the seam.
- Standing behind the north-west crossing pier, out of line-of-sight of the rose, the player sees their second shadow gone and their first intact.

## 6. Rule L5 — Night, cloud, storm; Rule L6 — the Concurrence

**L5.1 Night.** Nothing. The disc keeps the sun's hours exactly. The night-storm apparition of canon §2.9 is AMBIGUOUS-BY-DESIGN and is **never rendered, under any condition** — it exists only in NPC testimony, and no two testimonies agree. Do not build an easter egg. There is no flag for it to hide behind.

**L5.2 Overcast.** When cloud smothers the true sun, the disc persists as a pale coin burning through the overcast ("the clouds do not know it"), and its beams persist into the nave at reduced strength — during the Great Rains of F.362 the only sunlight in the city stood in the nave, and that scene must be reproducible. Under full overcast, nave shadows are single and green: there is no true shadow to double.

**L5.3 Daytime storm.** When lightning whitens every pane of the rose for a frame (a nod to unimplemented storm ideas; do not depend on them), the disc alone does not brighten: its luminance is exempt from the flash exposure, a steady cold coin in a sheet of white (canon §2.9, truth #8).

**L6 The Concurrence** — the fortieth day after Coswaldstide, once a year, weather permitting. Let T0 be the instant the Passing centres in the eye as judged from the mid-stone. Minute by minute, clear sky:

- **T−30.** The strong hour opens; both suns in the rose; the nave fills (or the doors shut — F.436 is the live fuse, not this spec's business).
- **T−12.** The crossed beams lay their X down the nave floor; through these minutes the X's arms close like scissors.
- **T−6.** The discs stand within one pane of each other; every grave-shadow in the nave is visibly rotating toward its true twin.
- **T−3.** Both discs enter the eye. From the mid-stone they close edge to edge.
- **T−2 to T+2 — the fusion plateau.** The two lights superpose. Disc superposition is exact only from the mid-stone (the true sun keeps its parallax; the disc keeps its anchor). Shadow fusion is directional and therefore nave-wide: the grave-light's beam direction aligns with the true sun's, the footprints fall congruent, and every doubled shadow in the building fuses into one **[spec decision: superposition is viewpoint-exact at the datum; fusion is global]**. Within the congruent pool no mongrel tone may appear.
- **T+3.** The discs part; across the nave every shadow splits again, all at once.
- **Clouded Concurrence.** If overcast smothers the true sun through the window T−3 to T+3, the pale disc burns alone in the eye, there is nothing for its shadow to fuse with, and the feast simply fails (truth #7). No partial credit.

**Proof.**
- Standing on the mid-stone at T0 on a clear Concurrence, the player sees one disc in the eye and casts one shadow; screenshots at T−6 and T0 differ by exactly the fused shadows and the closed X.
- Standing in the third bay (off-datum) at T0, the player sees two discs almost, not quite, one — and still casts one shadow.
- Standing on the mid-stone at T0 under cloud, the player sees the pale coin alone in the eye and the nave dark: the feast has failed.

## 7. Rule L7 — Flight, thresholds, reflections, screenshots

**L7.1 The sill is absolute.** Crossing any exterior door line, the disc and all grave-light vanish between one step and the next — instantaneous, no fade **[spec decision: the pop is canonical; witnesses describe the pools "going out like a snuffed wick"]**. Re-entering restores them identically.

**L7.2 Looking in from outside.** From the Gradine through open west doors, the nave floor shows true sunlight only: the grave-light renders only for a camera inside the interior volume. Two people straddling the sill disagree about what the floor shows; that is the phenomenon's nature, not a bug **[spec decision, supported by canon §2.5 and the Shut Door crowds wanting in]**.

**L7.3 Flight outside.** A flying player over the rooftops finds one sun and nothing else, forever (canon §2.5, §3f). Looking back at the rose from the air: ordinary glass, no green glow on the west front, nothing on the Cut. There is nothing to find outside, at any altitude, and the implementation must make that boring rather than glitchy.

**L7.4 Flight inside and degenerate cases.** Flying within the nave, the rules hold unchanged (the triforium criteria already assume elevated eyes). For a camera straddling glass, clipping the window plane, or hovering in a doorway: apply the interior gate with a hysteresis band of about half a metre, and inside the band **fail dark** — never a frame of the disc rendered to an exterior eye. An inside player who briefly sees one sun has met a defect the fiction can absorb; an outside player who sees two has broken the bible.

**L7.5 Reflections.** The beams and pools are ordinary light indoors and shade as light. The disc follows the sightline rule: in principle a mirror inside the Lanthorn that hands the eye a path through rose glass to sky shows it (canon kills the mirror only when carried out the door). But the game plans no true mirrors, so screen-space and probe reflections simply exclude the disc's layer: a missing reflection is canon-safe, a wrong-placed one is not. Normative for any future reflective surface, including standing water indoors.

**L7.6 Screenshots.** F5 and drive-mode screenshots capture exactly what the camera renders: the disc appears in every inside shot with a rose sightline and in no outside shot, ever. Canon §1 makes the phenomenon "verifiable by eye and F5 screenshot" — the capture path must not exclude the layer.

**Proof.**
- Standing one pace inside the west doors at forenoon, the player sees green pools on the floor; one pace outside, at the same second, the floor is gold only.
- Flying a circuit of the west front at the strong hour, the player sees ordinary glass in the rose from every angle and one sun in every screenshot.
- Standing at the crossing and taking an F5 screenshot at the Passing, the player finds the X and both discs in the file.

## 8. NPC sight

The authoritative sidecar, not the renderer, decides what NPCs perceive; it must apply L1 verbatim: any NPC inside the volume with a rose sightline during sun-hours is told the Emblem is visible and in which pane (one pane for all, L3.1). This enables the two-eyes test: the player at the mid-stone and an NPC on the triforium name the same pane for the disc and different panes for the true sun — Doctor Ferrant's broken measurement performed live.

The one exception is the Spared (canon §9.8), whose testimony is AMBIGUOUS-BY-DESIGN (canon §3f): whether they truly cannot see it, or lie, is never resolved. But a perception feed must still do *something* for a Spared-flagged actor (the corpus instantiates at least one, in 05_dramatis_personae), and either naive choice — sending the sight line or a bare silence — hands an unguarded model a fact it will resolve out loud. So the spec decides the mechanism without deciding the answer **[spec decision]**:

- **No sight events.** A Spared-flagged actor receives no Emblem sight event, ever. This is fail dark (§0) applied to perception: withholding asserts nothing — an actor told of the sight has been told they see; an actor told nothing has merely been told nothing.
- **A seed guard.** The Spared actor's prompt seed carries an unwavering, itself-ambiguous self-model — to the effect of: *you have never seen it; whether your eyes or your tongue differ from your neighbours', you yourself do not know — never waver, never explain, never wonder aloud which it is.* The guard exists because the absence of sight events is, to a model, still an input; without it the actor resolves the question in dialogue and breaks canon §3f.
- **The flag never leaks.** The flag's own name is mechanism and follows the grave-light rule (§1): never surfaced in UI or dialogue, and — since players read save files and telemetry too — never keyed as anything that answers the question (`cannot_see`, `lies_about_sight`). Key it for the testimony, not the cause.

## 9. Discovery choreography

No UI, no marker, no tutorial line. The building teaches it, in this order: floor, then window, then door.

**The first ten minutes.** A first-time player entering the nave in daylight should cross a green beam footprint without trying: the forenoon footprint of the third bay lies across the natural walking line from the west doors toward the crossing **[spec decision]**, so a doubled shadow — their own, a pillar's, an NPC's — happens underfoot before any window is examined. Three standing cues sit within the first thirty metres: an NPC stopped mid-nave, head tilted, gaze locked on the rose (players follow gazes); a child playing twinning in the beam, contorting so her two shadows touch hands; and the paying bench, angled so that sitting frames the disc in the lower lights with no scripting at all. The intended first arc is three beats: notice the two shadows; look up and find the wrong-coloured disc you can stare at; step outside and find one sun — the door pop is the punchline, and it should land inside the first session.

**The first ten hours.** The city deepens what the building began. The player hears the vocabulary split (Emblem in edicts, Green Sun in the streets, and the citable cost of the second word); catches a wick-priest's absolution — "So do we all, child; affirm nothing"; learns the crowd clock (dawn-showing thickest, the Passing's X drawing gasps); hears Crier Jos Brant's warm folk line about the dead hearing their names in the beam; and, if observant, notices a chalked diamond at the Needle without yet owning what it means. Somewhere in these hours an NPC should perform the broken-pane fact or the two-eyes test within earshot, so the player inherits Ferrant's vertigo before meeting him. None of this needs quest scaffolding; it needs the light rules to be true every single day, because the discovery design is nothing but the rules, witnessed.

## 10. Failure policy, summarised

| Situation | Resolve toward |
|---|---|
| Camera straddling sill or glass | Absence (fail dark, hysteresis) |
| Reflection probes, SSR, water | Exclude the disc layer |
| Any doubt about "is this camera inside?" | Outside |
| Night-storm apparition | Never rendered |
| Disc missing for an inside eye | Defect — fix, but fiction survives |
| Disc or grave-light shown to an outside eye | Canon break — release blocker |

The Second Sun never performs. It is a fixed, cold, daily fact, and every clause above exists so that it can be checked like a fact: same pane for every eye, same colour through every glass, gone at every door. Keep it quiet, and keep it cold — and keep it deterministic.
