# Search Reclamation Admission

006-P5 changes work admission inside the existing search and artifact-entry
owners. Previously both state-reclamation paths reserved
`(artifact capacity + 1) * state count` before inspection. The actual scan skips
active or empty states and stops at the first artifact pin. Schedule **22** now
charges each visited state and physical artifact slot before inspection,
including holes but excluding unused capacity. No new cache, pin-count table,
allocation, representation, family or search default is introduced.

Refusal can occur after completed cleanup. Already released states remain
released; pinned and unvisited states stay owned. Mandatory disposal still
releases remaining artifacts, identities, candidates and backing without a new
work allowance. This is more precise admission, not a new asymptotic algorithm
or measured wall/CPU optimization.

## Focused Checks

Four new tests cover 29 exact pin-lookup cutoffs (early/late hits, misses and
holes), active/empty state visits, five partial-reclaim cutoffs and seven
finish-discovery cutoffs. Real admitted artifacts and candidate identities are
used. Exact incumbents remain readable after refusal; final compilation
disposal leaves zero retained bytes.

There are **135 distinct accepted tests**, with no full suite or library build:

- [Initial qualification](../2026-09-19-artifact-service/run-2026-09-20T01-42-00.883Z/receipt.json):
  111 search-owner, entry-owner, public-search, policy and service checks pass.
  Incremental build takes 41.515 seconds; tests take 8.815 seconds.
- [Strengthened assertions and additional search checks](../2026-09-19-artifact-service/run-2026-09-20T01-43-28.125Z/receipt.json):
  six cursor checks and 16 staged/reuse/reference/group/naming checks pass.
  The overall receipt fails because `integrated_public_tests::` matches zero
  tests. That refusal is preserved. Only cursor test assertions/formatting
  change from the first build, so 105 unchanged non-cursor passes are reused.
- [Corrected integrated-JS filter](../2026-09-19-artifact-service/run-2026-09-20T01-44-07.345Z/receipt.json):
  all eight tests pass with the identical cached binary (`fresh=true`); the
  Cargo check takes 0.099 seconds. No compiler source changes between these two
  invocations. Test elapsed time is not compiler-performance evidence.

## Paired Search Replay

The final binary repeats the existing 32-row beam/schedule/objective/probe
matrix. [Receipt](receipt.json), SHA-256
`6ba8376c74031f670b8eb4b98f34abed7178abde531f72765e19ab79bbc208c7`,
compares it with the preserved [schedule-21 evidence](../2026-09-20-search-schedule/README.md).
All 24 oracle records and every field of all 32 search rows match exactly,
except optional and analysis logical work. This includes all winner bytes,
scores, 64 winner observations, explored helper subsets/naming styles, renders,
probes, other work kinds, peak retained memory and stop reasons.

The removed analysis tariff is **0-667 units per row**, positive in 28 rows.
See [comparison.json](comparison.json), [rows.json](rows.json) and
[oracle.json](oracle.json). These unchanged byte streams and scores reuse the
previous independent canonical codec replay after verifying its receipt,
all outputs and preserved codec binary. They are not new codec invocations.
No compression improvement or increased default effort is claimed.

## Provenance And Limits

The [root audit](audit.json), SHA-256
`d2fdabe5140a83af9e821ef0b41c1ce7acbf3e69c6ddae2beaba60af2ac15c4a`,
verifies all 980 final compiler inputs, 70 qualification/replay outputs, both
test binaries and the five changed source files. There is no independent agent
review. Final input digest is
`728ed9ca7f4aeedc901844d91385cb6c79d371e3d920a34cbb4703387e93adb6`.
The final 416,534,344-byte binary is preserved at
`/tmp/lilscript-search-reclamation-baseline-20260920/lilscript-c737e884cfca95f9`,
SHA-256 `b1457d368b0f0f8d8263b85f5ed9331f72bb8c7b41ae15c5c77ab58ee31a5720`.
Pre-edit sources and the initial accepted binary remain under the corresponding
`/tmp/lilscript-search-reclamation-before-20260920/` and
`/tmp/lilscript-search-reclamation-initial-20260920/` directories.

The target design is unchanged. No release, held-out library, fleet or
wall-time qualification runs. This does not resolve snapshot-qualified rewrite
rediscovery, emission cost, public-boundary decisions or any complete milestone
in the [single migration plan](../../../docs/migration/index.md).
