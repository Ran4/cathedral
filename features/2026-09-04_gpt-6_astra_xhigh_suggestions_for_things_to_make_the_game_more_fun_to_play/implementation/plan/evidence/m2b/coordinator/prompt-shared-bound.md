# Shared PromptEnv storage bound

The coordinator independently inspected the Cargo.lock-selected MiniJinja
2.21.0 source and installed Rust 1.96.0 x86_64 library. The executable auditor
records exact source hashes, compiler identity and counts in
`prompt-shared-bound.json`. This correction includes shared library owners that
an allocator counter around a subsequent `PromptEnv::new` does not observe.

`Environment::new` clones process-global Arc maps of filters, tests and globals,
and a formatter callback. Default auto-escape callbacks and optional syntax
delimiters also use OnceLock Arcs. The source contains 49 filter insertions,
42 test insertions and four global insertions across all feature branches.
Counting aliases independently is conservative. All keys are borrowed static
strings. Values are built from function items, without invoking those functions.
Each callable has one Arc for its zero-sized function closure and one Arc for the
BoxedFunction object; the optional debug name is a borrowed static string. The
type-erasure wrapper consumes the latter Arc and uses a static vtable, with no
additional heap allocation.

The library's own layout assertion specifies 24-byte Values. A conservative
32-byte bound for both Value and Cow strings gives at most 704 bytes for the
eleven key/value slots of a BTree node. Parent metadata, alignment and twelve
child pointers keep a node below 1,024 bytes. Nonempty retained nodes cannot
outnumber entries. Charging 128 entries at 1,024 bytes per node plus 128 bytes
per callable pair, and another 4,096 bytes for map/Arc roots, gives 151,552 bytes.
This deliberately counts shared alias functions more than once.

Code generation has two thread-local vector pools. Each retains at most 64
buffers of capacity at most 64. PendingBlock's largest variant contains a Vec
and scalar; Span contains six integer fields. Both fit below a conservative
64 bytes per element. Recycling clears PendingBlock values, disposing their
nested jump vectors before pooling the outer allocation. Therefore
`2 × 64 × 64 × 64 + 8,192 = 532,480` bytes bounds both retained pools and root
vectors. Borrowed old buffers used during compilation remain covered by this
allowance; new buffers and compilation scratch join the measured cumulative
factory allocation count.

The small-integer cache holds 256 boxed decimal strings of at most three bytes;
16,384 bytes covers their data, Vec slots and root. Another 4,096 bytes covers
the default borrowed delimiter record and callback Arcs. A final 8,192 bytes
covers empty TLS/environment roots. The PromptEnv constructor does not render
templates, call context macros or retain arbitrary serialized Value handles.
Arbitrary live rendering state or custom globals require their own ownership
bound and are outside this constructor proof.

These conservative components total **712,704 bytes**, below the explicit
**1,048,576-byte shared Prompt/runtime allowance**. The actual fixture bound is
the cumulative factory allocation requests, plus distinct retained shared
navigation storage, plus this allowance. All three must fit the unchanged
64 MiB asset lease; decoded-state and global cohort limits do not increase.
Instrumented timing includes allocator-counter overhead.

Static maps and thread-local scratch may outlive a hydrated owner. The asset
lease charges the full inherited bound while hydration is retained; coordinated
Running/process ownership continues covering those caches after the candidate
is dropped. The test retains Running throughout that scope. No test claims that
dropping HydratedEngine physically reclaims process-wide caches. M3 must preserve
the corresponding host-thread/process lifetime reservation.

The first exploratory library read used the separately installed 2.23.0 tree;
Cargo.lock was then checked and all derivation, assertions and recorded hashes
above use the actual 2.21.0 dependency. No result relies on the other version.
