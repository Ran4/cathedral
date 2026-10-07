Status: Planned (2026-09-05).

# M5 — Seeing, hearing and remembering observations

Give citizens causal knowledge of nearby events. This is required for both honest evidence and dynamic enforcement.

## Entry

M4 is accepted. The completed knowledge feature provides fact/source/learning APIs. M2/M3 provide persistence.

## Existing gaps

The current `Sight` trait is stored but not consumed by gameplay. `perception::sees` checks horizontal facing only. `you_see`, person-follow eligibility and updates of `last_seen` use proximity in important paths. Hearing uses Euclidean radius without walls or floors. Player gaze rays use static collision and omit the current dynamic gate barriers.

Connecting one trait method will not close these leaks. Audit every caller that uses distance as a substitute for visibility, attribution, reach or hearing.

## Implementation

Keep `characters_within` as a candidate query with its stable distance/ID ordering. Introduce semantic queries for visual perception, audibility, local interaction reach and observation of an act. These are related but not interchangeable: hearing a cry does not identify the assailant; seeing through a grille does not make it traversable.

Use M4 structural geometry and live portal revisions. Define actor foot/eye/source positions, facing, range and the relevant object target points. Use a spatial acceleration structure and candidate neighbourhoods, not every actor against every collider each frame.

Distinguish detection, recognition and visible detail, with vertical gaze and the declared effects of darkness/fog/weather visibility. Define a narrow exposed/carried/concealed presentation descriptor now for existing held props; M7 generalises it for placed/contained evidence. A visible actor does not expose their entire private inventory or the identity of a generic bundle. Host props/interpolation and observation must agree about which act is visibly available.

Open/closed portal tests use M4's actual physical state and fixture writers. They do not require M6's later legal access/key system. Add production access integration tests when M6 supplies those writers.

For hearing, implement a bounded room/opening model plus outdoor distance, with documented obstruction/attenuation behaviour. It must support an audible non-traversable window and prevent ordinary speech from granting detailed knowledge through two sealed rooms. Full acoustic wave simulation and NPC-eye screenshots are not prerequisites.

Migrate all semantic consumers together: prompt `you_see`, person-follow start/update, witness selection, sound attribution, speech recipients, identity learning, pocket/transfer observations, interaction focus, transcript delivery badges and relevant knowledge minting. Stage selection may remain broader for scheduling cost, but stage membership cannot grant perceived facts. Full authoritative recipient sets cannot become a player-facing count or an assertion that nobody else heard.

## Observation receipts

Record event-time recipients and the supported observation once. A receipt includes stable event/observation identity, observer, supported identity or description, time/interval, place/surface, channel, source event and geometry/access revision. Preserve uncertainty: an anonymous cry stays anonymous until another supported identification connects it.

Delayed transcription belongs to this temporal policy. Current host code samples position at `RecordingFinished`, despite a router comment saying recording start; `say` then selects hearers when transcription completes. Replace that latency-dependent audience with bounded capture-time coverage for the declared utterance interval, as specified in [SPATIAL_PROOF_PROTOCOL](SPATIAL_PROOF_PROTOCOL.md). A later arrival must not hear old words because an HTTP response was slow, and a departed listener must not lose an established receipt. Resolve semantic content only when available, without retroactively rewriting intervening decisions.

Process intermediate movement and portal changes in temporal order. Use conservative swept/subdivided checks and break continuity on an unresolved interval; two visible endpoints cannot certify a hidden exchange or door closure between them. Catch-up movement must not retroactively witness acts. Keep rendering-observation timing and open-panel world visibility explicit under [SPATIAL_PROOF_PROTOCOL](SPATIAL_PROOF_PROTOCOL.md).

Distinguish authored historical observations from live observations. Historical content can seed that Averil witnessed a past event, but the player must learn her account; no live camera or current geometry query retroactively proves it happened.

Apply retention through the shared knowledge system and durable referenced receipts. Do not store all camera frames or every movement tick forever. Keep compact observations needed by active facts, enquiries, institutional work and bounded recent perception.

Extend the accepted knowledge service with a bounded archived-identity/retention API as needed; M9 later registers inquiry/account roots. Current interim live-fact eviction can delete identities/holdings, so merely storing a `FactId` is insufficient. Separate cold referenced history from hot-news caps, retain transitive provenance roots and define quota/admission/saturation behavior under [RUNTIME_BUDGETS](RUNTIME_BUDGETS.md). Crossing 256 mints or six working holdings cannot erase essential supported observations.

## Following and continuity

A follow/search task stores a surface-qualified last-seen location and time. Update it only after actual current perception. Losing sight freezes it; hidden movement cannot update a marker, destination, prompt or notebook. Reacquisition requires another observation.

Watching a person carry a bundle does not prove it is a particular evidence packet. Continuous object identity is its own observation chain. If occlusion, a container or an exchange breaks that chain, mark it interrupted; later custody/examination can supply a different proof route.

Represent the early observation as a track with supported features, not authenticated knowledge of the private `ItemId`. Later uninterrupted handling/examination can connect that track to a specific original without rewriting what was visible earlier. M7 registers the richer object descriptor and transfer-phase data through this same interface.

Production spatial queries fail as unavailable/occluded where coverage is absent. `NullSight` cannot silently certify an investigation or an arrest. Explicit simple test worlds may supply their own complete fixture geometry.

## Tests

- Clear line, one wall, two walls, same XZ on another floor, open/closed door, visible grille and audible-only opening.
- A turned-away witness hears but does not visually identify an act.
- Prompt, action eligibility, follow update and knowledge receipt agree about the same blocked target.
- A hidden target moves while the follower's destination and player notebook remain unchanged.
- A bundle identity chain breaks at an occluded exchange and cannot be repaired by an omniscient item lookup.
- Identical visible endpoints with intermediate occlusion/portal changes preserve the temporal gap; a dropped movement span cannot mint continuity.
- A second unrendered held object, pocketed packet, two similar bundles and an observer unable to see the hands do not award exact item identity.
- Speech retains deterministic recipient ordering and the same hearing rules for the same channel/event. A listener arrival/departure or door change during STT latency follows captured utterance-time coverage, not completion-time geometry. Partial/uncertain coverage cannot grant the entire consequential proposition.
- Paired worlds differing only by an undetected listener retain identical player privacy feedback; authoritative hearing receipts may differ. No badge asserts an unseen audience count or absence.
- Save/load preserves last-seen age, source identity and interrupted observation state.
- Off-stage observations use the same queries as on-stage ones, without GPU screenshots or LLM calls.
- Profile default, 2,000 and 20,000-citizen cases; record candidate count and query time separately.

## Completion gate

No identified semantic consumer still treats proximity as proof of sight. Every evidence-producing sensory path has explicit support and negative tests. Covert following is now a real reusable capability, allowing M17 to include it without the GDD's previous stub-based deferral.
