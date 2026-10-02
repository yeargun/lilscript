# D3 shared graph, sessions and port integration

D3 follows D2 (`9a3cca29`) and is complete under the gates below. Its
[evidence](../../benchmarks/migration-results/2026-10-01-d3-integration/README.md)
records source ownership, focused checks and verification limits. Full fleet
comparisons remain V1; site/release qualification remains V2.

1. CLI, lint and editor semantic queries use the same configured entry graph and
   module interfaces. All open editor buffers override disk sources, including
   dependencies. Completion, hover and diagnostics agree on imported identities;
   dependency edits invalidate consumers. Incomplete syntax may still use a
   lexical editor fallback, without pretending it has checked types.
2. Retain parsed modules and reuse module elaboration under one graph/session
   owner. Reuse must preserve canonical declarations, source identities and
   initialization. Changes to dependency contracts, configuration or graph order
   invalidate affected state. Cold and reused diagnostics/results must agree.
   Bound retained storage; do not introduce a quadratic snapshot cache or an
   unchecked text-hash-only semantic cache.
3. Supply configured multi-format/subset output from a shared checked input,
   retaining independent raw/gzip/Brotli objectives, hard permissions, exact
   output manifests and source/config/artifact receipts. Document control use,
   scope, defaults and compile/size/runtime tradeoffs.
4. Move port source patches into their owning repositories and retire the compiler
   patch layer. Replace post-minifiers/rebundlers with compiler-written ESM, CJS
   and browser outputs. Start with motionlil's ten-entry graph and nominal rename
   reversions; include the recorded mobxlil development-host and katexlil metrics
   script debts. Keep assumption reasons beside port controls.
5. Record focused graph/edit/cache/config/format checks and representative port
   delivery consumers, update index/history/coverage, commit and push. V1 owns
   full maintained/held-out qualification; V2 owns reproducible release acceptance.

Expected effect: shared discovery/checking and safe reuse reduce repeated frontend
work; independent output formats/objectives still pay their own search and codec
costs. Compiler-written packaging makes those costs and bytes explicit. No broad
compression win is inferred merely from removing a post-minifier.
