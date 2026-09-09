# M2a14 cognition input admission — 2026-09-09

Status: M2a14 component implemented, verified and independently accepted (2026-09-09); see [coordinator review](coordinator/review.md). Complete M2 and host adoption remain pending.

All global ceilings remain unchanged: 128 MiB encoded J, 128 MiB expanded E, depth 64 and shared 1 GiB. The sidecar uses the existing streaming aggregate meter and retains `4,096 + 4*E + 3*J + 4,194,304` bytes per cohort. Raw padding and escape charges remain attached through candidate conversion. Typed parsing starts only after the entire raw lexical cost and working allowance fit; refusal/disposal release the lease.

Context construction only stores borrowed references. Export first streams the allocation-free flight view, then reserves the working allowance, then validates old owners, then clones at most two exact prompt strings and their identities. Missing legacy authority cannot serialize even during preflight. Decode reserves raw parsing and working bytes before invoking either existing owner's validation. Repeated candidate validation uses its retained lease.

The sidecar has no map, vector or dynamic validation index. At most two fixed row records hold actor/subject/root/request/lane/day/incarnation/method/budget and the exact prompt. Each row's outer object contributes 512 E plus its eight key charges before string/root/subject/scalar values. The five-field top object similarly contributes 512 E plus five key charges. Existing bounded strings account for owned contents and serde escape scratch. Four-E capacity covers the fixed DTO/candidate layouts, exact exported copies and bounded error/serializer work.

The 4 MiB working allowance reuses the established M2a11/M2a9 sequential proofs. Scheduler's saved-ledger validation completes before lane/root sorting; its scratch is released before Night validation starts. Night's ledger/clock binding completes before its subject/root sorting. No saved candidate, ledger or World is cloned during these checks. The two old flights gain only one inline enum; their original closed-record/object/key charges already exceed those layouts with substantial headroom, without increasing prior E/J or working charges.

The new sidecar does duplicate its two existing-owner prompts on export, at most 131,072 decoded bytes combined. These copies are fully charged for their retained lifetime; no claim treats them as free borrowed storage after export. Running gains only the two inline fields and no prompt duplication. Old V1 candidates explicitly erase the new enum and remain honest historical components.

Measurement uses opt-in modes of the established scheduler/Night authored520 and +2,000 all-placed2520 setups. Defaults keep their historical workload/metadata. Each opt-in run times six sidecar phases and retains exact actual submitted method/request/prompt/budget witnesses and final saved rows. These are component measurements, not full-save maxima or renderer/host-frame evidence. Actual composition must solve the already-known naive backbone+Round Save+Load peak of 1,256,093,444 B before Running; no cap increase is proposed.


Measured fixed layouts on rustc 1.96.0 x86_64: sidecar DTO/candidate 248 B, scheduler row 96 B, Night row 112 B, borrowing-only context 48 B and AcceptedOutputBudget 8 B. Existing scheduler Flight is now 144 B and NpcScheduler 384 B (V1 DTO 400 B, Engine V1 DTO 440 B); Night Flight is 104 B and NightOffice 312 B. Each live owner grows by 8 B. The original fixed record charges exceed these layouts; no old V1 allowance increased. The emitted scheduler scratch bound is 2,580,480 B for conservative borrowed-ledger sets, then separately 400,000 B lane pointers plus 4,112 B roots. Night validation runs after these allocations drop. [Current evidence](development/allocation_evidence.json) also verifies all eleven prior installed allocator/parser source hashes and matching compiler version output.

| Debug workload | Actors | J bytes | E bytes | Per-cohort peak | Save + Load excluding Running |
|---|---:|---:|---:|---:|---:|
| scheduler authored | 520 | 20,874 | 44,938 | 4,440,774 | 8,881,548 |
| scheduler populated | 2,520 | 20,034 | 43,258 | 4,431,534 | 8,863,068 |
| night authored | 520 | 5,395 | 14,512 | 4,272,633 | 8,545,266 |
| night populated | 2,520 | 5,395 | 14,512 | 4,272,633 | 8,545,266 |

All exact primary submitted inputs and saved rows remain in the debug JSONs. Scheduler uses 136 ordinary polls with observed maximum 0.040000000000000036 s; Night uses 405 nominal-50 ms polls with observed floating-point delta 0.05000000000000071 s (within 1e-12 s of 50 ms). Both report zero discarded physical work, and both populated variants place all 2,000 additions. Existing default-mode scenario/count/witness/mode/placement values exactly match historical accepted metadata. No debug timing is promoted to release or host-frame acceptance.
