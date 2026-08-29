# LilScript and Closure Compiler compression

This report compares the current LilScript tree with Google Closure Compiler at
commit [`73eee24`](https://github.com/google/closure-compiler/commit/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4),
dated 2026-08-28. It is a source-level comparison, not a benchmark result.

## Short answer

Closure has several compression-oriented techniques worth studying. Some are absent from
LilScript; others are broader variants of work LilScript already performs:

- drives interacting peepholes to a changed-scope fixed point inside larger optimization loops;
- minimizes positive and negated condition trees with a small dynamic-programming cost model;
- lazily marks dependencies in dead-code elimination and preserves only residual side effects;
- coordinates unused-return, unused/constant-parameter, function-inline, variable-inline, and DCE
  passes until later transformations stop exposing work;
- folds a broad matrix of known JavaScript methods and reconstructs literals from subsequent writes;
- has generated-code estimation for function inlining and a fixed raw syntax model for repeated
  prototype-prefix extraction;
- replaces configured string/ID domains with compact values, can emit maps when configured, and
  substitutes compile-time messages;
- splits one property spelling into independent type clusters;
- lets different property spellings share a name when their receiver sets cannot overlap;
- collapses qualified namespaces and object-literal properties into variables before mangling;
- coordinates alias removal, inlining, DCE, property analysis, coalescing, and renaming over a
  mature JavaScript AST pipeline;
- persists variable/property rename maps and emits pseudo names and renaming diagnostics;
- models chunks, raw global exports, coding conventions, and property-reflection intrinsics.

The reverse is also important. LilScript has compression mechanisms Closure's standard pipeline
does not:

- stores type, ownership, escape, and lowering-obligation metadata in IR and computes effect/range
  analyses over that IR;
- explores structural, optimizer-setting, layout, syntax, pooling, and naming alternatives;
- keeps the configured artifact as an incumbent instead of assuming every legal rewrite is smaller;
- scores complete candidates under exact raw, gzip, or Brotli output;
- scores chunk plans using transfer, request, dependency-depth, preload, and cache-reuse costs;
- it performs local allocation over typed SSA values with CFG liveness and phi affinity;
- it distinguishes owned, record, and host properties before JavaScript is emitted;
- it weights property uses in blocks designated as ordinary loop bodies or updates;
- it searches identifier alphabets, local permutations, and function layouts;
- it accepts a naming candidate using exact raw, gzip, or Brotli cost, rather than assuming
  occurrence frequency is the right compression proxy.

The best synthesis is not to imitate Closure's objective. It is to port its mature candidate
discovery, safety cases, and pass interactions into LilScript's typed IR or target-JS layer, then
let LilScript's existing bounded exact-codec machinery accept or reject each alternative.

## Whole-pipeline comparison

| Area | Closure Compiler | LilScript | Most useful lesson |
|---|---|---|---|
| DCE | Lazy dependency activation and side-effect-preserving sweep | Iterative effect-aware SSA DCE and call reachability | Cross-test residual-effect cases |
| Function inlining | Detailed generated-minified-byte estimate | Typed structural/growth gates plus scored variants | Add real-emitter inline candidates |
| Call signatures | Return, optional/constant/unused argument transforms in a loop | Unused returns/parameters and constant specialization | Search more phase orders jointly |
| Conditions | DP over positive/negative Boolean forms | Many targeted IR/emitted-JS folds | Add general costed condition alternatives |
| Peephole iteration | Dirty function/script scope fixed point | Bounded global/local rounds and beams | Add dirty-region scheduling under budgets |
| Known methods | Broad JS literal method evaluator | Typed intrinsic folding with notable gaps | Fill typed string/array fold matrix |
| Literal reconstruction | Collects subsequent object/array property writes | Fresh-object/push folds and typed aggregate lowering | Add indexed-write reconstruction with proofs |
| Repeated structures | Prototype-prefix extraction and optional function factories | Helper clustering, subsumption, pooling, function layout | Submit structural-prefix candidates to exact scoring |
| Strings/IDs/messages | Alias strings, replace configured strings, IDs, messages | Literal pooling/packing and dense tables | Add semantic string/ID domains, not generic raw pooling |
| Modules | File pruning plus module/global rewriting | Typed static linking and symbol reachability | Compare dynamic-import roots and public boundaries |
| Targets/polyfills | Feature-selective lowering/injection/removal | Target-aware emission without equivalent polyfill pipeline | Preserve newest legal syntax; DCE generated helpers |
| Selection objective | Mostly legality, AST progress, and local raw-size proxies | Bounded complete-artifact raw/gzip/Brotli scoring | Keep LilScript's objective |

## Comparison matrix

The remaining matrix focuses specifically on mangling.

| Area | Closure Compiler | LilScript | Assessment |
|---|---|---|---|
| Local identity | JavaScript variables normalized to scope slots | Typed SSA values and source-local hints | LilScript has richer input |
| Local reuse | Scope-slot reuse plus AST live-range coalescing | CFG interference coloring plus phi affinity | Comparable; LilScript is tighter to IR |
| Local ranking | AST-name occurrence count, then source order | Direct and phi-group orderings use parameter/use/degree or affinity signals, followed by codec search | LilScript is more search-oriented |
| Global ranking | AST-name occurrences; equal-length names regrouped by source proximity | `uses + 1`, deterministic kind/id ties, exact terminal search | Different strengths |
| Alphabet | Canonical; character favoritism exists but is not standard wiring | Artifact-derived and bounded permutation candidates | LilScript searches more alternatives |
| Compression objective | Raw-size heuristics plus a gzip-locality heuristic | Exact raw/gzip/Brotli scoring of bounded candidates | Different optimization strategy |
| Property identity | Source spelling, then optional type-cluster splitting | Typed `(owner, field/index)` for owned fields | Both type-aware in different ways |
| Property merging | Receiver-set graph coloring (`AmbiguateProperties`) | Reuse by owner/inheritance component | Closure is more general |
| Namespace collapse | Mature qualified-name and object-literal collapse | Modules disappear and aggregates may scalarize/be positional | Different representations |
| Dynamic/reflected names | Externs, quoted keys, coding conventions, rename intrinsics | Typed extern/host/record boundary and static-key analysis | LilScript's boundary is explicit for its own values; Closure syntax coverage is broader |
| Incremental stability | Input/output variable and property maps | Deterministic output; no persisted rename map | Closure is ahead |
| Debuggability | Pseudo names, maps, graph/log diagnostics | Trace hooks and tests; no complete rename provenance | Closure is ahead |
| Chunks | Alias/collapse placement respects chunk dependencies | Per-chunk deployment scoring and optional joint layout/reserve search | Different strengths; neither has per-symbol load weighting |
| Source maps | AST transformations preserve compiler source information | post-emission byte rewrites have no native map composition | Closure is ahead |
| Safety posture | Conservative invalidation, but ADVANCED has documented whole-world hazards | typed ownership and escape proofs, plus some late lexical risk | Neither is uniformly stronger |

## Report map

- [Objective and search](objective-and-search.md): how both compilers decide whether a
  transformation is worthwhile, including exact gzip/Brotli scoring.
- [Whole-program compression](whole-program-compression.md): DCE, effects, call signatures,
  inlining, scalar replacement, devirtualization, and fixed-point interactions.
- [Peephole and syntax](peephole-and-syntax.md): conditions, exits, statements, literals,
  known methods, and target-aware compact syntax.
- [Data, modules, and delivery](data-modules-delivery.md): strings, messages, generated IDs,
  prototype extraction, pruning, polyfills, and chunks.
- [Compression opportunities](compression-opportunities.md): ranked Closure-inspired work
  that fits LilScript's architecture and objective.
- [Compression migration](../docs/knowledge/migration/compression-migration.md): design-first
  implementation order, language surfaces, and progressive corpus gates.
- [Variable mangling](variable-mangling.md): name allocation, liveness, reuse, frequency,
  alphabets, and compressed-size heuristics.
- [Property mangling](property-mangling.md): ordinary renaming, disambiguation,
  ambiguation, owned fields, externs, and reflection.
- [Pipeline and safety](pipeline-and-safety.md): pass ordering, namespace collapse,
  chunks, ABI boundaries, determinism, maps, and known hazards.
- [Advanced opportunities](advanced-opportunities.md): prioritized changes that make
  sense for LilScript, and Closure ideas that should not be copied directly.
- [Source reference](source-reference.md): pinned Closure links and the corresponding
  LilScript implementation locations.

## Scope and caveats

"Compression" here means reducing delivered JavaScript, including transformations that remove
program structure, expose later simplification, choose shorter syntax, or improve compressed
repetition. Runtime-speed-only work is out of scope unless it directly enables byte reduction.

Mangling remains part of the report because Closure's strongest naming results do not come from
`RenameVars` alone. They come from property collapse, type clustering, ambiguation, inlining, DCE,
and live-range coalescing before the final allocator runs.

LilScript also has more than one naming layer. The typed IR emitter is authoritative; terminal
token-level remapping and `converge_local_names` are bounded, codec-scored alternatives. The
comparison keeps those layers separate.

The repository's checked-in [`lilscript.toml`](../lilscript.toml#L39-L60) explicitly enables
identifier and entropy-aware mangling but omits property and export mangling. Consequently,
some implemented property features are not active in builds that use this exact root config.
