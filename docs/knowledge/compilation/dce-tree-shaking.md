# Dead-code elimination and tree shaking

Parent: [compilation](README.md). Closed-world roots:
[language/closed world](../language/closed-world.md). Source anchors:
`eliminate_dead_control_flow_instructions`, `eliminate_unread_globals`,
`eliminate_dead_functions`, and `prune_unused_foreign_imports` in
`src/optimizer.rs`.

LilScript tree shaking is typed whole-program reachability, not an ESM text pass.
Static modules are linked before IR, so unused functions, methods, initializers,
globals, imports, fields, and whole type-only declarations may disappear across file
boundaries.

Roots depend on the boundary:

- closed executable: entry effects, observable output/host behavior, and reachable
  calls/globals; source `export` alone is not a root;
- reusable `js-module`: root runtime exports and their public behavior are roots;
- split/lazy: live namespace exports and dependency edges for each retained dynamic
  root;
- extern/foreign edges: conservatively effectful unless the typed contract proves
  purity; side-effect-only imports remain roots;
- closed `js-module` entries prune unused named `import extern` specifiers even when
  the program has no exports; script/native skip that prune so JavaScript module
  edges stay visible for native rejection;
- address-taken/indirect functions remain until target analysis proves them dead.

DCE runs repeatedly because folding, inlining, scalar replacement, compress passes,
and function sharing expose new dead values. After reachability, unread-global
elimination ignores `LoadGlobal` in dead functions, so a scheduler, unique-id
counter, or host alias that is only stored — or only read from an unreachable
helper — can disappear. Dropping the store does not drop an effectful producer;
SSA DCE still keeps `bump()` when `int unused=bump()` has no reader. Mutation-aware
removal first proves that an allocation and all aliases are unobserved; it cannot
delete a store merely because the immediate SSA result is unused. Unused pure calls
may disappear, but arguments and calls with effects remain in source order.

Typed non-mutating `Math`, string, and array intrinsics are pure language operations,
so their unused results may disappear. That fact does not cross into `JsValue`:
coercions that can run user code, proxy-sensitive access/checks, and dynamic
conversions that may throw are observable evaluations. DCE retains every such
evaluation, common-subexpression elimination does not merge repeated occurrences,
and a declared `pure` function containing one fails validation.

`strip_console` is a distinct output policy: it drops `print`/`debugLog` while
retaining argument side effects and does not remove `console.warn`. Do not report it
as ordinary semantic DCE without naming the production policy.

Verification pairs need live/dead exports, side-effect modules, unknown calls,
foreign imports, address-taken functions, mutation aliases, pure/impure initializers,
and reusable-vs-executable builds.
