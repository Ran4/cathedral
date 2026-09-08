# M2a5 development records

Every command in runs.json names exact command/environment/UTC/exit/raw bytes and SHA-256. Archives are lossless gzip with mtime=0; original /tmp logs remain unchanged. Development source changed between runs; only final workspace is tied to the final source manifest.

`engine-compile`: Compilation refused an attempted mutable prepare through Admitted::try_map's immutable reservation borrow. Export now admits its complete borrowed Engine+World shape before nested extraction; no shared budget API change.

`public-context`: Root fifth test reused a budget with an existing save payload for a new save reservation; refusal was before the target export. Root fixed it with a separate export budget. The next five-test run passed.

`store-rich`: The bounded string correctly refused but the test expected the word text rather than the actual string byte limit reason. Corrected assertion; no product boundary change.

`checkpoint-focused`: Two canonical fixture re-export assertions used serde_json::to_value, which widens f32 values, rather than serializing the actual f32 wire. Initial fixture/candidate validation passed. Corrected serialize-then-parse assertions and avoided Value dumps. This development-only failure log contains synthetic fixture provenance already in test inputs; coordinator explicitly required lossless preservation of the failure context. Runtime source Debug/serde-error privacy witnesses pass.
