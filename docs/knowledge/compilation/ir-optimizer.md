# IR optimizer

Parent: [Compilation](README.md). Sources:
[`src/optimizer.rs`](../../../src/optimizer.rs),
[`src/value_analysis.rs`](../../../src/value_analysis.rs). Gates:
[`[optimization]`](../config/optimization.md). Closure mapping:
[`docs/optimization-coverage.md`](../../optimization-coverage.md).

## Options

`OptimizationOptions` is resolved from `[optimization]` preset plus per-key overrides. JS compilation then **ANDs** several flags with JavaScript effort/compression (`js_optimizer_options` in `src/config.rs`): call-site specialization, capture cloning, identical folding, subsumption, compress passes, parameterized merging, inline limits from `priority`.

`preset = "none"` disables optional transforms but keeps mandatory IR normalization and correctness analyses.

## Pass order (`optimize_control_flow_inner`)

1. **Globals** — early `internalize_entry_globals`, then `eliminate_unread_globals` (`global_optimization` / DCE)
2. **mem2reg SSA** — skip exception-region and mutable-capture locals
3. Fuse linear array/plain-object builders
4. **Scalar fixed point** — fold, finite values, algebraic simplify, CSE, unreachable (`constant_folding`, `finite_value_propagation`, `algebraic_simplification`, `common_subexpression_elimination`)
5. Immutable global prop, **devirtualize** methods and known closures
6. **Specialize** constant parameters; profiled call sites; clone constant-capture closures; unused params/returns; validate `pure`
7. **Inlining fixed point** — DCE ↔ small-function inline ↔ single-use CFG inline; re-specialize. Limits: `inline_instruction_limit`, `inline_control_flow_limit`, `inline_growth_limit`. Closure factories skipped if `inline_closure_factories = false`
8. Second scalar fixed point
9. **Function subsumption** (off by default in native; size-first JS may search on/off)
10. Escape analysis; scalar replace classes/aggregates; dead field stores
11. **`run_compress_passes`** — [compress passes](compress-passes.md)
12. Re-fuse builders, scalar FP, byte-buffer collapse, DCE
13. Third inlining FP (compression can expose new inline targets)
14. Parameterized / permuted private-function merge; identical private-function fold
15. Final DCE — SSA DCE, **`eliminate_dead_functions`** (tree shake), then
    `eliminate_unread_globals` counting only live functions so a dead helper cannot
    keep a write-only global alive; SSA DCE and dead-function elimination run once
    more so empty stores and their callees disappear. When exactly one
    non-extern, non-entry-exported global remains and global optimization is enabled,
    retry entry-global internalization, re-run mem2reg/scalar simplification/DCE;
    then prune unused foreign imports

Inlining is refused for entry/extern, recursion, exports, async/generator, type parameters, multi-block (except the single-use CFG path), exception complexity, and over-budget bodies. Address-taken functions keep a body unless growth budget allows.

## Analyses the language makes possible

| Analysis | File | Feeds |
|---|---|---|
| Escape | `optimizer.rs` | scalar replacement, mangling legality |
| Integer ranges | `value_analysis.rs` | `|0` elision, mutation spelling |
| Finite values (≤4 alts) | `value_analysis.rs` | specialization, branch fold; widens at export/extern/indirect/closure/untyped aggregate |
| Effects / purity | `optimizer.rs` | DCE of unused calls, mutation graphs |
| Exact array lengths | value analysis + fold | callback snapshot, packing |

Finite-value facts become unknown at exported, extern, indirect-call, closure, or untyped aggregate boundaries.

## Proof-scoped nullable simplification

Optional member/index lowering records an `ExpressionPhi::OptionalAccess` carrying
the receiver identity in
[`src/ir.rs`](../../../src/ir.rs) and
[`src/lower.rs`](../../../src/lower.rs). The JS emitter reconstructs
`receiver?.member ?? absent` or `receiver?.[index] ?? absent` only when the surviving
structured region still proves all of the following: the branch is the matching null
comparison, the present access uses that receiver through identity-only nullable
unwraps, both arms reach the same phi, and every arm value can move into the
expression without crossing an effect/order barrier. `absent` is either canonical
`null` or the already-lazy fused `??` fallback. The coalesce is required because JS
optional chains produce `undefined` while LilScript nullable values use `null`. If an
integer normalization would turn a nullable absence into zero, or any region proof
fails, emission retains the explicit structured control flow. See
`render_optional_access_region` in
[`src/codegen_ir_js.rs`](../../../src/codegen_ir_js.rs).

Separately, scalar folding removes an optional-access null guard only when the
receiver's propagated IR type proves that `null`/`undefined` cannot occur. `Null`,
`Nullable`, `void`, type parameters, or a union containing one of those keep the
guard. This is a closed-world type proof, not a truthiness assumption about a host
value. Regressions: `folds_optional_access_guard_for_a_proven_non_null_receiver` in
[`src/optimizer.rs`](../../../src/optimizer.rs) and
`reconstructs_optional_access_conditional_expression_phis` in
[`src/codegen_ir_js.rs`](../../../src/codegen_ir_js.rs).

## Late entry-global internalization

The early global pass cannot localize a binding while a then-live helper also uses
it. Final inlining and tree shaking can remove that helper, so the optimizer retries
`internalize_entry_globals` after final reachability. Today this late representation
change is attempted only when exactly one non-extern, non-entry-exported global
remains; the ordinary entry-only, eager/lazy export, and sharing checks still decide
whether that binding is eligible. Multiple interacting globals could replace a
compact assignment with parallel phi copies and belong in a future whole-IR
candidate search.

When a newly eligible global becomes an entry local, `internalize_entry_globals`
marks the entry unpromoted and mem2reg runs again. Promotion is re-entrant: it seeds
phi placement from existing local-origin phis and fills incoming edges only for phis
created in the new round, so it neither duplicates nor mutates the first round's
SSA. The regression is `mem2reg_is_reentrant_for_newly_internalized_entry_globals`
in [`src/optimizer.rs`](../../../src/optimizer.rs).

## Subsumption and merging

**Subsumption:** a private direct-call-only function may redirect to a more-parameterized twin if binding extra args to typed scalars or known functions makes normalized SSA/CFG **exactly** equal. Calls get explicit arguments (never JS omitted-arg behavior). Exports, address-taken, methods, constructors, closures, type mismatch, non-equal bodies: rejected.

**Parameterized merging:** permute-parameter and single-operand-divergent private functions. Size-first + compression decision; `[optimization] parameterized_function_merging = false` kills it.

**Identical folding:** after inlining/specialization, private identical CFGs with compatible escape share one body.

## JS variants of this pipeline

When candidate search is on, `optimize_and_select_javascript` clones IR from
`SCORED_IR_VARIANTS` (plus phase-order / compress-pass probes) and re-runs with
those toggles. Each surviving emission is codec-scored. See
[candidate search](candidate-search.md) and
[decision registry](decision-registry.md#ir-optimizer-variants-level-1-search).

`keep-object` (`scalar_replacement = false`) is one of those clones when
`joint-representation-search` is on. A `LocalOnly` struct the pass can explode
still explodes on the incumbent clone; search can keep the object if that
artifact wins. Root `lilscript.toml` omits joint search, so language tests stay
on the incumbent pass.
