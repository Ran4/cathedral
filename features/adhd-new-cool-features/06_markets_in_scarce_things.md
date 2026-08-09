# Markets in scarce things

Not "dynamic bread prices" — markets over what is *genuinely* scarce in this
city: legal night hours, stall positions, attention, debt, information, and
the right to be forgiven.

### Curfew Scrip `[N8 V8 F8]`

Gate keepers issue a fixed nightly number of lawful-after-curfew passes, and
a secondary market forms by dusk: holders resell at whatever the desperate
will pay, and the sergeants honor the paper. Legal nighttime becomes a
genuinely scarce good the Snuffing already created. The player can corner the
night — buy all eight passes and the streets after curfew belong to you and
whoever you resell to.

### The Dawn Pitch Auction `[N8 V8 F8]`

Stall positions on the five squares are re-auctioned each morning, and
location value is set by where the fixed cast actually walks — foot traffic
is deterministic and therefore learnable. Sellers bid, winners sublet, and a
player who has walked the city long enough has genuine informational edge
over the crier's posted prices. Space itself becomes the traded good.

### Keys as Sequence-Breaks `[N7 V8 F9]`

Every locked door in the city has exactly one hand-authored key, held by a
named cast member — progression through borrowing, buying, stealing, or being
lent, turning the fixed cast into the tech tree with no XP, just access.
Keys are pocket items, and NPCs remember lending them, so every key is also a
social debt with a return-by expectation the Night Office can sour.
Speedrunners will chart which three conversations open the route from gate to
crypt.

### Split Tallies `[N8 V7 F8]`

Debts are physical tally-stick items whose halves must match to settle — and
the creditor's half can be sold to anyone, including people you'd rather not
owe. The friendly baker sells your tally to the Bondsman at a discount, and
suddenly your bread debt walks with a tether. NPCs hold, trade, forgive, or
call tallies through the existing offer verbs; the Night Office is where a
Major decides whether to call yours.

### Debt Walks Beside You `[N8 V7 F8]`

Credit is frictionless by design — anyone will sell to you on it — and the
friction arrives later as a standing relationship: creditors track you
through the gossip layer, send a child to stand silently near you at market,
and can lawfully invoke the tether-escort to walk you to a reckoning. Your
freedom of movement becomes exactly as good as your ledger.

### The Fence at the Water Stair `[N8 V8 F8]`

A canal-side black market where hot goods trade at a discount that decays as
their ward notice ages — a real depreciating information asymmetry, since
notices and restitution already exist. The fence buys hot cheap, sits on it
by the canal, resells clean once the notice fades; the player can be
supplier, customer, or the informer who re-raises the notice to crash the
fence's inventory value. Selling hot goods to the wrong buyer walks you into
the arrest pipeline.

### The Ward Hearth Pool `[N9 V6 F7]`

Each ward runs a mutual fire-insurance pool paying out on hearth and weather
losses — simulated for free from hearth_heat, the clock and the weather, no
LLM calls. Minors pay in at curfew, one Major per ward adjudicates claims,
and a bad storm week drains the pool so late claimants get nothing, visible
in ward notices and ward mood. The player can join, underwrite a share for
premium income, or quietly short a ward they've left.

### Rumor Provenance `[N8 V7 F8]`

Rumors are goods whose price falls with every retelling and whose worth
depends on how close the seller stands to the source: fresh first-hand
information sells dear, and by the time three Minors have carried it across
two wards it's near worthless. A false rumor traced back to its seller
settles as a ward notice against them. The player — who walks everywhere and
is known by no one — is the city's natural information arbitrageur.

### Rumor Herding `[N9 V7 F8]`

The attention gate means a ward you haven't visited is frozen in yesterday's
beliefs — make that diegetic: news travels at your walking speed, so you can
carry word from the harbor to the Wickmarket faster than the city can, sell
"fresh word" to a stalled ward, or front-run a price move by outrunning the
carriers. The engine's biggest cost-saving hack becomes the game's
information economy.

### The Bondsman's Ledger `[N8 V8 F8]`

See [03_custody_as_infrastructure.md](03_custody_as_infrastructure.md) — the
Bondsman fronts gaol fees at interest; listed here because he is also the
natural market-maker for split tallies and the buyer of last resort for bad
debt, tying the credit ideas into one hand-authored character.

### The Audience Broker `[N9 V5 F6]` ⚠ trap

A Major's conversational attention is openly scarce, queued and resold —
petitioners hold numbered places to speak with the Provost, a broker buys and
resells queue positions, and paying for priority actually reprioritizes the
turn scheduler. The real LLM-turn budget becomes diegetic lore. **Trap
because:** it welds the fiction to an engineering constraint that will change
out from under it; charming today, load-bearing lie after the next
infrastructure change.

### Rumors Are Merchandise `[N8 V6 F7]` ⚠ trap

Facts about the player circulate as literal catalog items — "words" that can
be bought, sold, intercepted and planted, physically traveling the map with
their carrier. **Trap because:** reifying free-text gossip into item payloads
explodes the item vocabulary and invites nonsense; Rumor Pollen
(04) delivers the same fantasy with a fixed vocabulary and no new item kinds.
