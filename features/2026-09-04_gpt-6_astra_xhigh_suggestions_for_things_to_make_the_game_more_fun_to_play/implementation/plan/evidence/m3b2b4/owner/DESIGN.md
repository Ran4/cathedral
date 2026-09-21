# Installed startup admission slice — 2026-09-21

Scope narrowed by coordinator during implementation: implement the explicit
installed recipe/accounting seam and integrate the startup path that can be
proved without a whole-App refactor. This is **not closure of complete b4
allocation** or permission for M3b2c adoption. Predecessor: `c2e23a7`.

## Actual production path

`main` creates one `InstalledRecipe` before starting session workers. Its shared
`CheckpointBudget` initially owns 4 KiB of fixed bootstrap control, not a mutable
world allowance. JSONL and stderr each start through the existing admitted sink
constructor and retain a disjoint 8 MiB service lease. The default/unadmitted
sink constructor is now test-only. Queue bounds, overload/refusal policy,
drive/session evidence fences and prompt archive behavior are unchanged.

App settings receive a separate 4 MiB lease before reading or parsing. Each
selected file is read into exactly 64 KiB plus one sentinel byte; a growing or
oversized file cannot extend this allocation. Oversized, malformed or non-UTF-8
overrides follow the existing fallback to default_config.ron and then built-in
defaults. Exactly 64 KiB remains supported. No setting field is truncated.

The concrete AppConfig contains eight String leaves and no arbitrary owned
collection: title, weather mode/quality, uv binary, TTS/STT selectors, idle mode,
and office. Total decoded strings cannot exceed source bytes plus the small
built-in defaults; repeat fields do not create a retained list. The 4 MiB scope
includes input/sentinel, parser/error scratch and up to sixteen startup copies
of these leaves. Main's config, PersistedConfig, plugin/resource and backend
option copies are within that supported startup scope. Backend environment,
keys, worker arguments, installed world definitions and mutable world storage
are explicitly outside it. InstalledConfig retains its lease through App::run
and App disposal; its own value drops before its lease. Scalar environment
overrides retain existing behavior. This is a source-specific startup owner,
not a general admitted API for arbitrary later configuration edits.

## Explicit recipe/accounting seam

`InstalledCost` separates source storage, navigation graph/index allocations
and the maximum retained derived navigation cache. Its borrowing-only two-role
inventory uses actual `NavStorageInventory`. Equal Arc graph identities count
once. A deep NavData clone has a separate graph but shares its OnceLock cache;
two independent parses have separate graphs and caches. Cache warmth changes
neither the maximum charge nor the inventory's behavior. This is inventory,
not an ownership wrapper for already-created NavData, and does not itself
admit graph parsing, cache-fill scratch or arbitrary escaping graph clones.

The actual shipped graph in focused-01 has **10,026 nodes**, graph/index plus
Arc storage **6,366,037 bytes**, maximum retained cache **402,644,224 bytes**,
and combined **409,010,261 bytes**. This inventory does not warm the cache.
The old `~60 MiB` comment in navigation source must not be used as an allocation
bound. Shared role identity matters particularly at this size. A NavData deep
clone also compacts some Vec capacities; each distinct graph must use its own
capacity inventory, not a multiple of the first graph's bytes.

`retain_sources` preflights borrowed bytes before any copying or index creation.
The closed supported limits are 4096 sources, 4 MiB per source and 32 MiB total
payload. Index headers, payload capacities, Arc control and allocation margins
are charged in addition to the payload. Sources become immutable boxed slices
behind one Arc owner. Cloning a handle does not copy or recharge payload; last
handle destruction drops source bytes and the entire index before releasing
the lease. Refusal does not consume caller data. This seam is ready for an
installed factory; production asset loading has not yet switched to it.

## Deliberately pending complete allocation work

The broader findings in `/tmp/alibi-m3b2b4-allocation-followup-notes.md` remain
open: bounded immutable backend Environment/BackendsConfig/WorkerSpec capture,
actual installed asset parsing/factory ownership, charges following all NavData
and shared cache aliases, cache-fill scratch, shared runtime/archive/preparation
startup, finite HTTP idle/redirect transport policy, ordered cognition payload
disposal, idle candidate realtime-task ownership, and mutable live/old/candidate
authority accounting. Session metadata/path startup scratch also remains outside
the diagnostic worker service allowance. This slice makes no aggregate
whole-App, two-world or RSS claim. Complete checkpoint operations still require
the unchanged aggregate 512 MiB Running allowance, 128 MiB typed ceiling and
1 GiB shared cap. The small bootstrap root intentionally cannot satisfy those
complete-operation checks. It must be replaced/extended by a proper adopted
authority owner before whole-App checkpoint startup is claimed.
