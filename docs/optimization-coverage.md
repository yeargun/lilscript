# LilScript Optimization Coverage

This document maps Google Closure Compiler `ADVANCED` optimization
responsibilities to LilScript. The reference is Closure Compiler
`v20260803`, including the optimization factories in its
[`DefaultPassConfig.java`](https://github.com/google/closure-compiler/blob/v20260803/src/com/google/javascript/jscomp/DefaultPassConfig.java).

LilScript does not run Closure and does not translate LilScript to annotated
JavaScript before optimizing it. The frontend resolves and links the complete
static `.lil` module graph, produces typed CFG IR, promotes
locals to pruned SSA, optimizes that closed-world representation, and feeds the
same module to the JavaScript and native C backends.

## Applicable optimization mapping

| Closure responsibility | LilScript implementation |
| --- | --- |
| Early/late peephole optimization | Constant folding, algebraic identities, boolean simplification, branch inversion, compact loops, conditional returns, declaration collapse, trailing-semicolon removal |
| Inline variables and constants | mem2reg SSA, constant propagation, single-assignment global propagation, constant rematerialization, one-use expression fusion |
| Inline functions and simple methods | Fixed-point expression inlining plus single-use multi-block CFG inlining |
| Inline/collapse properties | Nominal field resolution, positional field indexes, struct/class scalar replacement |
| Collapse object literals | Non-escaping structs dissolve into SSA scalars; remaining typed aggregates use positional arrays in JavaScript |
| Disambiguate/ambiguate/rename properties | Nominal owner types and field indexes remove internal property names entirely; boundary names remain ABI-stable |
| Devirtualize methods | Class calls become direct typed function calls before inlining |
| Optimize calls and constructors | Direct-call lowering, recursive-call protection, constructor inlining, allocation removal, effect summaries |
| Mark pure functions | Interprocedural fixed-point effect analysis over direct calls and known closure targets, plus checked `pure` contracts and trusted `pure extern` declarations |
| Dead assignment elimination | SSA promotion removes local stores; DCE removes unused value chains |
| Dead property assignment elimination | Overwritten typed field stores are removed between observation barriers |
| Remove unused code | Unread globals, unreachable blocks, unused pure calls, unused allocations, instructions, and call-graph-unreachable functions are removed |
| Flow-sensitive inline variables | SSA def-use counts and side-effect-aware deferred expression emission |
| Coalesce variable names | CFG liveness, interference graph coloring, and phi move affinity |
| Collapse variable declarations | Adjacent bindings and first phi assignments are combined by the JS backend |
| Rewrite/collapse anonymous functions | Small typed closures become expression or block arrows; capturing closures pass explicit environments |
| Alias strings | Repeated constants are value-numbered and profitable long strings receive shared short bindings |
| Rename variables and globals | Frequency-ranked base-54/base-64 identifiers with extern names reserved |
| Rescope globals | Entry-only globals become locals; immutable shared globals become constants |
| Rewrite modules and tree shake exports | Relative module graphs are linked into private symbol namespaces; import aliases resolve statically and exports do not become retention roots |
| Cross-chunk code/method motion | Static modules are intentionally emitted as one optimized bundle; runtime lazy chunks are outside v0.1 |
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
4. constant propagation, trivial-phi removal, algebraic simplification, local
   value numbering, branch folding, and unreachable removal;
5. immutable-global propagation, devirtualization, and explicit purity
   validation;
6. fixed-point expression and multi-block CFG inlining;
7. escape analysis, class/struct scalar replacement, and dead field stores;
8. another scalar fixed point;
9. effect-aware SSA DCE and whole-program function DCE;
10. liveness-based name coalescing, dependency-ordered phi copies, and minified
    backend peepholes.

## Executable evidence

`scripts/verify-matrix.sh` compiles 34 independent `.lil` programs, including a
multi-file module graph, with one
`--target all` invocation per program. Each invocation emits JavaScript, emits
C, and invokes Clang for a native executable. The script then compiles the
emitted C independently and requires the JavaScript, direct native executable,
independently compiled C executable, and checked-in expected output to match.

`benchmarks/run.sh` compiles nine behaviorally equivalent LilScript/JavaScript
workloads, runs both outputs, invokes Closure `ADVANCED`, and measures normalized
raw, gzip-9, and Brotli-11 bytes. The benchmark is a reproducible corpus result,
not a proof that any finite compiler beats another compiler on every possible
future program.
