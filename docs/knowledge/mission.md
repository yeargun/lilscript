# Mission

LilScript exists to be the most **compression-friendly** language that compiles to the web.

The primary artifact is JavaScript. Depending on config, it can be whole-program
optimized, dead-code eliminated, tree-shaken, mangled, and searched across several
equivalent representations. Every retained complete-artifact finalist is measured
under the selected `raw`, `gzip`, or `brotli` cost model;
`javascript.priority` decides how that measured size trades against the static
performance model. Native/`exec` output is a real second backend of the same typed IR.
It is not the current size race.

Return to the [tree](README.md). Children: [language](language/README.md),
[compilation](compilation/README.md), [config](config/README.md).
Implemented vs intended decision system:
[current architecture](compilation/current-architecture.md),
[goal architecture](compilation/goal-architecture.md),
[objectives](compilation/objectives.md) (size/performance × raw/gzip/Brotli),
[decision registry](compilation/decision-registry.md).
How to write so those objectives can win:
[compressor surface](language/compressor-surface.md).

## What “better than a JS bundler” means

A JS bundler (Vite, esbuild, Terser, Closure `ADVANCED`) starts from JavaScript or from TypeScript after type erasure. It can rename locals, drop unused exports, and apply local peepholes. It cannot, in general:

- dissolve a class into SSA scalars because the type system already proved the object never escapes;
- mangle a field name because the field is a positional index, not a string key;
- treat a host call as a typed ABI rather than an untyped property access;
- score two **semantically equal** whole programs under Brotli-11 and keep the one that transfers fewer bytes.

LilScript is designed so those proofs exist **before** JavaScript is spelled.
The current compiler searches a bounded subset of spelling, layout, inlining,
and chunking choices; the goal architecture makes that domain explicit and
auditable. See [global optima](compilation/global-optima.md).

The checked-in evidence does not establish that every program beats hand-specialized
JS or Closure. That is not permission to accept a maintained paired loss: for every
**supported, semantically equivalent paired case**, LilScript must be no larger than
the metric-specific minimum eligible JavaScript baseline in every required metric.
Any loss by an objective-specific build in its selected raw/gzip/Brotli metric is red
and enters triage; the same artifact's other metric sizes are diagnostic and may
trade off. A semantic mismatch is red before size is considered. This is an enforced engineering invariant over the declared
corpus and boundaries, not a theorem for arbitrary JavaScript. Host boundaries,
public identity, and missing proofs explain where compiler/language/port work remains;
they do not turn a loss into a win.

The durable route is to expand supported semantics and whole-program opportunities,
measure contested representations with the selected codec, and make every size claim
name its corpus, boundary, toolchain, and config. Under `size-first`, transfer bytes
are the primary rank key; the other priorities intentionally permit a size/runtime
trade. That completion rule is also in [`docs/roadmap.md`](../roadmap.md).

## Types instead of glue

TypeScript’s types are a glue layer: they check, then erase. The emitted JavaScript still has the shapes, wrappers, and `any` holes that minifiers must treat conservatively.

LilScript types **are** the compilation model:

- each type has a defined JavaScript and native representation;
- every exit from the closed world is explicit: typed `extern` / `import extern`, a
  reusable root export, dynamic-module/task delivery, portable `print`, or the narrow
  `JsValue` boundary;
- escape, effects, and purity are first-class analyses;
- internal aggregates can disappear; boundary aggregates keep a named or positional ABI by **config**, not by accident.

The target optimization architecture has a semantic firewall. The compiler
first freezes language semantics, application/library ABI, explicit source
lowering obligations, and unsafe host assumptions. Only then may
raw/gzip/Brotli and the selected priority choose among legal internal
representations. Under the planned v0.2 contract, a live source-written
`x | 0` remains a JavaScript `|0`; a compiler-generated redundant i32
normalization may disappear. Reusable-library output preserves a public ABI
manifest while private code remains fully closed-world and mangleable. This
firewall is migration work, not a claim about the current v0.1 pipeline.

The belief is that this can yield better compile-time safety, sometimes better runtime shape, and in some cases smaller bundles — because the optimizer is not reverse-engineering glue. Details: [types are not glue](language/types-not-glue.md).

## What the compiler optimizes

**Served bytes**, not editor bytes. Most deployments ship gzip or Brotli. `javascript.cost_model` selects the exact objective:

| Value | Measurement |
|---|---|
| `raw` | emitted UTF-8 length |
| `gzip` | statically bundled upstream stock zlib C 1.3.1, level 9, deterministic `mtime = 0` |
| `brotli` | statically bundled official Google Brotli C 1.1.0, generic, quality 11, `lgwin` 22 (default compiler scorer) |

Raw, gzip, and Brotli can disagree. A shorter raw spelling can lose under Brotli
context modeling. A helper that wins raw can lose gzip because it breaks repetition.
For gzip/Brotli, `max_candidate_raw_growth_percent = 0` still admits a candidate that
does not regress the configured transfer score even if its raw output is larger. The
percentage controls how much raw growth is admitted when that transfer condition is
not met; it is not a universal raw ceiling. See [cost model](config/cost-model.md).

## Tradeoff triangle

Every contested knob sits on three axes:

1. **Transfer size** (configured codec of the complete JS artifact, plus deploy cost for chunks)
2. **Compile time** (candidate count × Brotli-11 probes × IR variants)
3. **Runtime** (parse/compile/memory startup proxy; typed-IR deopt/allocation/indirect-call shape)

`javascript.priority` picks the ranking policy. `[optimization]` gates semantic IR
passes. `javascript.compression` / `javascript.optimizations` /
`optimization_level` / `candidate_*` control **which alternatives are even
considered**. CLI `--mode development` turns the multi-IR/emission candidate expansion
off; explicitly enabled finalization features may still run. The triangle is
tabulated in [tradeoffs](config/tradeoffs.md).

Changing `javascript.priority` never weakens type checking, mandatory IR normalization, DCE correctness, or host-boundary rules. It also does not change the native optimizer: `--target all` shares parse/semantics, then optimizes JS and C copies separately.

## Language must be built for delivery

Bundling is not a post-hoc tool concern. The language and the compiler share these invariants:

- static imports are compiler inputs, erased before SSA, so cross-file inlining and DCE are the default;
- `import("./feature")` is a typed lazy boundary, not an untyped `Promise`;
- lazy-only modules cannot run top-level statements (no silent eager init);
- whole-program optimize **then** partition (`single` / `split` / `preserve-modules`);
- progressive enhancement is a lint + host-boundary problem (`web/eager-host-access`), not a runtime wrapper;
- public names vs internal layout are **config** (`[mangle]`, `public_aggregate_abi`), so one codebase can ship a script-tag facade and a fully mangled LilScript app.

See [modules and lazy loading](language/modules-lazy.md) and [delivery](delivery/README.md).

## Native is secondary, not fake

The same typed IR lowers to C and a native executable. Features without a portable C ABI (`JsValue`, `Regex`, `Task`, `extern class`, dynamic import) are **rejected** on that target rather than approximated. That keeps the closed-world model honest: JS size wins must not rely on a second, incompatible semantics. See [JS vs native](language/js-vs-native.md).

## How to judge a change

1. Does it preserve explicit language/boundary semantics?
2. Do optimized and optimizer-disabled executions agree?
3. Where the feature is portable, do JS, C, and native agree?
4. Is the representation search scoring the **complete** artifact under the configured codec?
5. If it looks locally smaller, did the configured objective actually improve? Under
   `size-first`, never select more transfer bytes; an exact transfer tie may be broken
   by the performance model. Under another priority, record the deliberate
   runtime/size trade.

jQuery is the current large-library pressure test. Its checked-in public-library row
is pre-canonical, ineligible, and not a size win. That is evidence about the measured
port and boundary, not a current byte claim or proof that any single cause explains
the gap. See
[jQuery](evidence/jquery.md).
