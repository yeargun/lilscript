# Vite 8 and Closure ADVANCED minification audit

Parent: [Evidence](README.md). Mapping: [`docs/optimization-coverage.md`](../../optimization-coverage.md).

Audited 2026-08-09 against the current npm releases:

- Vite `8.2.1` (Rolldown `1.2.3`, Oxc `0.143.0`)
- Google Closure Compiler `20260804.0.0`

The release check is reproducible with:

```sh
npm view vite version --json
npm view rolldown version --json
npm view oxc-minify version --json
npm view google-closure-compiler version --json
```

Primary references are Vite's [v8 migration guide](https://vite.dev/guide/migration.html)
and [build options](https://vite.dev/config/build-options.html), Rolldown's
[dead-code elimination](https://rolldown.rs/in-depth/dead-code-elimination) and
[code-splitting](https://rolldown.rs/in-depth/automatic-code-splitting) notes,
Oxc's [minifier](https://oxc.rs/docs/guide/usage/minifier.html) and
[mangling](https://oxc.rs/docs/guide/usage/minifier/mangling) documentation,
and Closure's `v20260804`
[`DefaultPassConfig.java`](https://github.com/google/closure-compiler/blob/v20260804/src/com/google/javascript/jscomp/DefaultPassConfig.java)
and
[`RenameVars.java`](https://github.com/google/closure-compiler/blob/v20260804/src/com/google/javascript/jscomp/RenameVars.java).

## The systems optimize at different levels

Vite is the build coordinator, not one monolithic minifier. Vite 8 delegates
the JavaScript graph to Rolldown and JavaScript transforms/minification to Oxc.
Closure `ADVANCED`, by contrast, treats its JavaScript inputs and externs as a
closed program and schedules whole-program analysis and rewriting itself.
LilScript starts one level earlier than both: it owns typed source semantics,
links its static module graph, and optimizes typed CFG/SSA before spelling
JavaScript. This is why it can remove representations and coercions that a
general JavaScript tool must preserve.

### Vite 8 production pipeline

1. Vite resolves configuration, plugins, targets, `define` replacements,
   assets, CSS, workers, and HTML entrypoints.
2. Oxc transforms syntax to the configured browser target. Compile-time
   replacement can expose constants and unreachable branches.
3. Rolldown constructs the module graph. It includes an item only when it is
   used or has observable effects, using package/module `sideEffects`, pure
   annotations, and configurable property/global side-effect assumptions.
4. Rolldown scope-hoists modules and assigns static, dynamic-entry, and shared
   chunks. Shared chunks are primarily determined by entry reachability;
   singleton and execution-order constraints can outweigh fewer chunks.
5. Oxc normalizes JavaScript once, then runs its peephole compressor to a fixed
   point (bounded to 10 iterations by default in `oxc_minifier`). Its families
   cover constant folding, DCE, unused declarations, conditions and boolean
   contexts, logical/conditional expressions, statement fusion, loop forms,
   known methods, syntax substitution, and bounded inlining.
6. Oxc mangles variables and private fields. Its mangler builds scope-aware
   liveness slots, reuses a slot for noninterfering symbols, ranks slots by
   reference frequency, assigns shortest non-reserved base-54/base-64 names,
   and reorders equal-length name buckets by declaration order to improve gzip
   similarity. The last technique is explicitly derived from Closure.
7. Code generation removes whitespace/comments and chooses compact syntax.
   Vite intentionally preserves whitespace around ES library output where
   stripping it would destroy pure annotations needed by downstream tree
   shaking. Client builds default to Oxc minification; SSR defaults to no
   minification.

This pipeline is deliberately conservative about arbitrary JavaScript. It
cannot assume that getters, globals, prototypes, coercions, reflection, or
function/class names are unobservable unless configuration or analysis proves
otherwise.

### Closure `ADVANCED` pipeline

Closure gains power from a stronger contract: all relevant source is supplied,
external names are described by externs, and type/JSDoc information may inform
optimization. Its exact schedule changes over time, but `v20260804` retains
these important phases:

1. Normalize and rewrite modules, transpile, apply defines, collect extern and
   type information, and establish the property/call graph.
2. Inline and collapse properties, run early inlining and peepholes, and remove
   unused code to expose the main optimization loop.
3. Repeatedly optimize getters/properties and calls (unused returns,
   parameters, and constant arguments), inline functions, variables, and
   constants, remove dead assignments/code, collapse object literals, and run
   peepholes. Ordering is intentional: inlining creates cleanup opportunities,
   and cleanup creates new inlining opportunities.
4. Apply flow-sensitive inlining, constructor and method rewrites,
   cross-chunk motion, property disambiguation/ambiguation/renaming, string
   aliasing, and anonymous-function/declaration rewrites when enabled.
5. Coalesce noninterfering variables and rerun late peepholes because
   coalescing itself creates identity assignments and redundant syntax.
6. Rename variables by descending reference frequency. For generated names of
   equal length, assign them in source occurrence order so nearby declarations
   receive similar names and compress better. Rename properties and labels,
   denormalize, and print compact JavaScript.

`ADVANCED` therefore does much more interprocedural JavaScript rewriting than
Vite's default production path, but its public boundary must be described
correctly. Comparing a Closure artifact with renamed/deleted public API to a
Vite library artifact that preserves that API is not a valid size comparison.

## What LilScript already takes further

The detailed responsibility map lives in
[`docs/optimization-coverage.md`](../../optimization-coverage.md). The important
advantages over general JavaScript input are:

- typed closed-world CFG/SSA, finite/range propagation, and proven integer
  coercion removal;
- direct-call and nominal-field knowledge, devirtualization, specialization,
  scalar replacement, allocation/mutation graph removal, and effect summaries;
- liveness coloring, cross-scope name reuse, frequency/entropy-aware alphabets,
  property representation removal, and codec-window-aware declaration layout;
- exact raw/gzip/Brotli evaluation of syntax, naming, inlining, pooling,
  packing, layout, and chunk-plan candidates instead of treating the fewest
  source bytes as a proxy for transfer size;
- explicit public/extern ABI preservation and separate reusable-surface versus
  closed-app benchmark lanes.

## Residual minifier probe

Re-minifying LilScript's application artifacts with the Vite-pinned Oxc shows
why both structural compression and exact codec selection matter. Values are
raw/gzip-9/Brotli-11 bytes; the right side is Oxc applied to LilScript output.

| Workload | LilScript | LilScript then Oxc |
| --- | ---: | ---: |
| binary telemetry | 427/304/266 | 424/310/281 |
| event pipeline | 292/210/169 | 290/213/171 |
| module pricing | 227/188/157 | 226/187/157 |
| motion values | 462/323/280 | 454/323/285 |
| reactive store | 386/258/216 | 380/262/222 |

Oxc shortens raw output in every row, yet worsens Brotli in four of five. A
generic final minifier is therefore not an unconditional improvement. It is a
useful proposal generator, while the selected transfer codec must remain the
judge.

## Changes taken from the audit

Three bounded improvements were implemented:

1. Return-only typed CFG regions now recurse through nested structured
   branches. Guard-return ladders can become right-associated conditional
   expressions, while straight-line SSA remains on the coalescing-aware
   statement emitter.
2. Braced `if`/`else` arms containing only parsed expression statements can
   become a conditional expression with correctly grouped comma sequences.
   This local pass is capped at 16 KiB: applying it across the 23 KiB
   robust-predicates surface perturbed the bounded pre-final candidate beam and
   displaced a stronger existing artifact, even though the rewrite itself was
   legal. Large artifacts keep the proven path until beam retention can score
   post-rewrite potential at every pruning stage.
3. The final parsed peephole output competes with its untouched input under the
   exact raw/gzip/Brotli, startup, and performance policy. A shorter raw rewrite
   can no longer silently replace an artifact that compresses better.

The reusable benchmark impact was measurable: the micro-math app changed from
`396/247/210` to `319/224/201`, and the motion-easing public surface changed
from `447/315/281` to `438/310/280`. The latter now beats the corresponding
Closure `ADVANCED` public surface (`445/326/286`) in each reported size column.
That historical same-artifact observation is not the objective contract used by
the raw/gzip/Brotli comparison lanes, which compile and gate separate artifacts.

## Highest-value next work

1. Treat more statement/declaration/loop fusions as alternative codegen forms
   and exact-score them, especially `for` initializer/update fusion and return
   sequences. Make intermediate beam pruning aware of reachable post-rewrite
   codec cost; do not make raw-shorter forms mandatory.
2. Run selected typed simplification and DCE families to a measured fixed point
   after specialization/inlining, with convergence counters and compile-budget
   caps. Closure and Oxc both benefit from phase feedback.
3. Expand effect and escape summaries for higher-order calls and host contracts;
   most remaining large gaps are semantic proof gaps, not identifier spelling.
4. Add a benchmark audit that optionally feeds final LilScript output through
   Oxc and records raw/gzip/Brotli deltas. This is a cheap way to discover new
   syntax opportunities without adopting Oxc as an unconditional last pass.
5. Keep app, reusable public surface, chunk/deployment, runtime, and memory
   gates separate. Optimizing one can regress another, and Closure's closed
   contract is only comparable when the same observable boundary is retained.
