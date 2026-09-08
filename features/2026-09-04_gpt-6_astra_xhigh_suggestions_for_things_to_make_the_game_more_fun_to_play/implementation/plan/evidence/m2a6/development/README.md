# M2a6 development evidence

Every owner cargo invocation is listed in [logs.json](logs.json) with exact command, environment, UTC start, elapsed time, process exit, original `/tmp` path/bytes/SHA256 and lossless gzip archive/bytes/SHA256. Gzip uses mtime 0; decompression returns the original bytes exactly. No terminal newline normalization or redaction is applied to development logs. [run_checked.py](run_checked.py) is the invoked wrapper (run with uv); the final full workspace is indexed in final verification as well as in this complete sequence.

Development failures were retained and corrected:

- `owners-initial`: MAX_ID_CHARS was imported through the private ids module instead of its public crate-root definition. The current source constant is 128 Unicode scalars.
- `fixtures-initial`: the public opaque Custody iterator does not promise Clone; diagnostic counting now obtains a fresh borrowed iterator for each count. One unused test import was removed.
- `private-initial`: the corruption test assumed array rows instead of the established strict JSON map adapter and dereferenced a nonexistent JSON pointer. The escaped-string test correctly refused the input, but its assertion expected the word `text` rather than the actual `string byte limit` reason. Neither failure was a product acceptance of malformed state.
- `workspace-final`: both host binary/test linker processes were killed by signal 9 before any target tests ran. The unchanged source passed the full workspace when retried with `-j 1`, reducing concurrent linker memory pressure.
- `probe-authored`: the diagnostic initially expected a closing NPC. Ordinary follow_escorts moves that NPC to the officer before tick_custody, so the corrected reachable workload uses an unheld player beyond the leash for closing and a separate held NPC. A now-unused import was removed. No production law behavior changed.

The new law/Engine fixture generators were explicitly invoked only for the two new checkpoint fixtures. Existing historical fixtures and goldens were not regenerated. Their canonical byte tests pass. Later focused runs include all previous private checkpoint tests; the final source delta after focused-complete is only a clarified component-horizon/spacing comment.

The one-sample authored/populated JSON files are optimized-debug developer smokes, not release percentile evidence. Both use actual production population placement and normal law cache publication. They precede the final cache validation strengthening (fixed settlement prose, order and minimum grip duration); the final fixture tests and full workspace verify those checks, and both final frozen-source smokes passed with identical counts/bytes/witnesses. The populated smoke waited on Cargo's build-directory lock while the authored rebuild finished; these one-sample checks are functional diagnostics, not timing acceptance. The coordinator's release build must use the frozen source.

Scoped formatting passes on twelve changed Rust files. custody.rs alone has pre-existing formatting drift; [format_check.json](../format_check.json) records its exact baseline comparison. Removing only the new module/comment bytes yields HEAD byte-for-byte, as also recorded by [ordinary source comparison](../ordinary_source_comparison.json). The file's existing style is preserved. No ordinary CPU or renderer benchmark was repeated.

The later coordinator public boundary run is pinned separately in [public_boundary.json](../coordinator/public_boundary.json); the release build and measurements are linked by the [coordinator review](../coordinator/review.md). These later checks do not rewrite the original owner source freeze or workspace count.
