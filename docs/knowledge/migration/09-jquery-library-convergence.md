# Phase 09 — jQuery public-library convergence

Parent: [migration](README.md). Current evidence:
[jQuery](../evidence/jquery.md). Library boundary:
[library vs app](../delivery/library-vs-app.md).

## Objective

Make jQuery 3.7.1 the large host-heavy proof that the language, compiler, and port can
converge on an honest public-library win—not merely a source-port milestone or a
closed-app mangle result.

The latest checked-in generated row is explicitly **not** there: it is pre-canonical,
ineligible, not an exact surface claim, has no Closure ADVANCED lane or representative
performance/memory gate, and its recorded LilScript public artifacts are larger than
npm in raw, gzip, and Brotli. Refresh it through `lilscript-codec` and keep the red
state visible until the same generated report proves otherwise.

## Workstreams

1. **Freeze the boundary.** Enumerate script-tag globals, documented methods,
   descriptors, arity/constructibility, plugin extension points, enumeration,
   exceptions, DOM/events/ajax/effects, and `noConflict`. Separate public and closed
   app configs permanently.
2. **Build an eligible JS frontier.** Keep npm minified distribution, pinned
   Terser/Oxc/esbuild/Vite transformations that preserve the surface, and Closure
   ADVANCED only with correct extern/export protection. Choose minima per metric.
3. **Reduce port glue.** Replace fixed-shape `JsValue` bags, string `setProp` layers,
   adapter closures, and class+facade twins with typed/positional internals behind the
   thinnest stable facade. Preserve dynamic keys where jQuery truly exposes them.
4. **Grow compiler proofs.** Turn recurring typed host helpers into legal known-host
   operations, improve escape/effect/identity facts, and search sharing/inlining/layout
   alternatives as complete artifacts. Do not hard-code jQuery names into general
   semantics.
5. **Ablate every attribution.** For each port reshape or compiler change, capture the
   exact before/after public artifact, behavior result, raw/gzip/Brotli, candidate
   config, and runtime/memory result. Keep neutral/loss experiments as negative
   evidence.
6. **Scale behavior and runtime.** Run upstream-style selector/traversal/ajax/effects/
   global tests in a browser and add representative parse/startup, hot operations,
   allocation, and retained-memory non-inferiority gates.
7. **Grow bottom-up layers.** Extract the matching jQuery 3.7.1 `src` files for one
   dependency-closed slice, compile the LilScript entry for that slice, and refuse to
   grow until compiler-selected Brotli is `<=` the best minified JS extract. Harness:
   `benchmarks/popular/jquery-layers/`. Ladder starts at `utilities`.

## Exit criteria

- The selected entrypoint and public surface are exact and `eligible=true`.
- npm and every eligible JS toolchain artifact preserve the same boundary; Closure is
  either eligible with pinned externs/options or explicitly absent for a recorded
  reason.
- Independently selected raw-, gzip-, and Brotli-objective LilScript artifacts
  are each `<=` the metric-specific eligible minimum only in their matching
  metric; any designated strict-win claim is actually `<`. Cross-metrics may lose.
- Correctness, browser behavior, representative performance, memory, and
  reproducibility gates pass.
- The generated report owns the winning numbers. Historical app/full-mangle or
  post-minified experiments remain labelled and cannot substitute for the public row.

Until all exit criteria hold, jQuery is a convergence pressure test and source of
small regression cases—not evidence for a universal library-size claim.
