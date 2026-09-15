# M2b admission and retention

All global limits are unchanged:1 GiB shared across Running, SavePayload,
LoadCandidate and RetiringGeneration;64/128 MiB encoded profiles;128 MiB complete
typed expansion. The512 MiB Running requirement is the existing trusted scoped
minimum and never substitutes for the host's actual owner/lifetime accounting.

The complete raw candidate is not a charge for a new Engine. Preparation reserves
a disjoint subordinate asset lease in the same LoadCandidate slot before any
factory work. Hydration independently meters the complete decode and its256 KiB
new root/control/speech-index allowance. That allowance is included in the128 MiB
expansion ceiling. No allocation cap is enlarged to make populated input fit.

The scoped actual-city test factory reserves64 MiB. Its proof is the sum of:

1. Cumulative requested allocation bytes on the synchronous factory thread,
   including all alloc/alloc_zeroed/realloc calls without subtracting frees.
   Original seed parsing, deterministic extra-resident definition generation,
   PromptEnv compilation and deep clones of every non-nav installed definition
   occur in this scope. No construction threads are spawned. Fixture execution
   also counts source-App disposal inside that scope.
2. Current retained storage of each distinct inherited World/config NavData Arc,
   including graph/index capacity and current distance-cache rows, plus64 bytes
   per outer Arc. Shared identical Arcs count once; distinct graphs count
   independently. Any internally shared distance cache in distinct NavData
   values would be conservatively overcounted.
3. An explicit1,048,576-byte shared Prompt/runtime allowance. The coordinator's
   [source-hashed MiniJinja audit](coordinator/prompt-shared-bound.json) derives a
   712,704-byte upper bound covering builtins, compiler TLS pools, small-integer
   cache and shared delimiter/callback/environment roots. These allocations may
   predate the factory and are not assumed present in its allocation counter.

Every sample asserts this complete sum is within the pre-existing64 MiB lease
and reports all terms. Public HydratedEngine access cannot warm navigation, and
the probes assert equal before/after inventories across factory, capture,
validation, construction and all16 category observations. Read-only source
tracing is recorded in [owner-design](owner-design.md). The full possible
navigation cache is402,644,224B and is not charged to this64 MiB scoped lease;
caller-retained shared Arcs require coordinated admission for later cache growth.
Process-global/TLS prompt caches outlive individual candidates and remain part
of the caller's continuously retained Running/process scope after disposal.
M3 must implement the complete active/retiring/ECS/services/cache lifetime model.

## Phase and destruction ordering

The six reported hydration checkpoints have this exact order:

| Checkpoint | Work completed since preceding checkpoint |
| --- | --- |
| Assets | Admitted host factory; generation-zero rejection precedes the factory. |
| Definitions | Validation scratch/diagnostics and structural storage admitted, fresh actual-asset resolver built, candidate manifest rebound. |
| TypedOwners | Full strict outer parse and all16 owner/cross-owner validations, one private typed graph retained; saved execution-fence reuse refused. |
| Construction | Checked graph moved into exhaustive actual World/Engine literals and explicit speech/host/cognition continuation owners. |
| RawDisposal | Original raw candidate destroyed after the final protected owner is formed. |
| Retention | Primary lease shrunk only after raw/scratch disposal; separate asset lease remains attached. |

Final retained cost is decoded_upper_bytes plus assets_upper_bytes. The primary
Admitted wrapper destroys its value before its typed reservation. HydratedEngine
declares its asset reservation after Engine and continuation owners, so their
destructors run before asset capacity is released. Temporary factory/preparation
owners use the same field ordering. The mutable mapping helper retains its
original reservation until closure-local allocations are disposed on success,
error or unwind. Arbitrary observer panic therefore cannot release a charge
before the owned graph/assets it covered are destroyed.

Actual-owner category hashing is a separate admitted SavePayload phase in the
same shared budget. Foreign-budget reservations refuse before scratch or owner
staging. Expected source digests are computed while Save exists, then Save is
dropped before actual-owner observations. The report includes complete predecessor
capture encoded/raw/expanded costs separately from new hydration expansion and
retention, together with aggregate budget high-water marks.

The System allocator wrapper is confined to the host test binary. Its cumulative
counter is a conservative byte request bound, not process RSS or allocator peak.
It does not modify the production executable allocator; measured probe timings
include its diagnostic hook. Neither these synchronous timings nor Engine's
existing non-Send services establish M3 frame-budget/offload acceptance.
