# 50 Cool Feature Suggestions

Ideas for *The Cathedral-City of Impossible Light* — brainstormed across eight lenses
(atmosphere, smart actors, gameplay, procedural generation, sound, exploration,
narrative, experimental) and curated down to the strongest 50. Suggestions only;
nothing here is implemented.

The list leans hard on what makes this game unique — the open microphone, the LLM actors, and the "impossible light" premise — rather than open-world boilerplate. A taste of the top
  entries:

- The Second Sun — a wrong-colored second sun visible only through the rose window, with a heretic faction that has noticed.
- The Heresy of Flight — NPCs witness you using flight and their LLMs decide whether it was a miracle or demonry, feeding the gossip network.
- Breath-Bent Candleflame — your real microphone input physically disturbs candle flames; shouting snuffs votives.
- The City Names You — actors collectively coin and spread an emergent epithet for you instead of a reputation bar.
- The Anchoress in the Wall — a voice-only NPC bricked into the cathedral who trades lore for spoken descriptions of the city, with the LLM judging your honesty.
- The Campanile Simulation — physically modeled bells across the city as clock, map, and navigation system.

# Tierlist (by me)

## S

- **1. The Second Sun** — *Atmosphere & Light*
- **3. Rumor Drift** — *Smart Actors & Social Simulation*

## A

- **10. The Campanile Simulation** — *Sound & Acoustics*
- **40. Postcard Seeds** — *Procedural Generation*
- **35. Every Door Opens** — *Procedural Generation*
- **16. Trial by Earshot** — *Narrative & Quests*
- **21. The Lamplighter of the Five Squares** — *Atmosphere & Light*
- **30. The Errand of Words** — *Smart Actors & Social Simulation*
- **24. Cry Your Wares** — *Gameplay & Systems*
- **46. The City Keeps Real Time** — *Experimental & Wild*

## B

- **4. Breath-Bent Candleflame** — *Atmosphere & Light*
- **31. Steam, Seal, and Lie** — *Narrative & Quests*
- **5. The Light That Falls Wrong** — *Narrative & Quests*
- **11. The Anchoress in the Wall** — *Narrative & Quests*
- **22. Sworn by the Bell** — *Gameplay & Systems*
- **12. Whispered or Shouted** — *Smart Actors & Social Simulation*
- **49. Ask the Master** — *Gameplay & Systems*
- **13. The Confessional** — *Smart Actors & Social Simulation*
- **15. The Vespers Sermon** — *Smart Actors & Social Simulation*
- **34. The Quarter's Temperament** — *Procedural Generation*
- **33. The Unfinished Spire** — *Procedural Generation*
- **17. Palimpsest City** — *Procedural Generation*
- **23. The Tilewalkers' Guild** — *Gameplay & Systems*
- **25. Voces Sacrae: Voice-Spoken Words of Power** — *Sound & Acoustics*
- **26. The Choir That Learns Your Words** — *Sound & Acoustics*
- **27. Organum: The Playable Great Organ** — *Sound & Acoustics*
- **39. The Gargoyle Way** — *Exploration & Secrets*
- **47. Procession of the Broken Bell** — *Narrative & Quests*
- **36. Storm of White Glass** — *Atmosphere & Light*
- **45. Plague Ledger** — *Experimental & Wild*
- **42. The Long Memory of Bells** — *Experimental & Wild*
- **50. The Blind Pilgrim** — *Experimental & Wild*

## NoWay (don't implement these - they're dumb or too complicated/not worth it)

- **2. The Heresy of Flight** — *Smart Actors & Social Simulation*
- **38. The Illuminator's Commissions** — *Exploration & Secrets*
- **19. The Chapel of Saint Anselm's Paradox** — *Procedural Generation*
- **18. Marks of the Nameless Mason** — *Narrative & Quests*
- **6. The Rose Meridian** — *Atmosphere & Light*
- **7. Windows That Remember** — *Procedural Generation*
- **8. The City Names You** — *Smart Actors & Social Simulation*
- **9. Whispering Galleries, For Real** — *Sound & Acoustics*
- **41. Pilgrims of the Same Stone** — *Experimental & Wild*
- **14. Cathedral Convolution: Audio Raytracing Through Stone** — *Sound & Acoustics*
- **37. Fog Sea over the Rooftops** — *Atmosphere & Light*
- **48. The Finger of Saint Alduin** — *Narrative & Quests*
- **43. Scaffolding Time** — *Experimental & Wild*
- **32. A Gift Remembered** — *Smart Actors & Social Simulation*
- **20. The Ossuary Below** — *Procedural Generation*
- **28. Vespers in Eight Bells** — *Gameplay & Systems*
- **29. Poleman of the Canal** — *Gameplay & Systems*
- **44. Frescoes of What Was Said** — *Experimental & Wild*


# The actual suggestions

## 1. The Second Sun
*Atmosphere & Light*

Look at the sky through the rose window and there is a second, wrong-colored sun casting its own contradictory godrays into the nave — step outside and it is gone, and shadows indoors fall in two directions. Meanwhile a heretic cell meets in the covered passages, convinced this hidden sun is real and the Church has walled it out of the sky; joining their meetings requires whispering a rotating passphrase into your actual microphone. It is the purest expression of a game named 'Impossible Light': a render-layer trick that an entire faction answers wrongly on purpose.

## 2. The Heresy of Flight
*Smart Actors & Social Simulation*

Free flight (F) already works with collisions on, and actors already perceive the player — so let NPCs witness you flying. A washerwoman who sees you lift off an overhead bridge doesn't get a scripted bark; her LLM decides whether it was a miracle, a demon, or a trick of the light, and that interpretation enters the gossip network. Days later you overhear two masons arguing about 'the angel of the canal quarter' — or a priest denouncing you from the forecourt. It turns a debug-ish movement toggle into the single biggest lever on the social simulation.

## 3. Rumor Drift
*Smart Actors & Social Simulation*

Anything you say within an actor's 20 m hearing radius — and anything actors witness — becomes a rumor that propagates NPC-to-NPC, and each hop is a real LLM retelling that mutates like a game of telephone: 'he gave the beggar bread' becomes 'he's a nobleman in disguise.' Days later your own offhand joke about the bishop comes back from a stranger on a bridge, warped into an accusation with teeth. Because every actor genuinely holds its own version in memory, you can walk the streets and triangulate a rumor back to the eyewitness — no scripted rumor tables, pure emergence.

## 4. Breath-Bent Candleflame
*Atmosphere & Light*

The player's microphone is already open by default for talking to actors — so let real speech physically disturb nearby candle flames: a whisper makes them shiver, a shout snuffs the closest votives and plunges a chapel corner into dark. No other game can do this, because no other game has a hot mic wired into a candlelit cathedral. It makes players instinctively lower their voices in sacred spaces, which is exactly the mood the game wants.

## 5. The Light That Falls Wrong
*Narrative & Quests*

Sanctified light refuses geometry: a beam through the rose window lands in a courtyard three streets away, a candle in the crypt casts its glow onto a bridge above, shafts thread the covered passages as if walls were glass — following light that shouldn't be there becomes the game's native quest marker, with zero UI. A scholar recruits you to survey the violations, treating your F5 screenshots as camera obscura plates and your session directories as the survey journal. Plot the impossible beams across the city and they converge on one buried point beneath the forecourt — the mystery of who built the cathedral, told through the renderer itself.

## 6. The Rose Meridian
*Atmosphere & Light*

The rose window casts a huge volumetric disc of colored light that crawls across the nave floor with the sun, turning the cathedral into a walkable sundial that actors schedule their day around — gathering to pray when the light touches the altar, market NPCs breaking off when it reaches the transept. And once per season the sunrise aligns exactly with the nave axis, punching light the full length of the cathedral from the west door to the altar, the way Chartres was actually surveyed. Actors count down to the Alignment in ambient chatter, giving players a reason to be standing in the crossing at the exact minute.

## 7. Windows That Remember
*Procedural Generation*

Generative stained glass that composes its panels from what actually happened in your playthrough: the actor you gave your last loaf to appears as a haloed figure, an argument you had becomes a two-figure judgment scene, colored light pooling on the nave floor. It fuses the inventory system and voice conversations with the game's signature architecture — the rose window becomes a save file you can read by looking up. No two players' cathedrals ever glow the same.

## 8. The City Names You
*Smart Actors & Social Simulation*

The player has no displayed name — instead the actors collectively coin one. Based on accumulated gossip about what you've done, said, worn, and traded, NPCs start referring to you by an emergent epithet ('the Quiet One,' 'Bread-Giver,' 'the Bridge Witch'), spoken aloud via TTS and drifting as your behavior changes. Hearing your reputation compressed into a nickname you never chose — and hearing it change after you change — is a reputation system with no bars or numbers, only language.

## 9. Whispering Galleries, For Real
*Sound & Acoustics*

Model the acoustic focusing of curved stone: in the apse and under certain domes a whisper at one focal point is audible 40 m away at the other, and hidden speaking tubes and confessional grilles carry sound between floors — all locally breaking the 20 m voice rule. The open mic becomes a stealth-and-intrigue mechanic: eavesdrop on conspirators across a gallery, whisper into a wall to spook an actor who sees no one nearby, or find the grille that reaches the private chapel where an NPC will finally confess where the reliquary key is hidden. Secrets stop being things you see and become things you overhear.

## 10. The Campanile Simulation
*Sound & Acoustics*

Every bell in the city — the cathedral's great bourdon, the five parish churches, the tower clocks — is a real physical instrument with its own pitch, decay, and swing period, rung by rope-pulling NPC campanologists on the canonical hours. Across the 1.2 km city you hear genuine propagation delay and interference, and with no minimap the bells become the map: players navigate the doglegging alleys by ear, and a passerby asked for directions answers in bell-relative terms ('keep Saint Alrich's toll on your left shoulder'). The bells are the city's clock, its map, and its emotional weather all at once.

## 11. The Anchoress in the Wall
*Narrative & Quests*

A holy woman voluntarily bricked into the cathedral wall decades ago, reachable only through a fist-sized squint — a pure voice-only NPC with no body to render. She remembers the city before the light went wrong, but she trades her lore for eyes: she asks you to walk to places and describe what you see out loud, and the LLM judges whether your spoken description is honest, detailed, or a lie. It weaponizes the STT pipeline into a describe-the-world mechanic no other game has, and lying to her becomes its own dark quest branch.

## 12. Whispered or Shouted
*Smart Actors & Social Simulation*

The mic is already open with a flat 20 m hearing radius — make the radius track your actual voice amplitude. Whisper into one merchant's ear to cut a private deal; shout across a town square and every actor in it hears you and reacts in the existing turn-taking cadence. Combined with the pinching streets, covered passages, and overhead bridges, the city's geometry becomes acoustic gameplay: you learn where to stand to be overheard, and where to lean over a bridge rail to eavesdrop on the alley below.

## 13. The Confessional
*Smart Actors & Social Simulation*

A confessor sits behind a lattice in a side chapel where acoustic occlusion is diegetic UI: kneel and he hears you, muffled and intimate through the screen, while actors outside the booth hear nothing — a deliberate architectural inversion of the 20 m radius. He remembers every confession across sessions and is supposed to keep silence, but he is an LLM with his own judgment: admit to something that endangers the parish and he may quietly warn the sexton, and the leak propagates through the gossip network. A trust mechanic built entirely from the voice pipeline and actor memory, staged in the game's most dramatic space.

## 14. Cathedral Convolution: Audio Raytracing Through Stone
*Sound & Acoustics*

Trace audio rays against the actual procedural geometry so reverb is derived from the space you're standing in: six-plus seconds of bloom under the crossing vault, tight slapback in a doglegging alley, dead air inside a covered passage. Crucially, apply it to the NPC TTS voices too — an actor preaching in the nave sounds like a cathedral sermon, and the same voice at a market stall sounds dry and close. Since the city is procedurally assembled, every generated layout gets its own authentic acoustic fingerprint for free.

## 15. The Vespers Sermon
*Smart Actors & Social Simulation*

Once per in-game evening the bishop preaches in the cathedral — and the sermon is generated from the week's actual events in the social simulation: the feud between two market actors, the strange offers a stranger has been making, the flying figure seen over the canal. You can attend and hear the city's history moralized back at you through the voice pipeline, in a nave built for exactly this acoustics-of-authority moment. It's a diegetic quest log written by an LLM with an agenda.

## 16. Trial by Earshot
*Narrative & Quests*

Get overheard blaspheming, haggling in bad faith, or repeating heretic talk, and you can be hauled before an ecclesiastical court where actor-witnesses testify from what they actually heard through the 20 m voice radius — their real conversation memories become evidence. You defend yourself with your own voice, cross-examining witnesses whose recollections are honestly imperfect. The turn-based speaking system already exists, so a courtroom is just that system with stakes — and it makes players genuinely mind what they say near open windows.

## 17. Palimpsest City
*Procedural Generation*

Have the generator secretly build an older city first — a Romanesque predecessor with its own street graph — then ruin it and grow the current city on top, so a church apse survives as a tavern wall, an old gate arch strands mid-plaza, foundations misalign with today's streets. Ruins stop being decoration and become archaeology with real answers, because the buried layer genuinely existed in the generation pipeline. Smart actors can half-remember the old names ('the Fish Gate, before the fire'), giving the LLM cast a shared, procedurally true folklore.

## 18. Marks of the Nameless Mason
*Narrative & Quests*

Every procedurally-assembled wall gets masons' marks carved into its stones, attributed to generated builder lineages — except one recurring mark that appears on stones the assembly log says were never placed, in walls of every era across 400 years of construction. Taking charcoal rubbings (inventory items you can offer to historian actors, who argue about them) slowly reveals a builder who cannot have existed. It makes the seams of the algorithm in-fiction evidence of an impossible architect.

## 19. The Chapel of Saint Anselm's Paradox
*Procedural Generation*

One unassuming side-chapel that is impossibly larger inside than out — a portal-rendered nave that couldn't fit in the city block, corridors that reconnect wrong, a cloister you circle in three turns instead of four. Non-Euclidean space is devastating in a first-person game precisely because the rest of the city is so rigorously believable, and free-flight mode becomes a delicious way to try — and fail — to catch the trick from outside. It's the 'Impossible Light' of the title made architectural.

## 20. The Ossuary Below
*Procedural Generation*

Generate a second, inverted city under the first: catacombs whose tunnels literally follow the street plan above — crypts under churches, bone-stacked galleries under the squares, a buried culvert under the canal — so walking below means recognizing the topology of streets you know, rendered in fieldstone and femurs. A bone-lined stair descends further still, to a drowned undercroft where a hidden branch of the canal flows beneath the cathedral, with a counterweight lift shaft rising from there straight into a bell tower. Canal-bottom to spire-top in one vertical line: the single most dramatic free-flight ascent in the game.

## 21. The Lamplighter of the Five Squares
*Atmosphere & Light*

One dedicated smart actor walks a dusk route through the five squares lighting lanterns one by one, so the city's entire night lighting is literally authored by this NPC in real time. Because he's LLM-powered you can walk beside him and talk — delay him with conversation and a whole quarter stays dark longer, or offer him oil from your inventory to light an extra street. It turns the day/night transition from a shader lerp into a character you can befriend.

## 22. Sworn by the Bell
*Gameplay & Systems*

Spoken promises become binding game state: because actors are LLMs listening to your real speech, saying 'I'll bring your ledger before the evening bell' can be remembered, and the bell toll is the literal deadline. Keep your word and merchants extend credit; break it and the news spreads actor-to-actor through the gossip network. It turns the voice system into a reputation economy with zero UI — your mouth writes checks your legs must cash.

## 23. The Tilewalkers' Guild
*Gameplay & Systems*

A courier guild that pays double if you never touch the street: a continuous rooftop route stitched across the whole 1.2 km city — ridge beams, plank bridges between gables, drying-line gaps, drop-downs onto the existing overhead bridges. The controller's coyote time, buffered jumps, and air control finally get to star; actors give delivery jobs by voice, and arriving means right-click-offering the parcel to the recipient. Progression is spatial knowledge: you memorize the city's skyline the way the streets teach you its floor, and the generator marking jumpable roof pairs makes every seed a fresh parkour puzzle.

## 24. Cry Your Wares
*Gameplay & Systems*

Medieval vendors advertised in pitched, melodic cries — give each market NPC a TTS-sung cry ('Fresh eels! Fresh eeeels!') on a personal melodic contour, so the five squares develop their own overlapping polyphony and you can find a specific vendor by ear. Then flip the mic around: become a hawker yourself and literally shout your pitch into the microphone, with every LLM actor within 20 m evaluating it — confidence draws a crowd, mumbling draws nobody, and deals close through the existing offer system. No other game can do this, because no other game has an always-open mic feeding real NPC brains.

## 25. Voces Sacrae: Voice-Spoken Words of Power
*Sound & Acoustics*

A small lexicon of Latin phrases works as voice-driven commands through the open microphone: 'Fiat lux' kindles nearby lanterns, 'Silentium' hushes a rowdy square, a sung note at the right pitch resonates a rose window. The catch is that every smart actor within 20 m also hears you — mutter a word of power in a crowded alley and NPCs react with awe, fear, or a report to the sexton. Speech becomes simultaneously your magic system and your social risk.

## 26. The Choir That Learns Your Words
*Sound & Acoustics*

A resident schola of smart actors sings Gregorian chant in the quire at the canonical hours — real plainsong, spatialized, drenched in the vault's reverb. The twist: because actors already hear the player via STT, you can teach the cantor a phrase by speaking it, and hours later hear your own words woven into a psalm tone echoing through the nave. It's the LLM-NPC system producing something no scripted game can: liturgy that remembers you.

## 27. Organum: The Playable Great Organ
*Sound & Acoustics*

The cathedral organ is an instrument you (or an NPC organist) can actually play — pipe ranks mapped to keys, with each pipe a positional sound source so 32-foot pedal notes physically rumble from the west end and shake dust motes in the light shafts. Walk the triforium while it plays and the balance between ranks shifts with your position. Give the organist a personality via the sidecar and you can request pieces by voice, or offer them sheet music from your inventory.

## 28. Vespers in Eight Bells
*Gameplay & Systems*

A change-ringing minigame in the cathedral tower: eight ropes, real campanology permutation patterns, and your performance broadcast over the whole 1.2 km city as spatial audio. Because actors are LLM-driven, they genuinely react — pausing conversations for the angelus, remarking that today's ringing was sloppy, or gathering at the forecourt when you ring the festival peal. Bells become the city's clock, its mood, and your instrument all at once, with harder methods as a pure skill ladder.

## 29. Poleman of the Canal
*Gameplay & Systems*

Pilot a flat-bottomed barge with a physical punting pole: momentum, drift, and low medieval bridges that force you to crouch or lie flat as you glide under them. Passengers are smart actors who sit in your boat and hold real voice conversations during the crossing — a captive-audience twist on the 20 m speech radius, since for once the NPC can't walk away mid-sentence. Ferrying goods between the market squares plugs straight into the inventory and offer economy.

## 30. The Errand of Words
*Smart Actors & Social Simulation*

Most townsfolk can't write, so actors ask the player to carry spoken messages — 'tell the chandler on Rope Lane that the wax is spoiled.' You physically walk the doglegging alleys and relay it by voice, and the receiving actor's STT hears what you actually said: deliver it faithfully, soften it, or twist it into an insult and start a feud. The player becomes a node in the gossip graph rather than an observer of it, and every delivery is a small acting performance into your real microphone.

## 31. Steam, Seal, and Lie
*Narrative & Quests*

Letter delivery where the letter text is real: the recipient actor genuinely reads it and their LLM reacts to its actual contents. So you can steam a letter open over a brazier, read it, then dictate a forgery through the microphone before resealing it — and the recipient believes, acts, and spreads consequences based on the words you spoke. Combined with the offer/accept system already in place, it turns the humble fetch-quest into an instrument of social sabotage.

## 32. A Gift Remembered
*Smart Actors & Social Simulation*

The offer system already moves items between player and actors — now let items carry provenance in actor memory. Give a fishwife a terracotta figure and find it weeks later on a stall in a different square, the seller retelling a mutated story of where it came from; actors also judge gifts socially, gossiping about the stranger who gave the beggar a knife. Objects become message-carriers through the social network, tying inventory directly into memory and rumor.

## 33. The Unfinished Spire
*Procedural Generation*

Medieval cathedrals took centuries; let this one still be under construction, with the generator exposing its build order as scaffolding, treadwheel cranes, half-vaulted bays, and stacked ashlar that visibly advances over real play sessions. The masons are smart actors who can actually talk about the work — ask the master builder why the north transept is delayed and the LLM answers from the true generation state. Your F5 screenshot sessions become a longitudinal record of a building growing.

## 34. The Quarter's Temperament
*Procedural Generation*

Give each district a personality vector — pious, mercantile, decaying, proud — that drives BOTH the architecture generator (street pinch, ornament density, plaster vs. fieldstone, how crooked the alleys dogleg) and the system prompts of the smart actors who live there. Players learn to read mood from masonry: if the streets narrow and the timber sags, the people will be suspicious before they say a word. It makes the procgen legible as culture rather than noise.

## 35. Every Door Opens
*Procedural Generation*

Right now the city is a magnificent shell; procedurally furnishing interiors behind every façade — solved backwards from the exterior footprint, window positions, and district material palette — turns 1.2 km of scenery into 1.2 km of place. It multiplies the smart-actor system for free: an NPC you're mid-conversation with can invite you inside, and the voice radius suddenly means overhearing arguments through shutters. The generator already knows each building's mass and style, so interiors are a constraint-solving problem, not an authoring one.

## 36. Storm of White Glass
*Atmosphere & Light*

Thunderstorms where each lightning flash fires the rose window at full noon intensity for a single frame, stamping its colored tracery across the dark nave like a photographic exposure — then blackness and thunder rolling off the slate roofs. Outside, the flash freeze-frames the whole skyline in silhouette, and the delay between flash and thunder reads distance across the 1.2 km city. It's storm-chasing for the F5 screenshot system: players will hunt the one-frame image of the rose window burned onto the floor.

## 37. Fog Sea over the Rooftops
*Atmosphere & Light*

Height-based ground fog that pools in the pinching streets and doglegging alleys at dawn, dense at the canal and thinning up the hill, so navigation becomes about towers and the cathedral spire floating above a white sea. The free-flight toggle gets a payoff: rise through the fog ceiling and the whole city vanishes except its spires — an instant screenshot moment. Overhead bridges turn into causeways above the murk, giving existing geometry a second life.

## 38. The Illuminator's Commissions
*Exploration & Secrets*

A scribe NPC gives photo commissions by voice — 'bring me the rose window at dusk, seen from a rooftop' — and the F5 screenshot becomes an inventory item you hand over via the offer system, with the LLM judging (or the deterministic fake backend faking) whether the shot qualifies. It fuses three systems that already exist — screenshots, inventory offers, smart actors — into a quest loop with zero new UI. Commissions naturally push players up towers, down into the crypt, and across the rooftops hunting the perfect angle.

## 39. The Gargoyle Way
*Exploration & Secrets*

A ledge-grab climbing system where the cathedral facade, buttresses, and tower faces are hand-solvable problems with gargoyles and string courses as holds — and every gargoyle is subtly posed to stare at something. Chains of gaze-lines form a citywide treasure hunt: climb to one, sight along its stare to a bricked-up window, find the next gargoyle there, until the final gaze crosses the rose window into the hidden gallery behind it. Summiting the great tower is the game's obvious 'because it's there' goal, and screenshot players will document gaze-lines like real conspiracy boards.

## 40. Postcard Seeds
*Procedural Generation*

Embed the world seed plus the player's position and facing in every F5 screenshot's metadata, so any screenshot IS the city: drag a friend's image onto the game and you spawn on the exact cobbles where they stood, in their exact city. Seed sharing becomes show-don't-tell — people trade beautiful alleys and rose-window sunsets as literal travel destinations. The screenshot system and session directories already do most of the work.

## 41. Pilgrims of the Same Stone
*Experimental & Wild*

Asynchronous multiplayer where other players appear as hooded pilgrims walking routes recorded from their real sessions — ghosts with real trajectories through the pinching streets. Because every city is assembled from the same seed, two strangers can meet at the same canal bridge and know they're standing in literally the same impossible place. The voice pipeline already exists, so proximity voice chat within the 20 m hearing radius is a natural extension: overhear a real human arguing with an LLM merchant.

## 42. The Long Memory of Bells
*Experimental & Wild*

NPCs persist memories across sessions via the Python sidecar: the baker remembers you offered her a candle three weeks ago and declined her bread, and greets you accordingly. Combined with the always-open microphone, actors can recall your verbal promises ('you said you'd come back before vespers') and hold grudges or affection. The city stops being a diorama and becomes a place with a social ledger you can't reset — and the deterministic fake-backend cast makes it testable offline.

## 43. Scaffolding Time
*Experimental & Wild*

A bell-rope or relic lets you slide between construction eras of the same cathedral: 1180 (bare crossing piers, wooden cranes, no rose window), 1240 (scaffolded vaults), and the finished present. Since the city is procedurally assembled, generating 'earlier' states is a parameter sweep, not new content — walls lower, plaster gives way to fieldstone, whole squares revert to mud. Smart actors change with the era: talk to the master mason in 1180 about the rose window that only exists in your memory of the future.

## 44. Frescoes of What Was Said
*Experimental & Wild*

An AI image pipeline turns transcripts of your actual conversations with actors into period-style frescoes and stained glass panels that appear in side chapels over time — the STT log becomes the city's iconography. Tell a fisherman a lie about a sea monster and weeks later find it painted in a secondary church's apse, half-remembered and distorted by retelling between NPCs. It's the rare generative-AI feature that's diegetic: medieval art really was gossip and testimony frozen into walls.

## 45. Plague Ledger
*Experimental & Wild*

A slow simulation layer where fire, flood, and plague propagate through the actual city graph — fire jumps between half-timber houses but stalls at limestone, floodwater follows the canal's real elevation, plague spreads along NPC social contacts built from who talks to whom. Because actors are LLM-driven, they don't just play sick animations: they panic, spread rumors about which quarter is cursed, and beg you for items they believe are cures. Quarantining a district by persuading a gate guard with your literal voice would be an all-time emergent moment.

## 46. The City Keeps Real Time
*Experimental & Wild*

The cathedral-city runs on your actual clock and local weather: launch at 6 a.m. and hear matins, log in during a real thunderstorm and the canal swells while actors shelter under the covered passages. Certain conversations and doors are only reachable when the rose window's projected light touches a specific tomb — at the real hour it would. It makes the game a place you visit rather than a session you start, which suits a pilgrimage fantasy perfectly.

## 47. Procession of the Broken Bell
*Narrative & Quests*

On procession days the whole city reflows: a relic is carried on a dynamic route through the pinching streets and over the canal bridges, actors abandon their routines to line it, and the crowd becomes a quest surface — pickpockets working the crush, and somewhere in the murmur two voices planning to drop a mason's block from an overhead bridge onto the reliquary. You can shadow the plotters through the alleys, warn the marshal, or stand back and watch the city's story fork. It stages the game's best architecture — bridges, doglegs, squares — as the set of an emergent thriller.

## 48. The Finger of Saint Alduin
*Narrative & Quests*

Two of the secondary churches claim the same relic — a saint's finger bone — and both can't be right, but the true provenance is procedurally seeded each playthrough, so it's genuinely solvable rather than scripted. Actors give conflicting sworn testimony you gather by voice, and the physical relics travel through your inventory: you can offer the true bone, the fake, or a third fake you commissioned. The right-click offer mechanic becomes a lever on a city-splitting theological feud.

## 49. Ask the Master
*Gameplay & Systems*

Crafting skills are not unlocked from menus but taught: you apprentice under LLM master actors — stonemason, glazier, bellfounder — by literally asking them questions with your voice, and they quiz you verbally before granting the next rank. The knowledge is real: the glazier explains how rose-window tracery actually works, so player progression and player learning are the same thing. Ranks gate paid jobs and recipes, giving the smart-actor tech a long-term progression spine instead of just ambient chatter.

## 50. The Blind Pilgrim
*Experimental & Wild*

An accessibility-as-feature mode where the city is fully playable by ear: bell timbre identifies each district, canal water marks the west edge, your footsteps shift from cobble-slap to vault-bloom as spaces open up, and you can shout and let the audio-raytraced echo sketch the geometry around you — the open STT mic becomes a sonar tool. Actors verbally guide you because they can already hear and answer your voice. In a game whose whole identity is architecture and sound, playing it blind isn't a degraded mode — it's a second, arguably more medieval way to know the city.
