Status: Native backend retention accepted after independent source, boundary and evidence review (2026-09-15); owner source/Cargo/executable cession received at 10:31 UTC; ready for commit.

# M3b2b2 coordinator review

Accepted base is M3b2b1 commit
`b37d8fcff58efcd219766a98fd0e1a83b731df7c`, whose archive admission and
shared payload/held-result contracts remain intact. This cut closes bounded
native work and its real termination ownership. Complete application allocation,
startup admission and whole-App adoption remain successors.

## Accepted implementation

Local dependency inspection established an actual production blocking path:
reqwest 0.12.28 delegates default DNS to hyper-util 0.1.20's spawn_blocking,
and Tokio 1.52.3 also resolves string addresses on blocking workers. Every
production HTTP client now receives the actual runtime's finite resolver;
websocket connection resolves SocketAddr values before negotiating against the
original URI. No host src call site uses Tokio or BackendRuntime directly.

Runtime limits are two async plus two blocking workers, each with an explicit
2 MiB stack. Two DNS slots include queued/running non-abortable calls and retained
answer iterators. DnsWork owns host/operation before its permit; Answer owns
addresses before that permit. The shared retention core owns a 16 MiB allowance
through actual Runtime joins and surviving resolver/executor/answer owners.
Worker closures hold an executor that cannot own or join their Runtime.

STT batch/discard and TTS keep native JoinHandles and outer generation owners
through off-frame joins. Owned discard admission returns the exact PathBuf on
full/inactive/oversize refusal; one active unlink and 64 queued paths remain
owned through disposal. Each local Worker includes active children and unjoined
logger/reaper pairs in its two-generation bound. Close is terminal. Reaper-spawn
failure preserves the Child for synchronous reap on its existing native caller;
logger-spawn failure also reaps before returning. A process exit does not release
a slot while its inherited stderr reader or native reaper survives.

Realtime cleanup retains one close per SessionTask with a one-second timeout.
RetainedClose declares its close future before the endpoint and slot, and has a
Drop implementation preventing split capture. Actual cancellation before first
poll and while suspended cannot release admission before transport destruction.

## Actual owner paths and scoped allocation

LocalEngine::fail calls retire_runtime and preserves Engine/ForwardingServices.
Failed Engine::new drops adapters while real services remain retained. The
retirement transport disposes Send services before its BackendsHandle guard.
Startup parses shelters after constructing idle STT/TTS, but has submitted no
provider/speech jobs; realtime awaits its first action. Failed/cancelled prepared
services retain their original worker disposal route. Ordinary reserve, submit,
cancel and fence operations do not perform native joins. Normal final teardown
and off-frame disposal now wait for real native termination. A hung resolver can
retain admission and cause later refusal; it cannot release its charge early.

The 16 MiB runtime allowance comprises 8 MiB configured stacks plus separate
trusted 4 MiB DNS scratch and 4 MiB runtime/TLS/allocator/control scopes. One full
generation's speech bundle can retain eleven native threads with 22 MiB explicit
stack extent. The two local drivers bound four directly managed Child generations,
not arbitrary descendant processes or external model/GPU heaps. These are scoped
inventory/retention guarantees, not a process heap census or proof that an old
world reservation covers every service. Default production services are finite;
the tested admitted startup seam still needs the later installed recipe.

## Independent tests and corrected development failures

Root supplied three tests against actual admitted runtime/disposal workers:
exhausted DNS answer plus executor survival after Runtime join; two blocked DNS
calls after async waiter cancellation through off-frame shutdown; and a real
full recording-disposal queue through fenced off-frame cleanup. Each proves old
retirement/shared charges release while a separate Running authority survives.
Owner tests cover DNS host/answer capacity, retained answer slots, repeated real
child poison/reaper saturation, bounded realtime close/timeout and blocked
transport destruction at both cancellation states. All eight pass in the final
workspace, alongside 26 explicitly audited predecessor boundary witnesses.

The first root fixtures omitted the Running authority required by admitted
runtime startup; all three refused before creating native work. That failed run
is preserved. The later source review found the realtime capture destruction-order
gap and extended the same ordered-aggregate rule to queued DNS work. The first
workspace attempt was interrupted by its owner with exit 130 after 333.3252 s,
before source changes; its raw log, map and gzip remain preserved. The explicit
result records interruption finalization. Earlier passing focused maps remain
distinct from final acceptance. No failed or interrupted attempt is counted as
a final pass.

## Final evidence audit

Final source map: `320640c264e4646c374ccbc3c8681ea14a05dddede6b273810fff46e8feaf0ef`.
It contains 1,007 inputs: two new Rust files, eleven changed predecessor
Rust files, 994 unchanged inputs and none removed. Final-format-02 checks
all thirteen changed/new Rust files on this exact map. Focused-backend-02 passes
202 / 0 / 3, and final-workspace-02 passes **2,300 / 0 / 47** in 46 printed outer
groups, in 1172.202426 seconds. No unprinted fresh-process child total
is added. Source maps, environments, raw logs, deterministic gzip bytes and
runner identities are checked by the retained independent audit scripts.

- Workspace raw SHA-256: `6d231160b1feaa49ad5b1e9da7f550ecd3ffc8628783b6256b3c3aff1f40e682`.
- Workspace gzip SHA-256: `940bf29332fed3e949a6093abfb26e576d4dc2b040a7923994ca33f3fe79bdae`.
- Backend ELF: 387,831,896 bytes,
  `87709de68c2168a4b1518678450556e5234e2124e972a8ab1fb13a795a794024`.
- Sim ELF: 333,258,360 bytes,
  `6d18307944eb721d9fc1083068b6d676bcad1af29566f74d773e683594868888`.

All 46 read-only GDB layout queries are independently audited against those
actual test images and the frozen command. Preparation fixed control remains
**60,952 / 65,536 bytes**. These are test-ELF layouts, including test-only fields;
they do not measure dynamic heap/TLS allocations. Images were hashed in place;
no separate copies are preserved. A later build may replace these paths.

Diagnostic sinks, immutable configuration/assets, complete disjoint running/
candidate/retiring admission, whole-App adoption, controls and frame-budget
acceptance remain pending. The aggregate 512 MiB Running minimum, 1 GiB shared
cap and 128 MiB typed limit are unchanged. Renderer, provider, device and human
acceptance were not performed in this CPU/native lifecycle cut.


All 19 owner command records passed the completed-command audit, including the
explicit historical failure/interruption records. All five final checks share
the corrected map. Root's post-documentation git diff --check also passes.
The unrelated .codex/, docs/codex_gdd/, gauntlet/, reference/ and
features/2026_09_14_more_ambient_stuff.md paths remain unstaged and untouched.
