# M3c first controls seam — 2026-09-21

This cut adds explicit input ownership and honest player-facing refusal. It
does **not** complete M3c's named slots, browsing, confirmation, save or load.
The accepted M3b2c startup recipe still refuses complete application admission;
no control may bypass that prerequisite or turn a component capture into a
save-anywhere claim.

## Actual authority

The existing immutable Bridge generation envelope, ordered host command
allocator and shared retirement lock remain authoritative. Its shared boolean
is replaced by one equally sized atomic route: `Staged`, `Active`, `Retiring`.
LocalEngine now constructs the endpoint staged before constructing services,
then opens it only after successful startup, before handing it to the App.
Construction failure closes it. Startup activation takes the unpublished
handle mutably, permits only Staged -> Active and cannot reopen Retiring.
The production code has no candidate-adoption activation API.

Both ECS and non-ECS sender paths check the same route under the existing
retirement lock. Staged/retiring commands cannot enqueue or mint semantic IDs.
Retirement leaves already queued old-generation commands tagged as before;
it does not drain or rebind them. Worker clones follow the same irreversible
fence. A new generation is a fresh endpoint, never mutation of an old sender.

Control requests borrow the actual LocalEngine and BridgeHandle. Identity
requires a started, live Engine, exact runtime generation, exact endpoint Arc
identity and the same committed immutable recipe. Merely constructing another
endpoint with the same generation number cannot confer control. Pre-adoption
failure leaves those owners unchanged. A load refusal cannot roll back a
retirement fence. Physical/controller/clock routing across a real App swap
remains blocked on complete adoption; this cut introduces no inactive ECS
world that could accidentally take player control.

## Player controls and bounds

F6 requests quick-save and F9 requests quick-load through the ordinary
SmartActorsPlugin HUD registration, including when actors are disabled.
Neither key was previously assigned (F5 screenshot and F7 navigation remain).
The controls run without pausing, changing cursor capture, consuming gameplay
keys, clearing drafts or advancing an input cursor. Their short HUD messages
state that save/load is unavailable and play continues.

`CheckpointControls` owns one inline terminal receipt and a checked monotonic
request counter independent of a runtime generation. A request stores its
action, observed generation and explicit refusal. Counter exhaustion refuses
without wrapping or replacing the previous identified receipt. Simultaneous
keys are processed in fixed F6/F9 order, at most two attempts per frame. Only
new input edges trigger requests. There is no payload, string, candidate,
retiring owner, result queue or task retained by the control resource.

A disjoint 512-byte Running lease is reserved before constructing this fixed
resource during startup staging; duplicate install checks it before changing
App resources. The resource drops its fields before releasing that lease.
This is a fixed control-owner allowance, not new whole-App/HUD/ECS accounting.
The existing HUD has at most one toast; these controls use a fixed short
literal through that ordinary mechanism. Full HUD/App lifetime accounting
remains in M3b's explicit unfinished inventory.

Requests require complete startup admission before any IO or candidate work.
Even future admission success will still meet a separate closed application
publication gate. Current failure is terminal and synchronous: no operation is
marked Saving/Loading, no service starts, and there is nothing waiting for a
timeout. Inventing a timer for nonexistent disk work would misrepresent the
result. Future async integration must supply its own bounded admission,
timeout/cancellation/result conservation and operation-to-world binding.

## Verification and remaining work

Focused tests exercise actual sender queues, shared worker clones and startup
construction, the real key system, fixed storage under 10,000 refusals,
operation-ID exhaustion and real LocalEngine continuation. A queued typed
speech command is conserved across both refusals and processed by the next
ordinary pump; no save-side pump is introduced.

Named/manual slot metadata, non-pausing browsing, explicit replacement/load
confirmation, durable publication receipts, complete staged ECS/host adoption,
physical and fixed-clock routing, atomic commit/rollback, asynchronous failure
deadlines and hidden-window/full-frame performance acceptance remain pending.
No filesystem/provider/device/window was opened by a checkpoint control. The
tests are offline and use fake/off services and minimal Apps only. Existing
storage/prompt/session evidence ordering and all historical fixture bytes are
preserved.
