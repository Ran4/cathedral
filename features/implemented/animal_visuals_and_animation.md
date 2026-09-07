Status: implemented (2026-09-07). Four visual loops capped; remaining gaps recorded.

# Street dogs and rats: appearance and personality

Improve the existing animals in four sequential gauntlet passes: dog appearance,
dog animation, rat appearance, rat animation. Budget: 3–4 hours for dogs, then
about 1.5 hours for rats. Use independent, fresh-context critics of captured
artifacts; keep a record of failed rounds as well as accepted improvements.

| Milestone | State |
| --- | --- |
| M0: dog appearance | Implemented; six-round reference comparison capped |
| M1: dog animation and quiet behavior | Implemented; six-round reference comparison capped |
| M2: rat appearance | Implemented; six-round reference comparison capped |
| M3: rat animation, regression checks and HTML collage | Implemented; final R5 rat source selected after rejecting R6, checks complete |

The dogs should have readable canine anatomy, faces, distinct authored coats
and builds. Their quiet moments should reward watching: attention, sniffing,
ear movement, weight shifts and occasional longer actions with convincing
transitions. Locomotion must stay connected to the authoritative motion.

The rats should have readable rat proportions and faces, purposeful scurrying,
small alert and grooming actions, and responsive transitions into existing
scatter behavior. Preserve the existing navigation and colony rules.

## Scope and budgets

Own dog rendering and narrowly necessary dog simulation changes; own rat
rendering and animation in `src/city/vermin.rs`. Animal-only helper modules,
capture tooling and evidence are in scope. Another agent owns conversation
work: do not edit its prompts, bridge, interaction or scheduler changes.

Preserve shared dog mesh/material handles and the rats' reusable single mesh
batch. No fur shells, new per-animal lights, texture streaming in the update
loop, new colliders, cognition calls or world revisions for cosmetic behavior.
Measure animal CPU work and geometry budgets. Record the actual renderer and
avoid reporting software rendering measurements as GPU performance.

## Evidence and delivery

Use an invisible, silent Bevy capture harness with fixed side, front, three
quarter and street-distance views. Capture motion sequences, not only selected
poses. Keep baseline and iteration frames. Verify finite geometry, grounded
feet, transitions and distance behavior with focused tests as appropriate.

Deliver a browsable HTML collage covering both species: before/after images,
the steps taken, added animation sequences, critic findings and performance
evidence. Keep the artifact and its assets together for viewing tomorrow.

## Implementation record

Dog geometry, coat surfaces and motion now live in animal-only helper modules.
The torso and lower neck share a continuous surface; the face has shaped lids,
eyes, nose and lip details, and the legs have formed paws and rear hocks. Six
coat/build forms share mesh and material handles. Two small mipmapped coat
textures replace broad repeating vertex-color stripes. The appearance pass
settled below 6,000 triangles per dog after rejecting an over-detailed first
draft of roughly 14,600.

The animation pass adds explicit paw contacts, articulated hocks, blended skin
at the neck and limb joins, and deterministic per-dog quiet actions. It uses
the existing authoritative root and distance clock. DogSample and DogView
remain unchanged. Longer rests, a deterministic choice of walk or trot, gradual
starts, braking approaches and bounded turns are the narrowly scoped simulation
changes. The original destinations and waypoints remain authoritative. Cosmetic
animation itself publishes no events.

The rats retain one opaque vertex-colored batch and their existing colonies,
weather gates, baked routes and scatter validation. Their continuous hull now
has cupped ears, eye/nose details, formed feet and toes, whiskers and a tapered
curved tail: 604 vertices and 996 triangles per rat. The six-round appearance
loop remains below the external reference in muzzle/jaw and thigh continuity.

A fixed-size per-rat pose observes the final validated position, including the
scatter offset, to drive distance-based feet and retained stance anchors.
Pauses contain seeded sniffing, low alert poses and paired face-washing
strokes from a supported crouch, with nose/ear details and following tail motion. Action selection uses
the remaining route pause; interrupted grooming eases out into the escape.
The last experimental hand-landing delay was rejected because it stretched the
forelegs visibly; the exact tested fifth motion revision is selected as final. Local thresholds remove invisible easing residues that otherwise
caused a repeatable subnormal-arithmetic timing spike. Hidden-time jumps
reset the pose without advancing a giant stride. No new simulation events or
per-rat entities are introduced.

The studio compiles the production animal code, creates an invisible silent
window, and records source hashes with each capture. CPU measurements cover
dog systems, transform propagation, actual skinned-bounds/joint arithmetic,
and rat geometry separately. The available Vulkan device is llvmpipe; hardware
GPU timing and a completed full-city visual capture remain unverified.

The live review notebook is `gauntlet/animals.html`; `gauntlet/state.json`,
`gauntlet/NOTES.md`, source snapshots and captured frames preserve the audit
trail. `gauntlet/CAPTURE.md` documents reproduction commands.

## Final validation

52 focused tests pass across dog rendering, dog simulation and rats; the selected
full game passes cargo check. Final CPU fixture: 10 dogs at 0.022526 ms median,
50 rats at 0.748889 ms median, with zero steady-state allocations. Additional
skin bounds/joint arithmetic costs 0.012801 ms median in the isolated fixture.
The four visual loops each reached six rounds without meeting the two-pass
reference bar. Remaining anatomical and support/coordination gaps are explicit
in the notebook; rat R6 was rejected for overstretched forelegs, preserving R5.

The self-contained HTML includes 19 silent videos and 199 unique embedded media
files. Desktop and narrow-layout browser checks pass, all images decode and all
videos fully decode. Hardware GPU/upload performance and a completed full-city
visual capture remain unverified. See gauntlet/final_validation.json for evidence.
