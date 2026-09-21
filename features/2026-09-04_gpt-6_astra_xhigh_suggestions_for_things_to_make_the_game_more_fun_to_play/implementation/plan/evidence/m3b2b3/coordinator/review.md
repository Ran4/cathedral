Status: Diagnostic sink design and source review in progress (2026-09-15).

# M3b2b3 coordinator review

Accepted predecessor is native retention commit
7a16a572e5c23010eea5bacf72c26c10a07ba086, freeze
320640c264e4646c374ccbc3c8681ea14a05dddede6b273810fff46e8feaf0ef.
It passed 2,300 workspace tests and independent source/command/boundary/layout
audits. Diagnostic sinks, immutable installed recipe, complete application
allocation and whole-App adoption remain pending.

## Execution constraint

features/AGENTS.md and OWNERSHIP_AND_GATES.md request fresh-context sequential
owners. Both attempts to create a fresh M3b2b3 agent returned `agent thread limit
reached`; the inventory showed three completed non-root agents and no running
implementation owner. The available tools expose no close/reset-agent operation.
To continue the user's authorized work, root reused completed /root/m2a15_host
with a new scoped handoff and instructions to re-read current source. This is
an explicit context-isolation limitation, not a claim that the agent is fresh.
Only that owner may implement/run Cargo until cession; root independently reviews
and tests the new boundaries. Root owns top-level status and this coordinator
namespace. Native M3b2b2's owner is finished and is not working concurrently.

## Initial source findings

Session JSONL allocates complete record Maps/Strings before checking its staging
high-water mark, and synchronous overflow flush can reach the frame. Its staged
and active scratch Vec capacities are retained. FieldCollector can allocate
unbounded debug/string fields before sink admission. The separate stderr queue
is unbounded and falls back to synchronous output if its thread could not start.
Drive/session JSONL currently has explicit synchronous evidence flush behavior;
its truthful acceptance/exit policy must be preserved in any bounded design.

The concrete design, production seams, overload policy and new independent test
file are to be agreed before implementation. Required prompt archive admission
and all accepted native cancellation/termination contracts remain intact.
