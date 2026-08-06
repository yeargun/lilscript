# LilScript Optimization Coverage

This document maps Google Closure Compiler `ADVANCED` optimization
responsibilities to LilScript. The reference is Closure Compiler
`v20260803`, including the optimization factories in its
[`DefaultPassConfig.java`](https://github.com/google/closure-compiler/blob/v20260803/src/com/google/javascript/jscomp/DefaultPassConfig.java).
Variable ordering is compared against Closure's
[`RenameVars.java`](https://github.com/google/closure-compiler/blob/v20260803/src/com/google/javascript/jscomp/RenameVars.java),
and emitted-character ranking against Terser's documented
[`nth_identifier`](https://github.com/terser/terser#minify-options) frequency
analysis. Compressor-aware decisions use actual codec output because Brotli's
[LZ77, context modeling, and Huffman stages](https://datatracker.ietf.org/doc/rfc7932/)
cannot be represented faithfully by raw token counts alone.

LilScript does not run Closure and does not translate LilScript to annotated
JavaScript before optimizing it. The frontend resolves and links the complete
static `.lil` module graph, produces typed CFG IR, promotes locals to pruned
SSA, and optimizes separate copies of that closed-world representation for the
JavaScript and native C backends.

## Applicable optimization mapping

| Closure responsibility | LilScript implementation |
| --- | --- |
| Early/late peephole optimization | Constant folding, algebraic identities, boolean simplification, branch inversion, nested literal-capture branch folding, SSA-root binary precedence rendering, precedence-safe parenthesis removal at statements and expression delimiters, compact boolean literals, compact loops, conditional returns, declaration collapse, trailing-semicolon removal |
| Numeric representation lowering | Signed-i32 range analysis propagates bounded loop induction values, direct-call arguments and returns, and owned nominal fields; it removes coercions only for proven-safe operations. Ordinary multiplication emits `x*y|0` when normalization is required, while source-written `Math.imul` remains an explicit exact operation |
| Inline variables and constants | mem2reg SSA, constant propagation, single-assignment global propagation, constant rematerialization, one-use expression fusion, and exact array-parameter lengths across closed stable direct-call sets |
| Inline functions and simple methods | Fixed-point expression inlining plus single-use multi-block CFG inlining |
| Inline/collapse properties | Nominal field resolution, positional field indexes, struct/class scalar replacement, and owned-field range summaries invalidated at untyped boundaries |
| Collapse object literals | Non-escaping structs dissolve into SSA scalars; remaining typed aggregates use positional arrays in JavaScript |
| Disambiguate/ambiguate/rename properties | Nominal owner types and field indexes remove internal property names entirely; boundary names remain ABI-stable |
| Devirtualize methods | Class calls become direct typed function calls before inlining |
| Optimize calls and constructors | Direct-call lowering, recursive-call protection, constant-parameter specialization, unused parameter/return removal, constructor inlining, allocation removal, effect summaries |
| Mark pure functions | Interprocedural fixed-point summaries separate inherent effects from mutations of specific parameters across direct calls and known closure captures; local scratch-allocation mutation filtering, checked `pure` contracts, and trusted `pure extern` declarations share that model |
| Dead assignment elimination | SSA promotion removes local stores; DCE removes unused value chains, complete unobserved local array/map/set mutation graphs, and parameter-mutating helper calls when every affected allocation group is unobserved |
| Dead property assignment elimination | Overwritten typed field stores are removed between observation barriers |
| Remove unused code | Unread globals, unreachable blocks, unused pure calls, unused allocations and mutation graphs, instructions, and call-graph-unreachable functions are removed |
| Flow-sensitive inline variables | SSA def-use counts, side-effect-aware deferred expression emission, and one-use boolean merge phis fused into immediately following structured branches |
| Coalesce variable names | CFG liveness, interference graph coloring, phi move affinity, and reuse of dead locals as parallel-copy temporaries |
| Collapse variable declarations | Adjacent bindings and first phi assignments are combined by the JS backend; cyclic phi copies compare tuple and scalar schedules under the configured codec |
| Rewrite/collapse anonymous functions | Small typed closures become expression or structured block arrows; capturing closures pass explicit environments; literal captures expose dead branches during final emission |
| Alias strings | Repeated constants are value-numbered and profitable long strings receive shared short bindings; size-first also considers delimiter-packed immutable string tables; final pooling, packing, quote, and coercion variants are selected against exact raw/gzip/Brotli cost |
| Rename variables and globals | Use-frequency-ranked base-54/base-64 identifiers with extern names reserved, plus exact-compressor selection of emitted-character-ranked alphabets |
| Rescope globals | Entry-only globals become locals; immutable shared globals become constants |
| Rewrite modules and tree shake exports | Relative module graphs are linked into private symbol namespaces; executable exports remain shakeable, while `js-module` roots runtime exports and emits mangled ESM aliases |
| Cross-chunk code/method motion | Whole-program optimization runs before deterministic static ESM partitioning; preserve-module and shared size/import policies emit explicit imports, live exports, and a manifest. Dynamic/lazy imports remain unsupported |
| Prototype extraction and dotted-property conversion | Not applicable: LilScript has no prototype mutation or dynamic property grammar |

Closure also contains JavaScript-input processing for JSDoc, `goog.*`,
CommonJS, browser polyfills, Angular, Polymer, J2CL, and extern collection.
Those are not LilScript optimizations and are intentionally not accepted as
source syntax. LilScript's own static module graph is resolved before IR
lowering, and host interaction is explicit through typed `extern` declarations.

## Pass schedule

The current schedule is:

1. canonical module discovery, export/import validation, private-name linking,
   and dependency-order initialization;
2. entry-global internalization and unread-global removal;
3. pruned mem2reg SSA and phi insertion;
4. local/interprocedural constant and stable array-length propagation,
   trivial-phi removal, algebraic simplification, local value numbering, branch
   folding, and unreachable removal;
5. immutable-global propagation, devirtualization, and explicit purity
   validation;
6. constant-parameter specialization, unused parameter/return removal, and
   fixed-point expression and multi-block CFG inlining;
7. escape analysis, class/struct scalar replacement, allocation-root alias and
   parameter-effect summaries, unobserved collection-graph/call removal, and
   dead field stores;
8. another scalar fixed point;
9. effect-aware SSA DCE and whole-program function DCE;
10. module-level argument/return/field range analysis, liveness-based name
    coalescing, structured boolean-phi deferral, dependency-ordered phi copies,
    liveness-reused cycle temporaries, codec-selected scalar/tuple copy layouts,
    induction ranges, shortest numeric literals, SSA-root binary precedence,
    structured closure selection, string-table packing, minified backend
    peepholes, and deterministic compressor-aware candidate selection;
11. optional source ownership or shared-module chunk planning over the surviving
    IR, followed by cross-chunk binding analysis and deterministic ESM emission.

## Executable evidence

`scripts/verify-matrix.sh` compiles 60 independent `.lil` programs, including a
multi-file module graph, with one
`--target all` invocation per program. Each invocation emits JavaScript, emits
C, and invokes Clang for a native executable. The script then compiles the
emitted C independently and requires the JavaScript, direct native executable,
independently compiled C executable, and checked-in expected output to match.
The corpus includes collection mutation/identity/nullable lookup and binary
memory copy/view/coercion behavior under both maximum and disabled optional
optimization, for 120 backend-mode executions. This includes regressions for
interprocedural integer and exact array-length facts, unobserved collection
mutation removal, and
loop-carried values crossing an early return and a nested short-circuit
coalescing regression extracted from the Solid client-runtime gate.
`scripts/verify-bundles.mjs` additionally executes preserve-module and shared
split bundles, checks their manifests, and exercises live bindings across a
circular ESM dependency between the entry and a reader chunk.

`benchmarks/run.sh` compiles ten behaviorally equivalent LilScript/JavaScript
workloads, runs both outputs, invokes Closure `ADVANCED`, and measures normalized
raw, gzip-9, and Brotli-11 bytes. The benchmark is a reproducible corpus result,
not a proof that any finite compiler beats another compiler on every possible
future program.

`benchmarks/paired/run.mjs` removes manual source-style differences for a
second gate by generating both languages from one neutral integer workload
schema. Every generated case must match through Closure JavaScript, LilScript
JavaScript, emitted C, and native execution, and LilScript may not exceed
Closure in any raw/gzip/Brotli cell. `benchmarks/browser/run.mjs` separately
requires the 95% bootstrap upper bound for warmed Chromium runtime to remain at
or below `1.03` on those paired cases.

`benchmarks/libraries/run.mjs` builds six complete-root-entrypoint apps from
seven installed npm packages. Each LilScript port must match JavaScript,
generated C, and native execution before raw, gzip-9, Brotli-11, and runtime
results are published. The separate Solid client-runtime repository currently
passes 109 adapted behaviors through optimized/unoptimized JavaScript, emitted
C, and native execution (654 executions) while the unchanged pinned upstream
suite remains 469/469. It is explicitly partial and is not counted as complete
Solid compatibility.
