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
| Early/late peephole optimization | Constant folding, algebraic identities, boolean simplification, branch inversion, nested literal-capture branch folding, precedence-carrying unary/binary/conditional/call/member/integer-normalization expressions, token-safe negative operands, precedence-safe parenthesis removal, compact literals/loops/returns/declarations, and a complete-artifact lexer plus Pratt-parsed compound-assignment superoptimizer |
| Numeric representation lowering | Signed-i32 range analysis propagates bounded loop induction values, direct-call arguments and returns, and owned nominal fields; it removes coercions only for proven-safe operations. Ordinary multiplication emits `x*y|0` when normalization is required, while source-written `Math.imul` remains an explicit exact operation |
| Inline variables and constants | mem2reg SSA, constant propagation, single-assignment global propagation, constant rematerialization, one-use expression fusion, exact array-parameter lengths, and bounded boolean/string/null argument and return sets across closed stable direct-call sets |
| Inline functions and simple methods | Fixed-point expression inlining plus single-use multi-block CFG inlining; size-first single-file/ESM builds compare that IR with a fully outlined IR under the exact selected codec |
| Inline/collapse properties | Nominal field resolution, positional field indexes, struct/class scalar replacement, and owned-field range/finite-constant summaries invalidated at untyped boundaries |
| Collapse object literals | Non-escaping structs dissolve into SSA scalars; remaining typed aggregates use positional arrays in JavaScript |
| Disambiguate/ambiguate/rename properties | Nominal owner types and field indexes remove internal property names entirely; boundary names remain ABI-stable |
| Devirtualize methods | Class calls become direct typed function calls before inlining |
| Optimize calls and constructors | Direct-call lowering, recursive-call protection, constant-parameter specialization, optional profile-weighted constant and higher-order call cloning, constant-capture closure cloning, known-closure devirtualization, unused parameter/return removal, constructor inlining, allocation removal, and effect summaries |
| Mark pure functions | Interprocedural fixed-point summaries separate inherent effects from mutations of specific parameters across direct calls and known closure captures; local scratch-allocation mutation filtering, checked `pure` contracts, and trusted `pure extern` declarations share that model |
| Dead assignment elimination | SSA promotion removes local stores; DCE removes unused value chains, complete unobserved local array/map/set mutation graphs, and parameter-mutating helper calls when every affected allocation group is unobserved |
| Dead property assignment elimination | Overwritten typed field stores are removed between observation barriers |
| Remove unused code | Unread globals, unreachable blocks, unused pure calls, unused allocations and mutation graphs, instructions, and call-graph-unreachable functions are removed; residual identical private direct-call functions fold after inlining while observable identities remain distinct |
| Flow-sensitive inline variables | SSA def-use counts, side-effect-aware deferred expression emission, and one-use boolean merge phis fused into immediately following structured branches |
| Coalesce variable names | CFG liveness, interference graph coloring, direct-phi affinity across conservative deferred-expression barriers, contracted non-interfering phi groups, codec selection across all three layouts, and reuse of dead locals as parallel-copy temporaries |
| Collapse variable declarations | Adjacent bindings and first phi assignments are combined by the JS backend; cyclic phi copies compare tuple and scalar schedules under the configured codec |
| Rewrite/collapse anonymous functions | Small typed closures become expression or structured block arrows; capturing closures pass explicit environments; literal captures expose dead branches during final emission |
| Alias strings | Repeated constants are value-numbered and profitable long strings receive shared short bindings; size-first also considers delimiter-packed immutable string tables; final pooling, packing, quote, and coercion variants are selected against exact raw/gzip/Brotli cost |
| Rename variables and globals | Use-frequency-ranked base-54/base-64 identifiers with extern names reserved, cross-scope reuse of unreachable top-level colors, loop-weighted owned-property assignment, plus exact-compressor selection of emitted-character-ranked alphabets and similarity-clustered declaration order |
| Rescope globals | Entry-only globals become locals; immutable shared globals become constants |
| Rewrite modules and tree shake exports | Relative module graphs are linked into private symbol namespaces; executable exports remain shakeable, while `js-module` roots runtime exports and emits mangled ESM aliases |
| Cross-chunk code/method motion | Whole-program optimization runs before deterministic ESM partitioning; preserve-module, measured shared chunks, and typed lazy imports emit explicit dependencies, live exports, and a content-addressed manifest |
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
4. local constants, bounded interprocedural boolean/string/null values, owned
   nominal field constants, and stable array-length propagation,
   trivial-phi removal, algebraic simplification, local value numbering, branch
   folding, and unreachable removal;
5. immutable-global propagation, devirtualization, and explicit purity
   validation;
6. constant-parameter specialization, profile/byte-budgeted constant and
   higher-order call cloning, constant-capture closure cloning, unused
   parameter/return removal, and fixed-point expression and multi-block CFG
   inlining;
7. escape analysis, class/struct scalar replacement, allocation-root alias and
   parameter-effect summaries, unobserved collection-graph/call removal, and
   dead field stores;
8. another scalar fixed point;
9. effect-aware SSA DCE, late identical-private-function folding, and
   whole-program function DCE;
10. codec selection among configured inlining, closure-factory-preserving
    partial inlining, fully outlined, and unspecialized optimizer IRs,
    followed by module-level integer argument/return/field range analysis,
    liveness-based name coalescing, structured boolean-phi deferral,
    dependency-ordered phi copies,
    liveness-reused cycle temporaries, codec-selected conservative/direct-phi
    affinity/group and scalar/tuple copy layouts, induction ranges, shortest
    numeric literals, precedence-carrying expression nodes, structured closure
    selection, conditional/comma forms, structured/state-machine dispatch,
    `while`/`for`/`do` and update-clause layouts, range-proven prefix/postfix/
    compound mutation spelling, string-table packing, entropy-aware cross-scope
    names and properties, a parsed final peephole, deterministic startup guards,
    similarity-clustered declaration layout, typed-IR
    deoptimization/allocation/indirect-call/monomorphism scoring with
    optional hot function and loop weights, and compressor-aware candidate
    selection;
11. source ownership and mandatory lazy boundaries, followed by full-plan chunk
    candidate scoring over raw/gzip/Brotli bytes, requests, dependency depth,
    preload policy, shared reachability, and cache reuse;
12. cross-chunk binding analysis, per-namespace lazy export tree shaking, stable
    source-identity chunk names, and deterministic ESM/manifest emission.

## Executable evidence

`scripts/verify-matrix.sh` compiles 65 independent `.lil` programs, including a
multi-file module graph, with one
`--target all` invocation per program. Each invocation emits JavaScript, emits
C, and invokes Clang for a native executable. The script then compiles the
emitted C independently and requires the JavaScript, direct native executable,
independently compiled C executable, and checked-in expected output to match.
The corpus includes collection mutation/identity/nullable lookup and binary
memory copy/view/coercion behavior under both maximum and disabled optional
optimization, for 130 backend-mode executions. This includes regressions for
interprocedural integer ranges, finite values/fields, exact array lengths,
entry-length snapshots across all array callback methods, unobserved collection
mutation removal, multi-use conditional-return values, and
loop-carried values crossing an early return and a nested short-circuit
coalescing regression extracted from the Solid client-runtime gate.
`scripts/verify-bundles.mjs` additionally executes preserve-module, shared, and
lazy bundles; checks exact compressed manifest metadata and preload output;
exercises live bindings; verifies missing-chunk failure normalization; and
proves deterministic package locks plus stale-source rejection.

The native C emitter runs a conservative second storage-placement analysis.
Fixed non-escaping local arrays, class values, and eligible closure
environments use frame storage; larger bounded arrays use a per-function region
that is released along every generated return path. Resizable arrays and any
value crossing a return, global, phi, capture, unresolved call, or external ABI
boundary retain heap storage. Native allocation placement is configuration
controlled and covered by emitter tests plus the full C/native execution
matrix.

`lilscript-differential` independently evaluates generated typed AST programs
without lowering them to CFG/SSA. The fixed 64-case release batch exercises all
integer operators, overflow, zero divisors, shifts, direct calls, mutation,
short-circuit effects, branches, bounded loops, loop control, shadowing, array
identity and indexed mutation, push/pop, captured arrows, and callback-time
array growth. A fixed binary-memory kernel additionally covers byte coercion,
copying and aliasing views, shared storage, and negative slice indices. It
requires exact agreement from optimized JavaScript,
optimizer-disabled JavaScript, the direct native executable, and emitted C
compiled in a separate compiler invocation. The generated source and expected
output are retained for seed reproduction; the exact scope and commands are
documented in `docs/differential-testing.md`.

`benchmarks/finite-values/run.mjs` holds inlining, scalar replacement, source,
and every other optimizer setting constant while toggling only
`finite_value_propagation`. Both variants execute the matrix contract before
the gate requires a raw, gzip-9, and Brotli-11 win. The current checked workload
improves from `216/155/118` bytes to `143/108/77`.

`benchmarks/function-folding/run.mjs` disables inlining in both builds and
toggles only late identical private-function folding. Export and address-taken
identity barriers remain active. Both artifacts execute `95660`; folding one
residual body changes `177/139/111` to `123/129/105` raw/gzip/Brotli.

`benchmarks/function-layout/run.mjs` keeps raw output length constant and
compares source declaration order with a similarity path proposed from repeated
eight-byte runs. The complete codec chooses the result only after emission.
Both artifacts execute `-1393288640`; the checked fixture changes
`1133/460/369` to `1133/454/362` raw/gzip/Brotli.

`benchmarks/ir-variants/run.mjs` holds the source, final-emission search, and
all optimizer settings constant while omitting only `ir-inlining-variants`.
Both artifacts execute first. Exact Brotli selection retains a shared helper
and improves `267/144/109` bytes to `219/113/83`; the complete Emotion hash
port independently improves from `866/535/456` to `816/532/452`.

`benchmarks/closure-factory-variants/run.mjs` keeps ordinary inlining and the
fully outlined IR candidate available in both builds while omitting only
`ir-closure-factory-variants`. Twelve distinct capture signatures execute
through JavaScript, C, and native gates. Partial factory preservation wins the
selected Brotli objective and changes `677/244/177` to `648/255/176`
raw/gzip/Brotli. The gzip increase is reported rather than hidden; a gzip-cost
build retains the fully outlined IR as a competing candidate.

`benchmarks/loop-spelling/run.mjs` executes an order-sensitive control-flow
fixture with identical optimizer and final-emission policy while omitting only
`loop-spelling-selection`. With the broader `do`, update-clause, and structural
control-flow search active, both variants select `527/243/178`
raw/gzip/Brotli. Both artifacts must produce `137` before a three-codec
non-regression gate. The complete MurmurHash port is also byte-identical between
modes at `1734/831/733`; neither workload is presented as a loop-spelling win.

`benchmarks/mutation-spelling/run.mjs` compiles and executes the complete
Levenshtein port while omitting only `mutation-spelling-selection`. SSA use
counts and integer ranges gate every shorthand before exact prefix/postfix
scoring. The isolated production artifact changes from `1583/899/780` to
`1582/900/777` raw/gzip/Brotli under the Brotli objective. Candidate retention remains stratified by
loop-spelling family before mutation scoring so one early layout cannot erase
the other families. Gzip-targeted builds retain assignment spelling as a
candidate rather than forcing the Brotli winner.

`benchmarks/run.sh` compiles ten behaviorally equivalent LilScript/JavaScript
workloads, runs both outputs, invokes Closure `ADVANCED`, and measures normalized
raw, gzip-9, and Brotli-11 bytes. The benchmark is a reproducible corpus result,
not a proof that any finite compiler beats another compiler on every possible
future program.

`benchmarks/paired/run.mjs` removes manual source-style differences for a
second gate by generating both languages from one neutral integer workload
schema. Every generated case must match through Closure JavaScript, LilScript
JavaScript, emitted C, and native execution, and LilScript may not exceed
Closure in any Brotli cell under the project default objective. Raw and gzip
remain published because codec tradeoffs are evidence, not failures.
`benchmarks/browser/run.mjs` separately
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
