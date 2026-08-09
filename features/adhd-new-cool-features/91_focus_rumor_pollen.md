# Focus: Rumor Pollen

*News that travels at walking speed for zero marginal LLM calls — deepened
against the sim's event, scheduler and prompt seams.*

## Sketch

A whitelist of `DomainEvent` kinds (the custody commit, `raise_notice`, the
knell, a big accepted sale, a memorable stranger deed) is intercepted at
`World::emit` and minted into a new store in
`crates/cathedral-sim/src/rumor.rs` as a **Pollen** token —
`{kind: RumorKind, subject: ActorId, place: AreaId, day, heat: f32, hops: u8,
per-field confidence}` — dropped at the event's position. Pickup and
carrier-to-carrier hops run in pure Rust during the per-poll distance pass
the scheduler already does (any actor within ~25 m of a deposit or a carrier
absorbs a copy at `hops+1`), so spread continues city-wide even where
`attention.rs` has idle LLM turns switched off; heat decays per game hour and
per hop, and a cold token is silently dropped.

The hottest token an actor carries renders as exactly one extra sheet line
beside the existing word-in-the-ward section in `prompt/mod.rs` — "You have
heard, third-hand: Ede was taken at the Wickmarket, two days past" — riding
turns that were already going to happen. Garbling is a deterministic roll
seeded from `(rumor sequence, carrier id)` at each hop: a failed field roll
swaps the subject for another named actor of the same ward or trade, the
place for an adjacent area, the day by ±1 — drift bounded to the fixed
vocabulary, never inventing people (the no-procedural-characters rule holds).

The player perceives it as NPC small talk with provenance hedging that
degrades with hop count ("they say," "I had it fourth-hand"), and becomes a
carrier themselves: hearers of player speech can emit a `pass_word(rumor)`
intent on their reply turn, minting a fresh `hops=0` token at their feet —
which is what makes outrunning the pollen to a far ward mechanically real
rather than flavor.

## Load-bearing risk

Propagation must live entirely in the pure-Rust poll pass, never in LLM
turns: `attention.rs` gates idle cognition to the player's neighborhood, so
any hop or pickup tied to a cognition turn freezes the rumor field everywhere
the player isn't and kills the watch-it-spread / outrun-it fantasy. The
mirror half of the same risk is perceptibility: once spread is code-driven,
the LLM must actually voice the injected line often enough for the player to
feel the wave — without every mouth in a ward parroting the identical
sentence.

## First step

Create `crates/cathedral-sim/src/rumor.rs` (`RumorKind`, `Pollen`, a store on
`World`) with deposits hooked into `World::emit` for just two kinds — the
custody-commit arrest and the knell — a proximity hop+decay tick added to the
scheduler's existing poll pass, and one rendered line in the prompt sheet
beside word-in-the-ward in `prompt/mod.rs`. Prove it headlessly with
`cargo run -p cathedral-backends --bin cathedral-headless -- --fake -t 6`,
printing carriers-per-ward per game hour, plus a unit test asserting a token
crosses two hops, garbles deterministically under a fixed seed, and decays to
silence.

## Children

- **Hearsay rung** — a sergeant carrying arrest/wrong pollen may
  `raise_notice` on its strength at a new, explicitly lower rung in
  `notices.rs` — so a garbled subject field produces a wrongful summons the
  player can watch get raised, and settle. Makes garbling consequential to
  the law system rather than cosmetic dialog color.
- **The STRANGER token** — everything memorable the player does mints pollen
  about the player, so far wards greet you with a garbled second-hand version
  of yourself — and because you walk faster than pollen hops, you can beat
  your own story to a ward and pre-empt it with your account. Emergent
  reputation with no reputation system.
- **Night Office settlement** — include the ward's hottest pollen in the
  already-running curfew batch prompt so it shapes the returned Minor mood,
  and let a Major's nightly reflection settle a rumor into permanent memory —
  pollen graduating into canon, on prompts already paid for.
- **Bells as rumor amplifiers** — a civic peal re-heats matching pollen
  within earshot (the knell re-heats the death token; a summons re-mints the
  newest ward notice as fresh pollen at the Bellstand), so acoustics
  physically extend rumor range — and the false-bell prank gains a visible
  epistemic consequence.
- **Walk the chain** — because garbling is deterministically seeded per
  `(rumor, carrier)` hop, the transmission chain is reconstructible: a "who
  told you that?" affordance lets the player or a sergeant trace a slander
  backwards to the exact garble point. An implementation choice made for
  testability becomes investigation gameplay for free.
