# Compilation

The compiler turns a closed, typed module graph into one or more JavaScript artifacts (and optionally C/native). Contested choices are not decided by local heuristics when a complete-artifact codec score is available.

Parent: [Mission](../mission.md). Language invariants: [Language](../language/README.md). Knobs: [Config](../config/README.md).

## Pages

- [Global optima](global-optima.md) — why local “smaller” can lose gzip/Brotli
- [Pipeline](pipeline.md) — stages, files, bundle vs single vs native
- [Frontend, linking, and lowering](frontend-linking-lowering.md) — AST to typed
  control-flow IR and diagnostic boundaries
- [Analyses](analyses.md) — effects, escape, ranges, finite values, alias/call facts
- [IR optimizer](ir-optimizer.md) — pass order and `[optimization]` gates
- [DCE and tree shaking](dce-tree-shaking.md) — roots, effects, repeated cleanup
- [Inlining, specialization, and sharing](inlining-specialization-sharing.md) — opposing
  whole-program function transforms
- [Aggregate lowering](aggregate-lowering.md) — scalar, positional, named, record/host
  boundaries
- [Class identity](class-identity.md) — when a constructor must stay an ES `class`,
  and why object-lowering remains the default
- [Compress passes](compress-passes.md) — fusion, sinking, outlining, superopt
- [JavaScript emission](javascript-emission.md) — spelling, mangling, layout
- [Mangling, layout, and pooling](mangling-layout-pooling.md) — proof boundaries and
  codec-scored representation families
- [Candidate search](candidate-search.md) — two-level search, beam, ranking
- [Parsed peephole](peephole.md) — post-emit AST rewrites, still codec-scored
- [Chunk planning](chunk-planning.md) — split / preserve-modules deploy cost
- [Native backend](native-backend.md) — shared IR, C representation, storage placement
- [Correctness and fallbacks](correctness-fallbacks.md) — baseline retention,
  deterministic selection, oracles

## Invariants (do not violate)

1. The **configured** optimizer and emission are always a candidate. Experimental variants that fail to compile are dropped; they must not make a valid project uncompilable (`optimize_and_select_javascript` in `src/compiler.rs`).
2. Search never enables a tactic omitted from the exact `javascript.compression` allowlist. It may **disable** enabled tactics to compare.
3. `[optimization] foo = false` is a hard off. JS `optimization_level` cannot turn it back on.
4. Type checking and mandatory IR normalization do not depend on `priority` or `optimization_level`.
5. Split/preserve-modules still whole-program optimize before any chunk boundary.

## Where “clever” lives

| Layer | Local / heuristic | Global / scored |
|---|---|---|
| IR rewrite rules | Fold identities, inlining budgets | IR **variants** (no-inline, no-CSE, subsumption, compress on/off) scored as full JS |
| Emission | Default `IrJsOptions` | Beam over pooling, spelling, SSA destruction, layout, alphabets |
| Peephole | Legal AST shapes | Winner vs untouched under codec |
| Chunks | Greedy add-if-cheaper loop | Each trial plan re-emitted and scored for deploy cost |
