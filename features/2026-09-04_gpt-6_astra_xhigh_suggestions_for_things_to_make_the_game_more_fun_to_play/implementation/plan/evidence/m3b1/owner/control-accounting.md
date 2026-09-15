# Preparation service control allowance

The 4 MiB persistent Running sublease is a scoped conservative allocation
contract, not an RSS or whole-world measurement. It covers 2 MiB explicitly
requested worker stack plus 2 MiB fixed control/thread overhead. Variable-capacity
candidate records, raw bytes, installed assets, owned factory captures, concrete
services and retired graph/queue payloads retain their separate charges.

The worker has exactly one preparation entry and one retirement entry, no
unbounded job/terminal vector, no per-frame history, and fixed phase arrays.
Preparation service identity is an AtomicU64 and IDs are two u64 values.
Retirement completion is a Weak observer, not another retained value bundle.

## Layout derivation

Read-only GDB `sizeof` queries use the final backend test ELF's DWARF and do not
run the target. The exact command, image identity and output are preserved by
`final-control-layout-*` and `final-images.json`. Linux x86_64 layouts are:

| Root | Bytes |
| --- | ---: |
| Core (including Queue) |11696|
| Queue (already inside Core) |11640|
| PrepState |11504|
| PreparedDelivery |11600|
| DeliveryDisposal |11384|
| RetiredPayload (Box+lease handle only) |24|
| Job |440|
| PreparationPermit |448|
| RetirementPermit |32|

A conservative Core plus four simultaneous largest extra inline
owner/control slots, both permit roots and one extra Job is
11696 + 4 * 11600 + 448 + 32 + 440 = 59016 B, below 64 KiB. This deliberately
allows transient ownership moves across host return/worker delivery; actual
state-machine exclusivity reduces simultaneous values. Queue is not added twice.
Fixed slot/container capacity stays charged while empty. Pointed-to graphs and
captured recipe buffers are not included in this control calculation.

The existing pure hydration structural witness additionally measures the sum of
DecodedHydration, HydratedEngine, Engine, World, HydrationAssets, HydrationOwners,
ValidatedOwners and bounded speech index/wrappers against its unchanged 256 KiB
structural ceiling. Its final-workspace output records the actual sum. That
candidate allowance is not a credit against the service's fixed slot capacity.

Within the 2 MiB non-stack service allowance we reserve:

- 64 KiB for the above fixed roots and overlapping inline control slots;
- 64 KiB for Arc/Box/channel synchronization handles, allocation rounding and
 bounded small wrapper/control metadata;
- 32 KiB for bounded metadata/diagnostics: each retained error copies at most 4096
 UTF-8 bytes into bounded capacity, and there is only one preparation terminal;
- the remaining 1888 KiB as an explicit conservative native thread/TLS/allocator
 bookkeeping allowance. This last allowance is a trusted runtime assumption,
 not an independent allocation census. It does not absorb arbitrary graph,
 provider/device or process-global queue allocations.

Factories and destructors can produce larger error/panic allocations only under
their separate admitted recipe/asset/service/retirement charges. Those payloads
are destroyed before the corresponding charge releases; the retained control
terminal has bounded diagnostic capacity. Factory captures are constructed only
after recipe admission, or must already retain an independently admitted owner.

The LocalEngine retirement fixture prints its actual queue/domain storage
inventory and fixed authored/fake/embedded-navigation assumptions beside its
trusted 160 MiB retirement bound. It does not measure the complete live heap.
Global prompt writer queues and external provider/runtime allocations remain
M3b2 closure. The 512 MiB Running minimum is not substituted for that accounting,
and this cut makes no populated two-world 1 GiB residency claim.
