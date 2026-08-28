# jQuery port

Parent: [Evidence](README.md). Sources: `benchmarks/popular/ports/jquery/`, especially `STATUS.md`, `js-host.lil`, `entry.lil`, `lilscript*.toml`. Build: `benchmarks/popular/build-jquery.mjs`. Audit: `audit-jquery-configs.mjs`.

## Why it exists

A compression-first language that only wins on 200-line kernels has not met the mission. jQuery 3.7.1 is a large, host-heavy, string-key, plugin-facade library. It stresses:

- `extern` / `JsValue` density;
- public-name vs internal-layout split;
- inlining vs duplication;
- whether global codec search beats “inline more”.

Working note for the current Brotli chase (methods, rejected hypotheses,
remaining compressibility gap): [emit-07](../migration/board/notes/emit-07.md),
[jquery-01](../migration/board/notes/jquery-01.md).
The remaining gap is IR control-flow shape and missing language proofs
(ordinary `{}`, expression-if, constructor value), not a missing peephole:
[compressor surface](../language/compressor-surface.md).
Do not grow jQuery-specific compiler folds:
[current architecture](../compilation/current-architecture.md).

It is **not** currently an eligibility win. The latest checked-in, pre-canonical
generated
`benchmarks/popular/build/jquery-results.json` marks the row
`candidate-full-library`, `eligible: false`, `sizeGate: false`, and
`exactSurface: false`; Closure `ADVANCED` and representative performance/memory lanes
are open. That report records these historical public-library artifacts:

| Artifact | Raw | Gzip-9 | Brotli-11 |
|---|---:|---:|---:|
| npm `jquery.min.js` | 87,533 | 30,342 | 27,445 |
| LilScript compiler output (single selected artifact) | 162,971 | 44,309 | 38,560 |
| Downstream esbuild diagnostic (same LilScript output) | 99,709 | 36,653 | 32,786 |
| Downstream Terser diagnostic (same LilScript output, `passes=3`) | 98,176 | 35,487 | 31,645 |

This is pre-`lilscript-codec` public-port pressure evidence, not proof that
LilScript’s own emitted artifact beats a JavaScript baseline. Both downstream
post-minifiers consumed the same LilScript compiler output; neither result was a
compiler-selected or published LilScript candidate. The production contract is one
output selected inside one compiler invocation by the configured objective. A
downstream probe may reveal a generic rewrite to implement in the compiler, but it
cannot become an extra selection stage. The measured LilScript build is
configured for its then-current Brotli model, so raw and gzip are diagnostic
cross-metrics; every downstream post-minified number is attribution-only. The row
predates the current encoder-identity contract.
Regenerate the row with scorer provenance before citing any of these as current
canonical bytes.

Attribution is still open. Dynamic public-facade bags and adapter density are visible
targets, but each port reshape and compiler candidate must be ablated separately; the
size row alone cannot prove which layer owns how many bytes.

## Port perspective (must match language mission)

From `STATUS.md`:

- Behavior parity with 3.7.1. **Not** JS object-shape fidelity.
- Prefer `struct` / `class` (positional) for fixed shapes (`Pseudo`, `HandleObj`, `Tween`, …).
- Prefer typed locals over `createEmptyObject()` bags.
- `record{}` is null-proto and **breaks** jQuery — use `createEmptyObject()` → optimizer `{}`.
- Dual delivery:
  - **Public script-tag:** `lilscript.toml` — `exports=false`, `properties=true`. Export/public aggregate names stay stable while eligible LilScript-owned internal fields may still mangle.
  - **LilScript app:** `lilscript.app.toml` — `exports=true`, `properties=true`, production candidate search, larger explicit inline caps.

Public names are a **config** choice. Internals should still be LilScript-native.

## Host modeling

`js-host.lil` is a typed facade (`call0`–`call4`, `setProp`/`getProp`, `createJQuery`, `bindMethod*`, DOM helpers). `js-host.ts` implements it for esbuild.

Compiler-known lowerings (`lower_known_js_host_calls` in `src/optimizer.rs`) include:

- plain/null-prototype object and array construction;
- JS type/string/nullish/undefined/property operations;
- `callN`/`apply`, array/string methods, selected DOM fields/methods, regex, and
  throw helpers;
- direct built-in spelling for low-use eligible wrappers (limit 4) and shared aliases
  when call/value use makes that preferable.

`jsAssume<T>()` is identity for foreign toolchains; optimized LilScript JS erases it.

## What is still expensive (optimizer cannot invent proofs)

1. String-keyed `setProp` bags on the public API → **blocks property mangling**.
2. Adapter/function density remains visible; the historical audit estimated a large
   function-count tax, but current attribution must be rerun against the current
   artifact before quoting a count.
3. Residual `bindMethod*` / `call*` / `stringify` bridges.
4. Dual class + `JsValue` facade on Deferred / Callbacks / jqXHR.
5. `this`-method conversion **regressed** size; bindMethod+arrow restore is the current walk-back.

Those are [types-not-glue](../language/types-not-glue.md) violations at the port layer: dynamic bags where a `struct` would do.

## Config experiments (local vs global)

Checked-in TOMLs:

| File | Intent |
|---|---|
| `lilscript.toml` / `lilscript.public.toml` | `priority=balanced`, brotli, public names |
| `lilscript.app.toml` | app mangle, `candidate_search=production`, inline 96/240/64 |
| `lilscript.inline-size.toml` | `size-first`, `candidate_search=off`, maximum preset |
| `lilscript.inline-40/64/256.toml` | balanced, search off, increasing IR inline budgets |

The historical `audit-jquery-configs.mjs` run (roughly 30 search-off variants) found
that more inlining hurt that checkout: `lean-balanced` was about 98.5 KiB Terser raw
and `lean-inline-96` about 104 KiB. Keep this as a regression hypothesis, not a current
size row; rerun the audit after source/compiler changes. It illustrates the
[global-optima](../compilation/global-optima.md) risk that a local inline heuristic
duplicates tokens the codec could otherwise match.

Inline TOMLs are not yet wired into the main build script; they are hand variants of that axis.

## Compiler work that came from this port

Known-host lowering in the optimizer. Selector rewritten as LilScript-native
(`selector.lil` structs + QSA) instead of cloning Sizzle objects. That is the intended
loop: change the program to match the language, preserve the public contract, then let
search pick spellings.

## How to use this page when changing the compiler

- If a pass “should help jQuery” but only helps a 20-line test, check whether the port still uses `JsValue` bags — the pass may be correct and inapplicable.
- If you raise default inline limits because a kernel shrank, re-read the inline audit.
- If you add a host helper, prefer a typed `extern` the optimizer can recognize over a new `JsValue` convention.
- Measure with the **same** config family (`app` vs `public`) you claim to beat,
  compiling a separate artifact for every raw/gzip/Brotli objective claimed.
- Treat Terser/Oxc applied to LilScript output as attribution only. The eligible
  LilScript row is the compiler's single selected output for that configured
  objective, without downstream candidate selection.
- Use the active [verification contract](../verification/paired-case-contract.md)
  and the live [jquery-01](../migration/board/notes/jquery-01.md) note.
