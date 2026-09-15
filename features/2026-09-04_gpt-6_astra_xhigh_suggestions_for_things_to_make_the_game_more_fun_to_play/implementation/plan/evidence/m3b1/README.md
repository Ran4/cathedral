Status: M3b1 implemented and independently accepted (2026-09-15). Complete application adoption remains M3b2.

# M3b1 — Checkpoint preparation and retirement transport

Accepted predecessor: M3a `5559d26654faba677971ceeef66f0869495e6489`.
This leg consumes its actual admitted slot files, validates and decodes all
sixteen checkpoint owners on a bounded worker, and returns an opaque candidate
for host-thread Engine construction and continuation preparation. No creation
seeding or extra simulation poll occurs.

A candidate retains its original worker return slot through every preparation
stage, rejection, cancellation and service shutdown. The separate retirement
slot receives the actual old LocalEngine domain, services, queues and PromptLog.
It fences old producers before detachment and drains/destroys those owners on
the worker. Surviving mailbox jobs, endpoints and publication owners continue
to pin the retirement charge.

The worker's explicit stack and fixed control allowance add a disjoint 4 MiB
Running charge. Candidate, asset, recipe, service and retirement ownership keep
their own charges under the unchanged 1 GiB aggregate cap. These contracts do
not establish a whole-process heap census or a two-world residency bound.

## Review and verification

The [independent review](coordinator/review.md) explains the ownership
boundaries, eight coordinator tests and two defects found by full verification:

- The storage worker relied on closing its directory descriptor to release its
  flock. A deterministic duplicate-descriptor test reproduced the retained-lock
  failure. Explicit unlock now runs under the Store guard, including startup
  sync failure. Live concurrent writers remain refused.
- New failed-binding storage followed its reservations in struct field order.
  The existing host lifetime test caught services outliving their charge. The
  field order is corrected, with an additional retained-candidate regression.

Both failed workspace commands, the pre-fix lock witness, development failures
and the rejected Cargo target invocation remain in the evidence. They are
distinguished from the final unchanged-source acceptance run.

The focused public continuation module passes eight tests; the corrected
storage suite passes eighteen with two intentional ignores. Final formatting
checks cover all 25 changed/new Rust files. The final concurrent workspace run
passes **2,272 tests, zero failures, 47 ignored checks across 46 outer groups**.
The separately printed fresh-process child passes one additional test.
`final-workspace-03` ran for 771.977 seconds on an unchanged source map.

The [source delta](coordinator/source-delta.json) compares the actual frozen
inputs against accepted M3a: 1,002 inputs, nine additions, sixteen changes and
977 unchanged predecessor inputs. The freeze SHA-256 is
`ea4086cf10fdf5e6d0781f3191e2f73cc9dcd7460889a4d98646a1f993783d80`.

All independent audits pass:

- [Command audit](coordinator/command-audit.json): all 28 completed command
  records, original raw logs, lossless deterministic gzip archives, source maps,
  exact runner provenance and separate outer/printed-child test counts.
- [Phase audit](coordinator/phase-audit.json): fifteen required boundary tests,
  eight preparation samples, four shutdown stages and actual old-host retirement.
- [Image audit](coordinator/image-audit.json): final test ELF identity and nine
  read-only layouts, with 59,016 bytes of conservatively counted fixed control
  roots within the declared 64 KiB allowance.

The final raw log SHA-256 is
`8d3e8b9ca34a419953efb828e3f7e1441b1f8c6b3bf70f7fb3b461d2f72cc21e`;
its [lossless archive](owner/final-workspace-03.log.gz) SHA-256 is
`f9efa0599ef866ca66d9cac9fa2ee2a6f790a4871b3932e2baa4f041fa8c674e`.
The final test ELF was hashed in place; a separate executable copy and persistent
M3b1 slot fixture were not retained. The accepted M3a release image remains
unchanged.

## Evidence scope and next owner

The real-file fixture starts a fresh process from the same test image and
compares all sixteen hydrated owner digests to the captured source. Eight debug
authored samples separate worker preparation, host construction/continuation/
return and worker disposal. A real LocalEngine retirement test separately
records detachment and worker cleanup, including an injected PromptLog delay.
These are transport observations; populated-city p99, renderer frames and
whole-App acceptance remain unmeasured here.

See [implementation contracts](owner/implementation.md),
[fixed control accounting](owner/control-accounting.md),
[storage-lock correction](owner/storage-lock-lifetime.md) and
[service drop-order correction](owner/service-drop-order.md).

M3b2 must implement complete application adoption: actual allocation transfers,
remaining detached runtime/log ownership, inactive ECS staging, restored
controller/clocks/host state, initial publication and the final exclusive swap.
M3c supplies normal player controls; M3d verifies the complete host path.
M3 and M4–M19 remain incomplete.
