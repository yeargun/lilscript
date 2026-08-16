# Inlining, specialization, and function sharing

Parent: [compilation](README.md). Search behavior:
[candidate search](candidate-search.md). Source anchors: the inlining fixed point,
specialization, subsumption, parameterized merging, and identical folding families in
[`src/optimizer.rs`](../../../src/optimizer.rs), plus emission-only function
representations in [`src/codegen_ir_js.rs`](../../../src/codegen_ir_js.rs).

These transforms solve opposing size problems:

| Family | Opportunity | Main refusal |
|---|---|---|
| expression/small CFG inline | expose constants, erase call/setup overhead | recursion, export/identity, async/generator/exception complexity, budget |
| single-use CFG inline | remove a private one-call body | incompatible structured flow or growth |
| single-use function-expression emission | suppress one private declaration and emit its anonymous function/generator at the only call | module/public/reusable/loop/capture/identity/recursive use |
| constant parameter specialization | clone a direct callee around known values/functions | clone budget, generic/representation mismatch |
| profile call-site specialization | clone hot/profitable call groups | profile gates/count/body caps |
| capture-signature cloning | remove constant closure environment slots | escaping/identity/capture mismatch |
| identical folding | share equal private normalized CFGs | public/address-taken/escape incompatibility |
| function subsumption | redirect a private body to an equal more-parameterized body | non-exact normalized match or unsafe argument binding |
| parameterized merging | share permuted or single-operand-divergent private bodies | effects/type/identity mismatch |

Inlining is not monotonically smaller: duplication can lose codec repetition and
inflate jQuery. Size-first search can compare no-inline, no-factory-inline,
phase-order, and aggressive variants as complete artifacts. The configured pipeline
is always retained.

Specialized clones are not accepted one by one. A pipeline may contain multiple
clones; the emitted complete program competes against a pipeline with specialization
disabled when that search feature is active.

Sharing never changes JS omitted-argument behavior, declared arity, construction,
function identity, or exported names. Subsumption adds explicit typed arguments at
private direct calls; it does not depend on JavaScript defaults.

## Emission-only single-use function expressions

`IrJsOptions::inline_single_use_functions` is `false` in both its type default and
the configured baseline. When candidate search is enabled and
`structured-closure-inlining` is enabled by the compression policy,
`ProjectConfig::single_use_function_expression_candidates_enabled` lets the beam
also score `true`. This does not rewrite neutral IR: the emitter suppresses the named
declaration and renders an anonymous function or generator directly as the call's
callee. The configured/named artifact remains in competition, and raw, gzip, or
Brotli complete-artifact scoring decides the winner.

The whole-module proof requires script (not ESM/module) output; a live ordinary
`Function` with no captures; no eager or lazy export, closure/address-taking, method
call, constructor use, or recursion; and exactly one direct call from the module
entry. That call's block must be outside every loop-shaped region, since moving a
declaration into a repeated call would create a fresh function each iteration. A
call from a reusable helper is also refused even if that helper is invoked only once
in the current artifact. See `assign_inline_single_use_functions` and
`render_single_use_function_expression` in
[`src/codegen_ir_js.rs`](../../../src/codegen_ir_js.rs); the paired positive/refusal
regressions are `emits_private_single_use_functions_at_their_only_call_site` and
`preserves_named_functions_when_single_use_expression_is_disabled_or_public`.

A related structured-emission proof keeps a terminal one-use producer as the input
expression of `for in`/`for of`, including an effectful generator call, instead of
forcing an iterable temporary. `structured_iteration_input_can_defer` requires an
unconditional jump to the matching empty loop header and no intervening operation,
so the input still evaluates exactly once immediately before loop entry.

## Exclusive closure expressions

`IrJsOptions::inline_exclusive_closures` is on in the configured baseline. A live
private ordinary or closure function whose only use is one `Closure` instruction is
emitted as a function expression at that site instead of a top-level declaration plus
capture wrapper. Reusable callers and loop blocks stay eligible: the wrapper would
have allocated a fresh function on every evaluation anyway, and the body still
appears once in the artifact. Candidate search may disable the tactic when the named
declaration compresses better.

Exports, direct/method/constructor uses, adapters, defaults, async/generator
functions, and unstructured multi-block bodies are refused. See
`assign_inline_exclusive_closures` in [`src/codegen_ir_js.rs`](../../../src/codegen_ir_js.rs).

## Pure-helper expression substitution

`IrJsOptions::pure_helper_inlining` is `None` in the configured baseline. With
candidate search and the `pure-helper-inlining` compression decision, complete
artifacts also compete under `SingleStaticUse` and `AllEligible`. The former removes
only helpers with one static direct call; the latter may duplicate a reusable helper
when the selected raw, gzip, or Brotli objective rewards the resulting context. The
decision is emission-only: neutral typed IR and the named-helper baseline remain
available.

The whole-module proof accepts only live private ordinary functions whose declared
purity has already been validated. It rejects eager and lazy exports, capture or
address-taking, methods/constructors, recursion, async/generator functions, `try`,
`throw`, mutable/allocating/host operations, incomplete arity, and a helper DAG whose
callee is not itself renderable. The complete body must be a return-only structured
CFG expressible from a deliberately small pure operation set. Nested eligible helper
DAGs are closed to a fixed point; a declaration is suppressed only if every static
call site can be replaced.

Declared purity alone is not enough to cross a dynamic JavaScript boundary. Unary
and binary expression operands must have known non-null primitive scalar types;
`JsValue`, nullable/reference alternatives, dynamic property/proxy operations, and
coercions that can invoke user code or throw are refused. Non-coercive typed tests
such as `typeof`/nullish checks may remain eligible. The narrow grammar also admits
typed string `startsWith`/`endsWith` intrinsics and `FieldGet` only when the receiver
type is a LilScript struct or class (including instantiated forms). Record, host,
index, unknown, and proxy-visible reads remain outside the proof. This target-side
refusal is deliberately stricter than accepting an arbitrary expression-shaped IR
operation.

Substitution uses native `JsExpression` nodes, never textual name replacement, so
precedence and typed integer coercions survive. Already materialized caller names and
immutable literals can be reused directly. A single-use caller-cache expression is
consumed only transactionally; if rendering fails, the ordinary call still owns it.
A non-materialized cached actual keeps an eager single-evaluation binder at the call
point, including when the template repeats or does not use that parameter. Those
binders nest in original argument/instruction order, reserve caller identifiers
against capture, and remain even for an unused argument because an ordinary call
would still evaluate it. Only a one-actual identity template may collapse its sole
binder directly to that initializer. Functions observing mapped `arguments` are
refused. Observable instructions are bound at their original CFG position, so an
untaken branch cannot add or suppress an evaluation and multiple evaluations cannot
reverse order. Regressions cover repeated/unused/conditional actuals, recursion,
exports, address-taking, alias capture, structural barriers, and `arguments`
semantics.

`ControlFlowFunction::origin` distinguishes source, synthesized, and
`RepeatedRegionOutline` functions. The optimizer excludes the last category from
late private subsumption, parameterized merging, and identical folding so a scored
reuse boundary cannot silently become an ordinary helper. Under `AllEligible`, a
multi-use outlined helper therefore remains shared while eligible pure leaves inside
it are substituted; if only one call survives, the usual single-use rule may remove
the stale boundary. The IR-finalist pre-probe that makes this interaction visible is
described in [candidate search](candidate-search.md#level-1--ir-optimizer-variants).

The helper policy is crossed atomically with dense string-return-table off/on. That
joint search is important for dictionary and router algorithms: an isolated helper
rewrite or isolated table can lose while the combined complete artifact wins.
