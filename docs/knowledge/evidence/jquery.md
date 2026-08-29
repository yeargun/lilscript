# jQueryLil evidence

Parent: [evidence](README.md). Required row:
[library proof matrix](library-proof-matrix.md). Live snapshot:
[`docs/current-status.md`](../../current-status.md). Active investigation:
[jquery-01](../migration/board/notes/jquery-01.md).

## Boundary

jQueryLil is a reusable jQuery 3.7.1-compatible library surface, not a closed
LilScript application. Its public ESM/CJS/UMD names and jQuery object behavior
must remain stable while private typed internals may optimize.

The maintained semantic gate covers the exported surface, core utilities,
Deferred resolve/reject, direct compiler artifact packaging, CommonJS loading,
and representative DOM selection/class/event behavior. It does not prove every
upstream jQuery test or plugin.

## Evidence Status

The current compiler migration produced no direct-output size change against the
frozen pre-change compiler, and the public artifact still does not beat official
`jquery.min.js` on Brotli. Exact current values belong in tracked generated
evidence, not this page.

Older numbers in board/research notes refer to different revisions, boundaries,
configs, or post-minified diagnostics. They are useful attribution history but
are not interchangeable release claims.

## Why It Matters

jQuery stresses dynamic public facades, ordinary-object semantics, host calls,
function adapters, effect ordering, and control-flow/value placement. It is a
test of whether LilScript can keep a JavaScript-compatible public boundary while
making internals genuinely typed and closed.

Current durable directions:

- use landed `object{...}`, expression-if/scalar match, and constructor export
  where they preserve the API;
- widen array-ness and owned-object proofs only when sound;
- improve effect-safe producer sinking and control-flow representation;
- distinguish genuinely dynamic public `JsValue` from avoidable internal bags;
- retain public names/descriptors and observable ordinary-object behavior.

Measured rejected directions remain rejected unless new evidence changes their
preconditions: post-minifying LilScript, forced ternary contraction, broad
declaration hoisting, forced function spelling, and simply widening the beam.

## Claim Requirements

A publishable jQuery claim requires a fresh compiler-output row, official
production baseline, public API/DOM gate, source/compiler/config/scorer/harness
fingerprints, and selected-metric result. A downstream Terser/Oxc result is a
deployment diagnostic only.
