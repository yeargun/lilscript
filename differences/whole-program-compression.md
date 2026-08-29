# Whole-program compression

Parent: [comparison index](index.md). Selection model: [objective and search](objective-and-search.md).

## Closure's compression loop

Closure ADVANCED is not a single inliner followed by a minifier. Its main compression loop combines:

- property/simple inlining;
- dead property-assignment removal;
- unused-return and parameter optimization;
- function and variable inlining;
- object-literal scalar replacement;
- unused-code removal;
- local peepholes.

The loop is preceded by property collapse, purity analysis, early variable/object inlining and DCE,
and followed by flow-sensitive inlining, final DCE, chunk motion, and constructor cleanup. The
current schedule is in
[`DefaultPassConfig.java` lines 936-1143](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/DefaultPassConfig.java#L936-L1143)
and [lines 519-582](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/DefaultPassConfig.java#L519-L582).

The important idea is phase interaction. Removing a return makes a call removable; removing an
argument can make a parameter and then its initializer dead; inlining exposes constants and
property accesses; property collapse turns members into ordinary variables; DCE then removes the
newly isolated definitions.

## Lazy dead-code elimination

`RemoveUnusedCode` is Closure's current central DCE pass; the old `NameAnalyzer` implementation is
gone. It stores deferred traversals for apparently dead definitions and activates their initializer
or body dependencies only if the destination later becomes live. This naturally removes dead
mutually recursive groups without eagerly traversing everything.

See
[`RemoveUnusedCode.java` lines 49-121](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/RemoveUnusedCode.java#L49-L121)
and [lines 1645-1697](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/RemoveUnusedCode.java#L1645-L1697).

When the binding is dead but evaluation is not, Closure extracts residual effects. Depending on
the shape, deletion can retain the RHS, a computed key, or an ordered comma expression containing
both. See
[`RemoveUnusedCode.java` lines 2522-2669](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/RemoveUnusedCode.java#L2522-L2669).

Property liveness is mostly keyed by spelling, so one use can pin all candidate definitions with
that name. LilScript's typed field identity can be more precise.

## Effect summaries

`PureFunctionIdentifier` computes interprocedural summaries that separately track global mutation,
argument mutation, receiver mutation, and throws. It builds a reverse call graph and propagates
effects to a fixed point. Call nodes then carry compact flags used by DCE and motion. See
[`PureFunctionIdentifier.java` lines 53-154](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/PureFunctionIdentifier.java#L53-L154)
and [lines 725-1184](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/PureFunctionIdentifier.java#L725-L1184).

The summary is conservative by global/property spelling rather than exact function identity. Extern
annotations can refine behavior, while `@pureOrBreakMyCode` is an explicitly unsafe trust escape.

LilScript already has an interprocedural effect system that distinguishes inherent effects,
mutated parameters, retained parameters, dynamic coercions, host operations, getters/proxies, and
spread. See [`optimizer.rs` lines 11764-12431](../src/optimizer.rs#L11764-L12431).

## Call-signature optimization

Closure builds one normalized reference map shared by call optimizers. Type disambiguation improves
its precision because property references otherwise group by source spelling. See
[`OptimizeCalls.java` lines 36-154](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/OptimizeCalls.java#L36-L154).

`OptimizeParameters` can:

- remove optional formals no caller supplies;
- remove side-effect-free arguments for unused formals;
- use `0` to preserve positional holes when needed;
- move a constant/equivalent call-site argument into the function body;
- remove trailing `undefined`;
- materialize fixed rest arguments inside the callee.

See
[`OptimizeParameters.java` lines 44-153](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/OptimizeParameters.java#L44-L153),
[lines 318-438](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/OptimizeParameters.java#L318-L438),
and [lines 906-1155](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/OptimizeParameters.java#L906-L1155).
Spread, defaults, `arguments`, side-effect ordering, closures, `this`, `super`, and scope movement
produce extensive backoffs.

`OptimizeReturns` removes return values ignored by every known caller while preserving effects. It
must run repeatedly for call chains. See
[`OptimizeReturns.java` lines 31-153](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/OptimizeReturns.java#L31-L153).

LilScript already removes unused parameters and unobserved return values, specializes constant
parameters, and repeats these around scalar simplification and inlining in a fixed schedule. See
[`optimizer.rs` lines 258-355](../src/optimizer.rs#L258-L355) and
[lines 4505-4800](../src/optimizer.rs#L4505-L4800). Closure's useful influence is the breadth of
call-shape safety tests and its generic outer fixed point, not the uncosted rewrites themselves.

## Function and variable inlining

Closure's function inliner supports direct-expression and block inlining. Its standout feature is a
generated-minified-code estimate that charges argument aliases, call syntax, function declaration,
blocks, return labels, result assignments, and whether the original function can disappear. If
block inlining loses, it discards block sites and reconsiders direct sites. See
[`InlineFunctions.java` lines 698-825](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/InlineFunctions.java#L698-L825)
and
[`FunctionInjector.java` lines 905-1075](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/FunctionInjector.java#L905-L1075).

Variable inlining is less cost-sensitive: immutable single-assignment values may be duplicated, and
single-read values move when `LocalVarMotion` proves safety. Flow-sensitive inlining adds reaching
definition/use analysis but skips functions above 100 variables. See
[`InlineVariables.java` lines 455-586](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/InlineVariables.java#L455-L586)
and
[`FlowSensitiveInlineVariables.java` lines 151-237](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/FlowSensitiveInlineVariables.java#L151-L237).

LilScript has typed straight-line and single-use CFG inlining, profile/constant/capture
specialization, function folding/subsumption, closure-factory alternatives, and emission-only IIFE
or helper substitution candidates. Relevant ranges are
[`optimizer.rs` lines 4141-4391](../src/optimizer.rs#L4141-L4391),
[`optimizer.rs` lines 6044-6717](../src/optimizer.rs#L6044-L6717),
[`optimizer.rs` lines 10291-11518](../src/optimizer.rs#L10291-L11518), and
[`codegen_ir_js.rs` lines 2573-3537](../src/codegen_ir_js.rs#L2573-L3537).

The strongest adaptation is to emit both inline and non-inline LilScript candidates through the
real emitter and exact codec scorer, while retaining a structural growth cap.

## Object scalar replacement

Closure's `InlineObjectLiterals` splits an unescaped local object into per-property variables. It
rejects direct object use, method calls that need `this`, delete, computed keys, accessors, spread,
self-reference, and problematic loops. Reassigning the object expands into all field assignments and
fills absent fields with `undefined`. See
[`InlineObjectLiterals.java` lines 35-95](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/InlineObjectLiterals.java#L35-L95)
and [lines 270-473](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/InlineObjectLiterals.java#L270-L473).

There is no profitability check; the rewrite exists to expose constants and dead fields.

LilScript already has a typed form for owned aggregates: escape-driven scalar
replacement, positional layout, named layout, loop-carried struct phis, and optional retention of
the original object as a scored representation. See
[`optimizer.rs` lines 7142-7308](../src/optimizer.rs#L7142-L7308) and
[`optimizer.rs` lines 9531-9818](../src/optimizer.rs#L9531-L9818),
[`codegen_ir_js.rs` lines 4539-4663](../src/codegen_ir_js.rs#L4539-L4663), and
[`decision_registry.rs` lines 1564-1573](../src/decision_registry.rs#L1564-L1573).

## Devirtualization and namespace exposure

Closure turns eligible methods into free functions with an explicit receiver. The immediate syntax
can grow; the value is exposing a direct call edge to inlining, DCE, and chunk motion. It requires
all definitions and uses to be understood and rejects exports, accessors, constructors, `super`,
tear-offs, optional calls, and invalid chunk dependencies. See
[`DevirtualizeMethods.java` lines 38-150](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/DevirtualizeMethods.java#L38-L150)
and [lines 231-475](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/DevirtualizeMethods.java#L231-L475).

Property collapse similarly turns qualified paths into variables so ordinary optimizations can see
them. LilScript already devirtualizes known methods and erases many modules/aggregates before JS.
Residual namespace collapse should target only shapes that survive typed lowering.

## Dead assignments and trivial property/method inlining

Closure separately runs flow-sensitive dead local-assignment elimination. It uses CFG liveness,
skips functions containing inner functions, and applies the shared 100-variable analysis limit. See
[`DeadAssignmentsElimination.java` lines 34-131](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/DeadAssignmentsElimination.java#L34-L131).

`InlineProperties` replaces immutable properties assigned exactly once and unconditionally on a
constructor/prototype/static class identity. An extern, second definition, mutable value, unknown
type, or conditional execution invalidates the spelling. See
[`InlineProperties.java` lines 33-92](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/InlineProperties.java#L33-L92)
and [lines 174-223](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/InlineProperties.java#L174-L223).

`InlineSimpleMethods` recognizes equivalent no-argument methods that return a property path or
literal, plus empty methods. It replaces calls but retains declarations because computed calls may
remain unseen. See
[`InlineSimpleMethods.java` lines 30-55](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/InlineSimpleMethods.java#L30-L55)
and [lines 90-139](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/InlineSimpleMethods.java#L90-L139).

LilScript's SSA DCE/value numbering, owned-field forwarding, pure-helper substitution, and method
devirtualization cover analogous typed cases. Closure's edge-case tests remain useful for retained
identity classes and JavaScript interop.

`DeadPropertyAssignmentElimination` is a separate same-block overwrite pass. It does not build a
CFG: entering a block, hook, switch, accessor, or ambiguous `Object.defineProperty` boundary is
treated as reading/escaping properties. Extern and accessor spellings are excluded, functions with
inner functions are skipped, and only an earlier write followed by a provably safe overwrite is
removed. See
[`DeadPropertyAssignmentElimination.java` lines 39-132](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/DeadPropertyAssignmentElimination.java#L39-L132).

LilScript's [`eliminate_overwritten_field_stores`](../src/optimizer.rs#L2565-L2607) is similarly
barrier-delimited but works on typed owned fields and skips exception-bearing functions. Closure's
test corpus can broaden overwrite/barrier coverage without replacing that identity model.

## Smaller whole-program transforms

- `OptimizeConstructors` removes explicit constructors equivalent to implicit class behavior.
- `CollapseAnonymousFunctions` changes eligible `var f=function(){}` into `function f(){}`.
- `FunctionRewriter` can pool repeated empty/identity/constant/getter/setter function shapes behind
  factories, but is off in ADVANCED because of runtime regressions despite byte reduction.
- `ExtractPrototypeMemberDeclarations` aliases repeated `X.prototype` prefixes when setup cost is
  amortized.

Sources:

- [`OptimizeConstructors.java` lines 29-57](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/OptimizeConstructors.java#L29-L57)
- [`CollapseAnonymousFunctions.java` lines 23-131](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/CollapseAnonymousFunctions.java#L23-L131)
- [`FunctionRewriter.java` lines 28-148](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/FunctionRewriter.java#L28-L148)

## Comparison summary

| Mechanism | Closure advantage | LilScript advantage | Direction |
|---|---|---|---|
| DCE | Mature JS residual-effect cases and lazy dependencies | Typed SSA/effect/field identity | Differential-test and fill edge cases |
| Effects | Broad JS/extern convention support | More precise language-level facts | Keep typed model |
| Call signatures | Extensive syntax/order backoffs and fixed-point interactions | Typed direct-call graph and repeated fixed schedule | Test an outer fixed point or genuine order candidates |
| Function inlining | Detailed generated-code estimator | Real emitter and codec scorer available | Score actual alternatives |
| Object splitting | Mature JS escape/use guards | Typed aggregate representation search | Keep LilScript architecture |
| Devirtualization | Handles general JS method definitions | Direct semantic method identity | Port only uncovered interop shapes |
| Repeated function shapes | Factory extraction option | Function folding/subsumption/helper clustering | Compare as scored candidates |
