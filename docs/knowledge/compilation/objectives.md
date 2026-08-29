# Compilation objectives

Parent: [Compilation](README.md). Ranking implementation:
`javascript_candidate_rank` in `src/compiler.rs`. Knobs:
[priority](../config/javascript-priority.md),
[cost model](../config/cost-model.md). Philosophy:
[global optima](global-optima.md). Language leverage:
[compressor surface](../language/compressor-surface.md).

LilScript is not one compiler. It is one typed IR plus a **configured
objective**. “Compile this for Brotli” and “compile this for V8 shape” are
different programs. They must stay different. Mixing them into one locally
clever fold is glue.

## Two independent axes

| Axis | Values | Question |
|---|---|---|
| `javascript.cost_model` | `raw` · `gzip` · `brotli` | What is transfer size *T*? |
| `javascript.priority` | `size-first` · `balanced` · `realistic-performance-first` · `performance-first` | How do *T* and the static performance proxy *P* rank? |

Spoken names map onto **pairs**, not extra enums:

| Spoken | Config |
|---|---|
| raw-first | `priority = size-first`, `cost_model = raw` |
| gzip-first | `priority = size-first`, `cost_model = gzip` |
| brotli-first (repo default) | `priority = size-first`, `cost_model = brotli` |
| performance-first | `priority = performance-first`, `cost_model` still chooses which *T* is second |
| balanced | mixed *T*/*P*; still needs an exact `cost_model` for the *T* that enters the mix |

One invocation publishes **one** winner. Intermediate beams may keep
raw/gzip/Brotli-diverse shapes so a later interaction can recover, but the
terminal rank uses only the configured *T*. Serving all three codecs at their
individual minima requires three compiles. The paired-case gate already does
that. Mixing codecs into one locally clever fold (keep packing because gzip
liked it while this compile is Brotli) is glue.

Native/`exec` does not use this pair. It clones lowered IR and runs
`[optimization]` / `[native]`. JS search winners are not the C program.

## Target firewall: objective is not semantics

The planned architecture makes the two axes rank only programs already proved legal under one normalized
`CompilationContract`. That contract contains language semantics, the
application/library boundary and public ABI, explicit source lowering
obligations, target floor, and unsafe host assumptions. Changing `cost_model`,
`priority`, or search budget may alter internal layout, inlining, mangling,
pooling, and order. It may not alter exports, host names, public descriptors,
constructibility, evaluation behavior, or explicit source intent.

The first required distinction is source-written versus generated i32
normalization. A live source `x | 0` is a lowering obligation and remains JS
`|0` for raw, gzip, Brotli, and every priority. A `|0` inserted to implement
ordinary `int` semantics is generated compiler structure and may be elided when
proof permits. Dead enclosing code may still disappear. The first end-to-end
obligation is implemented conservatively: affected candidates preserve source
`|0` and skip target-text rewrites that cannot carry it. Globally unambiguous
operation identity and final-byte witnesses remain
[planned target work](planned-architecture.md#5-target-js-boundary).

Likewise, the target reusable-library mode does not disable optimization. It freezes an
`AbiManifest` for unknown JavaScript consumers, while the linked internal graph
remains closed and eligible for mangling, dissolution, specialization, and DCE.
A closed application simply has a smaller observable boundary. Every objective
artifact for a given world must pass the same API manifest.

## What is exact

These are not vibes. Changing them without a proof is a bug.

| Object | Definition |
|---|---|
| Legality | Conservative analyses: types, escape, effects, identity, SSA interference. A missing fact forbids a rewrite; it does not authorize speculation. |
| *T*<sub>raw</sub> | UTF-8 length of the complete artifact |
| *T*<sub>gzip</sub> | bundled zlib C 1.3.1, level 9, `mtime = 0` |
| *T*<sub>brotli</sub> | bundled Brotli C 1.1.0, generic, quality 11, `lgwin = 22` |
| size-first rank | exact *T* first (`javascript_candidate_rank` uses the byte count, not a basis-point ratio — ratios tied nearby sizes and let *P* steal the win) |
| balanced rank | `3 * T_ratio + 2 * P_ratio` (ratios are value/baseline in 10 000ths) |
| performance-first rank | *P* first, *T* second |
| realistic-performance-first | over-limit *P* bucket (`1_000_000 + T_ratio`) then *P* |
| Ties after priority | raw length, top-level declaration preference, startup score, lexical JS, then stable plan identity; terminal topology-preserving search may first prefer more resolved one-byte bindings |
| Function layout n ≤ 13 | Held-Karp exact on the similarity graph; source order remains a candidate |
| Coalescing | reuse a name only when live ranges do not interfere |

An entropy proxy, a local raw delta, or “this looks like Terser” never replaces
*T*. Encoder identity is part of *T*: another zlib/Brotli build can rank the
same strings differently.

## What is heuristic

The legal set is too large to enumerate. Search is **bounded combinatorial
optimization**: return the best **found** program under a work ledger, not the
mathematical argmin over every equivalent JS spelling.

| Heuristic | Why it exists | Failure mode |
|---|---|---|
| Sequential `IrJsOptions` flips + beam width 12 | compile time | late families starve; order is not monotone (zod −58 from early-only `stable_local_names`) |
| Inline instruction/CFG/growth caps | compile time + duplication | jQuery: more inlining lost *T* |
| Codec-conditioned incumbents | empirical priors for Brotli vs raw | a prior that search cannot reverse (ABI, unsafe, omitted compression name) |
| Broad-module phase-order collapse | >24 functions or >2048 ops | missed IR interaction |
| Function layout insertion for n > 13 | Held-Karp is exponential | not exact |
| Greedy chunk add-if-cheaper | partition space | two chunks that win only together are missed |
| Static *P* (deopt/alloc/indirect-call weights) | no browser in the compiler | not a runtime benchmark |
| `assume_pure_property_reads` | Terser-shaped port contract | not a type |

A prior is allowed. An irreversible prior is glue. Under `size-first`, a
heuristic that loses the configured *T* and still ships is a bug. Other
priorities must improve their declared composite rank and satisfy their guards.

Sequential beam search is **coordinate descent** on a discrete, non-monotone
*T*. A flip that is 0-delta alone can win only with a second flip. Measured:
on `callbacks.lil`, `function_spelling = arrow` and `stable_local_names =
false` are each tied with the incumbent; together they are −6 Brotli. Across
six jQuery modules the pair is −106; on the full artifact the same pair is
only −11 because a richer context already recovered most of it
([jquery-01](../migration/board/notes/jquery-01.md)). The elegant response is
a **declared joint family** for a measured non-monotone pair — the same
pattern as `pure_helper_inlining` × dense tables — not a 2⁷⁴ Cartesian and
not a port-local TOML that hard-wires the pair.

Compile time is a **budget** (`candidate_*`, `optimization_level`,
`candidate_search`), not a fourth rank key. Exhaustion is a failed search for
the starved families, not proof the incumbent is globally best.

Bundle output needs one explicit composition rule. The target design derives a
`DeliveryObjective`: apply `javascript.cost_model` to each emitted chunk,
weight by reachability/cache policy, add request/depth costs, then apply
`javascript.priority` to transfer and aggregate performance. The current
`[bundle.cost]` weighted raw+gzip+Brotli terms are retained during migration as
an explicitly named legacy mixed-codec objective; they must not silently replace
the selected JavaScript codec.

## Language is the other optimizer

Terser, Oxc, and Closure ADVANCED start from JavaScript (or from TypeScript
after erasure). They infer what LilScript is supposed to **state**:

- this value never escapes → dissolve the object;
- this name is a field index, not a string key;
- this call is `pure`;
- this constructor identity is unobserved.

If the port throws that away (`JsValue` bags, `JS.method*` tables, ordinary `{}`
when `Record` is null-proto, Proxy traps), the compiler is correctly forbidden
to invent the proof. Then you are competing with Terser **on Terser's terms**,
plus adapter tax. The result is still a LilScript product gap: classify whether
the durable fix belongs in the language surface, proof analysis, port, or
decision system. See [compressor surface](../language/compressor-surface.md).

The elegant loop:

1. **Contract** (semantics + world/ABI + explicit intent) — immutable.
2. **Proof** (language + analyses) — exact.
3. **Legal representations** (IR + emitter families) — exact eligibility.
4. **Bounded search** scored by exact *T* (and *P* if priority asks) — heuristic
   completeness, exact comparison among survivors.
5. **Configured baseline retained** — never uncompilable.

Glue is anything that skips (1) or (2) and patches JS after the fact.

## Remaining objective gaps

Several formerly missing competitors now exist: `keep-object`, reversible
packing/identifier pooling, expression `if` and scalar `match`, proof-marked
named classes, owner-scoped properties, and immutable capture snapshots. Their
existence does not mean every config admits them or every interaction is reached.

Current gaps are narrower:

- phase-order/compress probes, target contractions, entropy/naming, and chunk
  planning do not yet share one recipe/acceptance model;
- selected recipes are not fully serializable/replayable;
- global booleans cannot express every measured per-entity mixed winner;
- family/beam scheduling is not budget-prefix monotone;
- final artifacts lack a complete expected-versus-observed ABI and obligation
  witness;
- closure/call-graph choices remain split across optimizer, emitter, and target
  contraction.

Add an alternative or joint/entity-scoped family only after a minimized case or
fingerprinted corpus run proves the current model cannot express or reach it.

## Beating Terser / Oxc / Closure ADVANCED

The engineering invariant, not a theorem about arbitrary npm:

For every **supported, semantically equivalent** paired case and metric *m* in
`{raw, gzip-9, Brotli-11}`:

```text
size[m](LilScript compiled with cost_model = m)
  <= min(size[m] of eligible JS minifiers, including Closure ADVANCED where that lane exists)
```

Semantic mismatch is red before size. A loss is a compiler bug, a `.lil` that
was written as glue-TS, or a missing language proof — not a reason to weaken
the gate or post-minify.

Closure’s advantage on **JavaScript input** is closed-world ownership and
externs. LilScript’s advantage is that the closed world is the language. The
seven `comparison/apps/` programs show that stack working. jQuery / MobX /
markdown transliterations show what happens when the port refuses the language.

You do not beat ADVANCED by cloning its pass list. You beat it by giving the
search **equivalents Closure cannot legally emit** (dissolved structs, positional
fields, inferred purity) and then scoring the complete artifact with the codec
you actually serve.

A forked library that still loses is classified before any compiler change:

1. **Compiler bug** — identity/search ranked invalid JS (ident-05), unsound
   coalescing, missing legal representation the IR already could emit.
2. **Missing language proof** — the port cannot state a fact Terser guesses
   (`assume_pure_property_reads`, constructor-value export, ordinary `{}`).
3. **Port still written as JavaScript** — `JsValue` bags, `JS.method*` tables,
   vendored unminified host files. Rewrite representation, do not add a fold.
4. **Legitimate dynamic hatch** — clsx. Measure and keep `JsValue`.

The [planned migration](../migration/planned-migration.md) assigns those classes
to evidence, legality, incumbent recovery, reusable proof, port, or search work.
None belongs in a library-specific matcher in `js_peephole`.
