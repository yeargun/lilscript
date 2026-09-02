# 048 — the singleton engine is a positional handle

**Status: CONFIRMED on size, SPLIT on runtime. cnlil's loss on both axes was the port's shape: its
engine (66 fields), tables (24) and argument cache (5) were class singletons the compiler lowers to
positional array handles, so every hot field read was a bounds-checked element load and the init
spelled a 66-slot default literal; its `cn` passed the `arguments` object whole to the miss path.
Rewritten as module state on the same compiler, the port goes from +283 to **−382 Brotli** against
upstream (9401 vs 9783 at level 13, `size-first`, production search; raw −763, gzip −326), all
416358 upstream comparisons green. Runtime moves from 1.03-1.49x to 1.02-1.2x slower on the nine
lanes and stops there: the residual is measured (V8 reads module-scope state through a module
cell where upstream's closure reads a context slot — a function-scoped artifact is 0.94-1.00x on
the cold lanes — plus the argument-cache paths), and both are compiler items. The owner's gate
"faster or equal on every lane" is not met yet. Opened and closed 2026-09-02.**
Lane: port. Objective: brotli, with runtime ≤ upstream on every lane of the upstream harness as the
constraint the owner set ("smaller or same size, and more performant or exactly the same").
Ports: cnlil. Opened: 2026-09-02.

## Prior art

The technique class is *dissolving a global singleton aggregate into scalars* — the shape upstream
`cn` writes by hand (its engine is a closure over `const`/`let` bindings, `engine.ts:88-1166`) —
plus two call-path spellings V8 prices.

- **Closure** `InlineAndCollapseProperties` (javadoc: "inlining of aliases and collapsing of
  qualified names"): a global *qualified name* `a.b.c` set exactly once collapses to `a$b$c`;
  `GlobalNamespace.Name#canCollapseOrInline` (`GlobalNamespace.java:1844-1920`, fetched from
  master 2026-09-02) refuses externs, getters/setters, `@nocollapse`, `delete`, `toString`/`valueOf`
  literals, spread-followed and conditional definitions. Fields of an instance created with `new`
  are not names on the global namespace, so a constructed engine is never flattened.
  `InlineObjectLiterals` (`InlineObjectLiterals.java:28-31`; `isVarInlineForbidden` `:119-131`
  refuses `var.isGlobal()`; `isInlinableObject` `:146-230` wants a literal-only initializer, no
  `x.m()`, no getter/setter/spread/computed key) is local-only. Closure ADVANCED would leave cnlil's
  singleton exactly as our compiler did.
- **Terser** `hoist_props` (`TERSER/compress/index.js:883-948`, `reduce-vars.js:228-270`
  `mark_escaped`): `var o={…}` → `o_a, o_b` iff the literal never escapes, is never reassigned,
  has no `direct_access` and no getter/setter; `new` instances excluded, and our handle escapes into
  the argument-cache handle and the exported closures.
- **Oxc**: ABSENT as a pass (refs §D row "Object literal → scalars"); the peephole only drops
  never-read literal members.
- **LilScript**: `scalar_replace_linear_classes` (`LS/optimizer.rs:7149, 7186-7219`) dissolves a
  class only at `EscapeState::LocalOnly`; a module-global instance stored into another instance's
  field and read by exported closures is not, so the emitter chose the internal positional layout
  (`aggregate_layout`, default `positional`; `LS/codegen_ir_js.rs:30670`). The handle literal
  carries every field's typed default (`[null,0,new Int32Array(0),…]`, 571 minified bytes) and the
  `init` re-stores 66 slots. The `arguments` spelling: `JsCallingConvention::StaticRest` aliases the
  callback's `args` to `arguments` (`LS/codegen_ir_js.rs:7347-7348, 2426`), and a whole-`args` use
  is spelled `f(arguments)`. The record read: `IndexGet` on a record emits `{indexed}??null`
  (`LS/codegen_ir_js.rs:12912-12920`), and the `T?` test that follows re-tests `!= null`.
- **Upstream cn** documents the V8 facts this folder measures: "probe predictions in place over
  `arguments` (indexed reads only, so it never materializes) — a predicted render-loop call allocates
  nothing" (`engine.ts:1405-1413, 1435-1437`); the whole-string hit is "one object-property read
  with a V8-cached hash" (`:1085-1088`).

Refs §D.2 carries the three rows (singleton dissolution, `arguments` escape, null normalization).

## Claim

cnlil's +283 Brotli / +3182 raw and its 1.03-1.49x on the nine harness lanes are three shapes of
the port, not the merge algorithm, which is a faithful typed transliteration of upstream's: (1) the
engine, the tables and the argument cache are class singletons emitted as positional handles, so the
cold merge pays an element load per field read and the artifact pays the handle literal plus
slot-by-slot init; (2) `cn` passes `args` whole to `resolveArguments`, so V8 materializes an
arguments object on every call, predicted or not; (3) the hit path is `(cache[k] ?? null) != null`.
Confirms if the port written as module state — the shape upstream ships — is ≤ 9783 Brotli and
≤ 1.00x on every lane with the same compiler and config. Falsifies if the lanes stay above 1.05x
once the three shapes are gone: the residual would then be the compiler's spelling.

## Read

- `finer/objective.md`, `finer/status.md`
- `~/cnlil/src/engine.lil`, `default.lil`, `tables.lil` (generated by `scripts/generate-tables.mjs`)
- `~/cnlil/vendor/cn/packages/cn/src/engine.ts` and the harness
  `vendor/cn/packages/conformance/bench/{worker-ab,component-worker}.mjs`
- `docs/configuration.md:288-330`, `docs/knowledge/config/javascript-shape-abi.md`

## May touch

- `~/cnlil/src/*.lil`, `~/cnlil/scripts/generate-tables.mjs`, `~/cnlil/lilscript.toml`,
  `~/cnlil/reports/*.json`, `~/cnlil/README.md`; this folder; `finer/out/048/`; refs rows;
  status/log; the fold on branch `finer/048-nullish-fold`

## Method

One binary per comparison — main 4c25f05 for the shape attribution, main f9829a1 for the shipped
build; config frozen for every port-side comparison (level 8, search off, `performance-first`),
the level and priority chosen last by their own curve. Sizes: `lilscript-codec` on the port's boundary
(esbuild 0.25.12 minified browser ESM bundle of the entry, as `scripts/measure.mjs` does; Node's
zlib/brotli numbers in `reports/` differ). Perf: the upstream isolated-process harness, lil and
official interleaved per lane, five rounds, median of the per-round ratios
(`bench-ab.mjs`), on this host (Node 20, the port's declared runtime; 160+ CPU credits, nothing
else running) and on a Turin pool worker (Node 22) as a second opinion.
Attribution before the rewrite: hand rewrites of the shipped artifact, one shape each, upstream as
the oracle (`diff-check.mjs`, 20000 seeded strings plus the argument-cache sequences).
Microbenchmarks one process per variant (`micro-hit3.mjs`): several variants in one process make
the first one fastest, which cost one wrong conclusion before it was caught.

```sh
node finer/out/048/bench-ab.mjs LABEL DIR_WITH_INDEX_JS 5 all --json out.json   # CNLIL_ROOT selects the checkout
node finer/out/048/measure.mjs official=node_modules/cn/dist/index.js x=ARTIFACT
node finer/out/048/dissolve.mjs lil.pretty.js out/index.js --engine --tables --cache
node finer/out/048/iife.mjs dist/cn.raw.js out/index.js      # module-scope state → function scope
```

## Result

Sizes, codec, port boundary (upstream 27459 / 10835 / 9783):

| variant | raw | gzip9 | brotli11 | note |
|---|---:|---:|---:|---|
| committed dist (class singletons, level 8, search off) | 30641 | 11327 | 10066 | +3182 / +492 / +283 |
| config sweep on that source (8 variants) | 30119…35392 | | 10034…10866 | best `balanced` −32; `aggregate_layout = named` +1083; not the lever |
| module-state port, same binary and config | 27507 | 10760 | 9657 | +48 / −75 / −126 |
| + `Math.imul` at the ten hash sites (upstream's spelling) | 27597 | 10778 | 9676 | −107 |
| same source, level 13, production, `performance-first` | 26874 | 10540 | 9439 | −344; compiles in 73 s here, 25 s on a Turin worker |
| **same, `size-first` — shipped** | **26696** | **10509** | **9401** | **−763 / −326 / −382** |

Attribution by minified declaration (acorn ranges, `finer/out/048/sizes-by-decl.mjs`): the old
engine was 13.0 KB against upstream's 10.7 KB (+2250: handle literal 571, init 3022 against
~1600, three inlined copies of `rotateDoor`), tables +389, `cn` layer +620.

Perf, ratio lil/official, median of interleaved rounds — the full tables (Node 20 here and Node 22
on the Turin worker, hand rewrites E1-E5, the shipped build) and the residual's attribution are in
[`finer/out/048/results.md`](../../out/048/results.md). Headline, Node 20: committed dist 1.03-1.49x
on the nine lanes; module-state port 1.08-1.17x; shipped build (level 13, size-first) 1.05-1.30x
(dup-loop the worst, arb and ssr 1.18). Node 22, level 13: 1.02-1.23x, and 0.95-1.02x on six lanes once the artifact's
internals are wrapped in a function scope by hand.

**What the residual is** — measured, in [`finer/out/048/results.md`](../../out/048/results.md):
module-scope state read through a V8 module cell (the wrapper fixes it), a whole `arguments` pass
(the port reads by index and copies only on a miss), `int*int` spelled `a*b|0` (`Math.imul` now),
and the argument-cache walk, which is diffuse and continues as [049](../049-the-arg-cache-walk-is-diffusely-slower/README.md).

## Verdict

Confirmed for size and for the shape diagnosis: the class singletons were the whole size loss and
half of the runtime loss, and the port now ships smaller than upstream on all three codecs. Split
on the gate: every lane improved, none reached 1.00 on this host, and the causes left are the
compiler's (module-scope emission of bundle internals; the coalescing miscompile that blocks the
closure-shaped port; the `arguments` alias spelled whole). What status.md carries: a class
singleton in a port is a positional handle; module-scope state costs a module-cell read per
access; a whole `arguments` pass allocates on every call; `int*int` is not i32 multiply; the
peephole is off at level 8 / search off; single-process microbenchmarks order their variants.
Shipped: `~/cnlil` rewritten (module state, imul, index-only `arguments`, level 13 / production /
size-first), reports and site regenerated.

## Compiler follow-up (branch `finer/048-nullish-fold`, in verification)

After the owner restated the gate and required every compiler change to be generic and
fleet-verified (objective §8), three generic emission changes went on the branch:
`javascript.function_scope` (internals in one function scope, opt-in, +27 Brotli on cnlil, cold
lanes 1.15 → 1.00 by hand), `javascript.truthy_nullable_checks` (a nullable object test as
truthiness is 3.8 ns against 3.2 for the comparison; default follows the priority), and the
undefined-absent record read (bare read + `===void 0` when every use is a null test or the
narrowing after one). Claims, numbers and the verification log:
[`finer/out/048/results.md`](../../out/048/results.md) §"Compiler follow-up".

## Next

Compiler, in this order: (1) emit a single bundle's internal bindings inside a function scope
with the exports assigned outside, as a family the codec scores and the performance priorities
require — measured by `iife.mjs` at 0.94-1.00 on cnlil's cold lanes; (2) the name-coalescing
liveness across nested closures (`finer/out/048/factory-src/`, `na = function…`); (3) dissolve a
module-init `new` whose identity never reaches export/host/`JsValue` into module bindings, with
the class-shaped cnlil source as the oracle (must compile to the module-state artifact).
