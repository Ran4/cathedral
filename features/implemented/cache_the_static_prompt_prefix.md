# Cache the static prompt prefix

Status: **implemented 2026-07-14** (`f498706` the reorder, `9faa2d8` the
measurement). The reordering works and is in. Whether it *saves* anything
depends entirely on the provider, and on the one currently configured it does
not — see **Outcome**.

## Goal

Cut input-token cost by roughly 60–70% for a change with no effect on behavior.

## The observation

`assets/prompts/turn.j2` rendered in this order:

1. one line of preamble;
2. `{{ sheet_json }}` — the character's whole sheet, **different every turn and
   different for every actor**;
3. ~6k characters of instructions — **byte-identical for every actor and every
   turn** (modulo `sounds_enabled` / `emittable_sounds`, constant for a run).

Provider prompt caching keys on a **prefix** match. Because the variable part
came first, every prompt had a unique prefix and the cache could never hit.

## The fix

Static instructions **first**, sheet **last**: `[preamble][instructions][sheet]
[act]`. The instruction block (6,771 bytes, ~1,700 tokens) is now a prefix
shared by the entire cast for the whole session, and the sheet sits next to
"Take one or more actions" instead of six thousand characters from it.

It is a pure permutation — no prose added, removed or edited.

## Outcome: the provider decides, and ours is the wrong one

The feature said to verify the provider actually caches before claiming the win.
It does not. Measured against both configured providers with a 2k-token shared
prefix and differing tails:

| Provider | Shared prefix, different tails | Byte-identical prompt |
|---|---|---|
| **openai** `gpt-5.6-luna` (**configured**) | `cached_tokens: 0` — never hits | full hit |
| **moonshot** `kimi-k2.5` | `cached_tokens: 1792` (~84%) | full hit |

The openai endpoint does **whole-prompt** caching, not prefix caching. It
reports `cache_write_tokens` on every single call and `cached_tokens: 0`
forever. Only an exact, byte-identical prompt hits — which never happens in the
game, because the sheet differs every turn.

This was not taken on one probe's word. Every client-side lever was tried
against `{static}{sheet}`, and all nine read `cached_tokens: 0`:

| Lever tried | Hit rate |
|---|---|
| Static prefix inline in one user message (what we ship) | 0% |
| Static prefix as its own byte-identical `system` message | 0% |
| Static prefix at ~8k tokens, inline *and* as a system message | 0% |
| Plain-string content instead of the parts array | 0% |
| `prompt_cache_key` (openai's routing hint) | 0% |
| `prompt_cache_retention: "24h"` | 0% |
| `/v1/responses` with `instructions` (the field built to be the cached prefix) | 0% |
| `cache_control: {type: ephemeral}` breakpoint on the message | 0% |
| `cache_control` breakpoint on the content part | 0% |
| **byte-identical whole prompt** (the positive control) | **100%** |

Two of those deserve their own note, because they are the ones that would
otherwise leave the question open:

- **The ~8k-token prefix** rules out an unmet minimum. The documented threshold
  is 1,024 tokens; ours clears it four times over and still reads zero.
- **The `cache_control` breakpoints** *appear* to be accepted (HTTP 200), which
  looks like GPT-5.6's explicit-breakpoint feature working. It is not. The API
  silently ignores unknown fields **inside** messages while strictly rejecting
  unknown **top-level** ones (`Unknown parameter: 'prompt_cache'`). That
  asymmetry is itself the proof: `prompt_cache_key` and `prompt_cache_retention`
  are the *only* cache parameters this endpoint admits, so there is no explicit
  breakpoint to set, and the one that "worked" was being dropped on the floor.

The positive control is what makes this conclusive rather than a timing artifact:
a byte-identical prompt hits within seconds. The cache is live and fast. It just
has no prefix semantics.

Live 8-turn headless runs, same workload, real prompts:

```
moonshot: Run cost: 0.01 USD
          Input tokens: 17021 (10752 served from the provider's prompt cache, 63%)
openai:   Run cost: 0.02 USD
          Input tokens: 16971 (0 served from the provider's prompt cache, 0%)
```

**63% on moonshot — exactly the predicted 60–70% band. 0% on openai.**

So the acceptance criterion ("if the usage numbers do not move, it did not
work") is met on moonshot and fails on openai, for reasons outside this change.

## What was actually shipped

- `assets/prompts/turn.j2` reordered, and the 20 golden fixtures permuted by the
  same transform rather than re-rendered from Rust — so the sheet bytes are
  still the ones the Python HEAD produced, and `tests/golden_prompts.rs` stays
  an independent witness rather than a tautology. It passes byte-for-byte.
- `UsageLedger` now carries `ModelUsage { prompt_tokens, cached_prompt_tokens,
  completion_tokens }` and reads the cache hit from
  `prompt_tokens_details.cached_tokens` (openai + moonshot) or the top-level
  `cached_tokens` (moonshot). The headless runner prints the hit rate under the
  run cost. A 0% line is the standing alarm that the prefix is not being reused.
- Behavior unchanged: `e2e_fake`, `parse_tests`, `scheduler_tests`,
  `prompt_tests`, `prompt_quirks`, `golden_prompts` all pass; the fake cast
  still parses and acts.

`run_cost_usd` still bills every input token at full price, so it stays the
upper bound it already documented itself to be. Modelling the cached-input
discount would mean inventing a per-provider rate, and no such rate was
confirmed, so none was invented.

## The open decision (for the human)

The reorder is free and correct either way, but the ~60% input saving only
exists on moonshot. Realizing it means `LLM_PROVIDER=moonshot` in
`prompt_playgound/.env` — a model swap (kimi-k2.5 vs gpt-5.6-luna), which is a
character-quality call, not a cost call. Nothing was switched.

If openai stays, this feature's win is zero and the cost work has to come from
`features/gate_idle_cognition_on_novelty.md` (fewer calls) instead.

**Worth checking against the real bill:** the openai endpoint reports
`cache_write_tokens` ≈ the whole prompt on *every* call. Cache writes are said
to cost more than plain input on gpt-5.6 (~1.25×). If that holds here, the game
is paying a write premium every turn and never once reading the write back,
which would make the openai path *more* expensive than having no cache at all.
`PRICING` bills one flat input rate and cannot see the difference, so only the
provider's invoice can answer it.

## Related

- `features/gate_idle_cognition_on_novelty.md` — reduces the *number* of calls.
  This reduces the *cost of each*, on a provider that supports it. They multiply,
  and neither depends on the other.
