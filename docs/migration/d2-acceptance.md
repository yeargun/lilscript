# D2 consumer delivery and module semantics

D2 follows D1 (`13e9261c`) and is complete under the contract below.
[Evidence](../../benchmarks/migration-results/2026-10-01-d2-consumers/README.md)
records the focused runtime/consumer/map checks and six independently optimized
production artifacts. D3 follows.

1. Make initialization facts sound for interleaved entry consumers and lazy
   evaluation. Carry object-completion and hoisted-declaration constraints on
   the target tree. Placement must preserve initialization, shared identity,
   live bindings, failures and effect order, including source cycles.
2. Support effects in lazy modules and correct once-only single-file loading;
   lift the old format-independent checker refusals only with target coverage.
   Keep split/preserve validation and independent per-file admission.
3. Supply configurable consumer annotations/export placement from semantic
   discardability facts. Include bytes in each objective, qualify independent
   consumer bundles, and expose accurate per-file side-effect metadata.
4. Replace the host ESTree JSON traversal with typed Oxc lowering. Preserve
   external/embedded host behavior, live imports and typed syntax diagnostics.
5. Publish immutable product-containing graphs with distinct private/public
   representations, preserving reference aliases and the defined value copies.
6. Implement source maps from retained tree origins as part of exact delivery,
   with TOML controls, final filename mappings and manifest receipts.

Expected tradeoffs: correct lazy/interop graphs can require runtime bookkeeping;
consumer annotations and maps add bytes only in their configured outputs.
Conservative initialization may retain code previously optimized unsafely.
Every optional tactic remains subordinate to hard configuration vetoes and
independent raw/gzip/Brotli objectives. Test coherent batches, not each edit;
retain failures and run the broad final qualification only under V1.

The implemented boundary includes ESM/CJS source cycles, single-file lazy
activation, cached failure/live namespaces, separate embedded host ownership and
source-qualified maps through inlining. A foreign request that cannot preserve
its order/laziness in a single file is diagnosed; split/preserved files or typed
embedding provide the supported alternatives. Raw carried host text retains its
single-file and syntax limits. Public finite immutable product graphs preserve
reference aliases; unrestricted public mutable/erased ABI proposals remain
outside this contract. Native module tasks/static const ABI remain N1/N2.
