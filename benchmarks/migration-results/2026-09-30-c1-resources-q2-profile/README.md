# C1 effective resources and Q2 rule attribution

Source `35a80337`. Compiler `~/lilscript-work/bin/c1-resources-q2-profile-1/lilscript` SHA-256 `2618ba195893094876168e06fb9887dac87083c7fd28a3303080a927680bde47`; codec SHA-256 `40c973ea9fc90fd41e4fbd6558336393bdc3108a5f2bd8868247e981d1b3efad`.

The service and CLI now resolve actual caller work/memory ceilings together with TOML hard limits before fingerprinting. Explicit CLI controls override the deprecated environment adapter; invalid adapter values fail clearly. The defaults and search opportunities are unchanged. Static timing counters identify each target rule when diagnostics are enabled.

The [generic gate](ratchet.md), with [full receipt](ratchet.json), passes all 642 programs under each objective. Every table entry, including its emitted-byte hash, equals the [qualified private-field compiler](../2026-09-30-g2-private-fields/README.md). No baseline update is needed.

All 20 [port artifact records](ports.json) also match that compiler exactly. Reference tests pass 29/29, 1,353/1,353 and 21/21. Library verification plus focused corrections covered 1,718 active tests with four existing ignores and the previously qualified large table case excluded; all eight CLI tests pass. The isolated timing oracle also passes with identical output and logical resource use when timing is enabled or disabled.

One [diagnostic Zod profile](zod-profile.json) uses the frozen effort-13 workload and pins the runner, compiler, manifest, inputs and output. It retains 67 terminal exact judgments and the same 26,573 Brotli bytes. Target rules take 9.160 seconds in that run: single-call block inlining accounts for 3.473 seconds over 156 invocations and single-use forwarding for 2.280 seconds over 228 invocations. These nested elapsed counters locate work; they are neither independent additive CPU measurements nor a paired speed comparison. They motivate inspecting repeated call-site lookup before changing cache ownership or search opportunities.

This qualifies the resource receipt migration and profiler. Remaining family/risk controls, compatibility migration, the ambient proxy audit, persistent formation reuse and broader calibration keep their existing owners. Default effort 13 continues to prioritize size; useful extra search is retained.
