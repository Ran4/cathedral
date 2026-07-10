Python authority for cathedralbevy's LLM-driven character simulation. The
terminal prototype and persistent Bevy sidecar share this domain model, prompt
format, action parser, and orchestration code.

## How it works

Characters take turns in a round-robin tick loop. Each turn:

1. `prompt.render_prompt` renders the character's full sheet (backstory,
   location, visible people, held items, memories, goal) plus a
   `since_your_last_turn` field drained from the character's **inbox**.
2. The prompt is sent to Kimi (`llm_client.complete`) — one stateless call, no chat
   history. `stored_memories` and `current_goal` are the only persistence,
   so the prompt tells the model to use `remember`/`forget` deliberately.
3. The reply is parsed as `VERB {json}` lines (`prompt.parse_reply`) and each
   action is applied to the world (`sim.apply_action`).

"Hearing" = the inbox: `say` appends an event string to its recipients within
20 metres (including nearby bystanders). Metric queries use full 3D distance,
inclusive boundaries, and stable distance/ID ordering. Events accumulate
between a character's turns and are perceived all at once on their next turn.

Invalid reply lines and failed actions become `system:` events in the actor's
own inbox so the model can self-correct next turn (also echoed to stderr).

**Unknown people:** each character has a `knows` set of character ids. People
outside it are rendered as `(unknown - you don't know the name of this
person)` in `you_see` and as `a stranger (id <id>)` in heard events —
`sim.identify(observer, subject)` is the single place this perspective
rendering happens. There is no introduction verb: characters introduce
themselves in speech, and listeners keep names as memories (the prompt tells
them to, e.g. `remember {"memory": "The pilgrim with id k0fb1 is called
Ilse"}`). `knows` is only seeded at world creation.

## Actions

Format: one action per line, `VERB {json args}`, optional `# comment` after.
Parsing uses `JSONDecoder.raw_decode`, so `#` inside quoted strings is safe.

- `say {"target": "<id>", "text": "..."}` — target's inbox gets "said to
  you", and in-range bystanders get "said to X". Omitted/null `target`
  broadcasts within 20 m. An invalid or out-of-range explicit target is an
  error and never falls back to broadcast.
- `offer_item {"item_id": "<item id>", "target": "<char id>"}` — offer a held
  item; it stays in the giver's `holds` until accepted. Omitted/null `target`
  = broadcast (anyone within 4 m may accept, first wins); a bad target
  id is an error, NOT a fallback to broadcast. Re-offering replaces the
  pending offer (a jilted target gets a "withdrew" event).
- `accept_offered_item {"item_id": "<item id>"}` — take an item offered to
  you (or broadcast) by someone still within 4 m; moves it giver → you
  and clears the offer.
- `decline_offer {"item_id": "<item id>"}` — turn down an offer targeted at
  you; the giver keeps the item. Broadcast offers can only be ignored.
- `retract_offer {"item_id": "<item id>"}` — withdraw your own pending offer
  (the target, if any, is notified).
- `eat {"item_id": "<item id>"}` — consume a held item: it is removed from the
  world (any pending offer of it is implicitly retracted, with notification).
- General concept: omit and null is the same thing, {"foo": null, ...} <=> {...}
- `item_id` takes ids only — a name like `"fish"` is an error, no fallback.

Pending offers live in `world.offers` (item id → `(giver, target | None)`)
and are rendered on the character sheet every turn as `you_offer` /
`offered_to_you`, since inbox events alone would be forgotten. Offer inbox
events are past-tense history with no accept hint — they can be stale by the
time they're read (someone earlier in the round may have taken a broadcast
offer); the accept syntax appears only in `offered_to_you`, which is always
current. Full design: `../features/giving_things.md`.

## Files

- `main.py` — entry point; tick loop and the seeded demo world (Sven, who
  holds a fish and owes Conny two coppers; Conny the fishmonger; and Ilse, a
  hungry pilgrim stranger holding a copper coin — seeded so a purchase can
  emerge). uv inline script; `sim`/`prompt`/`llm` are plain modules it
  imports.
- `sim.py` — `Character`, `Item`, `World`, `apply_action`. Items are world
  entities with ids; `world.add(entity)` takes either a `Character` or an
  `Item`, and `Character.holds` is a list of item ids that the prompt
  resolves to `{"id", "name"}` objects. Id strings are typed as `ItemIdStr` /
  `CharIdStr` (`typing.NewType`) so the type checker keeps them apart.
- `prompt.py` — prompt rendering + reply parsing (the LLM text format lives
  here and nowhere else).
- `server.py` — uv inline-script JSON-lines sidecar and protocol state thread.
- `protocol.py` — strict version-1 envelope parsing and compact encoding.
- `scheduler.py` — one non-blocking global NPC turn stream with priority and
  provider backoff.
- `speech_client.py` — completed-utterance WAV OpenAI STT/TTS adapter; unavailable
  credentials degrade independently from text cognition.
- `llm_client.py` — `complete(prompt) -> str` against the configured provider (see
  Configuration below).
- `kimi.py` — standalone one-shot CLI: send a file to the configured LLM,
  print the reply (`./kimi.py [file]`, default `think.md`).
- `think.md` — the original hand-written prompt sketch that `prompt.py` is
  derived from; used with `./kimi.py` for one-off prompt experiments.

## Configuration

`llm_client.py` loads `.env` from this directory (gitignored; real environment
variables take precedence over it):

- `LLM_PROVIDER` — `moonshot` or `openai` (default `moonshot`)
- `LLM_MODEL` — optional model override; unset/empty means the provider
  default: `kimi-k2.5` for moonshot, `gpt-5.6-luna` for openai
- `MOONSHOT_API_KEY` / `OPENAI_API_KEY` — key for the chosen provider
  (moonshot also falls back to `~/.config/moonshot/key`)

Moonshot calls run with temperature 0.6 and thinking disabled (instant
mode, for speed); openai calls use API defaults. Override per run like
`LLM_PROVIDER=moonshot ./main.py`.

## Running

```
./main.py            # 6 ticks (make sim)
./main.py -t 10      # more ticks
./main.py -v         # dump full prompts and raw replies to stderr
./kimi.py think.md   # one-off prompt test (make think)
```

Every run makes live API calls (a few seconds per tick). stdout is the
transcript + final state + total run cost in USD (priced from the per-model
table in `llm_client.PRICING`; cache discounts not modeled); diagnostics go
to stderr.

The sidecar has a deterministic fake mode for offline integration tests. Run
the complete Python suite without a project environment or network access:

```
uv run --offline --no-project python -m unittest discover -s tests -v
```

## Known gaps (intentional, for now)

- `move_to` and other verbs from `think.md` are not implemented (`eat` is) —
  characters may narrate world changes the sim doesn't model (e.g. claiming a
  fish is sold before it is).
- Memory/goal hygiene is prompt-enforced only: the prompt tells characters to
  record outcomes the turn they happen, forget superseded memories, and
  clear/replace achieved goals (`set_goal {"goal": null}` clears). This works
  in test runs but nothing in the sim guarantees it.
