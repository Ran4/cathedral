# M2a10 existing social authority — 2026-09-08

Status: Implemented, independently reviewed and accepted (2026-09-08). Existing component scope; complete M2/M3 remain pending and global caps are unchanged.

| Owner | Exact saved authority |
|---|---|
| Conversation.engagement | Required nullable actor, exact at, reciprocal flag and unique prior-witness BTreeSet. Maximum 128 witnesses. Target need not be in witnesses. |
| Conversation.invitation | Independent required nullable actor/at; unsolicited invitations never replace the incumbent during decode. |
| Conversation.focus | Required nullable actor/since/last, including partial dwell, backwards samples, expired samples and future historical anchors. No since<=last assertion. |
| Conversation tokens | next_utterance and latest_applied_utterance, with latest<=next. All u64 values survive, including legal saturating MAX. |
| WarmExchanges.pairs | Exact BTreeMap of canonical distinct unordered actor endpoints to last-line at. Explicit 25,000 pair format cap independent of current roster. No load-time expiry or current-body membership gate. |
| Novelty.last_told | Exact unique actor map, each Memory with required nullable opaque context:u64, opaque visit:u64 and touched_at. 25,000 memory format cap; historical identities survive. Neither opaque integer is recomputed or interpreted as a timestamp. |
| Engine composition | conversation, npc_exchanges and novelty; original config.idle_mode, stage.radius_m/max_actors, idle_requires_news and idle_curiosity.enabled/scale; immutable player_id binding. |

All social actor identities obey the existing nonempty, at most 128 Unicode scalar, no-control rule. Logical anchors retain their original IEEE value in the supported finite nonnegative range through MAX_LOGICAL_SECONDS, including signed zero and subnormals. The component boundary must agree bitwise with the supplied logical boundary. Historical public/debug samples can exceed the supplied boundary, so this component does not impose a universal temporal ordering. Complete accepted-time/all-consumer horizons remain a full-envelope gate.

Stage radius and Curiosity scale retain raw IEEE bits through strict closed {bits:u64} records, including NaN payloads, infinities and negative zero, because ordinary consumers deliberately apply fallback/clamp behavior. max_actors is encoded by checked u64 conversion and decoded by checked usize conversion. 0 and usize::MAX remain valid: it is a comparison against actual stage size, not an allocation request. Initial idle mode does not reconstruct the scheduler's actual order. The later scheduler owner must preserve that order independently.

Engine context borrows either World or an unadopted BackboneCandidate and an independently supplied immutable player identity. Both exact ID agreement and the player's body are required. Other historical IDs are not required to exist today. No World constructor, seed loader, prompt rendering, social expiry, context hashing, scheduler creation or ordinary poll runs in production decode/candidate conversion. Candidates expose only immutable owners, configuration values and counts; test-only private installations first scramble all covered state and demand immediate canonical equality.

CapturedAttention and SpeechRouter pending microphone/STT execution are explicitly outside this component. Inventory draft policy discards interrupted unsent captures/jobs; do not auto-speak on load. Conversation counters and earlier committed attribution suffixes remain authority through their owners. The private old-capture test supplies a controlled external captured value to restored Conversation; it does not claim restoration of active STT. Runtime generation fencing, scheduler/Floor/SpeechRouter adoption and all other pending execution remain future owner/M2c work.

The old attention comments that forbade saving WarmExchanges/Novelty are corrected. Outside new checkpoint module declarations those comments are the only changes to existing ordinary source; behavior is preserved. Complete owner/root/envelope agreement, exact content/build/target/toolchain/DefaultHasher binding, Running/Save/Load/retiring lifetimes, M2b capture/hydration, host initial publication and full M2c continuation remain pending. Prior naive backbone+Round Save+Load already exceeds 1 GiB; these social components do not resolve it or increase caps.
