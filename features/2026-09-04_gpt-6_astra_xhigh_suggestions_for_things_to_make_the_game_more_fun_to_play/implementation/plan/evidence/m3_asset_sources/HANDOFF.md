Status: Focused checks and normal production build passed. Source/Cargo ownership ceded to root; review/commit pending, 2026-09-22.

# Owner handoff

The patch is applied to the shared tree on base b73c1eb. No commit has been made.
All owned commands have exited; no Cargo process or other owned command remains
live. Source/Cargo ownership is explicitly ceded to root. Unrelated untracked
paths are preserved.

Changed code: backend world_data/capture and its tests, a pure source-composition
entry point, a borrowed LoreCast entry point sharing the old implementation,
installed startup/owner storage, and the installed LocalEngine read path with
startup witnesses. Existing owned-input APIs remain. Source admission refusal
maps to StartupRefusal::Admission; IO/content limits map to Sources. Disabled
actors skip capture while retaining existing backend staging behavior.

The source owner reserves its supported peak before discovery/IO/copies, then
shrinks only after scratch disposal. Boxed data and indexes drop before their
reservation; shared handles retain it through the last consumer. The supported
filesystem-owner inventory is reviewed for x86_64 Linux/GNU. Native DIR allocation
and other platforms remain outside that proof.

The current actual evidence is in README.md and owner/. Both focused iterations
passed with unchanged maps. The first backend9/lore5 witnesses retain their own
source/build identity; a source-delta record proves their implementations/data
were untouched by the disabled-actor correction. The final host modules passed
six and two tests, including the maximal source-allocation witness and real
builder routing. The normal build is a separate exit-zero record with the same final source map
and an executable hash; the executable was not run. No full workspace rerun is
claimed. The owner audit checks all archived hashes/maps against the current tree.

Remaining gates: native directory allocation, parser/compiler/generated-crowd
bounds, backend environment/configuration and transport ownership, the production
HydrationAssets factory, actual Running/candidate/retired mutable authority,
inactive ECS staging, adoption/controller/projection restoration, functioning
save/load controls and host-frame/renderer acceptance. Complete admission remains
unconditionally refused. No new 512 MiB world root or parser allowance was added.

No window, audio/device or provider was invoked. The normal executable is only
built and hashed. Root should review this partial slice and its scope before
committing; no M3–M6 completion or M7 advancement is authorized by these results.
