# Q2 reuse, scheduling and resource contract

Q2 is the sole active milestone after S4 (`c85ad5d2`). Complete it before Q3.
Existing score memoization, effect/view caches, retained target heads and stable
scheduler suffixes remain useful starting points; do not rebuild their owners.

1. [x] Replace the target's private repeated function-pruning walk with an
   admitted dependency/use index. Close unused declaration chains and cycles
   from actual roots, preserving effects, exports, pinned bindings, captures,
   initialization and strictness. Remove the old four-iteration traversal.
2. [ ] Drive remaining source/target normalization with dirty dependencies,
   retaining deterministic structural edit order and fail-closed convergence.
   Reuse stable facts only for their immutable dependencies; edited bodies and
   changed interfaces invalidate their consumers.
3. [ ] Complete incremental/shared formation and bounded persistent reuse.
   Repeated assignments and unchanged delivered files must not repeat physical
   work. Include the content-addressed build/elaboration cache and the explicit
   fingerprinted decision-lock replay from M3.9. Keys include complete
   source/config/compiler/encoder identities;
   cached evidence grants neither a permission nor extra search work.
4. [ ] Finish resource admission for graph/cycle/delivery and analysis/edit
   storage, including failure cleanup. Entry graph, cycle checking and the
   target use index are admitted; delivery placement, journals and remaining
   analysis/edit owners still need completion. Count logical work independently from
   physical cache hits and retain scaling/work/byte telemetry.
5. [ ] Wire/document cache and scheduling controls with hard TOML behavior,
   lifetime/capacity, invalidation, defaults and tradeoffs. Transparent controls
   may change physical cost but not chosen output or admission decisions under
   the same deterministic logical limits. Q3 owns search breadth and heuristics.
6. [ ] After coherent batches, compare dense/cold and reused behavior under all
   objectives, limits and relevant delivery forms. Use affected oracles and
   pinned measurements; reserve full-fleet/held-out qualification for V1.
7. [ ] Remove replaced paths, retain actual results and negative evidence,
   update coverage/checklist/history/handoff, then commit/push before Q3.

First implementation batch: the target use-index consumer and graph resource
admission. Prediction: fewer repeated liveness traversals, complete dead-cycle
removal, unchanged retained behavior and occasionally smaller output. The index
uses bounded admitted storage/work. Qualification covers roots, recursive
cycles, captures, effectful initializers, admission cleanup and affected
raw/gzip/Brotli artifacts. No full library rerun.

The first batch is [implemented and qualified](../../benchmarks/migration-results/2026-10-01-q2-use-index/README.md).
It also closes stale print proofs after delivery inserts cross-file setters.
Q2 remains active for tasks 2–7; this is not a milestone completion claim.
