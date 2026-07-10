Python prototype of the LLM-driven character simulation for cathedralbevy.
This is where the prompt format, action format, and orchestration loop get
tuned before being ported to the Bevy app (Rust) in the parent repo.

## How it works

Characters take turns in a round-robin tick loop. Each turn:

1. `prompt.render_prompt` renders the character's full sheet (backstory,
   location, visible people, held items, memories, goal) plus a
   `since_your_last_turn` field drained from the character's **inbox**.
2. The prompt is sent to Kimi (`llm.complete`) — one stateless call, no chat
   history. `stored_memories` and `current_goal` are the only persistence,
   so the prompt tells the model to use `remember`/`forget` deliberately.
3. The reply is parsed as `VERB {json}` lines (`prompt.parse_reply`) and each
   action is applied to the world (`sim.apply_action`).

"Hearing" = the inbox: `say` appends an event string to the target's inbox
("Sven said to you: ...") and to bystanders at the same location ("Sven said
to Conny: ..."). Events accumulate between a character's turns and are
perceived all at once on their next turn.

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
  you", bystanders get "said to X". Without `target` (or an invalid one):
  broadcast to everyone at the location.
- `set_goal {"goal": "..."}` — replaces `current_goal`.
- `remember {"memory": "..."}` — appends to memories (deduped).
- `forget {"memory": "..."}` — removes by exact match, falls back to
  substring match.

## Files

- `main.py` — entry point; tick loop and the seeded demo world (Sven and
  Conny, who know each other, plus Ilse, a pilgrim stranger, on a town
  square). uv inline script; `sim`/`prompt`/`llm` are plain modules it
  imports.
- `sim.py` — `Character`, `Item`, `World`, `apply_action`. Items are world
  entities with ids; `world.add(entity)` takes either a `Character` or an
  `Item`, and `Character.holds` is a list of item ids that the prompt
  resolves to `{"id", "name"}` objects.
- `prompt.py` — prompt rendering + reply parsing (the LLM text format lives
  here and nowhere else).
- `llm.py` — `complete(prompt) -> str` against the configured provider (see
  Configuration below).
- `kimi.py` — standalone one-shot CLI: send a file to the configured LLM,
  print the reply (`./kimi.py [file]`, default `think.md`).
- `think.md` — the original hand-written prompt sketch that `prompt.py` is
  derived from; used with `./kimi.py` for one-off prompt experiments.

## Configuration

`llm.py` loads `.env` from this directory (gitignored; real environment
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
transcript + final state; diagnostics go to stderr.

## Known gaps (intentional, for now)

- Only the four verbs above are implemented; `move_to`, item transfer, etc.
  from `think.md` are not — characters may narrate world changes the sim
  doesn't model (e.g. handing over an item while `holds` stays unchanged).
- Models rarely `forget`, so stale memories linger; needs prompt tuning.
