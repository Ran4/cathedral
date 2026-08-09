# Stigmergy fields — the city that runs itself

No central planner: fields, gradients, marks and decay that make city life
solve itself with zero or near-zero LLM calls. The engineering constraint
(scarce turns) becomes the design language.

### Rumor Pollen `[N8 V9 F9]` → deepened in [91_focus_rumor_pollen.md](91_focus_rumor_pollen.md)

Notable events drop a fixed-vocabulary fact-token where they happened; NPCs
who idle there absorb it and carry it, and it rides as one prompt line on LLM
turns they were already going to have. News travels at walking speed for zero
extra calls, hops carriers, decays, and garbles a field as it spreads. The
player can watch a scandal physically propagate across the map — or outrun
it, and *be* the news in a far ward before the pollen arrives.

### Chalk and Tallow `[N9 V8 F8]` → deepened in [92_focus_chalk_and_tallow.md](92_focus_chalk_and_tallow.md)

NPCs leave rule-triggered physical marks — a chalk cross on a debtor's door,
a tally notch at the well, a ribbon at a shrine — and other NPCs' cheap rules
read them: refuse credit at a chalked door, avoid a marked lane. The
environment is the database, and the player can tamper with the medium
instead of the agents: scrub a cross at night and commerce resumes, forge one
on an enemy's door and watch the ward's rules turn on them. Zero LLM cost
until someone catches you at it.

### The Filth Ledger `[N8 V7 F7]`

The poop clock's outputs persist as per-cell street filth that decays slowly
and reeks in the soundscape (flies bed) when it crests. Two hand-authored
gong farmers need zero LLM calls: they climb the stink gradient every night,
work down the maximum, and dump in the canal — sanitation as a visible
nightly circulation. Ward filth thresholds can raise real ward notices, so a
neglected ward literally becomes a legal problem no one scripted.

### Scent Plumes `[N7 V8 F8]`

Bakehouse, tannery, fish stall and canal each emit a diffusing scalar field
drifting on the weather system's wind. Hungry walkers' path costs tilt toward
the bread plume with pure local math; stink pushes foot traffic to the far
pavement, giving streets a lived-in asymmetry that emerges from chemistry,
not authoring. The player gets the same field as HUD whiffs — "warm bread,
from the west" — and can navigate the city by nose.

### The Window Automaton `[N8 V9 F8]`

At dusk each household lights its windows partly as a function of visible lit
neighbors — a one-rule cellular automaton that ripples lamplight down streets
differently every evening — and after curfew the ambient cast pathfinds away
from streets below a light threshold. Honest traffic pools under lit windows,
so whoever you meet in a black lane *chose* to be there. Costs a per-building
byte, and makes the curfew bell reorganize the city's geometry of fear.

### Depletion Pheromone `[N7 V8 F8]`

Every sale deposits "want" for that item kind on the stall's cell, and the
named gate carriers route their next load toward the strongest want they can
smell from the gates. Bread selling out at Maren's Green raises a spike, the
next carrier hauls flour that way, and shortages self-heal at cart speed — or
visibly fail to when rain keeps carriers home. The player can arbitrage the
field itself: buy where want is low, walk, sell where it is high.

### Misdeed Scent `[N8 V8 F8]`

Witnessed infractions stamp a slow-decaying trouble pheromone on the spot,
and sergeant beat-routing bends a few percent per night toward the strongest
accumulations. Nobody dispatches the watch — their loops deform over
game-days until the rowdy corner has a sergeant on it most evenings, exactly
like ants finding sugar. The player can read police pressure off the map, or
seed false trouble somewhere to thin the watch near what they actually intend.

### The Vermin Layer `[N8 V7 F8]`

A few hundred stateless rats, cats and dogs run pure gradient rules on the
hidden fields: rats climb filth-and-scraps, cats climb rat density, dogs
orbit people and bark at the misdeed scent. Three boids-grade rule sets, no
cognition, no cast additions — and the payoff is diegetic debugging: a player
who sees rats streaming into a courtyard has learned something true about the
sim that no NPC had to say out loud.

### Wax Accretion `[N8 V8 F8]`

Shrines accumulate candle-and-offering piles by preferential attachment — the
bigger the pile a passerby sees, the likelier they add, nudged by the ward
mood the Night Office already produces. Popular shrines run away, neglected
ones go cold and dark, and a plague scare visibly grows one saint's pile at
the expense of another's. The player reads the city's collective anxiety in
candle wax, and can seed a cold shrine to manufacture a cult.

### Stranger Scent `[N7 V8 F9]`

The player's presence deposits a per-ward familiarity-and-suspicion trace
that decays over days: loitering, being seized, or getting chalk-marked
spikes suspicion; buying bread daily and paying on time converts to
familiarity, both leaking across gates with the minors who cross them. It
becomes one cheap prompt line ("you half-recognize this stranger; they are
trouble in Bell Ward"), so the cast's warmth is earned through the field —
somewhere you are greeted, somewhere the dogs start barking.

### The Want-Lines `[N8 V8 F9]`

NPCs drop tiny errand-markers ("needs a knife mended," "seeks firewood") at
their doorstep when a body stat or inventory rule trips, and any capable
neighbor whose path brushes within a few meters absorbs the errand — task
allocation the ant way, no market, no dispatcher, no quest log. Wards with
bad geography visibly go under-served: emergent inequality from pure
adjacency. The player is a capable neighbor too — brushing a marker surfaces
a HUD whisper, odd jobs without a single scripted quest existing anywhere.

### The Bucket Chain `[N9 V4 F7]` ⚠ trap

A smolder field seeded by hearths, wind and thatch occasionally ignites, and
firefighting emerges from one local rule: an able body adjacent to
fire-or-chain joins the chain toward the nearest water. Whether the
Wickmarket burns depends on the real local population at that hour — an
outcome no one, including the developers, decided. **Trap because:** fire
that consumes hand-authored geometry is a world-persistence nightmare; the
chain is gorgeous, the burn-down is the problem. A cosmetic smolder that
never destroys could keep the chain and dodge the trap.
